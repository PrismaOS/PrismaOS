use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Kernel heap configuration
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB - increased for console buffer and kernel allocations

// Small bootstrap heap for early allocations before we set up virtual memory
#[repr(align(16))]
struct BootstrapHeap([u8; 64 * 1024]); // 64KB bootstrap heap

static mut BOOTSTRAP_HEAP: BootstrapHeap = BootstrapHeap([0; 64 * 1024]);
static mut BOOTSTRAP_ACTIVE: bool = false;

/// Initialize bootstrap heap for early kernel allocations
pub unsafe fn init_bootstrap_heap() {
    ALLOCATOR.lock().init(BOOTSTRAP_HEAP.0.as_mut_ptr(), 64 * 1024);
    BOOTSTRAP_ACTIVE = true;
}

/// Initialize the main kernel heap with proper virtual memory mapping
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // Map each page of the heap to physical memory
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { 
            let flush_result = mapper.map_to(page, frame, flags, frame_allocator)?;
            flush_result.flush();
        };
    }

    // Switch from bootstrap heap to main heap
    unsafe {
        // Re-initialize allocator with the new larger heap
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
        BOOTSTRAP_ACTIVE = false;
    }

    Ok(())
}

/// Get heap usage statistics
pub fn heap_stats() -> HeapStats {
    // The linked list allocator doesn't provide usage stats by default
    // This is a placeholder for future heap instrumentation
    HeapStats {
        total_size: HEAP_SIZE,
        used_size: 0, // Would need custom allocator to track this
        free_size: HEAP_SIZE,
        fragmentation_ratio: 0.0,
    }
}

#[derive(Debug)]
pub struct HeapStats {
    pub total_size: usize,
    pub used_size: usize, 
    pub free_size: usize,
    pub fragmentation_ratio: f32,
}

/// Custom allocation error handling
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error: failed to allocate {} bytes with alignment {}", 
           layout.size(), layout.align())
}