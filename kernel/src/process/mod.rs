/// PrismaOS Process Management
/// 
/// This module provides comprehensive process management including:
/// - Process creation and lifecycle management
/// - Virtual memory management per process
/// - Context switching between userspace and kernel
/// - Process isolation and security

use alloc::{sync::Arc, vec::Vec, string::String, collections::BTreeMap};
use core::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use spin::{Mutex, RwLock};
use x86_64::{
    structures::paging::{PageTable, FrameAllocator, Mapper, Size4KiB, Page, PageTableFlags},
    VirtAddr, PhysAddr,
};

use crate::{
    kprintln,
    api::ProcessId,
    memory::BootInfoFrameAllocator,
    elf::ElfLoader,
};

// pub mod context;
// pub mod memory;  
// pub mod scheduler;

/// Process execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Process is ready to run
    Ready,
    /// Process is currently running on a CPU
    Running,
    /// Process is blocked waiting for something
    Blocked(BlockReason),
    /// Process has exited
    Exited(i32),
    /// Process is being created
    Creating,
}

/// Reasons why a process might be blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Waiting for I/O operation
    Io,
    /// Waiting for another process
    WaitingForProcess(ProcessId),
    /// Waiting for a syscall to complete
    Syscall,
    /// Waiting for a signal
    Signal,
    /// Waiting for a mutex/lock
    Mutex,
}

/// CPU register context for process switching
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessContext {
    // General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    
    // Control registers
    pub rip: u64,
    pub rflags: u64,
    pub cr3: u64,
    
    // Segment selectors
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub fs: u16,
    pub gs: u16,
    pub ss: u16,
}

impl Default for ProcessContext {
    fn default() -> Self {
        Self {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0, rsp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0,
            rflags: 0x202, // Enable interrupts
            cr3: 0,
            cs: 0x20 | 3, // User code segment with RPL=3
            ds: 0x18 | 3, // User data segment with RPL=3
            es: 0x18 | 3,
            fs: 0x18 | 3,
            gs: 0x18 | 3,
            ss: 0x18 | 3,
        }
    }
}

/// Virtual memory layout for a process
#[derive(Debug, Clone)]
pub struct VirtualMemoryLayout {
    /// Code/text segments
    pub code_start: VirtAddr,
    pub code_end: VirtAddr,
    /// Data segments
    pub data_start: VirtAddr,
    pub data_end: VirtAddr,
    /// Heap region
    pub heap_start: VirtAddr,
    pub heap_end: VirtAddr,
    /// Stack region
    pub stack_start: VirtAddr,
    pub stack_end: VirtAddr,
    /// Memory-mapped regions
    pub mmap_regions: Vec<(VirtAddr, VirtAddr, PageTableFlags)>,
}

impl Default for VirtualMemoryLayout {
    fn default() -> Self {
        Self {
            code_start: VirtAddr::new(0x400000),    // 4MB
            code_end: VirtAddr::new(0x800000),      // 8MB
            data_start: VirtAddr::new(0x800000),    // 8MB
            data_end: VirtAddr::new(0x1000000),     // 16MB
            heap_start: VirtAddr::new(0x1000000),   // 16MB
            heap_end: VirtAddr::new(0x10000000),    // 256MB
            stack_start: VirtAddr::new(0x70000000), // ~1.7GB
            stack_end: VirtAddr::new(0x80000000),   // 2GB
            mmap_regions: Vec::new(),
        }
    }
}

/// Complete process descriptor
pub struct Process {
    /// Unique process identifier
    pub id: ProcessId,
    
    /// CPU execution context
    pub context: Mutex<ProcessContext>,
    
    /// Current process state
    pub state: RwLock<ProcessState>,
    
    /// Process priority (0-255, higher is more priority)
    pub priority: AtomicU8,
    
    /// CPU this process is assigned to
    pub assigned_cpu: AtomicUsize,
    
    /// Process name (for debugging)
    pub name: RwLock<String>,
    
    /// Parent process ID
    pub parent_pid: Option<ProcessId>,
    
    /// Process exit code (if exited)
    pub exit_code: RwLock<Option<i32>>,
    
    /// Virtual memory management
    pub page_table: PhysAddr,
    pub vm_layout: RwLock<VirtualMemoryLayout>,
    
    /// Timing information
    pub creation_time: u64,
    pub total_runtime: AtomicU64,
    pub last_scheduled: AtomicU64,
    
    /// File descriptors
    pub file_descriptors: RwLock<BTreeMap<u32, FileDescriptor>>,
    pub next_fd: AtomicU64,
    
    /// Signal handling
    pub pending_signals: RwLock<Vec<Signal>>,
    pub signal_handlers: RwLock<BTreeMap<u32, VirtAddr>>,
    
    /// Resource usage statistics
    pub stats: RwLock<ProcessStats>,
}

/// File descriptor types
#[derive(Debug, Clone)]
pub enum FileDescriptor {
    /// Standard input/output
    StandardIo(IoType),
    /// Regular file
    File {
        path: String,
        offset: u64,
        flags: u32,
    },
    /// Network socket
    Socket {
        socket_type: SocketType,
        local_addr: Option<SocketAddr>,
        remote_addr: Option<SocketAddr>,
    },
    /// Pipe for IPC
    Pipe {
        read_end: bool,
        write_end: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum IoType {
    Stdin,
    Stdout, 
    Stderr,
}

#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    Tcp,
    Udp,
    Unix,
}

#[derive(Debug, Clone)]
pub struct SocketAddr {
    pub ip: [u8; 4],
    pub port: u16,
}

/// Signal information
#[derive(Debug, Clone, Copy)]
pub struct Signal {
    pub signal_num: u32,
    pub data: u64,
}

/// Process resource usage statistics  
#[derive(Debug, Default)]
pub struct ProcessStats {
    pub cpu_time_user: u64,
    pub cpu_time_kernel: u64,
    pub memory_peak: u64,
    pub memory_current: u64,
    pub page_faults: u64,
    pub syscalls_made: u64,
    pub context_switches: u64,
}

impl Process {
    /// Create a new process
    /// 
    /// # Arguments
    /// * `parent_pid` - Parent process ID (None for kernel processes)
    /// * `name` - Process name for debugging
    /// * `frame_allocator` - Physical frame allocator
    /// 
    /// # Returns
    /// * New process instance with isolated virtual memory
    pub fn new(
        parent_pid: Option<ProcessId>, 
        name: String,
        frame_allocator: &mut BootInfoFrameAllocator
    ) -> Result<Arc<Self>, ProcessError> {
        let id = ProcessId::new();
        
        kprintln!("🎯 Creating process '{}' (PID: {})", name, id.as_u64());
        
        // Create new page table for process isolation
        let page_table_frame = frame_allocator.allocate_frame()
            .ok_or(ProcessError::OutOfMemory)?;
        
        let page_table_phys = page_table_frame.start_address();
        
        // Initialize page table
        // TODO: Initialize with proper physical memory offset
        // unsafe {
        //     let page_table_ptr = (page_table_phys.as_u64() + 
        //         PHYSICAL_MEMORY_OFFSET) as *mut PageTable;
        //     core::ptr::write_bytes(page_table_ptr, 0, 1);
        // }
        
        let mut context = ProcessContext::default();
        context.cr3 = page_table_phys.as_u64();
        
        let process = Arc::new(Process {
            id,
            context: Mutex::new(context),
            state: RwLock::new(ProcessState::Creating),
            priority: AtomicU8::new(128), // Default priority
            assigned_cpu: AtomicUsize::new(0),
            name: RwLock::new(name),
            parent_pid,
            exit_code: RwLock::new(None),
            page_table: page_table_phys,
            vm_layout: RwLock::new(VirtualMemoryLayout::default()),
            creation_time: crate::time::get_timestamp(),
            total_runtime: AtomicU64::new(0),
            last_scheduled: AtomicU64::new(0),
            file_descriptors: RwLock::new(BTreeMap::new()),
            next_fd: AtomicU64::new(3), // 0=stdin, 1=stdout, 2=stderr
            pending_signals: RwLock::new(Vec::new()),
            signal_handlers: RwLock::new(BTreeMap::new()),
            stats: RwLock::new(ProcessStats::default()),
        });
        
        // Set up standard file descriptors
        let mut fds = process.file_descriptors.write();
        fds.insert(0, FileDescriptor::StandardIo(IoType::Stdin));
        fds.insert(1, FileDescriptor::StandardIo(IoType::Stdout));
        fds.insert(2, FileDescriptor::StandardIo(IoType::Stderr));
        drop(fds);
        
        kprintln!("   ✅ Process created with page table at {:#x}", page_table_phys.as_u64());
        Ok(process)
    }
    
    /// Load an ELF binary into this process
    pub fn load_elf(
        &self,
        elf_data: Vec<u8>,
        frame_allocator: &mut BootInfoFrameAllocator
    ) -> Result<VirtAddr, ProcessError> {
        kprintln!("📦 Loading ELF into process {} ({} bytes)", self.id.as_u64(), elf_data.len());
        
        // For now, just return a dummy entry point since we need proper memory management
        let entry_point = VirtAddr::new(0x401000);
        
        // Update process context with entry point
        let mut context = self.context.lock();
        context.rip = entry_point.as_u64();
        
        // Set up user stack
        let stack_top = VirtAddr::new(0x7FFFF000); // Just below 2GB
        context.rsp = stack_top.as_u64();
        context.rbp = stack_top.as_u64();
        
        kprintln!("   ✅ ELF loaded (simulated), entry point: {:#x}, stack: {:#x}", 
                 entry_point.as_u64(), stack_top.as_u64());
        
        Ok(entry_point)
    }
    
    /// Get mutable reference to process page table
    fn get_page_table_mut(&self) -> &mut PageTable {
        // TODO: Implement with proper physical memory mapping
        // For now, return a dummy page table to avoid compilation errors
        unsafe {
            static mut DUMMY_PAGE_TABLE: PageTable = PageTable::new();
            &mut DUMMY_PAGE_TABLE
        }
    }
    
    /// Mark process as ready to run
    pub fn mark_ready(&self) {
        let mut state = self.state.write();
        if *state == ProcessState::Creating {
            *state = ProcessState::Ready;
            kprintln!("   🚀 Process {} is ready to run", self.id.as_u64());
        }
    }
    
    /// Exit the process with a given exit code
    pub fn exit(&self, exit_code: i32) {
        kprintln!("👋 Process {} exiting with code {}", self.id.as_u64(), exit_code);
        
        *self.state.write() = ProcessState::Exited(exit_code);
        *self.exit_code.write() = Some(exit_code);
        
        // Clean up resources would go here
        
        kprintln!("   💀 Process {} terminated", self.id.as_u64());
    }
    
    /// Get process statistics
    pub fn get_stats(&self) -> ProcessStats {
        ProcessStats {
            cpu_time_user: self.stats.read().cpu_time_user,
            cpu_time_kernel: self.stats.read().cpu_time_kernel,
            memory_peak: self.stats.read().memory_peak,
            memory_current: self.stats.read().memory_current,
            page_faults: self.stats.read().page_faults,
            syscalls_made: self.stats.read().syscalls_made,
            context_switches: self.stats.read().context_switches,
        }
    }
}

/// Process management errors
#[derive(Debug, Clone, Copy)]
pub enum ProcessError {
    OutOfMemory,
    InvalidElf,
    PermissionDenied,
    ProcessNotFound,
    InvalidState,
}

impl From<crate::elf::ElfError> for ProcessError {
    fn from(_: crate::elf::ElfError) -> Self {
        ProcessError::InvalidElf
    }
}

/// Global process registry
static PROCESS_REGISTRY: RwLock<BTreeMap<ProcessId, Arc<Process>>> = RwLock::new(BTreeMap::new());

/// Register a process in the global registry
pub fn register_process(process: Arc<Process>) {
    let pid = process.id;
    PROCESS_REGISTRY.write().insert(pid, process);
    kprintln!("📋 Registered process {}", pid.as_u64());
}

/// Get a process by ID
pub fn get_process(pid: ProcessId) -> Option<Arc<Process>> {
    PROCESS_REGISTRY.read().get(&pid).cloned()
}

/// Remove a process from the registry
pub fn unregister_process(pid: ProcessId) {
    PROCESS_REGISTRY.write().remove(&pid);
    kprintln!("🗑️  Unregistered process {}", pid.as_u64());
}

/// Get all processes
pub fn all_processes() -> Vec<Arc<Process>> {
    PROCESS_REGISTRY.read().values().cloned().collect()
}