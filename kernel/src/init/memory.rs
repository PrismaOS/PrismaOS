//! Memory initialization
//!
//! Parse the Limine memory map and HHDM response, initialize paging and the
//! frame allocator, and then initialize the kernel heap. On success this
//! returns Ok(()). On failure returns a descriptive &'static str.

use lib_kernel::{
    memory,
    scheduler,
    kprintln,
    consts::{HHDM_REQUEST, MEMORY_MAP_REQUEST},
};

/// Initialize memory structures and the kernel heap.
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

            let (mut mapper, mut frame_allocator) = memory::init_memory(memory_entries, phys_mem_offset);

            kprintln!("[INFO] Initializing kernel heap: {} MiB at {:#x}", memory::HEAP_SIZE / (1024 * 1024), memory::HEAP_START);
            match unsafe { memory::init_heap(&mut mapper, &mut frame_allocator)} {
                Ok(_) => {
                    kprintln!("[OK] Kernel heap initialized with proper virtual memory mapping");
                }
                Err(_) => {
                    return Err("Failed to initialize kernel heap");
                }
            }

            // Initialize scheduler (single CPU for now)
            scheduler::init_scheduler(1);
            kprintln!("[OK] Scheduler initialized");

            Ok(())
        } else {
            Err("No HHDM response")
        }
    } else {
        Err("No memory map response")
    }
}
