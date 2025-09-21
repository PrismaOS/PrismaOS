//! ELF section header parsing and validation.

use crate::error::{ElfError, Result};
use crate::header::{ElfFile, ElfClass};

/// Section types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    /// Inactive section
    Null = 0,
    /// Program data
    ProgBits = 1,
    /// Symbol table
    SymTab = 2,
    /// String table
    StrTab = 3,
    /// Relocation entries with addends
    Rela = 4,
    /// Symbol hash table
    Hash = 5,
    /// Dynamic linking information
    Dynamic = 6,
    /// Notes
    Note = 7,
    /// Program space with no data (bss)
    NoBits = 8,
    /// Relocation entries, no addends
    Rel = 9,
    /// Reserved
    ShLib = 10,
    /// Dynamic linker symbol table
    DynSym = 11,
    /// Array of constructors
    InitArray = 14,
    /// Array of destructors
    FiniArray = 15,
    /// Array of pre-constructors
    PreInitArray = 16,
    /// Section group
    Group = 17,
    /// Extended section indices
    SymTabShndx = 18,
    /// Compressed section
    Compressed = 0x0ff00000,
    /// GNU version definitions
    GnuVerDef = 0x6ffffffd,
    /// GNU version needs
    GnuVerNeed = 0x6ffffffe,
    /// GNU version symbol table
    GnuVerSym = 0x6fffffff,
}

/// Section header flags
#[derive(Debug, Clone, Copy)]
pub struct SectionFlags {
    flags: u64,
}

impl SectionFlags {
    /// Create new section flags
    pub fn new(flags: u64) -> Self {
        Self { flags }
    }

    /// Section contains writable data
    pub fn writable(&self) -> bool {
        (self.flags & 0x1) != 0
    }

    /// Section occupies memory during execution
    pub fn alloc(&self) -> bool {
        (self.flags & 0x2) != 0
    }

    /// Section contains executable machine instructions
    pub fn executable(&self) -> bool {
        (self.flags & 0x4) != 0
    }

    /// Section may be merged
    pub fn merge(&self) -> bool {
        (self.flags & 0x10) != 0
    }

    /// Section contains null-terminated strings
    pub fn strings(&self) -> bool {
        (self.flags & 0x20) != 0
    }

    /// Section header table index
    pub fn info_link(&self) -> bool {
        (self.flags & 0x40) != 0
    }

    /// Special ordering requirement
    pub fn link_order(&self) -> bool {
        (self.flags & 0x80) != 0
    }

    /// Section requires special OS handling
    pub fn os_nonconforming(&self) -> bool {
        (self.flags & 0x100) != 0
    }

    /// Section is a member of a group
    pub fn group(&self) -> bool {
        (self.flags & 0x200) != 0
    }

    /// Section holds thread-local storage
    pub fn tls(&self) -> bool {
        (self.flags & 0x400) != 0
    }

    /// Section is compressed
    pub fn compressed(&self) -> bool {
        (self.flags & 0x800) != 0
    }

    /// Get raw flags value
    pub fn raw(&self) -> u64 {
        self.flags
    }
}

/// ELF section header
#[derive(Debug, Clone)]
pub struct SectionHeader {
    /// Section name (string table index)
    pub name: u32,
    /// Section type
    pub section_type: SectionType,
    /// Section flags
    pub flags: SectionFlags,
    /// Section virtual address at execution
    pub addr: u64,
    /// Section file offset
    pub offset: u64,
    /// Section size in bytes
    pub size: u64,
    /// Section header table index link
    pub link: u32,
    /// Extra information
    pub info: u32,
    /// Section alignment
    pub addralign: u64,
    /// Entry size if section holds table
    pub entsize: u64,
}

impl SectionHeader {
    /// Parse section header from data
    pub fn parse(data: &[u8], offset: usize, is_64bit: bool, is_little_endian: bool) -> Result<Self> {
        let entry_size = if is_64bit { 64 } else { 40 };

        if data.len() < offset + entry_size {
            return Err(ElfError::BufferTooSmall);
        }

        let name = read_u32(data, offset, is_little_endian);
        let section_type = match read_u32(data, offset + 4, is_little_endian) {
            0 => SectionType::Null,
            1 => SectionType::ProgBits,
            2 => SectionType::SymTab,
            3 => SectionType::StrTab,
            4 => SectionType::Rela,
            5 => SectionType::Hash,
            6 => SectionType::Dynamic,
            7 => SectionType::Note,
            8 => SectionType::NoBits,
            9 => SectionType::Rel,
            10 => SectionType::ShLib,
            11 => SectionType::DynSym,
            14 => SectionType::InitArray,
            15 => SectionType::FiniArray,
            16 => SectionType::PreInitArray,
            17 => SectionType::Group,
            18 => SectionType::SymTabShndx,
            0x0ff00000 => SectionType::Compressed,
            0x6ffffffd => SectionType::GnuVerDef,
            0x6ffffffe => SectionType::GnuVerNeed,
            0x6fffffff => SectionType::GnuVerSym,
            _ => return Err(ElfError::InvalidSectionHeader),
        };

        if is_64bit {
            let flags = SectionFlags::new(read_u64(data, offset + 8, is_little_endian));
            let addr = read_u64(data, offset + 16, is_little_endian);
            let file_offset = read_u64(data, offset + 24, is_little_endian);
            let size = read_u64(data, offset + 32, is_little_endian);
            let link = read_u32(data, offset + 40, is_little_endian);
            let info = read_u32(data, offset + 44, is_little_endian);
            let addralign = read_u64(data, offset + 48, is_little_endian);
            let entsize = read_u64(data, offset + 56, is_little_endian);

            Ok(SectionHeader {
                name,
                section_type,
                flags,
                addr,
                offset: file_offset,
                size,
                link,
                info,
                addralign,
                entsize,
            })
        } else {
            let flags = SectionFlags::new(read_u32(data, offset + 8, is_little_endian) as u64);
            let addr = read_u32(data, offset + 12, is_little_endian) as u64;
            let file_offset = read_u32(data, offset + 16, is_little_endian) as u64;
            let size = read_u32(data, offset + 20, is_little_endian) as u64;
            let link = read_u32(data, offset + 24, is_little_endian);
            let info = read_u32(data, offset + 28, is_little_endian);
            let addralign = read_u32(data, offset + 32, is_little_endian) as u64;
            let entsize = read_u32(data, offset + 36, is_little_endian) as u64;

            Ok(SectionHeader {
                name,
                section_type,
                flags,
                addr,
                offset: file_offset,
                size,
                link,
                info,
                addralign,
                entsize,
            })
        }
    }

    /// Validate section header
    pub fn validate(&self, file_size: u64) -> Result<()> {
        // Check file size bounds for sections with data
        if self.section_type != SectionType::NoBits && self.size > 0 {
            let end_offset = self.offset.checked_add(self.size)
                .ok_or(ElfError::ArithmeticOverflow)?;
            if end_offset > file_size {
                return Err(ElfError::InvalidOffset);
            }
        }

        // Check alignment
        if self.addralign > 0 && (self.addralign & (self.addralign - 1)) != 0 {
            return Err(ElfError::InvalidAlignment);
        }

        // Check address alignment
        if self.addralign > 1 && (self.addr % self.addralign) != 0 {
            return Err(ElfError::InvalidAlignment);
        }

        Ok(())
    }

    /// Get section data from the ELF file
    pub fn data<'a>(&self, elf_data: &'a [u8]) -> Result<&'a [u8]> {
        if self.section_type == SectionType::NoBits {
            return Ok(&[]);
        }

        if self.size == 0 {
            return Ok(&[]);
        }

        let start = self.offset as usize;
        let end = start.checked_add(self.size as usize)
            .ok_or(ElfError::ArithmeticOverflow)?;

        if end > elf_data.len() {
            return Err(ElfError::InvalidOffset);
        }

        Ok(&elf_data[start..end])
    }

    /// Check if section is allocatable
    pub fn is_alloc(&self) -> bool {
        self.flags.alloc()
    }

    /// Check if section contains strings
    pub fn is_string_table(&self) -> bool {
        self.section_type == SectionType::StrTab
    }

    /// Check if section contains symbols
    pub fn is_symbol_table(&self) -> bool {
        matches!(self.section_type, SectionType::SymTab | SectionType::DynSym)
    }

    /// Check if section contains relocations
    pub fn is_relocation_table(&self) -> bool {
        matches!(self.section_type, SectionType::Rel | SectionType::Rela)
    }
}

/// Section header table iterator
pub struct SectionHeaderIter<'a> {
    data: &'a [u8],
    offset: usize,
    count: usize,
    current: usize,
    entry_size: usize,
    is_64bit: bool,
    is_little_endian: bool,
}

impl<'a> SectionHeaderIter<'a> {
    /// Create a new section header iterator
    pub fn new(elf: &'a ElfFile) -> Result<Self> {
        let header = &elf.header;
        let is_64bit = header.ident.class == ElfClass::Elf64;
        let is_little_endian = header.is_little_endian();

        Ok(SectionHeaderIter {
            data: elf.data,
            offset: header.shoff as usize,
            count: header.shnum as usize,
            current: 0,
            entry_size: header.shentsize as usize,
            is_64bit,
            is_little_endian,
        })
    }

    /// Get section header by index
    pub fn get(&self, index: usize) -> Result<SectionHeader> {
        if index >= self.count {
            return Err(ElfError::IndexOutOfBounds);
        }

        let offset = self.offset + index * self.entry_size;
        SectionHeader::parse(self.data, offset, self.is_64bit, self.is_little_endian)
    }

    /// Get the number of section headers
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if there are no section headers
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a> Iterator for SectionHeaderIter<'a> {
    type Item = Result<SectionHeader>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        let offset = self.offset + self.current * self.entry_size;
        let result = SectionHeader::parse(self.data, offset, self.is_64bit, self.is_little_endian);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for SectionHeaderIter<'a> {}

/// String table for resolving section names
#[derive(Debug)]
pub struct StringTable<'a> {
    data: &'a [u8],
}

impl<'a> StringTable<'a> {
    /// Create a new string table from section data
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Get a string by offset
    pub fn get_string(&self, offset: u32) -> Result<&'a str> {
        let start = offset as usize;
        if start >= self.data.len() {
            return Err(ElfError::InvalidOffset);
        }

        // Find null terminator
        let end = self.data[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|pos| start + pos)
            .unwrap_or(self.data.len());

        let bytes = &self.data[start..end];
        core::str::from_utf8(bytes).map_err(|_| ElfError::InvalidStringTable)
    }
}

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