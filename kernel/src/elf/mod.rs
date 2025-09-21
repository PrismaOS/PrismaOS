/// PrismaOS ELF Loader
/// 
/// This module provides ELF (Executable and Linkable Format) loading capabilities
/// for userspace Rust programs. It handles parsing ELF files, validating them,
/// and loading them into userspace virtual memory with proper permissions.

use alloc::{vec::Vec, format};
use x86_64::{
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
    VirtAddr, PhysAddr,
};
use crate::{memory::BootInfoFrameAllocator, kprintln};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ElfHeader {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

#[derive(Debug)]
pub enum ElfError {
    InvalidMagic,
    UnsupportedClass,
    UnsupportedArch,
    InvalidHeader,
    LoadError,
}

pub struct ElfLoader {
    data: Vec<u8>,
    header: ElfHeader,
}

impl ElfLoader {
    pub fn new(data: Vec<u8>) -> Result<Self, ElfError> {
        if data.len() < core::mem::size_of::<ElfHeader>() {
            return Err(ElfError::InvalidHeader);
        }

        let header = unsafe {
            core::ptr::read(data.as_ptr() as *const ElfHeader)
        };

        // Verify ELF magic
        if &header.e_ident[0..4] != b"\x7fELF" {
            return Err(ElfError::InvalidMagic);
        }

        // Check for 64-bit
        if header.e_ident[4] != 2 {
            return Err(ElfError::UnsupportedClass);
        }

        // Check for x86_64
        if header.e_machine != 62 {
            return Err(ElfError::UnsupportedArch);
        }

        Ok(ElfLoader { data, header })
    }

    pub fn entry_point(&self) -> VirtAddr {
        VirtAddr::new(self.header.e_entry)
    }

    pub fn load_segments(
        &self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut BootInfoFrameAllocator,
    ) -> Result<(), ElfError> {
        let ph_offset = self.header.e_phoff as usize;
        let ph_size = self.header.e_phentsize as usize;
        let ph_count = self.header.e_phnum as usize;

        for i in 0..ph_count {
            let ph_start = ph_offset + i * ph_size;
            if ph_start + core::mem::size_of::<ProgramHeader>() > self.data.len() {
                return Err(ElfError::InvalidHeader);
            }

            let ph = unsafe {
                core::ptr::read((self.data.as_ptr().add(ph_start)) as *const ProgramHeader)
            };

            if ph.p_type == PT_LOAD {
                self.load_segment(&ph, mapper, frame_allocator)?;
            }
        }

        Ok(())
    }

    fn load_segment(
        &self,
        ph: &ProgramHeader,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut BootInfoFrameAllocator,
    ) -> Result<(), ElfError> {
        let virt_start = VirtAddr::new(ph.p_vaddr);
        let virt_end = virt_start + ph.p_memsz;
        
        kprintln!("    📦 Loading segment: {:#x}-{:#x} (file: {:#x}, mem: {:#x})", 
                 ph.p_vaddr, ph.p_vaddr + ph.p_memsz, ph.p_filesz, ph.p_memsz);
        
        let start_page = Page::containing_address(virt_start);
        let end_page = Page::containing_address(virt_end - 1u64);
        let page_range = Page::range_inclusive(start_page, end_page);

        // Determine page flags from segment flags
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if ph.p_flags & PF_W != 0 {
            flags |= PageTableFlags::WRITABLE;
        }
        if ph.p_flags & PF_X == 0 {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        let perms = format!("{}{}{}",
            if ph.p_flags & PF_R != 0 { "R" } else { "-" },
            if ph.p_flags & PF_W != 0 { "W" } else { "-" },
            if ph.p_flags & PF_X != 0 { "X" } else { "-" }
        );
        kprintln!("    🔒 Permissions: {}", perms);

        // Allocate and map pages
        for page in page_range {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(ElfError::LoadError)?;
            
            unsafe {
                mapper.map_to(page, frame, flags, frame_allocator)
                    .map_err(|_| ElfError::LoadError)?
                    .flush();
            }
        }

        // Copy segment data
        let file_data_start = ph.p_offset as usize;
        let file_data_end = file_data_start + ph.p_filesz as usize;
        
        if file_data_end > self.data.len() {
            return Err(ElfError::LoadError);
        }

        if ph.p_filesz > 0 {
            let src_slice = &self.data[file_data_start..file_data_end];
            let dst_ptr = ph.p_vaddr as *mut u8;
            
            unsafe {
                core::ptr::copy_nonoverlapping(
                    src_slice.as_ptr(),
                    dst_ptr,
                    ph.p_filesz as usize,
                );
            }
            kprintln!("    💾 Copied {} bytes from file", ph.p_filesz);
        }

        // Zero out BSS section if memsz > filesz
        if ph.p_memsz > ph.p_filesz {
            let bss_start = (ph.p_vaddr + ph.p_filesz) as *mut u8;
            let bss_size = (ph.p_memsz - ph.p_filesz) as usize;
            
            unsafe {
                core::ptr::write_bytes(bss_start, 0, bss_size);
            }
            kprintln!("    🧹 Zeroed {} bytes BSS section", bss_size);
        }

        Ok(())
    }
}