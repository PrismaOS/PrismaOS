use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, Page, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

// Unified memory management system
pub mod aligned;
pub mod unified_gdt;
pub mod unified_allocator;
pub mod unified_frame_allocator;
pub mod tests;
pub mod gdt_debug;
// pub mod integration_tests;

// Legacy modules (kept for compatibility during transition)
pub mod allocator;
pub mod paging;
pub mod dma;
//pub mod mmio;

// Re-export unified interfaces
pub use aligned::{Aligned16, Aligned8, Aligned4, PageAligned, AlignedBytes16, AlignedBytes8, AlignedBytes4, PageAlignedBytes, AlignedStack, AlignedHeap};
pub use unified_gdt::{init as init_unified_gdt, get_selectors, setup_syscall_msrs, validate_gdt};
pub use unified_allocator::{
    init_bootstrap_heap, init_kernel_heap, get_allocator_stats,
    test_heap_allocation, stress_test_allocations, validate_heap,
    HEAP_SIZE, HEAP_START
};
pub use unified_frame_allocator::{
    init_global_frame_allocator, get_global_frame_allocator,
    test_global_frame_allocator, get_frame_allocator_stats
};

// Legacy compatibility exports
pub use allocator::{init_heap, heap_stats};
//pub use mmio::{XhciMmioMapper, init_mmio_mapping, mmio_stats};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameAllocatorError;

pub struct BootInfoFrameAllocator {
    memory_regions: [Option<(PhysAddr, PhysAddr)>; 16], // Max 16 memory regions
    region_count: usize,
    current_region: usize,
    next_frame: PhysAddr,
}

/// Global frame allocator instance for the kernel
static mut KERNEL_FRAME_ALLOCATOR: Option<BootInfoFrameAllocator> = None;

/// Global kernel page table access
static mut KERNEL_MAPPER: Option<*mut dyn Mapper<Size4KiB>> = None;

/// Initialize the global kernel memory management
pub unsafe fn init_kernel_memory_management(
    frame_allocator: BootInfoFrameAllocator,
    mapper: *mut dyn Mapper<Size4KiB>,
) {
    KERNEL_FRAME_ALLOCATOR = Some(frame_allocator);
    KERNEL_MAPPER = Some(mapper);
}

/// Get access to the kernel's frame allocator
pub fn get_kernel_frame_allocator() -> Option<&'static mut dyn FrameAllocator<Size4KiB>> {
    unsafe {
        KERNEL_FRAME_ALLOCATOR.as_mut().map(|fa| fa as &mut dyn FrameAllocator<Size4KiB>)
    }
}

/// Get access to the kernel's page table mapper
pub fn get_kernel_mapper() -> Option<&'static mut dyn Mapper<Size4KiB>> {
    unsafe {
        KERNEL_MAPPER.and_then(|ptr| ptr.as_mut())
    }
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &[&limine::memory_map::Entry]) -> Self {
        let mut allocator = BootInfoFrameAllocator {
            memory_regions: [None; 16],
            region_count: 0,
            current_region: 0,
            next_frame: PhysAddr::new(0),
        };

        // Store only usable memory regions without using Vec (no heap required)
        for &entry in memory_map.iter() {
            // Check if this is a usable memory region
            // Limine memory map entry types: we want usable regions only
            if entry.length > 0 && allocator.region_count < 16 {
                // For safety, skip the first 1MB to avoid potential firmware/boot loader areas
                let safe_start = if entry.base < 0x100000 {
                    0x100000
                } else {
                    entry.base
                };

                if safe_start < entry.base + entry.length {
                    // Align to 4KB boundaries
                    let start_addr = PhysAddr::new((safe_start + 4095) & !4095);
                    let end_addr = PhysAddr::new((entry.base + entry.length) & !4095);

                    if start_addr < end_addr {
                        allocator.memory_regions[allocator.region_count] = Some((start_addr, end_addr));
                        allocator.region_count += 1;
                    }
                }
            }
        }

        // Start with the first region
        if allocator.region_count > 0 {
            if let Some((start, _)) = allocator.memory_regions[0] {
                allocator.next_frame = start;
            }
        }

        allocator
    }
}

unsafe impl FrameAllocator<x86_64::structures::paging::Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        while self.current_region < self.region_count {
            if let Some((_start, end)) = self.memory_regions[self.current_region] {
                if self.next_frame < end {
                    let frame = PhysFrame::containing_address(self.next_frame);
                    self.next_frame += 4096u64;
                    return Some(frame);
                }
            }

            // Move to next region
            self.current_region += 1;
            if self.current_region < self.region_count {
                if let Some((start, _)) = self.memory_regions[self.current_region] {
                    self.next_frame = start;
                }
            }
        }

        None // No more frames available
    }
}

pub fn init_memory(
    memory_map: &[&limine::memory_map::Entry],
    physical_memory_offset: VirtAddr,
) -> (impl Mapper<x86_64::structures::paging::Size4KiB>, BootInfoFrameAllocator) {
    unsafe {
        let level_4_table = paging::init(physical_memory_offset);
        let frame_allocator = BootInfoFrameAllocator::init(memory_map);
        (level_4_table, frame_allocator)
    }
}

pub unsafe fn create_example_mapping(
    page: Page,
    mapper: &mut impl Mapper<x86_64::structures::paging::Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<x86_64::structures::paging::Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = mapper.map_to(page, frame, flags, frame_allocator);
    map_to_result.expect("map_to failed").flush();
}
