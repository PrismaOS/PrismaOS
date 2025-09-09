use alloc::{string::String, vec::Vec};
use core::mem;
use elf::{ElfBytes, endian::LittleEndian, ParseError};
use x86_64::{
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
    VirtAddr, PhysAddr
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfLoadError {
    InvalidElf(&'static str),
    ParseError,
    UnsupportedArchitecture,
    UnsupportedBitness,
    NoEntryPoint,
    InvalidVirtualAddress,
    MemoryAllocationFailed,
    InvalidSegment,
    LoadSegmentFailed,
}

impl From<ParseError> for ElfLoadError {
    fn from(_: ParseError) -> Self {
        ElfLoadError::ParseError
    }
}

#[derive(Debug)]
pub struct LoadedProgram {
    pub entry_point: VirtAddr,
    pub stack_top: VirtAddr,
    pub heap_base: VirtAddr,
    pub loaded_segments: Vec<LoadedSegment>,
}

#[derive(Debug, Clone)]
pub struct LoadedSegment {
    pub virtual_addr: VirtAddr,
    pub size: u64,
    pub flags: SegmentFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentFlags {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

impl SegmentFlags {
    pub fn to_page_flags(&self) -> PageTableFlags {
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        
        if self.writable {
            flags |= PageTableFlags::WRITABLE;
        }
        
        if !self.executable {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        
        flags
    }
}

pub struct ElfLoader;

impl ElfLoader {
    /// Load an ELF binary from raw bytes
    pub fn load_elf<M, A>(
        elf_data: &[u8],
        mapper: &mut M,
        frame_allocator: &mut A,
    ) -> Result<LoadedProgram, ElfLoadError>
    where
        M: Mapper<Size4KiB>,
        A: FrameAllocator<Size4KiB>,
    {
        // Parse the ELF file
        let elf = ElfBytes::<LittleEndian>::minimal_parse(elf_data)?;
        
        // Validate ELF architecture and bitness
        Self::validate_elf(&elf)?;
        
        // Get entry point
        let entry_point = VirtAddr::new(elf.ehdr.e_entry);
        if entry_point.is_null() {
            return Err(ElfLoadError::NoEntryPoint);
        }
        
        // Load program segments
        let loaded_segments = Self::load_segments(&elf, elf_data, mapper, frame_allocator)?;
        
        // Set up stack and heap
        let stack_top = VirtAddr::new(0x7FFF_FF00_0000); // User stack top
        let heap_base = VirtAddr::new(0x0000_1000_0000);  // User heap base
        
        // Allocate stack pages
        Self::allocate_stack(stack_top, mapper, frame_allocator)?;
        
        Ok(LoadedProgram {
            entry_point,
            stack_top,
            heap_base,
            loaded_segments,
        })
    }
    
    fn validate_elf(elf: &ElfBytes<LittleEndian>) -> Result<(), ElfLoadError> {
        // Check architecture
        match elf.ehdr.e_machine {
            elf::abi::EM_X86_64 => {},
            _ => return Err(ElfLoadError::UnsupportedArchitecture),
        }
        
        // Check bitness  
        match elf.ehdr.class {
            elf::file::Class::ELF64 => {},
            _ => return Err(ElfLoadError::UnsupportedBitness),
        }
        
        // Check if it's an executable
        match elf.ehdr.e_type {
            elf::abi::ET_EXEC => {},
            elf::abi::ET_DYN => {}, // Position independent executables
            _ => return Err(ElfLoadError::InvalidElf("Not an executable")),
        }
        
        Ok(())
    }
    
    fn load_segments<M, A>(
        elf: &ElfBytes<LittleEndian>,
        elf_data: &[u8],
        mapper: &mut M,
        frame_allocator: &mut A,
    ) -> Result<Vec<LoadedSegment>, ElfLoadError>
    where
        M: Mapper<Size4KiB>,
        A: FrameAllocator<Size4KiB>,
    {
        let mut loaded_segments = Vec::new();
        
        // Get program header table
        let phdrs = elf.segments().ok_or(ElfLoadError::InvalidElf("No program headers"))?;
        
        for phdr in phdrs {
            // Only load LOAD segments
            if phdr.p_type != elf::abi::PT_LOAD {
                continue;
            }
            
            let virt_addr = VirtAddr::new(phdr.p_vaddr);
            let size = phdr.p_memsz;
            let file_size = phdr.p_filesz;
            let file_offset = phdr.p_offset as usize;
            
            // Validate virtual address
            if !virt_addr.is_aligned(4096u64) {
                return Err(ElfLoadError::InvalidVirtualAddress);
            }
            
            // Convert ELF flags to our segment flags
            let flags = SegmentFlags {
                readable: (phdr.p_flags & elf::abi::PF_R) != 0,
                writable: (phdr.p_flags & elf::abi::PF_W) != 0,
                executable: (phdr.p_flags & elf::abi::PF_X) != 0,
            };
            
            // Calculate pages needed
            let start_page = Page::containing_address(virt_addr);
            let end_addr = virt_addr + size - 1u64;
            let end_page = Page::containing_address(end_addr);
            let page_flags = flags.to_page_flags();
            
            // Allocate and map pages
            for page in Page::range_inclusive(start_page, end_page) {
                let frame = frame_allocator
                    .allocate_frame()
                    .ok_or(ElfLoadError::MemoryAllocationFailed)?;
                
                unsafe {
                    mapper
                        .map_to(page, frame, page_flags, frame_allocator)
                        .map_err(|_| ElfLoadError::LoadSegmentFailed)?
                        .flush();
                }
            }
            
            // Copy segment data from ELF file to memory
            if file_size > 0 {
                let segment_data = &elf_data[file_offset..(file_offset + file_size as usize)];
                unsafe {
                    let dest_ptr = virt_addr.as_mut_ptr::<u8>();
                    core::ptr::copy_nonoverlapping(
                        segment_data.as_ptr(),
                        dest_ptr,
                        file_size as usize,
                    );
                }
            }
            
            // Zero out BSS section (if memory size > file size)
            if size > file_size {
                let bss_start = virt_addr + file_size;
                let bss_size = size - file_size;
                unsafe {
                    let bss_ptr = bss_start.as_mut_ptr::<u8>();
                    core::ptr::write_bytes(bss_ptr, 0, bss_size as usize);
                }
            }
            
            loaded_segments.push(LoadedSegment {
                virtual_addr: virt_addr,
                size,
                flags,
            });
        }
        
        if loaded_segments.is_empty() {
            return Err(ElfLoadError::InvalidElf("No loadable segments found"));
        }
        
        Ok(loaded_segments)
    }
    
    fn allocate_stack<M, A>(
        stack_top: VirtAddr,
        mapper: &mut M,
        frame_allocator: &mut A,
    ) -> Result<(), ElfLoadError>
    where
        M: Mapper<Size4KiB>,
        A: FrameAllocator<Size4KiB>,
    {
        // Allocate stack pages (8 pages = 32KB stack)
        const STACK_PAGES: usize = 8;
        let stack_start = stack_top - (STACK_PAGES * 4096) as u64;
        
        let start_page = Page::containing_address(stack_start);
        let end_page = Page::containing_address(stack_top - 1u64);
        
        let stack_flags = PageTableFlags::PRESENT 
            | PageTableFlags::WRITABLE 
            | PageTableFlags::USER_ACCESSIBLE
            | PageTableFlags::NO_EXECUTE;
        
        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(ElfLoadError::MemoryAllocationFailed)?;
            
            unsafe {
                mapper
                    .map_to(page, frame, stack_flags, frame_allocator)
                    .map_err(|_| ElfLoadError::LoadSegmentFailed)?
                    .flush();
            }
        }
        
        Ok(())
    }
    
    /// Get the size of an ELF file without loading it
    pub fn get_elf_info(elf_data: &[u8]) -> Result<ElfInfo, ElfLoadError> {
        let elf = ElfBytes::<LittleEndian>::minimal_parse(elf_data)?;
        Self::validate_elf(&elf)?;
        
        let entry_point = elf.ehdr.e_entry;
        let mut memory_size = 0u64;
        let mut segments = Vec::new();
        
        if let Some(phdrs) = elf.segments() {
            for phdr in phdrs {
                if phdr.p_type == elf::abi::PT_LOAD {
                    memory_size += phdr.p_memsz;
                    segments.push(SegmentInfo {
                        virtual_addr: phdr.p_vaddr,
                        memory_size: phdr.p_memsz,
                        file_size: phdr.p_filesz,
                        flags: SegmentFlags {
                            readable: (phdr.p_flags & elf::abi::PF_R) != 0,
                            writable: (phdr.p_flags & elf::abi::PF_W) != 0,
                            executable: (phdr.p_flags & elf::abi::PF_X) != 0,
                        },
                    });
                }
            }
        }
        
        Ok(ElfInfo {
            entry_point,
            memory_size,
            segments,
        })
    }
}

#[derive(Debug)]
pub struct ElfInfo {
    pub entry_point: u64,
    pub memory_size: u64,
    pub segments: Vec<SegmentInfo>,
}

#[derive(Debug)]
pub struct SegmentInfo {
    pub virtual_addr: u64,
    pub memory_size: u64,
    pub file_size: u64,
    pub flags: SegmentFlags,
}

/// Helper function to create a simple test ELF binary
#[cfg(test)]
pub fn create_test_elf() -> Vec<u8> {
    // This would create a minimal valid ELF binary for testing
    // For now, we'll return empty - real implementation would need
    // a proper ELF binary with correct headers
    Vec::new()
}