//! Memory initialization
//!
//! Parse the Limine memory map and HHDM response, initialize unified frame allocator,
//! paging system, and kernel heap. This now uses the unified memory management system.

use lib_kernel::{
    memory::{self, unified_frame_allocator, tests::MemoryTestRunner, integration_tests},
    scheduler,
    kprintln,
    consts::{HHDM_REQUEST, MEMORY_MAP_REQUEST},
};

/// Initialize memory structures and the kernel heap using unified systems.
pub fn init_memory_and_heap() -> Result<(), &'static str> {
    if let Some(memory_map_response) = MEMORY_MAP_REQUEST.get_response() {
        let memory_entries = memory_map_response.entries();
        let entry_count = memory_entries.len();
        if entry_count == 0 {
            return Err("No memory map entries");
        }
        kprintln!("[OK] Memory map validated ({} entries)", entry_count);

        if let Some(hhdm_response) = HHDM_REQUEST.get_response() {
            let phys_mem_offset = x86_64::VirtAddr::new(hhdm_response.offset());
            kprintln!("[OK] Physical memory offset: {:#x}", phys_mem_offset.as_u64());

            // Initialize unified frame allocator first
            unified_frame_allocator::init_global_frame_allocator(memory_entries)
                .map_err(|_| "Failed to initialize unified frame allocator")?;

            // Initialize legacy paging system for compatibility
            let (mut mapper, mut _legacy_allocator) = memory::init_memory(memory_entries, phys_mem_offset);

            // Initialize main kernel heap with unified allocator
            kprintln!("[INFO] Initializing unified kernel heap: {} MiB at {:#x}", 
                     memory::HEAP_SIZE / (1024 * 1024), memory::HEAP_START);

            // Get frame allocator from the global instance for heap initialization
            let frame_allocator_mutex = unified_frame_allocator::get_global_frame_allocator()?;
            {
                let mut frame_allocator_guard = frame_allocator_mutex.lock();
                if let Some(ref mut frame_allocator) = *frame_allocator_guard {
                    match memory::init_kernel_heap(&mut mapper, frame_allocator) {
                        Ok(_) => {
                            kprintln!("[OK] Unified kernel heap initialized successfully");
                            let stats = memory::get_allocator_stats();
                            kprintln!("     Heap size: {} MiB, Start: {:#x}", 
                                     stats.total_heap_size / (1024 * 1024), 
                                     stats.heap_start_addr);
                        }
                        Err(_) => {
                            return Err("Failed to initialize unified kernel heap");
                        }
                    }
                } else {
                    return Err("Frame allocator not properly initialized");
                }
            }

            // Run memory system validation
            kprintln!("[INFO] Running memory system validation...");
            match MemoryTestRunner::run_quick_validation() {
                Ok(()) => kprintln!("[OK] Memory system validation passed"),
                Err(e) => {
                    kprintln!("[WARN] Memory system validation failed: {}", e);
                    // Continue anyway, but log the warning
                }
            }

            // Run integration tests for Galleon2 and Luminal compatibility
            kprintln!("[INFO] Running Galleon2 and Luminal integration tests...");
            match integration_tests::run_integration_tests() {
                Ok(()) => kprintln!("[OK] All integration tests passed - systems ready for complex operations"),
                Err(e) => {
                    kprintln!("[WARN] Integration test failed: {}", e);
                    kprintln!("[WARN] Some complex systems may not work properly");
                    // Continue anyway for basic functionality
                }
            }

            // Initialize scheduler (single CPU for now)
            scheduler::init_scheduler(1);
            kprintln!("[OK] Scheduler initialized");

            kprintln!("[INFO] Unified memory management system ready");

            Ok(())
        } else {
            Err("No HHDM response")
        }
    } else {
        Err("No memory map response")
    }
}
