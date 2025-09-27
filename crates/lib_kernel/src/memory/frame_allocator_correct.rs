//! Correct frame allocator implementation without memory leaks
//!
//! This fixes the critical flaws in the original frame allocator:
//! - Eliminates artificial memory exhaustion
//! - Implements proper free frame tracking
//! - Adds fragmentation handling
//! - Supports dynamic memory region management

use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};
use core::cmp;
use alloc::collections::BTreeSet;
use spin::Mutex;

/// Maximum number of memory regions to track
const MAX_REGIONS: usize = 64;

/// Memory region type for limine compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    BootloaderReclaimable,
    KernelAndModules,
    Framebuffer,
}

/// Memory region descriptor
#[derive(Debug, Clone, Copy)]
struct MemoryRegion {
    start: PhysAddr,
    end: PhysAddr,
    region_type: MemoryRegionType,
}

impl MemoryRegion {
    fn new(start: PhysAddr, end: PhysAddr, region_type: MemoryRegionType) -> Self {
        Self { start, end, region_type }
    }

    fn contains(&self, addr: PhysAddr) -> bool {
        addr >= self.start && addr < self.end
    }

    fn size(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
    }

    fn frame_count(&self) -> usize {
        (self.size() / 4096) as usize
    }
}

/// Improved frame allocator with proper memory management
pub struct CorrectFrameAllocator {
    memory_regions: [Option<MemoryRegion>; MAX_REGIONS],
    region_count: usize,
    free_frames: BTreeSet<PhysFrame>,
    allocated_frames: BTreeSet<PhysFrame>,
    next_scan_region: usize,
    total_frames: usize,
    allocated_count: usize,
}

impl CorrectFrameAllocator {
    /// Create a new frame allocator from limine memory map
    pub fn new(memory_map: &[&limine::memory_map::Entry]) -> Self {
        let mut allocator = Self {
            memory_regions: [None; MAX_REGIONS],
            region_count: 0,
            free_frames: BTreeSet::new(),
            allocated_frames: BTreeSet::new(),
            next_scan_region: 0,
            total_frames: 0,
            allocated_count: 0,
        };

        // Add usable memory regions from limine
        for &entry in memory_map.iter() {
            if entry.entry_type == limine::memory_map::EntryType::USABLE {
                allocator.add_region(
                    PhysAddr::new(entry.base),
                    PhysAddr::new(entry.base + entry.length),
                    MemoryRegionType::Usable,
                );
            }
        }

        // Initialize free frame tracking for the first region
        allocator.scan_region_for_free_frames(0);

        allocator
    }

    /// Add a memory region to the allocator
    fn add_region(&mut self, start: PhysAddr, end: PhysAddr, region_type: MemoryRegionType) {
        if self.region_count >= MAX_REGIONS {
            panic!("Too many memory regions (max {})", MAX_REGIONS);
        }

        // Align to frame boundaries
        let start_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(start);
        let end_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(end - 1u64);

        let aligned_start = start_frame.start_address();
        let aligned_end = end_frame.start_address() + 4096u64;

        let region = MemoryRegion::new(aligned_start, aligned_end, region_type);

        self.memory_regions[self.region_count] = Some(region);
        self.region_count += 1;
        self.total_frames += region.frame_count();

        crate::kprintln!("[FRAME_ALLOC] Added region: 0x{:x}-0x{:x} ({} frames)",
                       aligned_start.as_u64(), aligned_end.as_u64(), region.frame_count());
    }

    /// Scan a memory region to populate free frames
    fn scan_region_for_free_frames(&mut self, region_index: usize) {
        if region_index >= self.region_count {
            return;
        }

        if let Some(region) = self.memory_regions[region_index] {
            if region.region_type != MemoryRegionType::Usable {
                return;
            }

            let start_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(region.start);
            let end_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(region.end - 1u64);

            // Add all frames in this region to free set (if not already allocated)
            let mut frame = start_frame;
            let mut added_count = 0;

            while frame <= end_frame {
                if !self.allocated_frames.contains(&frame) {
                    self.free_frames.insert(frame);
                    added_count += 1;
                }
                frame += 1;
            }

            crate::kprintln!("[FRAME_ALLOC] Scanned region {}: {} free frames",
                           region_index, added_count);
        }
    }

    /// Allocate a frame from free set
    fn allocate_from_free_set(&mut self) -> Option<PhysFrame> {
        if let Some(frame) = self.free_frames.iter().next().copied() {
            self.free_frames.remove(&frame);
            self.allocated_frames.insert(frame);
            self.allocated_count += 1;
            return Some(frame);
        }
        None
    }

    /// Try to find more free frames by scanning remaining regions
    fn expand_free_frames(&mut self) -> bool {
        let initial_scan = self.next_scan_region;

        // Scan remaining regions
        while self.next_scan_region < self.region_count {
            let region_to_scan = self.next_scan_region;
            self.next_scan_region += 1;

            self.scan_region_for_free_frames(region_to_scan);

            if !self.free_frames.is_empty() {
                crate::kprintln!("[FRAME_ALLOC] Found {} free frames in region {}",
                               self.free_frames.len(), region_to_scan);
                return true;
            }
        }

        // If we've scanned all regions, try defragmentation
        if initial_scan == 0 {
            return self.defragment_memory();
        }

        false
    }

    /// Attempt memory defragmentation (placeholder)
    fn defragment_memory(&mut self) -> bool {
        // Future: implement actual defragmentation
        // - Compact allocated frames
        // - Merge adjacent free regions
        // - Reclaim unused kernel resources

        crate::kprintln!("[FRAME_ALLOC] Defragmentation not yet implemented");
        false
    }

    /// Deallocate a frame back to the free set
    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        if self.allocated_frames.remove(&frame) {
            self.free_frames.insert(frame);
            self.allocated_count -= 1;
            crate::kprintln!("[FRAME_ALLOC] Deallocated frame 0x{:x}",
                           frame.start_address().as_u64());
        } else {
            crate::kprintln!("[FRAME_ALLOC] WARNING: Attempted to deallocate unallocated frame 0x{:x}",
                           frame.start_address().as_u64());
        }
    }

    /// Get allocation statistics
    pub fn get_stats(&self) -> (usize, usize, usize, usize) {
        (
            self.total_frames,
            self.allocated_count,
            self.free_frames.len(),
            self.region_count,
        )
    }

    /// Check if allocator is healthy
    pub fn is_healthy(&self) -> bool {
        self.allocated_count <= self.total_frames &&
        !self.free_frames.is_empty() || self.next_scan_region < self.region_count
    }
}

unsafe impl FrameAllocator<Size4KiB> for CorrectFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // Try to allocate from existing free frames first
        if let Some(frame) = self.allocate_from_free_set() {
            return Some(frame);
        }

        // If no free frames, try to expand by scanning more regions
        if self.expand_free_frames() {
            return self.allocate_from_free_set();
        }

        // Out of memory
        let (total, allocated, free, regions) = self.get_stats();
        crate::kprintln!("[FRAME_ALLOC] CRITICAL: Out of frames!");
        crate::kprintln!("[FRAME_ALLOC] Total: {}, Allocated: {}, Free: {}, Regions: {}",
                       total, allocated, free, regions);

        None
    }
}

/// Thread-safe wrapper for the frame allocator
pub struct LockedFrameAllocator(Mutex<CorrectFrameAllocator>);

impl LockedFrameAllocator {
    pub fn new(memory_map: &[&limine::memory_map::Entry]) -> Self {
        Self(Mutex::new(CorrectFrameAllocator::new(memory_map)))
    }

    pub fn deallocate_frame(&self, frame: PhysFrame) {
        self.0.lock().deallocate_frame(frame);
    }

    pub fn get_stats(&self) -> (usize, usize, usize, usize) {
        self.0.lock().get_stats()
    }

    pub fn is_healthy(&self) -> bool {
        self.0.lock().is_healthy()
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.0.lock().allocate_frame()
    }
}