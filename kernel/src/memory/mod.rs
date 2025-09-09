use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTable, PageTableFlags, PhysFrame,
    },
    PhysAddr, VirtAddr,
};
use alloc::vec::Vec;

pub mod allocator;
pub mod paging;

pub use allocator::{init_heap, HEAP_SIZE};
pub use paging::init;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameAllocatorError;

pub struct BootInfoFrameAllocator {
    memory_map: &'static [limine::memory_map::Entry],
    next: usize,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static [limine::memory_map::Entry]) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
    
    pub unsafe fn init_from_refs(memory_map: &'static [&limine::memory_map::Entry]) -> Self {
        // For now, we'll create a new allocator that works directly with references
        // This is a simpler approach than trying to convert the slice
        BootInfoFrameAllocator {
            memory_map: core::slice::from_raw_parts(
                memory_map.as_ptr() as *const limine::memory_map::Entry,
                memory_map.len()
            ),
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // Simplified implementation to avoid Step trait issues
        let mut frames = Vec::new();
        for entry in self.memory_map.iter() {
            // Skip zero-length entries to avoid issues
            if entry.length == 0 {
                continue;
            }
            
            // Only process usable memory entries to avoid invalid regions
            // Check if this is likely a usable memory type (basic heuristic)
            let is_usable = entry.base > 0 && entry.length > 0 && entry.base < 0xFFFF_FFFF_FFFF_F000;
            if is_usable { // Basic memory region validation
                let frame_start = entry.base;
                
                // Use saturating arithmetic to prevent overflow
                let frame_end = frame_start.saturating_add(entry.length);
                
                // Skip if we got an overflow (saturated to max value)
                if frame_end == u64::MAX {
                    continue;
                }
                
                let start_addr = PhysAddr::new(frame_start);
                
                // Ensure we don't underflow when subtracting 1
                if frame_end == 0 {
                    continue;
                }
                
                let end_addr = PhysAddr::new(frame_end - 1);
                
                // Skip invalid address ranges
                if start_addr > end_addr {
                    continue;
                }
                
                let start_frame = PhysFrame::<x86_64::structures::paging::Size4KiB>::containing_address(start_addr);
                let _end_frame = PhysFrame::<x86_64::structures::paging::Size4KiB>::containing_address(end_addr);
                
                // Use a safer iteration approach with bounds checking
                let mut addr = start_addr;
                let page_size = 4096u64;
                
                while addr <= end_addr {
                    frames.push(PhysFrame::<x86_64::structures::paging::Size4KiB>::containing_address(addr));
                    
                    // Check for overflow before adding
                    if addr.as_u64().saturating_add(page_size) < addr.as_u64() {
                        break; // Overflow would occur
                    }
                    
                    addr += page_size;
                    
                    // Additional safety check to prevent infinite loops
                    if addr.as_u64() >= 0xFFFF_FFFF_FFFF_F000 {
                        break;
                    }
                }
            }
        }
        frames.into_iter()
    }
}

unsafe impl FrameAllocator<x86_64::structures::paging::Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub fn init_memory(
    memory_map: &'static [limine::memory_map::Entry],
    physical_memory_offset: VirtAddr,
) -> (impl Mapper<x86_64::structures::paging::Size4KiB>, BootInfoFrameAllocator) {
    unsafe {
        let level_4_table = paging::init(physical_memory_offset);
        let frame_allocator = BootInfoFrameAllocator::init(memory_map);
        (level_4_table, frame_allocator)
    }
}

pub fn init_memory_from_refs(
    memory_map: &'static [&limine::memory_map::Entry],
    physical_memory_offset: VirtAddr,
) -> (impl Mapper<x86_64::structures::paging::Size4KiB>, BootInfoFrameAllocator) {
    unsafe {
        let level_4_table = paging::init(physical_memory_offset);
        let frame_allocator = BootInfoFrameAllocator::init_from_refs(memory_map);
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