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

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // Simplified implementation to avoid Step trait issues
        let mut frames = Vec::new();
        for entry in self.memory_map.iter() {
            // For now, just use all memory entries (will be refined later)
            if true { // TODO: Fix EntryType variant naming
                let frame_start = entry.base;
                let frame_end = entry.base + entry.length;
                let start_addr = PhysAddr::new(frame_start);
                let end_addr = PhysAddr::new(frame_end - 1);
                let start_frame = PhysFrame::<x86_64::structures::paging::Size4KiB>::containing_address(start_addr);
                let end_frame = PhysFrame::<x86_64::structures::paging::Size4KiB>::containing_address(end_addr);
                
                let mut addr = start_addr;
                while addr <= end_addr {
                    frames.push(PhysFrame::<x86_64::structures::paging::Size4KiB>::containing_address(addr));
                    addr += 4096u64; // 4KB page size
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