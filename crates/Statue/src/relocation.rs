//! ELF relocation processing support.

use crate::error::{ElfError, Result};
use crate::header::ElfMachine;
use crate::symbol::SymbolResolver;

/// Relocation types for x86_64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RelocationType {
    /// No relocation
    None = 0,
    /// Direct 64 bit
    R64 = 1,
    /// PC relative 32 bit signed
    Pc32 = 2,
    /// 32 bit GOT entry
    Got32 = 3,
    /// 32 bit PLT address
    Plt32 = 4,
    /// Copy symbol at runtime
    Copy = 5,
    /// Create GOT entry
    GlobDat = 6,
    /// Create PLT entry
    JumpSlot = 7,
    /// Adjust by program base
    Relative = 8,
    /// 32 bit offset to GOT
    GotPcRel = 9,
    /// Direct 32 bit zero extended
    R32 = 10,
    /// Direct 32 bit sign extended
    R32S = 11,
    /// Direct 16 bit zero extended
    R16 = 12,
    /// PC relative 16 bit signed
    Pc16 = 13,
    /// Direct 8 bit sign extended
    R8 = 14,
    /// PC relative 8 bit signed
    Pc8 = 15,
}

/// Relocation types for AArch64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AArch64RelocationType {
    /// No relocation
    None = 0,
    /// Direct 64 bit
    Abs64 = 257,
    /// Direct 32 bit
    Abs32 = 258,
    /// Direct 16 bit
    Abs16 = 259,
    /// PC-relative 64 bit
    PcRel64 = 260,
    /// PC-relative 32 bit
    PcRel32 = 261,
    /// PC-relative 16 bit
    PcRel16 = 262,
    /// Copy symbol at runtime
    Copy = 1024,
    /// Create GOT entry
    GlobDat = 1025,
    /// Create PLT entry
    JumpSlot = 1026,
    /// Adjust by program base
    Relative = 1027,
}

/// Relocation types for RISC-V
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiscVRelocationType {
    /// No relocation
    None = 0,
    /// Direct 32 bit
    R32 = 1,
    /// Direct 64 bit
    R64 = 2,
    /// Adjust by program base
    Relative = 3,
    /// Copy symbol at runtime
    Copy = 4,
    /// Create PLT entry
    JumpSlot = 5,
    /// TLS descriptor
    TlsDtpmod32 = 6,
    /// TLS descriptor
    TlsDtpmod64 = 7,
    /// TLS descriptor
    TlsDtprel32 = 8,
    /// TLS descriptor
    TlsDtprel64 = 9,
    /// TLS descriptor
    TlsTprel32 = 10,
    /// TLS descriptor
    TlsTprel64 = 11,
}

/// Generic relocation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationType {
    /// x86_64 relocation
    X86_64(X86_64RelocationType),
    /// AArch64 relocation
    AArch64(AArch64RelocationType),
    /// RISC-V relocation
    RiscV(RiscVRelocationType),
    /// Unknown relocation type
    Unknown(u32),
}

impl RelocationType {
    /// Create relocation type from raw value and architecture
    pub fn from_raw(value: u32, machine: ElfMachine) -> Self {
        match machine {
            ElfMachine::X86_64 => match value {
                0 => RelocationType::X86_64(X86_64RelocationType::None),
                1 => RelocationType::X86_64(X86_64RelocationType::R64),
                2 => RelocationType::X86_64(X86_64RelocationType::Pc32),
                3 => RelocationType::X86_64(X86_64RelocationType::Got32),
                4 => RelocationType::X86_64(X86_64RelocationType::Plt32),
                5 => RelocationType::X86_64(X86_64RelocationType::Copy),
                6 => RelocationType::X86_64(X86_64RelocationType::GlobDat),
                7 => RelocationType::X86_64(X86_64RelocationType::JumpSlot),
                8 => RelocationType::X86_64(X86_64RelocationType::Relative),
                9 => RelocationType::X86_64(X86_64RelocationType::GotPcRel),
                10 => RelocationType::X86_64(X86_64RelocationType::R32),
                11 => RelocationType::X86_64(X86_64RelocationType::R32S),
                12 => RelocationType::X86_64(X86_64RelocationType::R16),
                13 => RelocationType::X86_64(X86_64RelocationType::Pc16),
                14 => RelocationType::X86_64(X86_64RelocationType::R8),
                15 => RelocationType::X86_64(X86_64RelocationType::Pc8),
                _ => RelocationType::Unknown(value),
            },
            ElfMachine::AArch64 => match value {
                0 => RelocationType::AArch64(AArch64RelocationType::None),
                257 => RelocationType::AArch64(AArch64RelocationType::Abs64),
                258 => RelocationType::AArch64(AArch64RelocationType::Abs32),
                259 => RelocationType::AArch64(AArch64RelocationType::Abs16),
                260 => RelocationType::AArch64(AArch64RelocationType::PcRel64),
                261 => RelocationType::AArch64(AArch64RelocationType::PcRel32),
                262 => RelocationType::AArch64(AArch64RelocationType::PcRel16),
                1024 => RelocationType::AArch64(AArch64RelocationType::Copy),
                1025 => RelocationType::AArch64(AArch64RelocationType::GlobDat),
                1026 => RelocationType::AArch64(AArch64RelocationType::JumpSlot),
                1027 => RelocationType::AArch64(AArch64RelocationType::Relative),
                _ => RelocationType::Unknown(value),
            },
            ElfMachine::RiscV => match value {
                0 => RelocationType::RiscV(RiscVRelocationType::None),
                1 => RelocationType::RiscV(RiscVRelocationType::R32),
                2 => RelocationType::RiscV(RiscVRelocationType::R64),
                3 => RelocationType::RiscV(RiscVRelocationType::Relative),
                4 => RelocationType::RiscV(RiscVRelocationType::Copy),
                5 => RelocationType::RiscV(RiscVRelocationType::JumpSlot),
                6 => RelocationType::RiscV(RiscVRelocationType::TlsDtpmod32),
                7 => RelocationType::RiscV(RiscVRelocationType::TlsDtpmod64),
                8 => RelocationType::RiscV(RiscVRelocationType::TlsDtprel32),
                9 => RelocationType::RiscV(RiscVRelocationType::TlsDtprel64),
                10 => RelocationType::RiscV(RiscVRelocationType::TlsTprel32),
                11 => RelocationType::RiscV(RiscVRelocationType::TlsTprel64),
                _ => RelocationType::Unknown(value),
            },
            _ => RelocationType::Unknown(value),
        }
    }
}

/// ELF relocation entry (Rel format)
#[derive(Debug, Clone)]
pub struct Relocation {
    /// Address of relocation
    pub offset: u64,
    /// Relocation type
    pub reloc_type: RelocationType,
    /// Symbol index
    pub symbol: u32,
}

/// ELF relocation entry with addend (Rela format)
#[derive(Debug, Clone)]
pub struct RelocationAddend {
    /// Address of relocation
    pub offset: u64,
    /// Relocation type
    pub reloc_type: RelocationType,
    /// Symbol index
    pub symbol: u32,
    /// Addend value
    pub addend: i64,
}

impl Relocation {
    /// Parse Rel relocation from data
    pub fn parse(data: &[u8], offset: usize, is_64bit: bool, is_little_endian: bool, machine: ElfMachine) -> Result<Self> {
        let entry_size = if is_64bit { 16 } else { 8 };

        if data.len() < offset + entry_size {
            return Err(ElfError::BufferTooSmall);
        }

        if is_64bit {
            let addr = read_u64(data, offset, is_little_endian);
            let info = read_u64(data, offset + 8, is_little_endian);
            let symbol = (info >> 32) as u32;
            let reloc_type = RelocationType::from_raw(info as u32, machine);

            Ok(Relocation {
                offset: addr,
                reloc_type,
                symbol,
            })
        } else {
            let addr = read_u32(data, offset, is_little_endian) as u64;
            let info = read_u32(data, offset + 4, is_little_endian);
            let symbol = info >> 8;
            let reloc_type = RelocationType::from_raw(info & 0xff, machine);

            Ok(Relocation {
                offset: addr,
                reloc_type,
                symbol,
            })
        }
    }
}

impl RelocationAddend {
    /// Parse Rela relocation from data
    pub fn parse(data: &[u8], offset: usize, is_64bit: bool, is_little_endian: bool, machine: ElfMachine) -> Result<Self> {
        let entry_size = if is_64bit { 24 } else { 12 };

        if data.len() < offset + entry_size {
            return Err(ElfError::BufferTooSmall);
        }

        if is_64bit {
            let addr = read_u64(data, offset, is_little_endian);
            let info = read_u64(data, offset + 8, is_little_endian);
            let addend = read_i64(data, offset + 16, is_little_endian);
            let symbol = (info >> 32) as u32;
            let reloc_type = RelocationType::from_raw(info as u32, machine);

            Ok(RelocationAddend {
                offset: addr,
                reloc_type,
                symbol,
                addend,
            })
        } else {
            let addr = read_u32(data, offset, is_little_endian) as u64;
            let info = read_u32(data, offset + 4, is_little_endian);
            let addend = read_i32(data, offset + 8, is_little_endian) as i64;
            let symbol = info >> 8;
            let reloc_type = RelocationType::from_raw(info & 0xff, machine);

            Ok(RelocationAddend {
                offset: addr,
                reloc_type,
                symbol,
                addend,
            })
        }
    }
}

/// Relocation iterator for Rel format
pub struct RelocationIter<'a> {
    data: &'a [u8],
    count: usize,
    current: usize,
    entry_size: usize,
    is_64bit: bool,
    is_little_endian: bool,
    machine: ElfMachine,
}

impl<'a> RelocationIter<'a> {
    /// Create a new relocation iterator
    pub fn new(section_data: &'a [u8], is_64bit: bool, is_little_endian: bool, machine: ElfMachine) -> Result<Self> {
        let entry_size = if is_64bit { 16 } else { 8 };
        let count = section_data.len() / entry_size;

        if section_data.len() % entry_size != 0 {
            return Err(ElfError::InvalidRelocation);
        }

        Ok(RelocationIter {
            data: section_data,
            count,
            current: 0,
            entry_size,
            is_64bit,
            is_little_endian,
            machine,
        })
    }

    /// Get relocation by index
    pub fn get(&self, index: usize) -> Result<Relocation> {
        if index >= self.count {
            return Err(ElfError::IndexOutOfBounds);
        }

        let offset = index * self.entry_size;
        Relocation::parse(self.data, offset, self.is_64bit, self.is_little_endian, self.machine)
    }

    /// Get the number of relocations
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if there are no relocations
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a> Iterator for RelocationIter<'a> {
    type Item = Result<Relocation>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        let offset = self.current * self.entry_size;
        let result = Relocation::parse(self.data, offset, self.is_64bit, self.is_little_endian, self.machine);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for RelocationIter<'a> {}

/// Relocation iterator for Rela format
pub struct RelocationAddendIter<'a> {
    data: &'a [u8],
    count: usize,
    current: usize,
    entry_size: usize,
    is_64bit: bool,
    is_little_endian: bool,
    machine: ElfMachine,
}

impl<'a> RelocationAddendIter<'a> {
    /// Create a new relocation addend iterator
    pub fn new(section_data: &'a [u8], is_64bit: bool, is_little_endian: bool, machine: ElfMachine) -> Result<Self> {
        let entry_size = if is_64bit { 24 } else { 12 };
        let count = section_data.len() / entry_size;

        if section_data.len() % entry_size != 0 {
            return Err(ElfError::InvalidRelocation);
        }

        Ok(RelocationAddendIter {
            data: section_data,
            count,
            current: 0,
            entry_size,
            is_64bit,
            is_little_endian,
            machine,
        })
    }

    /// Get relocation by index
    pub fn get(&self, index: usize) -> Result<RelocationAddend> {
        if index >= self.count {
            return Err(ElfError::IndexOutOfBounds);
        }

        let offset = index * self.entry_size;
        RelocationAddend::parse(self.data, offset, self.is_64bit, self.is_little_endian, self.machine)
    }

    /// Get the number of relocations
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if there are no relocations
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a> Iterator for RelocationAddendIter<'a> {
    type Item = Result<RelocationAddend>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        let offset = self.current * self.entry_size;
        let result = RelocationAddend::parse(self.data, offset, self.is_64bit, self.is_little_endian, self.machine);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for RelocationAddendIter<'a> {}

/// Relocation processor for applying relocations
pub struct RelocationProcessor {
    base_address: u64,
}

impl RelocationProcessor {
    /// Create a new relocation processor
    pub fn new(base_address: u64) -> Self {
        Self { base_address }
    }

    /// Apply a relocation without addend
    pub fn apply_relocation(
        &self,
        reloc: &Relocation,
        _symbol_resolver: &SymbolResolver,
        memory: &mut [u8],
    ) -> Result<()> {
        let symbol_value = if reloc.symbol == 0 {
            0
        } else {
            // Symbol lookup would require symbol table access
            // This is a simplified implementation
            0
        };

        self.apply_relocation_value(reloc.offset, reloc.reloc_type, symbol_value, 0, memory)
    }

    /// Apply a relocation with addend
    pub fn apply_relocation_addend(
        &self,
        reloc: &RelocationAddend,
        _symbol_resolver: &SymbolResolver,
        memory: &mut [u8],
    ) -> Result<()> {
        let symbol_value = if reloc.symbol == 0 {
            0
        } else {
            // Symbol lookup would require symbol table access
            // This is a simplified implementation
            0
        };

        self.apply_relocation_value(reloc.offset, reloc.reloc_type, symbol_value, reloc.addend, memory)
    }

    /// Apply relocation with computed values
    fn apply_relocation_value(
        &self,
        offset: u64,
        reloc_type: RelocationType,
        symbol_value: u64,
        addend: i64,
        memory: &mut [u8],
    ) -> Result<()> {
        let location = offset as usize;
        if location >= memory.len() {
            return Err(ElfError::InvalidOffset);
        }

        match reloc_type {
            RelocationType::X86_64(X86_64RelocationType::None) => {
                // No operation
            }
            RelocationType::X86_64(X86_64RelocationType::R64) => {
                let value = symbol_value.wrapping_add(addend as u64);
                if location + 8 <= memory.len() {
                    memory[location..location + 8].copy_from_slice(&value.to_le_bytes());
                } else {
                    return Err(ElfError::InvalidOffset);
                }
            }
            RelocationType::X86_64(X86_64RelocationType::Relative) => {
                let value = self.base_address.wrapping_add(addend as u64);
                if location + 8 <= memory.len() {
                    memory[location..location + 8].copy_from_slice(&value.to_le_bytes());
                } else {
                    return Err(ElfError::InvalidOffset);
                }
            }
            RelocationType::X86_64(X86_64RelocationType::R32) => {
                let value = symbol_value.wrapping_add(addend as u64) as u32;
                if location + 4 <= memory.len() {
                    memory[location..location + 4].copy_from_slice(&value.to_le_bytes());
                } else {
                    return Err(ElfError::InvalidOffset);
                }
            }
            _ => {
                return Err(ElfError::UnsupportedRelocation);
            }
        }

        Ok(())
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

fn read_i32(data: &[u8], offset: usize, little_endian: bool) -> i32 {
    let bytes = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
    if little_endian {
        i32::from_le_bytes(bytes)
    } else {
        i32::from_be_bytes(bytes)
    }
}

fn read_i64(data: &[u8], offset: usize, little_endian: bool) -> i64 {
    let bytes = [
        data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
    ];
    if little_endian {
        i64::from_le_bytes(bytes)
    } else {
        i64::from_be_bytes(bytes)
    }
}