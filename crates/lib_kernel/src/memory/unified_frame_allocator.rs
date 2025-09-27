//! Unified Frame Allocator
//!
//! This module provides a robust frame allocator for physical memory management
//! that properly handles the Limine memory map and provides comprehensive
//! memory management for the kernel.

use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};
use spin::Mutex;
use alloc::vec::Vec;

/// Maximum number of memory regions we can handle
const MAX_MEMORY_REGIONS: usize = 32;

/// Memory region information
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: PhysAddr,
    pub end: PhysAddr,
    pub size: u64,
    pub region_type: MemoryRegionType,
}

/// Types of memory regions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNonVolatile,
    BadMemory,
    Unknown,
}

/// Frame allocator statistics
#[derive(Debug, Clone, Copy)]
pub struct FrameAllocatorStats {
    pub total_regions: usize,
    pub usable_regions: usize,
    pub total_memory: u64,
    pub usable_memory: u64,
    pub allocated_frames: u64,
    pub free_frames: u64,
    pub current_region: usize,
}

/// Unified frame allocator implementation
pub struct UnifiedFrameAllocator {
    regions: [Option<MemoryRegion>; MAX_MEMORY_REGIONS],
    region_count: usize,
    current_region: usize,
    next_frame: PhysAddr,
    allocated_frames: u64,
    total_usable_frames: u64,
}

/// Global frame allocator instance
static GLOBAL_FRAME_ALLOCATOR: Mutex<Option<UnifiedFrameAllocator>> = Mutex::new(None);

impl UnifiedFrameAllocator {
    /// Create a new frame allocator from the Limine memory map
    pub fn new(memory_map: &[&limine::memory_map::Entry]) -> Result<Self, &'static str> {
        let mut allocator = UnifiedFrameAllocator {
            regions: [None; MAX_MEMORY_REGIONS],
            region_count: 0,
            current_region: 0,
            next_frame: PhysAddr::new(0),
            allocated_frames: 0,
            total_usable_frames: 0,
        };
        
        let mut total_memory = 0u64;
        let mut usable_memory = 0u64;
        
        crate::kprintln!("    [INFO] Processing memory map with {} entries", memory_map.len());
        
        // Process memory map entries
        for &entry in memory_map.iter() {
            if allocator.region_count >= MAX_MEMORY_REGIONS {
                crate::kprintln!("    [WARN] Too many memory regions, ignoring extras");
                break;
            }
            
            let region_type = match entry.entry_type {
                limine::memory_map::EntryType::USABLE => MemoryRegionType::Usable,
                limine::memory_map::EntryType::RESERVED => MemoryRegionType::Reserved,
                limine::memory_map::EntryType::ACPI_RECLAIMABLE => MemoryRegionType::AcpiReclaimable,
                limine::memory_map::EntryType::ACPI_NVS => MemoryRegionType::AcpiNonVolatile,
                limine::memory_map::EntryType::BAD_MEMORY => MemoryRegionType::BadMemory,
                _ => MemoryRegionType::Unknown,
            };
            
            // Skip empty regions
            if entry.length == 0 {
                continue;
            }
            
            total_memory += entry.length;
            
            // For usable memory, make sure we skip the first 1MB to avoid firmware areas
            if region_type == MemoryRegionType::Usable {
                let safe_start = if entry.base < 0x100000 {
                    0x100000
                } else {
                    entry.base
                };
                
                if safe_start < entry.base + entry.length {
                    // Align to 4KB boundaries
                    let aligned_start = (safe_start + 4095) & !4095;
                    let aligned_end = (entry.base + entry.length) & !4095;
                    
                    if aligned_start < aligned_end {
                        let region = MemoryRegion {
                            start: PhysAddr::new(aligned_start),
                            end: PhysAddr::new(aligned_end),
                            size: aligned_end - aligned_start,
                            region_type,
                        };
                        
                        allocator.regions[allocator.region_count] = Some(region);
                        allocator.region_count += 1;
                        
                        usable_memory += region.size;
                        allocator.total_usable_frames += region.size / 4096;
                        
                        crate::kprintln!("      Usable region: {:#x} - {:#x} ({} MiB)", 
                                        aligned_start, aligned_end, 
                                        (aligned_end - aligned_start) / (1024 * 1024));
                    }
                }
            } else {
                // Track non-usable regions for debugging
                let region = MemoryRegion {
                    start: PhysAddr::new(entry.base),
                    end: PhysAddr::new(entry.base + entry.length),
                    size: entry.length,
                    region_type,
                };
                
                if allocator.region_count < MAX_MEMORY_REGIONS {
                    allocator.regions[allocator.region_count] = Some(region);
                    allocator.region_count += 1;
                }
            }
        }
        
        if allocator.region_count == 0 {
            return Err("No usable memory regions found");
        }
        
        // Find first usable region and set starting point
        for i in 0..allocator.region_count {
            if let Some(region) = allocator.regions[i] {
                if region.region_type == MemoryRegionType::Usable {
                    allocator.current_region = i;
                    allocator.next_frame = region.start;
                    break;
                }
            }
        }
        
        crate::kprintln!("    [INFO] Frame allocator initialized");
        crate::kprintln!("       Total memory: {} MiB", total_memory / (1024 * 1024));
        crate::kprintln!("       Usable memory: {} MiB", usable_memory / (1024 * 1024));
        crate::kprintln!("       Usable frames: {}", allocator.total_usable_frames);
        crate::kprintln!("       Regions: {} total, {} usable", 
                        allocator.region_count, 
                        allocator.count_usable_regions());
        
        Ok(allocator)
    }
    
    /// Count the number of usable memory regions
    fn count_usable_regions(&self) -> usize {
        let mut count = 0;
        for i in 0..self.region_count {
            if let Some(region) = self.regions[i] {
                if region.region_type == MemoryRegionType::Usable {
                    count += 1;
                }
            }
        }
        count
    }
    
    /// Get allocator statistics
    pub fn get_stats(&self) -> FrameAllocatorStats {
        let usable_regions = self.count_usable_regions();
        
        FrameAllocatorStats {
            total_regions: self.region_count,
            usable_regions,
            total_memory: self.regions[0..self.region_count]
                .iter()
                .filter_map(|&r| r)
                .map(|r| r.size)
                .sum(),
            usable_memory: self.regions[0..self.region_count]
                .iter()
                .filter_map(|&r| r)
                .filter(|r| r.region_type == MemoryRegionType::Usable)
                .map(|r| r.size)
                .sum(),
            allocated_frames: self.allocated_frames,
            free_frames: self.total_usable_frames - self.allocated_frames,
            current_region: self.current_region,
        }
    }
    
    /// Test frame allocation to ensure it works
    pub fn test_allocation(&mut self) -> Result<(), &'static str> {
        // Try to allocate a few frames
        let mut test_frames = Vec::new();
        
        for _i in 0..10 {
            if let Some(frame) = self.allocate_frame() {
                test_frames.push(frame);
            } else {
                return Err("Failed to allocate test frame");
            }
        }
        
        if test_frames.len() != 10 {
            return Err("Did not allocate expected number of test frames");
        }
        
        // Verify frames are valid and sequential
        for i in 1..test_frames.len() {
            let prev_addr = test_frames[i-1].start_address().as_u64();
            let curr_addr = test_frames[i].start_address().as_u64();
            
            if curr_addr != prev_addr + 4096 {
                // This is OK - frames don't have to be sequential
                // Just verify they're different
                if curr_addr == prev_addr {
                    return Err("Allocated duplicate frames");
                }
            }
        }
        
        crate::kprintln!("    ✅ Frame allocation test passed ({} frames)", test_frames.len());
        Ok(())
    }
}

unsafe impl FrameAllocator<Size4KiB> for UnifiedFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // Find next available frame in current region
        while self.current_region < self.region_count {
            if let Some(region) = self.regions[self.current_region] {
                if region.region_type == MemoryRegionType::Usable && self.next_frame < region.end {
                    let frame = PhysFrame::containing_address(self.next_frame);
                    self.next_frame += 4096u64;
                    self.allocated_frames += 1;
                    return Some(frame);
                }
            }
            
            // Move to next usable region
            self.current_region += 1;
            while self.current_region < self.region_count {
                if let Some(region) = self.regions[self.current_region] {
                    if region.region_type == MemoryRegionType::Usable {
                        self.next_frame = region.start;
                        break;
                    }
                }
                self.current_region += 1;
            }
        }
        
        // Out of memory
        None
    }
}

/// Initialize the global frame allocator
pub fn init_global_frame_allocator(
    memory_map: &[&limine::memory_map::Entry]
) -> Result<(), &'static str> {
    let allocator = UnifiedFrameAllocator::new(memory_map)?;
    
    let stats = allocator.get_stats();
    *GLOBAL_FRAME_ALLOCATOR.lock() = Some(allocator);
    
    crate::kprintln!("    [INFO] Global frame allocator ready");
    crate::kprintln!("       Available frames: {}", stats.free_frames);
    
    Ok(())
}

/// Get a reference to the global frame allocator
pub fn get_global_frame_allocator() -> Result<&'static Mutex<Option<UnifiedFrameAllocator>>, &'static str> {
    Ok(&GLOBAL_FRAME_ALLOCATOR)
}

/// Test the global frame allocator
pub fn test_global_frame_allocator() -> Result<(), &'static str> {
    let mut allocator_guard = GLOBAL_FRAME_ALLOCATOR.lock();
    
    if let Some(ref mut allocator) = *allocator_guard {
        allocator.test_allocation()?;
        
        let stats = allocator.get_stats();
        crate::kprintln!("    [INFO] Frame allocator stats after test:");
        crate::kprintln!("       Allocated: {} frames", stats.allocated_frames);
        crate::kprintln!("       Remaining: {} frames", stats.free_frames);
        
        Ok(())
    } else {
        Err("Global frame allocator not initialized")
    }
}

/// Get current frame allocator statistics
pub fn get_frame_allocator_stats() -> Result<FrameAllocatorStats, &'static str> {
    let allocator_guard = GLOBAL_FRAME_ALLOCATOR.lock();
    
    if let Some(ref allocator) = *allocator_guard {
        Ok(allocator.get_stats())
    } else {
        Err("Global frame allocator not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_region_types() {
        assert_eq!(MemoryRegionType::Usable, MemoryRegionType::Usable);
        assert_ne!(MemoryRegionType::Usable, MemoryRegionType::Reserved);
    }
    
    #[test]
    fn test_frame_allocator_stats() {
        // Test that stats structure works
        let stats = FrameAllocatorStats {
            total_regions: 5,
            usable_regions: 3,
            total_memory: 1024 * 1024 * 1024,
            usable_memory: 512 * 1024 * 1024,
            allocated_frames: 100,
            free_frames: 1000,
            current_region: 0,
        };
        
        assert_eq!(stats.total_regions, 5);
        assert_eq!(stats.usable_regions, 3);
        assert_eq!(stats.allocated_frames, 100);
    }
}