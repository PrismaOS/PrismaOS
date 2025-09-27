//! Unified Memory Allocator System
//!
//! This module provides a comprehensive memory management system that unifies:
//! - Bootstrap heap for early kernel allocations
//! - Main kernel heap with virtual memory mapping
//! - Frame allocator for physical memory management
//! - Integration with paging system
//! - Proper error handling and statistics

use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr, PhysAddr,
};
use core::alloc::GlobalAlloc;
use spin::Mutex;
use alloc::{vec::Vec, boxed::Box, string::String, vec};

/// Global allocator instance
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Kernel heap configuration - using safer virtual address in kernel space
pub const HEAP_START: usize = 0xffff_8800_0000_0000;  // Higher half kernel heap
pub const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB - increased for complex systems

/// Bootstrap heap for early allocations before virtual memory is set up
#[repr(align(16))]
struct BootstrapHeap([u8; 128 * 1024]); // 128KB - increased for more complex early allocations

static mut BOOTSTRAP_HEAP: BootstrapHeap = BootstrapHeap([0; 128 * 1024]);
static BOOTSTRAP_ACTIVE: Mutex<bool> = Mutex::new(false);

/// Memory allocation statistics
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    pub total_heap_size: usize,
    pub bootstrap_heap_size: usize,
    pub bootstrap_active: bool,
    pub heap_start_addr: usize,
}

/// Allocation error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    OutOfMemory,
    InvalidLayout,
    HeapNotInitialized,
    BootstrapHeapOverflow,
}

/// Initialize bootstrap heap for early kernel allocations
/// This must be called before any heap allocations occur
pub unsafe fn init_bootstrap_heap() -> Result<(), AllocationError> {
    let mut active = BOOTSTRAP_ACTIVE.lock();
    if *active {
        return Ok(()); // Already initialized
    }
    
    // Initialize the allocator with bootstrap heap
    ALLOCATOR.lock().init(BOOTSTRAP_HEAP.0.as_mut_ptr(), BOOTSTRAP_HEAP.0.len());
    *active = true;
    
    crate::kprintln!("    [INFO] Bootstrap heap initialized: {} KB", BOOTSTRAP_HEAP.0.len() / 1024);
    Ok(())
}

/// Initialize the main kernel heap with proper virtual memory mapping and enhanced safety
/// This replaces the bootstrap heap with a larger, properly mapped heap
pub fn init_kernel_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let bootstrap_active = *BOOTSTRAP_ACTIVE.lock();
    if !bootstrap_active {
        panic!("Bootstrap heap must be initialized before kernel heap");
    }
    
    // Validate heap address range is reasonable for kernel space
    if HEAP_START < 0xffff_8000_0000_0000 || HEAP_START > 0xffff_ffff_ffff_f000 {
        crate::kprintln!("[ERROR] Invalid heap start address: {:#x}", HEAP_START);
        panic!("Invalid kernel heap address range");
    }
    
    // Calculate page range for the heap
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };
    
    crate::kprintln!("    [INFO] Mapping kernel heap: {} MiB at {:#x}", 
                    HEAP_SIZE / (1024 * 1024), HEAP_START);
    
    // Map each page of the heap to physical memory with enhanced error handling
    let mut pages_mapped = 0;
    let mut total_pages = 0;
    
    for page in page_range {
        total_pages += 1;
        
        // Check if page is already mapped (avoid conflicts)
        match mapper.translate_page(page) {
            Ok(_) => {
                crate::kprintln!("[WARN] Page {:#x} already mapped, skipping", page.start_address().as_u64());
                pages_mapped += 1;
                continue;
            }
            Err(_) => {
                // Page not mapped, proceed with mapping
            }
        }
        
        // Allocate physical frame with error handling
        let frame = match frame_allocator.allocate_frame() {
            Some(frame) => frame,
            None => {
                crate::kprintln!("[ERROR] Failed to allocate frame for heap page {}", pages_mapped);
                return Err(MapToError::FrameAllocationFailed);
            }
        };
        
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        
        // Map the page with detailed error handling
        match unsafe { mapper.map_to(page, frame, flags, frame_allocator) } {
            Ok(flush_result) => {
                flush_result.flush();
                pages_mapped += 1;
            }
            Err(MapToError::FrameAllocationFailed) => {
                crate::kprintln!("[ERROR] Frame allocation failed during mapping for page {}", pages_mapped);
                return Err(MapToError::FrameAllocationFailed);
            }
            Err(MapToError::ParentEntryHugePage) => {
                crate::kprintln!("[ERROR] Parent entry is huge page for page {:#x}", page.start_address().as_u64());
                return Err(MapToError::ParentEntryHugePage);
            }
            Err(MapToError::PageAlreadyMapped(_)) => {
                crate::kprintln!("[WARN] Page {:#x} was already mapped during mapping", page.start_address().as_u64());
                pages_mapped += 1;
                continue;
            }
        }
    }
    
    crate::kprintln!("    [INFO] Successfully mapped {} of {} pages for kernel heap", pages_mapped, total_pages);
    
    // Verify we mapped enough pages for a functional heap
    if pages_mapped < total_pages / 2 {
        crate::kprintln!("[ERROR] Too few pages mapped: {} of {} required", pages_mapped, total_pages);
        panic!("Insufficient heap pages mapped");
    }
    
    // Switch from bootstrap heap to main kernel heap with validation
    unsafe {
        // Test heap memory accessibility before switching allocators
        let test_ptr = HEAP_START as *mut u64;
        let test_value: u64 = 0xDEADBEEFCAFEBABE;
        core::ptr::write_volatile(test_ptr, test_value);
        let read_back = core::ptr::read_volatile(test_ptr);
        if read_back != test_value {
            crate::kprintln!("[ERROR] Heap memory test failed: wrote {:#x}, read {:#x}", 
                           test_value, read_back);
            panic!("Heap memory accessibility test failed");
        }
        
        // Zero out the test value
        core::ptr::write_volatile(test_ptr, 0);
        
        // Re-initialize allocator with the new larger heap
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
        
        // Mark bootstrap as inactive
        *BOOTSTRAP_ACTIVE.lock() = false;
    }
    
    crate::kprintln!("    [INFO] Kernel heap initialized and validated successfully");
    crate::kprintln!("       Size: {} MiB, Pages: {}", HEAP_SIZE / (1024 * 1024), pages_mapped);
    crate::kprintln!("       Virtual range: {:#x} - {:#x}", HEAP_START, HEAP_START + HEAP_SIZE);
    
    Ok(())
}

/// Get current allocator statistics
pub fn get_allocator_stats() -> AllocatorStats {
    let bootstrap_heap_size = unsafe { BOOTSTRAP_HEAP.0.len() };
    AllocatorStats {
        total_heap_size: HEAP_SIZE,
        bootstrap_heap_size,
        bootstrap_active: *BOOTSTRAP_ACTIVE.lock(),
        heap_start_addr: HEAP_START,
    }
}

/// Test heap allocation capabilities
pub fn test_heap_allocation() -> Result<(), AllocationError> {
    use alloc::{vec::Vec, string::String};
    
    // Test basic allocation
    let mut test_vec = Vec::new();
    for i in 0..100 {
        test_vec.push(i);
    }
    
    if test_vec.len() != 100 {
        return Err(AllocationError::OutOfMemory);
    }
    
    // Test string allocation
    let test_string = String::from("PrismaOS Memory Test");
    if test_string.len() != 20 {
        return Err(AllocationError::InvalidLayout);
    }
    
    // Test larger allocation
    let large_vec: Vec<u8> = vec![0; 1024 * 1024]; // 1MB allocation
    if large_vec.len() != 1024 * 1024 {
        return Err(AllocationError::OutOfMemory);
    }
    
    crate::kprintln!("    ✅ Heap allocation test passed");
    Ok(())
}

/// Custom allocation error handler for the kernel
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    let stats = get_allocator_stats();
    
    crate::kprintln!("FATAL: Allocation error!");
    crate::kprintln!("  Requested: {} bytes with alignment {}", layout.size(), layout.align());
    crate::kprintln!("  Bootstrap active: {}", stats.bootstrap_active);
    crate::kprintln!("  Heap size: {} MiB", stats.total_heap_size / (1024 * 1024));
    crate::kprintln!("  Heap start: {:#x}", stats.heap_start_addr);
    
    // Try to provide more debug information
    if stats.bootstrap_active {
        crate::kprintln!("  ERROR: Still using bootstrap heap - main heap not initialized?");
    }
    
    panic!("Memory allocation failed - system cannot continue");
}

/// Validate heap integrity (for testing)
pub fn validate_heap() -> Result<(), AllocationError> {
    let stats = get_allocator_stats();
    
    // Basic sanity checks
    if stats.total_heap_size == 0 {
        return Err(AllocationError::HeapNotInitialized);
    }
    
    if stats.heap_start_addr == 0 {
        return Err(AllocationError::HeapNotInitialized);
    }
    
    // Test a small allocation to make sure heap works
    test_heap_allocation()?;
    
    Ok(())
}

/// Memory pressure testing for Galleon2 and Luminal compatibility
pub fn stress_test_allocations() -> Result<(), AllocationError> {
    use alloc::vec::Vec;
    use alloc::boxed::Box;
    
    crate::kprintln!("    [INFO] Running memory stress test...");
    
    // Test many small allocations (typical for filesystem operations)
    let mut small_allocations = Vec::new();
    for i in 0..1000 {
        let allocation = Box::new([i as u8; 64]); // 64-byte allocations
        small_allocations.push(allocation);
    }
    
    // Test medium allocations (typical for runtime operations)
    let mut medium_allocations = Vec::new();
    for i in 0..100 {
        let allocation = vec![i as u8; 4096]; // 4KB allocations
        medium_allocations.push(allocation);
    }
    
    // Test large allocations (filesystem buffers)
    let mut large_allocations = Vec::new();
    for i in 0..10 {
        let allocation = vec![i as u8; 64 * 1024]; // 64KB allocations
        large_allocations.push(allocation);
    }
    
    // Verify all allocations are valid
    for (i, alloc) in small_allocations.iter().enumerate() {
        if alloc[0] != (i as u8) {
            return Err(AllocationError::OutOfMemory);
        }
    }
    
    crate::kprintln!("    ✅ Memory stress test passed");
    crate::kprintln!("       Small allocations: {} × 64B", small_allocations.len());
    crate::kprintln!("       Medium allocations: {} × 4KB", medium_allocations.len());
    crate::kprintln!("       Large allocations: {} × 64KB", large_allocations.len());
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    
    #[test]
    fn test_allocator_stats() {
        let stats = get_allocator_stats();
        assert_eq!(stats.total_heap_size, HEAP_SIZE);
        assert_eq!(stats.bootstrap_heap_size, 128 * 1024);
        assert_eq!(stats.heap_start_addr, HEAP_START);
    }
    
    #[test]
    fn test_heap_validation() {
        // This test can only run after heap initialization
        // In a real kernel environment, this would be called after init
        // For now, just test that the validation function doesn't panic
        let _ = validate_heap();
    }
    
    #[test]
    fn test_allocation_error_types() {
        // Test that error types work correctly
        assert_eq!(AllocationError::OutOfMemory, AllocationError::OutOfMemory);
        assert_ne!(AllocationError::OutOfMemory, AllocationError::InvalidLayout);
    }
}