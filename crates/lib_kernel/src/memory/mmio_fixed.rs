//! Memory-Mapped I/O (MMIO) management for kernel drivers
//!
//! This module provides safe abstractions for mapping and accessing MMIO regions,
//! particularly for hardware devices like USB controllers, network cards, etc.

use x86_64::{
    structures::paging::{
        Page, PageTableFlags, PhysFrame, Mapper, OffsetPageTable,
        Size4KiB, FrameAllocator, mapper::MapToError
    },
    PhysAddr, VirtAddr, instructions::tlb
};
use spin::Mutex;
use alloc::collections::BTreeMap;

/// MMIO region tracker for proper cleanup
static MMIO_REGIONS: Mutex<BTreeMap<usize, MmioRegion>> = Mutex::new(BTreeMap::new());

/// Information about a mapped MMIO region
#[derive(Debug, Clone)]
struct MmioRegion {
    physical_addr: PhysAddr,
    virtual_addr: VirtAddr,
    size: usize,
    page_count: usize,
}

/// Production-ready MMIO mapper using OffsetPageTable only (most common case)
pub struct MmioMapper {
    mapper: &'static mut OffsetPageTable<'static>,
    frame_allocator: &'static mut dyn FrameAllocator<Size4KiB>,
}

impl MmioMapper {
    /// Create a new MMIO mapper with OffsetPageTable
    pub fn new(
        mapper: &'static mut OffsetPageTable<'static>,
        frame_allocator: &'static mut dyn FrameAllocator<Size4KiB>,
    ) -> Self {
        Self {
            mapper,
            frame_allocator,
        }
    }

    /// Map a physical MMIO region to virtual memory with appropriate flags
    pub unsafe fn map_mmio(&mut self, phys_addr: PhysAddr, size: usize) -> Result<VirtAddr, MapToError<Size4KiB>> {
        // Ensure proper alignment
        let aligned_phys = PhysAddr::new(phys_addr.as_u64() & !0xFFF);
        let offset = phys_addr.as_u64() - aligned_phys.as_u64();
        let aligned_size = (size + offset as usize + 0xFFF) & !0xFFF;
        let page_count = aligned_size / 0x1000;

        // Find a suitable virtual address in kernel space
        let virt_addr = self.find_free_virtual_region(aligned_size)?;

        // Create page table entries with MMIO-appropriate flags
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_CACHE          // Disable caching for MMIO
            | PageTableFlags::WRITE_THROUGH;    // Write-through for consistency

        // Map each page in the region
        for i in 0..page_count {
            let page_phys = PhysFrame::from_start_address(
                aligned_phys + (i as u64 * 0x1000)
            ).map_err(|_| MapToError::FrameAllocationFailed)?;

            let page_virt = Page::<Size4KiB>::from_start_address(
                virt_addr + (i as u64 * 0x1000)
            ).map_err(|_| MapToError::FrameAllocationFailed)?;

            self.mapper.map_to(page_virt, page_phys, flags, self.frame_allocator)?;
        }

        // Flush TLB for all mapped pages
        for i in 0..page_count {
            let page_virt = Page::<Size4KiB>::from_start_address(virt_addr + (i as u64 * 0x1000))
                .map_err(|_| MapToError::FrameAllocationFailed)?;
            tlb::flush(page_virt.start_address());
        }

        // Track this mapping for cleanup
        let final_virt = virt_addr + offset;
        let region = MmioRegion {
            physical_addr: phys_addr,
            virtual_addr: final_virt,
            size,
            page_count,
        };

        MMIO_REGIONS.lock().insert(final_virt.as_u64() as usize, region);

        Ok(final_virt)
    }

    /// Unmap a previously mapped MMIO region
    pub fn unmap_mmio(&mut self, virt_addr: VirtAddr, _size: usize) -> Result<(), &'static str> {
        let mut regions = MMIO_REGIONS.lock();

        if let Some(region) = regions.remove(&(virt_addr.as_u64() as usize)) {
            // Calculate aligned addresses for unmapping
            let aligned_virt = VirtAddr::new(region.virtual_addr.as_u64() & !0xFFF);

            // Unmap each page
            for i in 0..region.page_count {
                let page = Page::<Size4KiB>::from_start_address(aligned_virt + (i as u64 * 0x1000))
                    .map_err(|_| "Invalid virtual address")?;

                match self.mapper.unmap(page) {
                    Ok((_, flush)) => flush.flush(),
                    Err(_) => continue, // Continue if unmap fails
                }
            }
        }

        Ok(())
    }

    /// Find a free virtual memory region in kernel space for MMIO mapping
    fn find_free_virtual_region(&self, size: usize) -> Result<VirtAddr, MapToError<Size4KiB>> {
        // Start searching from high kernel memory
        let mut search_addr = VirtAddr::new(0xFFFF_8000_0000_0000);
        let search_end = VirtAddr::new(0xFFFF_FFFF_0000_0000);
        let aligned_size = (size + 0xFFF) & !0xFFF;

        while search_addr < search_end {
            if self.is_virtual_region_free(search_addr, aligned_size) {
                return Ok(search_addr);
            }
            search_addr += 0x1000; // Try next page
        }

        Err(MapToError::FrameAllocationFailed)
    }

    /// Check if a virtual memory region is free
    fn is_virtual_region_free(&self, addr: VirtAddr, size: usize) -> bool {
        let page_count = (size + 0xFFF) / 0x1000;

        for i in 0..page_count {
            let page = Page::<Size4KiB>::from_start_address(addr + (i as u64 * 0x1000));
            if let Ok(page) = page {
                if self.mapper.translate_page(page).is_ok() {
                    return false; // Page is already mapped
                }
            }
        }

        true
    }
}

/// xHCI-compatible mapper that wraps our MMIO mapper
pub struct XhciMmioMapper {
    inner: MmioMapper,
}

impl XhciMmioMapper {
    /// Create a new XhciMmioMapper using OffsetPageTable
    pub fn new(
        mapper: &'static mut OffsetPageTable<'static>,
        frame_allocator: &'static mut dyn FrameAllocator<Size4KiB>,
    ) -> Self {
        Self {
            inner: MmioMapper::new(mapper, frame_allocator),
        }
    }

    /// Create a new XhciMmioMapper using the kernel's memory management
    pub fn new_from_kernel() -> Result<Self, &'static str> {
        let _mapper = super::get_kernel_mapper()
            .ok_or("Kernel mapper not available")?;
        let _frame_allocator = super::get_kernel_frame_allocator()
            .ok_or("Kernel frame allocator not available")?;

        // This requires proper integration with kernel memory management
        Err("Kernel memory management integration not yet complete")
    }
}

impl xhci::accessor::Mapper for XhciMmioMapper {
    unsafe fn map(&mut self, phys_addr: usize, bytes: usize) -> core::num::NonZeroUsize {
        crate::kprintln!("MMIO: Mapping phys={:#x} size={:#x}", phys_addr, bytes);

        let phys = PhysAddr::new(phys_addr as u64);
        match self.inner.map_mmio(phys, bytes) {
            Ok(virt_addr) => {
                crate::kprintln!("MMIO: Mapped to virt={:#x}", virt_addr.as_u64());
                core::num::NonZeroUsize::new(virt_addr.as_u64() as usize)
                    .expect("Virtual address should be non-zero")
            }
            Err(e) => {
                crate::kprintln!("MMIO: Failed to map: {:?}", e);
                panic!("Failed to map MMIO region: {:?}", e);
            }
        }
    }

    fn unmap(&mut self, virt_addr: usize, bytes: usize) {
        crate::kprintln!("MMIO: Unmapping virt={:#x} size={:#x}", virt_addr, bytes);

        let virt = VirtAddr::new(virt_addr as u64);
        if let Err(e) = self.inner.unmap_mmio(virt, bytes) {
            crate::kprintln!("MMIO: Failed to unmap: {}", e);
        }
    }
}

unsafe impl Send for XhciMmioMapper {}
unsafe impl Sync for XhciMmioMapper {}

/// Initialize MMIO mapping capabilities
pub fn init_mmio_mapping() {
    crate::kprintln!("Initializing production MMIO mapping system");
    // MMIO regions tracker is already initialized
}

/// Get MMIO mapping statistics for debugging
pub fn mmio_stats() -> (usize, usize) {
    let regions = MMIO_REGIONS.lock();
    let region_count = regions.len();
    let total_size = regions.values().map(|r| r.size).sum();
    (region_count, total_size)
}