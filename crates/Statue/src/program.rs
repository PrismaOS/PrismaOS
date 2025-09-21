//! ELF program header parsing and validation.

use crate::error::{ElfError, Result};
use crate::header::{ElfFile, ElfClass};

/// Program header types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramType {
    /// Unused entry
    Null = 0,
    /// Loadable program segment
    Load = 1,
    /// Dynamic linking information
    Dynamic = 2,
    /// Program interpreter
    Interp = 3,
    /// Auxiliary information
    Note = 4,
    /// Reserved
    Shlib = 5,
    /// Entry for header table itself
    Phdr = 6,
    /// Thread-local storage template
    Tls = 7,
    /// GCC .eh_frame_hdr segment
    GnuEhFrame = 0x6474e550,
    /// Indicates stack executability
    GnuStack = 0x6474e551,
    /// Read-only after relocation
    GnuRelRo = 0x6474e552,
}

/// Program header flags
#[derive(Debug, Clone, Copy)]
pub struct ProgramFlags {
    flags: u32,
}

impl ProgramFlags {
    /// Create new program flags
    pub fn new(flags: u32) -> Self {
        Self { flags }
    }

    /// Check if segment is executable
    pub fn executable(&self) -> bool {
        (self.flags & 0x1) != 0
    }

    /// Check if segment is writable
    pub fn writable(&self) -> bool {
        (self.flags & 0x2) != 0
    }

    /// Check if segment is readable
    pub fn readable(&self) -> bool {
        (self.flags & 0x4) != 0
    }

    /// Get raw flags value
    pub fn raw(&self) -> u32 {
        self.flags
    }
}

/// ELF program header
#[derive(Debug, Clone)]
pub struct ProgramHeader {
    /// Segment type
    pub segment_type: ProgramType,
    /// Segment flags (32-bit ELF)
    pub flags: ProgramFlags,
    /// Segment file offset
    pub offset: u64,
    /// Segment virtual address
    pub vaddr: u64,
    /// Segment physical address
    pub paddr: u64,
    /// Segment size in file
    pub filesz: u64,
    /// Segment size in memory
    pub memsz: u64,
    /// Segment alignment
    pub align: u64,
}

impl ProgramHeader {
    /// Parse program header from data
    pub fn parse(data: &[u8], offset: usize, is_64bit: bool, is_little_endian: bool) -> Result<Self> {
        let entry_size = if is_64bit { 56 } else { 32 };

        if data.len() < offset + entry_size {
            return Err(ElfError::BufferTooSmall);
        }

        let segment_type = match read_u32(data, offset, is_little_endian) {
            0 => ProgramType::Null,
            1 => ProgramType::Load,
            2 => ProgramType::Dynamic,
            3 => ProgramType::Interp,
            4 => ProgramType::Note,
            5 => ProgramType::Shlib,
            6 => ProgramType::Phdr,
            7 => ProgramType::Tls,
            0x6474e550 => ProgramType::GnuEhFrame,
            0x6474e551 => ProgramType::GnuStack,
            0x6474e552 => ProgramType::GnuRelRo,
            _ => return Err(ElfError::InvalidProgramHeader),
        };

        if is_64bit {
            let flags = ProgramFlags::new(read_u32(data, offset + 4, is_little_endian));
            let file_offset = read_u64(data, offset + 8, is_little_endian);
            let vaddr = read_u64(data, offset + 16, is_little_endian);
            let paddr = read_u64(data, offset + 24, is_little_endian);
            let filesz = read_u64(data, offset + 32, is_little_endian);
            let memsz = read_u64(data, offset + 40, is_little_endian);
            let align = read_u64(data, offset + 48, is_little_endian);

            Ok(ProgramHeader {
                segment_type,
                flags,
                offset: file_offset,
                vaddr,
                paddr,
                filesz,
                memsz,
                align,
            })
        } else {
            let file_offset = read_u32(data, offset + 4, is_little_endian) as u64;
            let vaddr = read_u32(data, offset + 8, is_little_endian) as u64;
            let paddr = read_u32(data, offset + 12, is_little_endian) as u64;
            let filesz = read_u32(data, offset + 16, is_little_endian) as u64;
            let memsz = read_u32(data, offset + 20, is_little_endian) as u64;
            let flags = ProgramFlags::new(read_u32(data, offset + 24, is_little_endian));
            let align = read_u32(data, offset + 28, is_little_endian) as u64;

            Ok(ProgramHeader {
                segment_type,
                flags,
                offset: file_offset,
                vaddr,
                paddr,
                filesz,
                memsz,
                align,
            })
        }
    }

    /// Validate program header
    pub fn validate(&self, file_size: u64) -> Result<()> {
        // Check file size bounds
        if self.filesz > 0 {
            let end_offset = self.offset.checked_add(self.filesz)
                .ok_or(ElfError::ArithmeticOverflow)?;
            if end_offset > file_size {
                return Err(ElfError::InvalidOffset);
            }
        }

        // Check memory size consistency
        if self.memsz < self.filesz {
            return Err(ElfError::InvalidProgramHeader);
        }

        // Check alignment
        if self.align > 0 && (self.align & (self.align - 1)) != 0 {
            return Err(ElfError::InvalidAlignment);
        }

        // Check virtual address alignment
        if self.align > 1 && (self.vaddr % self.align) != (self.offset % self.align) {
            return Err(ElfError::InvalidAlignment);
        }

        Ok(())
    }

    /// Check if this is a loadable segment
    pub fn is_loadable(&self) -> bool {
        self.segment_type == ProgramType::Load
    }

    /// Check if this segment contains data
    pub fn has_data(&self) -> bool {
        self.filesz > 0
    }

    /// Get the virtual address range for this segment
    pub fn virtual_range(&self) -> (u64, u64) {
        (self.vaddr, self.vaddr + self.memsz)
    }

    /// Get the file offset range for this segment
    pub fn file_range(&self) -> (u64, u64) {
        (self.offset, self.offset + self.filesz)
    }
}

/// Program header table iterator
pub struct ProgramHeaderIter<'a> {
    data: &'a [u8],
    offset: usize,
    count: usize,
    current: usize,
    entry_size: usize,
    is_64bit: bool,
    is_little_endian: bool,
}

impl<'a> ProgramHeaderIter<'a> {
    /// Create a new program header iterator
    pub fn new(elf: &'a ElfFile) -> Result<Self> {
        let header = &elf.header;
        let is_64bit = header.ident.class == ElfClass::Elf64;
        let is_little_endian = header.is_little_endian();

        Ok(ProgramHeaderIter {
            data: elf.data,
            offset: header.phoff as usize,
            count: header.phnum as usize,
            current: 0,
            entry_size: header.phentsize as usize,
            is_64bit,
            is_little_endian,
        })
    }

    /// Get program header by index
    pub fn get(&self, index: usize) -> Result<ProgramHeader> {
        if index >= self.count {
            return Err(ElfError::IndexOutOfBounds);
        }

        let offset = self.offset + index * self.entry_size;
        ProgramHeader::parse(self.data, offset, self.is_64bit, self.is_little_endian)
    }

    /// Get the number of program headers
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if there are no program headers
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a> Iterator for ProgramHeaderIter<'a> {
    type Item = Result<ProgramHeader>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        let offset = self.offset + self.current * self.entry_size;
        let result = ProgramHeader::parse(self.data, offset, self.is_64bit, self.is_little_endian);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for ProgramHeaderIter<'a> {}

/// Utility functions for reading values
fn read_u32(data: &[u8], offset: usize, little_endian: bool) -> u32 {
    let bytes = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
    if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    }
}

fn read_u64(data: &[u8], offset: usize, little_endian: bool) -> u64 {
    let bytes = [
        data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
    ];
    if little_endian {
        u64::from_le_bytes(bytes)
    } else {
        u64::from_be_bytes(bytes)
    }
}