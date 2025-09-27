use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

// NOTE: Legacy allocator - global allocator now defined in unified_allocator.rs
// #[global_allocator]
// static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Legacy heap configuration - maintained for compatibility
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 8 * 1024 * 1024; // 8 MiB - reasonable size for kernel heap

// Legacy bootstrap heap - now handled by unified system
// #[repr(align(16))]
// struct BootstrapHeap([u8; 64 * 1024]); // 64KB bootstrap heap

// static mut BOOTSTRAP_HEAP: BootstrapHeap = BootstrapHeap([0; 64 * 1024]);
// static mut BOOTSTRAP_ACTIVE: bool = false;

/// Legacy function - now redirects to unified system
pub unsafe fn init_bootstrap_heap() {
    crate::memory::unified_allocator::init_bootstrap_heap()
        .expect("Failed to initialize bootstrap heap");
}

/// Legacy init_heap function - now redirects to unified system
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    crate::memory::unified_allocator::init_kernel_heap(mapper, frame_allocator)
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

// Legacy allocation error handling - now handled by unified system
// #[alloc_error_handler]
// fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
//     panic!("Allocation error: failed to allocate {} bytes with alignment {}", 
//            layout.size(), layout.align())
// }