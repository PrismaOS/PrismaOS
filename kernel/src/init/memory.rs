//! Memory initialization
//!
//! Parse the Limine memory map and HHDM response, initialize unified frame allocator,
//! paging system, and kernel heap. This now uses the unified memory management system.

use lib_kernel::{
    memory::{self, unified_frame_allocator, tests::MemoryTestRunner},
    scheduler,
    kprintln,
    consts::{HHDM_REQUEST, MEMORY_MAP_REQUEST},
};

/// Memory initialization error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryInitError {
    NoMemoryMapEntries,
    NoHhdmResponse,
    FrameAllocatorInitFailed,
    HeapInitializationFailed,
    FrameAllocatorNotReady,
    MemoryValidationFailed,
    SchedulerInitFailed,
}

impl ::core::fmt::Display for MemoryInitError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            Self::NoMemoryMapEntries => write!(f, "No memory map entries found"),
            Self::NoHhdmResponse => write!(f, "No HHDM response available"),
            Self::FrameAllocatorInitFailed => write!(f, "Unified frame allocator initialization failed"),
            Self::HeapInitializationFailed => write!(f, "Kernel heap initialization failed"),
            Self::FrameAllocatorNotReady => write!(f, "Frame allocator not properly initialized"),
            Self::MemoryValidationFailed => write!(f, "Memory system validation failed"),
            Self::SchedulerInitFailed => write!(f, "Scheduler initialization failed"),
        }
    }
}

/// Initialize memory structures and the kernel heap using unified systems.
pub fn init_memory_and_heap() -> Result<(), MemoryInitError> {
    let memory_map_response = MEMORY_MAP_REQUEST.get_response()
        .ok_or(MemoryInitError::NoMemoryMapEntries)?;
    
    let memory_entries = memory_map_response.entries();
    let entry_count = memory_entries.len();
    if entry_count == 0 {
        return Err(MemoryInitError::NoMemoryMapEntries);
    }
    kprintln!("[OK] Memory map validated ({} entries)", entry_count);

    let hhdm_response = HHDM_REQUEST.get_response()
        .ok_or(MemoryInitError::NoHhdmResponse)?;
    
    let phys_mem_offset = x86_64::VirtAddr::new(hhdm_response.offset());
    kprintln!("[OK] Physical memory offset: {:#x}", phys_mem_offset.as_u64());

    // Initialize unified frame allocator first
    unified_frame_allocator::init_global_frame_allocator(memory_entries)
        .map_err(|_| MemoryInitError::FrameAllocatorInitFailed)?;

    // Initialize legacy paging system for compatibility
    let (mut mapper, mut _legacy_allocator) = memory::init_memory(memory_entries, phys_mem_offset);

    // Initialize main kernel heap with unified allocator
    kprintln!("[INFO] Initializing unified kernel heap: {} MiB at {:#x}",
             memory::HEAP_SIZE / (1024 * 1024), memory::HEAP_START);

    // Get frame allocator from the global instance for heap initialization
    let frame_allocator_mutex = unified_frame_allocator::get_global_frame_allocator()
        .map_err(|_| MemoryInitError::FrameAllocatorNotReady)?;
    
    {
        let mut frame_allocator_guard = frame_allocator_mutex.lock();
        if let Some(ref mut frame_allocator) = *frame_allocator_guard {
            memory::init_kernel_heap(&mut mapper, frame_allocator)
                .map_err(|_| MemoryInitError::HeapInitializationFailed)?;
            
            kprintln!("[OK] Unified kernel heap initialized successfully");
            let stats = memory::get_allocator_stats();
            kprintln!("     Heap size: {} MiB, Start: {:#x}",
                     stats.total_heap_size / (1024 * 1024),
                     stats.heap_start_addr);
        } else {
            return Err(MemoryInitError::FrameAllocatorNotReady);
        }
    }

    // Run memory system validation
    kprintln!("[INFO] Running memory system validation...");
    MemoryTestRunner::run_quick_validation()
        .map_err(|_| MemoryInitError::MemoryValidationFailed)?;
    kprintln!("[OK] Memory system validation passed");

    // Initialize scheduler (single CPU for now)
    match scheduler::init_scheduler(1) {
        Ok(_) => kprintln!("[OK] Scheduler initialized"),
        Err(_) => return Err(MemoryInitError::SchedulerInitFailed),
    }

    kprintln!("[INFO] Unified memory management system ready");
    Ok(())
}
