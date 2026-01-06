//! Kernel heap allocator using Talc
//!
//! This module provides the kernel's global memory allocator using the Talc
//! allocator, a dlmalloc-style allocator optimized for no_std environments.
//!
//! ## Configuration
//! - Heap size: 256 MiB
//! - Heap location: 0x4444_4444_0000 (virtual address)
//! - Locking: spin::Mutex for thread safety
//! - OOM handling: ErrOnOom (panics via alloc_error_handler)
//!
//! ## Features
//! - Statistics tracking via "counters" feature
//! - O(n) allocation, O(1) deallocation
//! - Minimum allocation: 24 bytes (3 * usize)
//! - Automatic coalescing and defragmentation

use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;
use talc::{Talc, Talck, ErrOnOom, Span};
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

// Kernel heap configuration
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 128 * 1024 * 1024; // 128 MiB

// Store actual claimed heap size for diagnostics
static CLAIMED_HEAP_SIZE: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

// Global allocator using Talc with ErrOnOom handler
type TalcAllocator = Talck<spin::Mutex<()>, ErrOnOom>;

#[global_allocator]
static ALLOCATOR: TalcAllocator = {
    let talc = Talc::new(ErrOnOom);
    talc.lock()
};

/// Initialize the main kernel heap with proper virtual memory mapping
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    crate::kprintln!("[HEAP] ===== STARTING MAIN HEAP INIT =====");

    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    let total_pages = ((HEAP_SIZE + 4095) / 4096) as u64;
    crate::kprintln!("[HEAP] Allocating {} pages ({} MiB) for heap...",
        total_pages, HEAP_SIZE / (1024 * 1024));

    let mut allocated_pages = 0u64;

    // Map each page of the heap to physical memory
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or_else(|| {
                crate::kprintln!("[HEAP ERROR] Failed to allocate frame after {} pages!", allocated_pages);
                crate::kprintln!("[HEAP ERROR] Needed {} total pages, got {} pages",
                    total_pages, allocated_pages);
                MapToError::FrameAllocationFailed
            })?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            let flush_result = mapper.map_to(page, frame, flags, frame_allocator)?;
            flush_result.flush();
        };
        allocated_pages += 1;

        // Progress indicator every 1000 pages
        if allocated_pages % 1000 == 0 {
            crate::kprintln!("[HEAP] Allocated {} / {} pages...", allocated_pages, total_pages);
        }
    }

    crate::kprintln!("[HEAP] Successfully allocated all {} pages", allocated_pages);

    // Test write to verify pages are actually writable
    crate::kprintln!("[HEAP] Testing heap write access...");
    unsafe {
        let test_ptr = HEAP_START as *mut u64;
        core::ptr::write_volatile(test_ptr, 0xDEADBEEFCAFEBABE);
        let readback = core::ptr::read_volatile(test_ptr);
        if readback != 0xDEADBEEFCAFEBABE {
            crate::kprintln!("[HEAP ERROR] Heap write test failed! Wrote 0xDEADBEEFCAFEBABE, read back {:#x}", readback);
            return Err(MapToError::FrameAllocationFailed);
        }
        crate::kprintln!("[HEAP] Heap write test passed");
    }

    // Initialize the Talc allocator
    crate::kprintln!("[HEAP] Initializing Talc allocator at {:#x} with {} bytes",
        HEAP_START, HEAP_SIZE);

    unsafe {
        let heap_ptr = HEAP_START as *mut u8;

        // Debug: Verify alignment
        crate::kprintln!("[HEAP DEBUG] Heap pointer: {:#x}, alignment check: {}",
            heap_ptr as usize,
            (heap_ptr as usize) % core::mem::align_of::<usize>());

        let span = Span::new(heap_ptr, heap_ptr.add(HEAP_SIZE));
        crate::kprintln!("[HEAP DEBUG] Span created: base={:#x}, acme={:#x}, size={}",
            heap_ptr as usize, heap_ptr.add(HEAP_SIZE) as usize, HEAP_SIZE);

        let claim_result = ALLOCATOR.lock().claim(span);

        let claimed_span = claim_result.map_err(|_| {
            MapToError::FrameAllocationFailed
        })?;

        // Store the actual claimed size for stats
        CLAIMED_HEAP_SIZE.store(claimed_span.size(), core::sync::atomic::Ordering::Relaxed);
    }

    crate::kprintln!("[HEAP] Talc allocator initialized: {} MiB",
        HEAP_SIZE / (1024 * 1024));

    // Debug: Test initial allocator state
    crate::kprintln!("[HEAP DEBUG] Testing initial allocator state...");
    let test_stats = heap_stats();
    crate::kprintln!("[HEAP DEBUG] Initial stats: total={}, used={}, free={}",
        test_stats.total_size, test_stats.used_size, test_stats.free_size);

    Ok(())
}

/// Get heap usage statistics
pub fn heap_stats() -> HeapStats {
    let talc = ALLOCATOR.lock();
    let counters = talc.get_counters();
    let claimed_size = CLAIMED_HEAP_SIZE.load(core::sync::atomic::Ordering::Relaxed);

    HeapStats {
        total_size: claimed_size, // Use actual claimed size, not HEAP_SIZE constant
        used_size: counters.allocated_bytes,
        free_size: claimed_size.saturating_sub(counters.allocated_bytes),
        fragmentation_ratio: 0.0, // Disable fragmentation calc to avoid FPU issues
        total_allocations: counters.total_allocation_count,
        total_deallocations: counters.total_allocation_count.saturating_sub(counters.allocation_count as u64),
        active_allocations: counters.allocation_count as u64,
        failed_allocations: 0, // Talc panics on OOM, so this is always 0
    }
}

#[derive(Debug)]
pub struct HeapStats {
    pub total_size: usize,
    pub used_size: usize,
    pub free_size: usize,
    pub fragmentation_ratio: f32,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub active_allocations: u64,
    pub failed_allocations: u64,
}

/// Custom allocation error handling
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!(
        "Allocation error: failed to allocate {} bytes with alignment {}",
        layout.size(),
        layout.align()
    );
}
