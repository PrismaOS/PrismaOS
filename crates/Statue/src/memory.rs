//! Real memory management with page tables and protection

use crate::error::{ElfError, Result};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;

/// Page size constants
pub const PAGE_SIZE: usize = 4096;
/// Bitmask for the offset within a page (used to extract the offset from an address).
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

/// Memory protection flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryProtection {
    /// Page is readable
    pub read: bool,
    /// Page is writable
    pub write: bool,
    /// Page is executable
    pub execute: bool,
    /// Page is present in memory
    pub present: bool,
}

impl MemoryProtection {
    /// Create read-only protection
    pub fn read_only() -> Self {
        Self { read: true, write: false, execute: false, present: true }
    }

    /// Create read-write protection
    pub fn read_write() -> Self {
        Self { read: true, write: true, execute: false, present: true }
    }

    /// Create read-execute protection
    pub fn read_execute() -> Self {
        Self { read: true, write: false, execute: true, present: true }
    }

    /// Create read-write-execute protection
    pub fn read_write_execute() -> Self {
        Self { read: true, write: true, execute: true, present: true }
    }
}

/// Virtual memory page
#[derive(Debug)]
struct VirtualPage {
    /// Virtual address (page-aligned)
    vaddr: u64,
    /// Physical address (page-aligned)
    paddr: u64,
    /// Memory protection
    protection: MemoryProtection,
    /// Reference count
    ref_count: usize,
}

/// Page table entry (simplified x86_64 format)
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct PageTableEntry(u64);

impl PageTableEntry {
    /// Create a new page table entry
    fn new(paddr: u64, protection: MemoryProtection) -> Self {
        let mut entry = paddr & !PAGE_MASK as u64;

        if protection.present { entry |= 1 << 0; }    // Present
        if protection.write { entry |= 1 << 1; }      // Writable
        if !protection.execute { entry |= 1 << 63; }  // NX bit

        Self(entry)
    }

    /// Get physical address from entry
    fn physical_addr(&self) -> u64 {
        self.0 & !PAGE_MASK as u64
    }

    /// Check if page is present
    fn is_present(&self) -> bool {
        (self.0 & 1) != 0
    }
}

/// Real memory manager with page tables
#[derive(Debug)]
pub struct RealMemoryManager {
    /// Physical memory pool
    physical_memory: Vec<u8>,
    /// Virtual to physical page mapping
    page_mappings: BTreeMap<u64, VirtualPage>,
    /// Free physical pages
    free_pages: Vec<u64>,
    /// Next virtual address for allocation
    next_vaddr: u64,
    /// Memory regions for different purposes
    code_region: (u64, u64),
    data_region: (u64, u64),
    heap_region: (u64, u64),
    stack_region: (u64, u64),
}

impl RealMemoryManager {
    /// Create a new memory manager
    pub fn new(memory_size: usize) -> Result<Self> {
        if memory_size % PAGE_SIZE != 0 {
            return Err(ElfError::InvalidAlignment);
        }

        let num_pages = memory_size / PAGE_SIZE;
    let physical_memory = vec![0u8; memory_size];
        let mut free_pages = Vec::with_capacity(num_pages);

        // Initialize free page list
        for i in 0..num_pages {
            free_pages.push((i * PAGE_SIZE) as u64);
        }

        // Set up memory layout
        let code_start = 0x400000;
        let data_start = 0x600000;
        let heap_start = 0x800000;
        let stack_start = 0x7ffff000;

        Ok(Self {
            physical_memory,
            page_mappings: BTreeMap::new(),
            free_pages,
            next_vaddr: code_start,
            code_region: (code_start, code_start + 0x200000),  // 2MB for code
            data_region: (data_start, data_start + 0x200000),  // 2MB for data
            heap_region: (heap_start, heap_start + 0x10000000), // 256MB for heap
            stack_region: (stack_start - 0x100000, stack_start), // 1MB for stack
        })
    }

    /// Allocate a physical page
    fn allocate_physical_page(&mut self) -> Result<u64> {
        self.free_pages.pop().ok_or(ElfError::AllocationFailed)
    }

    /// Free a physical page
    fn free_physical_page(&mut self, paddr: u64) {
        self.free_pages.push(paddr);
    }

    /// Map a virtual page to a physical page
    pub fn map_page(&mut self, vaddr: u64, protection: MemoryProtection) -> Result<u64> {
        let page_vaddr = vaddr & !(PAGE_MASK as u64);

        // Check if already mapped
        if self.page_mappings.contains_key(&page_vaddr) {
            return Err(ElfError::InvalidAddress);
        }

        // Allocate physical page
        let paddr = self.allocate_physical_page()?;

        // Create mapping
        let page = VirtualPage {
            vaddr: page_vaddr,
            paddr,
            protection,
            ref_count: 1,
        };

        self.page_mappings.insert(page_vaddr, page);
        Ok(paddr)
    }

    /// Unmap a virtual page
    pub fn unmap_page(&mut self, vaddr: u64) -> Result<()> {
        let page_vaddr = vaddr & !(PAGE_MASK as u64);

        if let Some(page) = self.page_mappings.remove(&page_vaddr) {
            self.free_physical_page(page.paddr);
            Ok(())
        } else {
            Err(ElfError::InvalidAddress)
        }
    }

    /// Change page protection
    pub fn protect_page(&mut self, vaddr: u64, protection: MemoryProtection) -> Result<()> {
        let page_vaddr = vaddr & !(PAGE_MASK as u64);

        if let Some(page) = self.page_mappings.get_mut(&page_vaddr) {
            page.protection = protection;
            Ok(())
        } else {
            Err(ElfError::InvalidAddress)
        }
    }

    /// Get physical address for virtual address
    pub fn virtual_to_physical(&self, vaddr: u64) -> Result<u64> {
        let page_vaddr = vaddr & !(PAGE_MASK as u64);
        let offset = vaddr & PAGE_MASK as u64;

        if let Some(page) = self.page_mappings.get(&page_vaddr) {
            Ok(page.paddr + offset)
        } else {
            Err(ElfError::InvalidAddress)
        }
    }

    /// Read from virtual memory
    pub fn read_virtual(&self, vaddr: u64, size: usize) -> Result<&[u8]> {
        // For simplicity, assume single page access
        let paddr = self.virtual_to_physical(vaddr)?;
        let offset = paddr as usize;

        if offset + size > self.physical_memory.len() {
            return Err(ElfError::InvalidOffset);
        }

        Ok(&self.physical_memory[offset..offset + size])
    }

    /// Write to virtual memory
    pub fn write_virtual(&mut self, vaddr: u64, data: &[u8]) -> Result<()> {
        // Check write permission
        let page_vaddr = vaddr & !(PAGE_MASK as u64);
        if let Some(page) = self.page_mappings.get(&page_vaddr) {
            if !page.protection.write {
                return Err(ElfError::PermissionDenied);
            }
        } else {
            return Err(ElfError::InvalidAddress);
        }

        let paddr = self.virtual_to_physical(vaddr)?;
        let offset = paddr as usize;

        if offset + data.len() > self.physical_memory.len() {
            return Err(ElfError::InvalidOffset);
        }

        self.physical_memory[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Map a contiguous range of virtual memory
    pub fn map_range(&mut self, vaddr: u64, size: usize, protection: MemoryProtection) -> Result<Vec<u64>> {
        let start_page = vaddr & !(PAGE_MASK as u64);
        let end_addr = vaddr + size as u64;
        let end_page = (end_addr + PAGE_MASK as u64) & !(PAGE_MASK as u64);
        let num_pages = ((end_page - start_page) / PAGE_SIZE as u64) as usize;

        let mut mapped_pages = Vec::new();

        for i in 0..num_pages {
            let page_vaddr = start_page + (i * PAGE_SIZE) as u64;
            match self.map_page(page_vaddr, protection) {
                Ok(paddr) => mapped_pages.push(paddr),
                Err(e) => {
                    // Cleanup on failure
                    for &paddr in &mapped_pages {
                        let _ = self.unmap_page(paddr);
                    }
                    return Err(e);
                }
            }
        }

        Ok(mapped_pages)
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> MemoryStats {
        let total_pages = self.physical_memory.len() / PAGE_SIZE;
        let free_pages = self.free_pages.len();
        let used_pages = total_pages - free_pages;

        MemoryStats {
            total_memory: self.physical_memory.len(),
            used_memory: used_pages * PAGE_SIZE,
            free_memory: free_pages * PAGE_SIZE,
            mapped_pages: self.page_mappings.len(),
        }
    }
}

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Total memory size
    pub total_memory: usize,
    /// Used memory size
    pub used_memory: usize,
    /// Free memory size
    pub free_memory: usize,
    /// Number of mapped pages
    pub mapped_pages: usize,
}

/// Page fault handler type
pub type PageFaultHandler = fn(vaddr: u64, error_code: u64) -> Result<()>;

/// Memory management unit for handling page faults and TLB
pub struct MemoryManagementUnit {
    /// Page fault handler
    fault_handler: Option<PageFaultHandler>,
    /// TLB cache (simplified)
    tlb_cache: BTreeMap<u64, u64>,
}

impl MemoryManagementUnit {
    /// Create a new MMU
    pub fn new() -> Self {
        Self {
            fault_handler: None,
            tlb_cache: BTreeMap::new(),
        }
    }

    /// Set page fault handler
    pub fn set_fault_handler(&mut self, handler: PageFaultHandler) {
        self.fault_handler = Some(handler);
    }

    /// Handle page fault
    pub fn handle_page_fault(&self, vaddr: u64, error_code: u64) -> Result<()> {
        if let Some(handler) = self.fault_handler {
            handler(vaddr, error_code)
        } else {
            Err(ElfError::InvalidAddress)
        }
    }

    /// Flush TLB cache
    pub fn flush_tlb(&mut self) {
        self.tlb_cache.clear();
    }

    /// Invalidate TLB entry
    pub fn invalidate_tlb_entry(&mut self, vaddr: u64) {
        let page_vaddr = vaddr & !(PAGE_MASK as u64);
        self.tlb_cache.remove(&page_vaddr);
    }
}

impl Default for MemoryManagementUnit {
    fn default() -> Self {
        Self::new()
    }
}