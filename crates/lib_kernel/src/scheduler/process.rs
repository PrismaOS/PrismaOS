use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use spin::{Mutex, RwLock};
use x86_64::{
    VirtAddr, PhysAddr,
};

use super::{ProcessState, BlockReason};
use crate::api::ProcessId;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessContext {
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
    pub rip: u64,
    pub rflags: u64,
    pub cr3: u64,
}

impl ProcessContext {
    pub fn new(entry_point: VirtAddr, stack_pointer: VirtAddr, page_table: PhysAddr) -> Self {
        ProcessContext {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: stack_pointer.as_u64(),
            rsp: stack_pointer.as_u64(),
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: entry_point.as_u64(),
            rflags: 0x200, // Enable interrupts
            cr3: page_table.as_u64(),
        }
    }
}

pub struct Process {
    id: ProcessId,
    context: Mutex<ProcessContext>,
    state: RwLock<ProcessState>,
    priority: AtomicU8,
    assigned_cpu: AtomicUsize,
    
    // Timing information
    creation_time: u64,
    total_runtime: AtomicU64,
    last_scheduled: AtomicU64,
    
    // Memory management
    page_table: PhysAddr,
    stack_base: VirtAddr,
    heap_base: VirtAddr,
    
    // Process metadata
    name: RwLock<[u8; 32]>,
    parent_pid: Option<ProcessId>,
    exit_code: RwLock<Option<i32>>,
    block_reason: RwLock<Option<BlockReason>>,
    
    // Capabilities and resources
    capability_table: RwLock<Vec<crate::api::Capability>>,
    file_descriptors: RwLock<Vec<Option<FileDescriptor>>>,
    
    // Statistics
    context_switches: AtomicU64,
    page_faults: AtomicU64,
    syscalls: AtomicU64,
}

#[derive(Debug)]
pub struct FileDescriptor {
    pub fd: u32,
    pub flags: u32,
    pub offset: u64,
    pub resource: FileResource,
}

#[derive(Debug)]
pub enum FileResource {
    Console,
    Framebuffer,
    Socket,
    SharedMemory,
}

impl Process {
    pub fn new(entry_point: VirtAddr, priority: u8) -> Result<Self, super::SchedulerError> {
        let id = ProcessId::new();
        let current_time = crate::time::current_tick();
        
        // Allocate page table and stack
        let page_table = Self::allocate_page_table()?;
        let stack_base = VirtAddr::new(0x7FFF_FF00_0000); // User stack
        let heap_base = VirtAddr::new(0x0000_1000_0000);  // User heap
        
        let context = ProcessContext::new(entry_point, stack_base, page_table);
        
        let mut name = [0u8; 32];
        let name_str = alloc::format!("process_{}", id.as_u64());
        let name_bytes = name_str.as_bytes();
        let copy_len = name_bytes.len().min(31);
        name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        
        Ok(Process {
            id,
            context: Mutex::new(context),
            state: RwLock::new(ProcessState::Ready),
            priority: AtomicU8::new(priority),
            assigned_cpu: AtomicUsize::new(0),
            creation_time: current_time,
            total_runtime: AtomicU64::new(0),
            last_scheduled: AtomicU64::new(0),
            page_table,
            stack_base,
            heap_base,
            name: RwLock::new(name),
            parent_pid: None,
            exit_code: RwLock::new(None),
            block_reason: RwLock::new(None),
            capability_table: RwLock::new(Vec::new()),
            file_descriptors: RwLock::new(Vec::new()),
            context_switches: AtomicU64::new(0),
            page_faults: AtomicU64::new(0),
            syscalls: AtomicU64::new(0),
        })
    }

    pub fn id(&self) -> ProcessId {
        self.id
    }

    pub fn state(&self) -> ProcessState {
        *self.state.read()
    }

    pub fn set_state(&self, new_state: ProcessState) {
        *self.state.write() = new_state;
    }

    pub fn priority(&self) -> u8 {
        self.priority.load(Ordering::Relaxed)
    }

    pub fn set_priority(&self, priority: u8) {
        self.priority.store(priority, Ordering::Relaxed);
    }

    pub fn assigned_cpu(&self) -> usize {
        self.assigned_cpu.load(Ordering::Relaxed)
    }

    pub fn set_assigned_cpu(&self, cpu_id: usize) {
        self.assigned_cpu.store(cpu_id, Ordering::Relaxed);
    }

    pub fn context(&self) -> &Mutex<ProcessContext> {
        &self.context
    }

    pub fn set_block_reason(&self, reason: Option<BlockReason>) {
        *self.block_reason.write() = reason;
    }

    pub fn block_reason(&self) -> Option<BlockReason> {
        *self.block_reason.read()
    }

    pub fn set_exit_code(&self, code: Option<i32>) {
        *self.exit_code.write() = code;
    }

    pub fn exit_code(&self) -> Option<i32> {
        *self.exit_code.read()
    }

    pub fn page_table_addr(&self) -> PhysAddr {
        self.page_table
    }

    pub fn increment_context_switches(&self) {
        self.context_switches.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_page_faults(&self) {
        self.page_faults.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_syscalls(&self) {
        self.syscalls.fetch_add(1, Ordering::Relaxed);
    }

    pub fn update_runtime(&self, ticks: u64) {
        self.total_runtime.fetch_add(ticks, Ordering::Relaxed);
        self.last_scheduled.store(ticks, Ordering::Relaxed);
    }

    pub fn get_runtime(&self) -> u64 {
        self.total_runtime.load(Ordering::Relaxed)
    }

    pub fn add_capability(&self, capability: crate::api::Capability) {
        self.capability_table.write().push(capability);
    }

    pub fn has_capability(&self, handle: crate::api::ObjectHandle, rights: crate::api::Rights) -> bool {
        let caps = self.capability_table.read();
        caps.iter().any(|cap| cap.handle == handle && cap.rights.has(rights))
    }

    pub fn allocate_fd(&self, resource: FileResource) -> u32 {
        let mut fds = self.file_descriptors.write();
        let fd_num = fds.len() as u32;
        
        let fd = FileDescriptor {
            fd: fd_num,
            flags: 0,
            offset: 0,
            resource,
        };
        
        fds.push(Some(fd));
        fd_num
    }

    pub fn get_fd(&self, fd: u32) -> Option<FileDescriptor> {
        let fds = self.file_descriptors.read();
        if let Some(Some(descriptor)) = fds.get(fd as usize) {
            Some(FileDescriptor {
                fd: descriptor.fd,
                flags: descriptor.flags,
                offset: descriptor.offset,
                resource: match &descriptor.resource {
                    FileResource::Console => FileResource::Console,
                    FileResource::Framebuffer => FileResource::Framebuffer,
                    FileResource::Socket => FileResource::Socket,
                    FileResource::SharedMemory => FileResource::SharedMemory,
                },
            })
        } else {
            None
        }
    }

    pub fn close_fd(&self, fd: u32) {
        let mut fds = self.file_descriptors.write();
        if let Some(descriptor) = fds.get_mut(fd as usize) {
            *descriptor = None;
        }
    }

    fn allocate_page_table() -> Result<PhysAddr, super::SchedulerError> {
        // In a real implementation, this would allocate a physical page
        // and set up the process page table
        Ok(PhysAddr::new(0x1000)) // Placeholder
    }
}

unsafe impl Send for Process {}
unsafe impl Sync for Process {}