//! ELF header parsing and validation.

use crate::error::{ElfError, Result};

/// ELF magic number bytes
pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

/// ELF class constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfClass {
    /// 32-bit ELF
    Elf32 = 1,
    /// 64-bit ELF
    Elf64 = 2,
}

/// ELF data encoding constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfData {
    /// Little-endian
    LittleEndian = 1,
    /// Big-endian
    BigEndian = 2,
}

/// ELF file types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfType {
    /// No file type
    None = 0,
    /// Relocatable file
    Relocatable = 1,
    /// Executable file
    Executable = 2,
    /// Shared object file
    SharedObject = 3,
    /// Core file
    Core = 4,
}

/// ELF machine architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfMachine {
    /// No machine
    None = 0,
    /// x86-64
    X86_64 = 62,
    /// AArch64
    AArch64 = 183,
    /// RISC-V
    RiscV = 243,
}

/// ELF identification bytes
#[derive(Debug, Clone, Copy)]
pub struct ElfIdent {
    /// Magic number
    pub magic: [u8; 4],
    /// File class
    pub class: ElfClass,
    /// Data encoding
    pub data: ElfData,
    /// File version
    pub version: u8,
    /// OS/ABI identification
    pub osabi: u8,
    /// ABI version
    pub abiversion: u8,
    /// Padding bytes
    pub padding: [u8; 7],
}

impl ElfIdent {
    /// Parse ELF identification from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(ElfError::BufferTooSmall);
        }

        let magic = [data[0], data[1], data[2], data[3]];
        if magic != ELF_MAGIC {
            return Err(ElfError::InvalidMagic);
        }

        let class = match data[4] {
            1 => ElfClass::Elf32,
            2 => ElfClass::Elf64,
            _ => return Err(ElfError::UnsupportedClass),
        };

        let data_encoding = match data[5] {
            1 => ElfData::LittleEndian,
            2 => ElfData::BigEndian,
            _ => return Err(ElfError::UnsupportedEncoding),
        };

        let version = data[6];
        if version != 1 {
            return Err(ElfError::UnsupportedVersion);
        }

        let osabi = data[7];
        let abiversion = data[8];
        let mut padding = [0u8; 7];
        padding.copy_from_slice(&data[9..16]);

        Ok(ElfIdent {
            magic,
            class,
            data: data_encoding,
            version,
            osabi,
            abiversion,
            padding,
        })
    }
}

/// ELF header structure
#[derive(Debug, Clone)]
pub struct ElfHeader {
    /// ELF identification
    pub ident: ElfIdent,
    /// Object file type
    pub file_type: ElfType,
    /// Architecture
    pub machine: ElfMachine,
    /// Object file version
    pub version: u32,
    /// Entry point virtual address
    pub entry: u64,
    /// Program header table file offset
    pub phoff: u64,
    /// Section header table file offset
    pub shoff: u64,
    /// Processor-specific flags
    pub flags: u32,
    /// ELF header size in bytes
    pub ehsize: u16,
    /// Program header table entry size
    pub phentsize: u16,
    /// Program header table entry count
    pub phnum: u16,
    /// Section header table entry size
    pub shentsize: u16,
    /// Section header table entry count
    pub shnum: u16,
    /// Section header string table index
    pub shstrndx: u16,
}

impl ElfHeader {
    /// Parse ELF header from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let ident = ElfIdent::parse(data)?;

        let header_size = match ident.class {
            ElfClass::Elf32 => 52,
            ElfClass::Elf64 => 64,
        };

        if data.len() < header_size {
            return Err(ElfError::BufferTooSmall);
        }

        let is_little_endian = ident.data == ElfData::LittleEndian;

        let file_type = match read_u16(data, 16, is_little_endian) {
            0 => ElfType::None,
            1 => ElfType::Relocatable,
            2 => ElfType::Executable,
            3 => ElfType::SharedObject,
            4 => ElfType::Core,
            _ => return Err(ElfError::InvalidHeader),
        };

        let machine = match read_u16(data, 18, is_little_endian) {
            0 => ElfMachine::None,
            62 => ElfMachine::X86_64,
            183 => ElfMachine::AArch64,
            243 => ElfMachine::RiscV,
            _ => return Err(ElfError::UnsupportedArchitecture),
        };

        let version = read_u32(data, 20, is_little_endian);
        if version != 1 {
            return Err(ElfError::UnsupportedVersion);
        }

        let (entry, phoff, shoff) = match ident.class {
            ElfClass::Elf32 => (
                read_u32(data, 24, is_little_endian) as u64,
                read_u32(data, 28, is_little_endian) as u64,
                read_u32(data, 32, is_little_endian) as u64,
            ),
            ElfClass::Elf64 => (
                read_u64(data, 24, is_little_endian),
                read_u64(data, 32, is_little_endian),
                read_u64(data, 40, is_little_endian),
            ),
        };

        let flags_offset = match ident.class {
            ElfClass::Elf32 => 36,
            ElfClass::Elf64 => 48,
        };

        let flags = read_u32(data, flags_offset, is_little_endian);
        let ehsize = read_u16(data, flags_offset + 4, is_little_endian);
        let phentsize = read_u16(data, flags_offset + 6, is_little_endian);
        let phnum = read_u16(data, flags_offset + 8, is_little_endian);
        let shentsize = read_u16(data, flags_offset + 10, is_little_endian);
        let shnum = read_u16(data, flags_offset + 12, is_little_endian);
        let shstrndx = read_u16(data, flags_offset + 14, is_little_endian);

        Ok(ElfHeader {
            ident,
            file_type,
            machine,
            version,
            entry,
            phoff,
            shoff,
            flags,
            ehsize,
            phentsize,
            phnum,
            shentsize,
            shnum,
            shstrndx,
        })
    }

    /// Check if this is a 64-bit ELF
    pub fn is_64bit(&self) -> bool {
        self.ident.class == ElfClass::Elf64
    }

    /// Check if this uses little-endian encoding
    pub fn is_little_endian(&self) -> bool {
        self.ident.data == ElfData::LittleEndian
    }
}

/// Complete ELF file representation
#[derive(Debug)]
pub struct ElfFile<'a> {
    /// Raw binary data
    pub data: &'a [u8],
    /// Parsed ELF header
    pub header: ElfHeader,
}

impl<'a> ElfFile<'a> {
    /// Parse an ELF file from binary data
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let header = ElfHeader::parse(data)?;

        // Validate header consistency
        if header.ehsize as usize != match header.ident.class {
            ElfClass::Elf32 => 52,
            ElfClass::Elf64 => 64,
        } {
            return Err(ElfError::InvalidHeader);
        }

        // Validate program header table
        if header.phnum > 0 {
            let expected_phentsize = match header.ident.class {
                ElfClass::Elf32 => 32,
                ElfClass::Elf64 => 56,
            };
            if header.phentsize != expected_phentsize {
                return Err(ElfError::InvalidProgramHeader);
            }

            let ph_table_size = header.phnum as u64 * header.phentsize as u64;
            let ph_end = header.phoff.checked_add(ph_table_size)
                .ok_or(ElfError::ArithmeticOverflow)?;

            if ph_end > data.len() as u64 {
                return Err(ElfError::InvalidOffset);
            }
        }

        // Validate section header table
        if header.shnum > 0 {
            let expected_shentsize = match header.ident.class {
                ElfClass::Elf32 => 40,
                ElfClass::Elf64 => 64,
            };
            if header.shentsize != expected_shentsize {
                return Err(ElfError::InvalidSectionHeader);
            }

            let sh_table_size = header.shnum as u64 * header.shentsize as u64;
            let sh_end = header.shoff.checked_add(sh_table_size)
                .ok_or(ElfError::ArithmeticOverflow)?;

            if sh_end > data.len() as u64 {
                return Err(ElfError::InvalidOffset);
            }

            if header.shstrndx >= header.shnum && header.shstrndx != 0 {
                return Err(ElfError::InvalidHeader);
            }
        }

        Ok(ElfFile { data, header })
    }

    /// Get the raw data slice
    pub fn data(&self) -> &[u8] {
        self.data
    }

    /// Get the ELF header
    pub fn header(&self) -> &ElfHeader {
        &self.header
    }
}

/// Read a 16-bit value with endianness handling
fn read_u16(data: &[u8], offset: usize, little_endian: bool) -> u16 {
    let bytes = [data[offset], data[offset + 1]];
    if little_endian {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    }
}

/// Read a 32-bit value with endianness handling
fn read_u32(data: &[u8], offset: usize, little_endian: bool) -> u32 {
    let bytes = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
    if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    }
}

/// Read a 64-bit value with endianness handling
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