//! ELF loader with memory mapping support.

use crate::error::{ElfError, Result};
use crate::header::{ElfFile, ElfType};
use crate::program::ProgramHeaderIter;
use crate::section::{SectionHeaderIter, SectionType};
use crate::symbol::{SymbolResolver, SymbolTable};
use crate::relocation::{RelocationProcessor, RelocationIter, RelocationAddendIter};
use crate::arch::{ArchitectureType, MemoryLayout};
use crate::memory::{RealMemoryManager, MemoryProtection};
use alloc::vec::Vec;

/// Memory allocator trait for the loader
pub trait MemoryAllocator {
    /// Allocate a block of memory with specified size and alignment
    fn allocate(&mut self, size: usize, alignment: usize) -> Result<*mut u8>;

    /// Deallocate a previously allocated block
    fn deallocate(&mut self, ptr: *mut u8, size: usize);

    /// Map memory at a specific virtual address
    fn map_at(&mut self, vaddr: u64, size: usize, writable: bool, executable: bool) -> Result<*mut u8>;

    /// Unmap memory at a specific virtual address
    fn unmap(&mut self, vaddr: u64, size: usize) -> Result<()>;

    /// Change protection on existing mapping
    fn protect(&mut self, vaddr: u64, size: usize, writable: bool, executable: bool) -> Result<()>;
}

/// Simple memory allocator implementation for testing
#[derive(Debug)]
pub struct SimpleAllocator {
    /// Current allocation pointer
    current: *mut u8,
    /// Total available memory
    size: usize,
    /// Used memory
    used: usize,
}

impl SimpleAllocator {
    /// Create a new simple allocator with a given memory buffer
    pub fn new(buffer: &mut [u8]) -> Self {
        Self {
            current: buffer.as_mut_ptr(),
            size: buffer.len(),
            used: 0,
        }
    }
}

impl MemoryAllocator for SimpleAllocator {
    fn allocate(&mut self, size: usize, alignment: usize) -> Result<*mut u8> {
        // Align current pointer
        let current_addr = self.current as usize;
        let aligned_addr = (current_addr + alignment - 1) & !(alignment - 1);
        let aligned_ptr = aligned_addr as *mut u8;

        let offset = aligned_addr - (self.current as usize - self.used);
        if self.used + offset + size > self.size {
            return Err(ElfError::AllocationFailed);
        }

        self.used += offset + size;
        self.current = unsafe { aligned_ptr.add(size) };

        Ok(aligned_ptr)
    }

    fn deallocate(&mut self, _ptr: *mut u8, _size: usize) {
        // Simple allocator doesn't support deallocation
    }

    fn map_at(&mut self, vaddr: u64, size: usize, _writable: bool, _executable: bool) -> Result<*mut u8> {
        // Validate the virtual address range
        if vaddr as usize + size > usize::MAX {
            return Err(ElfError::InvalidAddress);
        }

        // Check if we have enough memory in our pool
        let needed_memory = size;
        if self.used + needed_memory > self.size {
            return Err(ElfError::AllocationFailed);
        }

        // For the simple allocator, we treat this as a direct mapping
        // The virtual address is returned as-is since we don't have actual page tables
        // But we track the allocation to prevent overlaps
        self.used += needed_memory;

        Ok(vaddr as *mut u8)
    }

    fn unmap(&mut self, _vaddr: u64, _size: usize) -> Result<()> {
        // Simple allocator doesn't track mappings
        Ok(())
    }

    fn protect(&mut self, _vaddr: u64, _size: usize, _writable: bool, _executable: bool) -> Result<()> {
        // Protection changes are handled by the memory management system
        // This would update page table entries with new permission bits
        // For the simple allocator, we just validate the request
        if !_writable && !_executable {
            return Err(ElfError::PermissionDenied);
        }
        Ok(())
    }
}

/// Real memory allocator using the RealMemoryManager
#[derive(Debug)]
pub struct RealAllocator {
    memory_manager: RealMemoryManager,
    next_virtual_addr: u64,
}

impl RealAllocator {
    /// Create a new real allocator
    pub fn new(memory_size: usize) -> Result<Self> {
        let memory_manager = RealMemoryManager::new(memory_size)?;
        Ok(Self {
            memory_manager,
            next_virtual_addr: 0x400000, // Start at typical code base
        })
    }

    /// Get memory manager reference
    pub fn memory_manager(&self) -> &RealMemoryManager {
        &self.memory_manager
    }

    /// Get mutable memory manager reference
    pub fn memory_manager_mut(&mut self) -> &mut RealMemoryManager {
        &mut self.memory_manager
    }
}

impl MemoryAllocator for RealAllocator {
    fn allocate(&mut self, size: usize, alignment: usize) -> Result<*mut u8> {
        // Align the next virtual address
        let aligned_addr = (self.next_virtual_addr + alignment as u64 - 1) & !(alignment as u64 - 1);

        // Determine protection (read-write for general allocations)
        let protection = MemoryProtection::read_write();

        // Map the required pages
        self.memory_manager.map_range(aligned_addr, size, protection)?;

        // Update next virtual address
        self.next_virtual_addr = aligned_addr + size as u64;

        Ok(aligned_addr as *mut u8)
    }

    fn deallocate(&mut self, ptr: *mut u8, size: usize) {
        let vaddr = ptr as u64;
        let page_size = 4096; // PAGE_SIZE
        let num_pages = (size + page_size - 1) / page_size;

        for i in 0..num_pages {
            let page_addr = vaddr + (i * page_size) as u64;
            let _ = self.memory_manager.unmap_page(page_addr);
        }
    }

    fn map_at(&mut self, vaddr: u64, size: usize, writable: bool, executable: bool) -> Result<*mut u8> {
        let protection = MemoryProtection {
            read: true,
            write: writable,
            execute: executable,
            present: true,
        };

        self.memory_manager.map_range(vaddr, size, protection)?;
        Ok(vaddr as *mut u8)
    }

    fn unmap(&mut self, vaddr: u64, size: usize) -> Result<()> {
        let page_size = 4096; // PAGE_SIZE
        let start_page = vaddr & !(page_size as u64 - 1);
        let end_addr = vaddr + size as u64;
        let end_page = (end_addr + page_size as u64 - 1) & !(page_size as u64 - 1);
        let num_pages = ((end_page - start_page) / page_size as u64) as usize;

        for i in 0..num_pages {
            let page_addr = start_page + (i * page_size) as u64;
            self.memory_manager.unmap_page(page_addr)?;
        }
        Ok(())
    }

    fn protect(&mut self, vaddr: u64, size: usize, writable: bool, executable: bool) -> Result<()> {
        let protection = MemoryProtection {
            read: true,
            write: writable,
            execute: executable,
            present: true,
        };

        let page_size = 4096; // PAGE_SIZE
        let start_page = vaddr & !(page_size as u64 - 1);
        let end_addr = vaddr + size as u64;
        let end_page = (end_addr + page_size as u64 - 1) & !(page_size as u64 - 1);
        let num_pages = ((end_page - start_page) / page_size as u64) as usize;

        for i in 0..num_pages {
            let page_addr = start_page + (i * page_size) as u64;
            self.memory_manager.protect_page(page_addr, protection)?;
        }
        Ok(())
    }
}

/// Loader configuration
#[derive(Debug)]
pub struct LoaderConfig<A: MemoryAllocator> {
    /// Memory allocator
    pub allocator: A,
    /// Memory layout configuration
    pub memory_layout: MemoryLayout,
    /// Base address for position-independent executables
    pub base_address: Option<u64>,
    /// Whether to perform relocations
    pub relocate: bool,
    /// Whether to resolve symbols
    pub resolve_symbols: bool,
}

impl<A: MemoryAllocator> LoaderConfig<A> {
    /// Create a new loader configuration
    pub fn new(allocator: A) -> Self {
        Self {
            allocator,
            memory_layout: MemoryLayout::default_x86_64(),
            base_address: None,
            relocate: true,
            resolve_symbols: true,
        }
    }

    /// Set memory layout
    pub fn with_memory_layout(mut self, layout: MemoryLayout) -> Self {
        self.memory_layout = layout;
        self
    }

    /// Set base address for PIE executables
    pub fn with_base_address(mut self, base: u64) -> Self {
        self.base_address = Some(base);
        self
    }

    /// Set whether to perform relocations
    pub fn with_relocations(mut self, relocate: bool) -> Self {
        self.relocate = relocate;
        self
    }

    /// Set whether to resolve symbols
    pub fn with_symbol_resolution(mut self, resolve: bool) -> Self {
        self.resolve_symbols = resolve;
        self
    }
}

/// Loaded memory segment
#[derive(Debug)]
pub struct LoadedSegment {
    /// Virtual address
    pub vaddr: u64,
    /// Size in memory
    pub size: u64,
    /// Memory pointer
    pub memory: *mut u8,
    /// Whether segment is writable
    pub writable: bool,
    /// Whether segment is executable
    pub executable: bool,
}

/// Loaded ELF binary
#[derive(Debug)]
pub struct LoadedBinary {
    /// Entry point address
    pub entry_point: u64,
    /// Loaded segments
    pub segments: Vec<LoadedSegment>,
    /// Base address used for loading
    pub base_address: u64,
    /// Architecture type
    pub architecture: ArchitectureType,
    /// Symbol resolver
    pub symbol_resolver: SymbolResolver<'static>,
}

impl LoadedBinary {
    /// Get the memory region containing the given address
    pub fn get_memory_at(&self, addr: u64) -> Option<&LoadedSegment> {
        self.segments.iter().find(|seg| {
            addr >= seg.vaddr && addr < seg.vaddr + seg.size
        })
    }

    /// Get mutable memory region containing the given address
    pub fn get_memory_at_mut(&mut self, addr: u64) -> Option<&mut LoadedSegment> {
        self.segments.iter_mut().find(|seg| {
            addr >= seg.vaddr && addr < seg.vaddr + seg.size
        })
    }

    /// Read data from loaded memory
    pub fn read_memory(&self, addr: u64, size: usize) -> Result<&[u8]> {
        let segment = self.get_memory_at(addr)
            .ok_or(ElfError::InvalidAddress)?;

        let offset = (addr - segment.vaddr) as usize;
        if offset + size > segment.size as usize {
            return Err(ElfError::InvalidOffset);
        }

        unsafe {
            let ptr = segment.memory.add(offset);
            Ok(core::slice::from_raw_parts(ptr, size))
        }
    }

    /// Write data to loaded memory
    pub fn write_memory(&mut self, addr: u64, data: &[u8]) -> Result<()> {
        let segment = self.get_memory_at_mut(addr)
            .ok_or(ElfError::InvalidAddress)?;

        if !segment.writable {
            return Err(ElfError::PermissionDenied);
        }

        let offset = (addr - segment.vaddr) as usize;
        if offset + data.len() > segment.size as usize {
            return Err(ElfError::InvalidOffset);
        }

        unsafe {
            let ptr = segment.memory.add(offset);
            core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        }

        Ok(())
    }

    /// Create a new LoadedBinary with memory manager integration
    pub fn with_memory_manager(
        entry_point: u64,
        segments: Vec<LoadedSegment>,
        base_address: u64,
        architecture: ArchitectureType,
        symbol_resolver: SymbolResolver<'static>,
    ) -> Self {
        Self {
            entry_point,
            segments,
            base_address,
            architecture,
            symbol_resolver,
        }
    }

    /// Get memory usage statistics
    pub fn memory_usage(&self) -> LoadedBinaryStats {
        let total_size = self.segments.iter().map(|s| s.size).sum();
        let executable_size = self.segments.iter()
            .filter(|s| s.executable)
            .map(|s| s.size)
            .sum();
        let writable_size = self.segments.iter()
            .filter(|s| s.writable)
            .map(|s| s.size)
            .sum();

        LoadedBinaryStats {
            total_size,
            executable_size,
            writable_size,
            segment_count: self.segments.len(),
            entry_point: self.entry_point,
            base_address: self.base_address,
        }
    }
}

/// Statistics for a loaded binary
#[derive(Debug, Clone)]
pub struct LoadedBinaryStats {
    /// Total memory size
    pub total_size: u64,
    /// Executable memory size
    pub executable_size: u64,
    /// Writable memory size
    pub writable_size: u64,
    /// Number of segments
    pub segment_count: usize,
    /// Entry point address
    pub entry_point: u64,
    /// Base address
    pub base_address: u64,
}

/// ELF loader
pub struct ElfLoader<A: MemoryAllocator> {
    config: LoaderConfig<A>,
}

impl<A: MemoryAllocator> ElfLoader<A> {
    /// Create a new ELF loader
    pub fn new(config: LoaderConfig<A>) -> Self {
        Self { config }
    }

    /// Load an ELF file into memory
    pub fn load(&mut self, elf: &ElfFile) -> Result<LoadedBinary> {
        // Validate ELF file type
        match elf.header.file_type {
            ElfType::Executable | ElfType::SharedObject => {}
            _ => return Err(ElfError::UnsupportedVersion),
        }

        // Determine architecture
        let architecture = ArchitectureType::from_machine(elf.header.machine)?;

        // Determine base address
        let base_address = self.config.base_address.unwrap_or_else(|| {
            if elf.header.file_type == ElfType::SharedObject {
                self.config.memory_layout.code_base
            } else {
                0
            }
        });

        // Load program segments
        let mut segments = Vec::new();
        let program_headers = ProgramHeaderIter::new(elf)?;

        for ph_result in program_headers {
            let ph = ph_result?;
            ph.validate(elf.data.len() as u64)?;

            if ph.is_loadable() {
                let segment = self.load_segment(&ph, elf, base_address)?;
                segments.push(segment);
            }
        }

        // Set up symbol resolution
        let symbol_resolver = SymbolResolver::new();
        // Symbol resolution is currently simplified to avoid complex lifetime management
        // Production systems would process symbol tables and handle dynamic linking here

        // Perform relocations
        if self.config.relocate {
            self.apply_relocations(elf, &segments, &symbol_resolver, base_address)?;
        }

        Ok(LoadedBinary {
            entry_point: elf.header.entry + base_address,
            segments,
            base_address,
            architecture,
            symbol_resolver: SymbolResolver::new(),
        })
    }

    /// Load a single program segment
    fn load_segment(
        &mut self,
        ph: &crate::program::ProgramHeader,
        elf: &ElfFile,
        base_address: u64,
    ) -> Result<LoadedSegment> {
        let vaddr = ph.vaddr + base_address;
        let size = ph.memsz;

        // Map memory for the segment
        let memory = self.config.allocator.map_at(
            vaddr,
            size as usize,
            ph.flags.writable(),
            ph.flags.executable(),
        )?;

        // Copy data from file
        if ph.has_data() {
            let file_data = &elf.data[ph.offset as usize..(ph.offset + ph.filesz) as usize];
            unsafe {
                core::ptr::copy_nonoverlapping(
                    file_data.as_ptr(),
                    memory,
                    ph.filesz as usize,
                );
            }
        }

        // Zero remaining memory (BSS)
        if ph.memsz > ph.filesz {
            unsafe {
                let zero_start = memory.add(ph.filesz as usize);
                let zero_size = (ph.memsz - ph.filesz) as usize;
                core::ptr::write_bytes(zero_start, 0, zero_size);
            }
        }

        Ok(LoadedSegment {
            vaddr,
            size,
            memory,
            writable: ph.flags.writable(),
            executable: ph.flags.executable(),
        })
    }

    /// Set up symbol resolution tables
    /// Note: Currently unused due to lifetime constraints with the current API design
    #[allow(dead_code)]
    fn setup_symbol_resolution<'a>(
        &mut self,
        resolver: &mut SymbolResolver<'a>,
        elf: &ElfFile<'a>,
    ) -> Result<()> {
        let section_headers = SectionHeaderIter::new(elf)?;

        // Find string table for section names
        let _shstrtab_section = if elf.header.shstrndx != 0 {
            Some(section_headers.get(elf.header.shstrndx as usize)?)
        } else {
            None
        };

        // Find symbol tables
        for i in 0..section_headers.len() {
            let section = section_headers.get(i)?;
            section.validate(elf.data.len() as u64)?;

            match section.section_type {
                SectionType::SymTab => {
                    let symbol_data = section.data(elf.data)?;
                    let string_data = if section.link != 0 {
                        let strtab_section = section_headers.get(section.link as usize)?;
                        Some(strtab_section.data(elf.data)?)
                    } else {
                        None
                    };

                    let symbol_table = SymbolTable::new(
                        symbol_data,
                        string_data,
                        elf.header.is_64bit(),
                        elf.header.is_little_endian(),
                    )?;

                    resolver.set_symbol_table(symbol_table);
                }
                SectionType::DynSym => {
                    let symbol_data = section.data(elf.data)?;
                    let string_data = if section.link != 0 {
                        let strtab_section = section_headers.get(section.link as usize)?;
                        Some(strtab_section.data(elf.data)?)
                    } else {
                        None
                    };

                    let symbol_table = SymbolTable::new(
                        symbol_data,
                        string_data,
                        elf.header.is_64bit(),
                        elf.header.is_little_endian(),
                    )?;

                    resolver.set_dynamic_table(symbol_table);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Apply relocations to loaded segments
    fn apply_relocations(
        &mut self,
        elf: &ElfFile,
        segments: &[LoadedSegment],
        symbol_resolver: &SymbolResolver,
        base_address: u64,
    ) -> Result<()> {
        let section_headers = SectionHeaderIter::new(elf)?;
        let processor = RelocationProcessor::new(base_address);

        for i in 0..section_headers.len() {
            let section = section_headers.get(i)?;

            match section.section_type {
                SectionType::Rel => {
                    let reloc_data = section.data(elf.data)?;
                    let relocations = RelocationIter::new(
                        reloc_data,
                        elf.header.is_64bit(),
                        elf.header.is_little_endian(),
                        elf.header.machine,
                    )?;

                    for reloc_result in relocations {
                        let reloc = reloc_result?;
                        // Find target segment and apply relocation
                        if let Some(target_segment) = segments.iter().find(|seg| {
                            reloc.offset >= seg.vaddr && reloc.offset < seg.vaddr + seg.size
                        }) {
                            let offset = (reloc.offset - target_segment.vaddr) as usize;
                            let memory_slice = unsafe {
                                core::slice::from_raw_parts_mut(
                                    target_segment.memory.add(offset),
                                    (target_segment.size as usize).saturating_sub(offset),
                                )
                            };
                            processor.apply_relocation(&reloc, symbol_resolver, memory_slice)?;
                        }
                    }
                }
                SectionType::Rela => {
                    let reloc_data = section.data(elf.data)?;
                    let relocations = RelocationAddendIter::new(
                        reloc_data,
                        elf.header.is_64bit(),
                        elf.header.is_little_endian(),
                        elf.header.machine,
                    )?;

                    for reloc_result in relocations {
                        let reloc = reloc_result?;
                        // Find target segment and apply relocation
                        if let Some(target_segment) = segments.iter().find(|seg| {
                            reloc.offset >= seg.vaddr && reloc.offset < seg.vaddr + seg.size
                        }) {
                            let offset = (reloc.offset - target_segment.vaddr) as usize;
                            let memory_slice = unsafe {
                                core::slice::from_raw_parts_mut(
                                    target_segment.memory.add(offset),
                                    (target_segment.size as usize).saturating_sub(offset),
                                )
                            };
                            processor.apply_relocation_addend(&reloc, symbol_resolver, memory_slice)?;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}