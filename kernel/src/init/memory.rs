//! Memory initialization
//!
//! Parse the Limine memory map and HHDM response, initialize paging and the
//! frame allocator, and then initialize the kernel heap. On success this
//! returns Ok(()). On failure returns a descriptive &'static str.

/// Initialize memory structures and the kernel heap.
pub fn init_memory_and_heap() -> Result<(), &'static str> {
    if let Some(memory_map_response) = crate::MEMORY_MAP_REQUEST.get_response() {
        let memory_entries = memory_map_response.entries();
        let entry_count = memory_entries.len();
        if entry_count == 0 {
            return Err("No memory map entries");
        }
        crate::kprintln!("[OK] Memory map validated ({} entries)", entry_count);

        if let Some(hhdm_response) = crate::HHDM_REQUEST.get_response() {
            let phys_mem_offset = x86_64::VirtAddr::new(hhdm_response.offset());
            crate::kprintln!("[OK] Physical memory offset: {:#x}", phys_mem_offset.as_u64());

            let (mut mapper, mut frame_allocator) = crate::memory::init_memory(memory_entries, phys_mem_offset);

            crate::kprintln!("[INFO] Initializing kernel heap: {} MiB at {:#x}", crate::memory::HEAP_SIZE / (1024 * 1024), crate::memory::HEAP_START);
            match crate::memory::init_heap(&mut mapper, &mut frame_allocator) {
                Ok(_) => {
                    crate::kprintln!("[OK] Kernel heap initialized with proper virtual memory mapping");
                    let stats = crate::memory::heap_stats();
                    crate::kprintln!("     Heap size: {} MiB, Start: {:#x}", stats.total_size / (1024 * 1024), crate::memory::HEAP_START);
                }
                Err(_) => {
                    return Err("Failed to initialize kernel heap");
                }
            }

            // Initialize scheduler (single CPU for now)
            crate::scheduler::init_scheduler(1);
            crate::kprintln!("[OK] Scheduler initialized");

            Ok(())
        } else {
            Err("No HHDM response")
        }
    } else {
        Err("No memory map response")
    }
}
