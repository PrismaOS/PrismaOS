//! ELF symbol table parsing and resolution.

use crate::error::{ElfError, Result};
use crate::section::StringTable;
use alloc::vec::Vec;

/// Symbol binding types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolBinding {
    /// Local symbol
    Local = 0,
    /// Global symbol
    Global = 1,
    /// Weak symbol
    Weak = 2,
}

/// Symbol types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolType {
    /// No type
    NoType = 0,
    /// Data object
    Object = 1,
    /// Function
    Func = 2,
    /// Section
    Section = 3,
    /// File name
    File = 4,
    /// Common symbol
    Common = 5,
    /// Thread-local storage
    Tls = 6,
}

/// Symbol visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolVisibility {
    /// Default visibility
    Default = 0,
    /// Internal visibility
    Internal = 1,
    /// Hidden visibility
    Hidden = 2,
    /// Protected visibility
    Protected = 3,
}

/// Special section indices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolSection {
    /// Undefined section
    Undefined,
    /// Absolute value
    Absolute,
    /// Common symbol
    Common,
    /// Index in section header table
    Index(u16),
}

impl SymbolSection {
    /// Create from raw section index
    pub fn from_raw(index: u16) -> Self {
        match index {
            0 => SymbolSection::Undefined,
            0xfff1 => SymbolSection::Absolute,
            0xfff2 => SymbolSection::Common,
            idx => SymbolSection::Index(idx),
        }
    }

    /// Get raw section index
    pub fn raw(&self) -> u16 {
        match self {
            SymbolSection::Undefined => 0,
            SymbolSection::Absolute => 0xfff1,
            SymbolSection::Common => 0xfff2,
            SymbolSection::Index(idx) => *idx,
        }
    }
}

/// ELF symbol table entry
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name (string table index)
    pub name: u32,
    /// Symbol binding
    pub binding: SymbolBinding,
    /// Symbol type
    pub symbol_type: SymbolType,
    /// Symbol visibility
    pub visibility: SymbolVisibility,
    /// Symbol value
    pub value: u64,
    /// Symbol size
    pub size: u64,
    /// Section index
    pub section: SymbolSection,
}

impl Symbol {
    /// Parse symbol from data
    pub fn parse(data: &[u8], offset: usize, is_64bit: bool, is_little_endian: bool) -> Result<Self> {
        let entry_size = if is_64bit { 24 } else { 16 };

        if data.len() < offset + entry_size {
            return Err(ElfError::BufferTooSmall);
        }

        if is_64bit {
            let name = read_u32(data, offset, is_little_endian);
            let info = data[offset + 4];
            let other = data[offset + 5];
            let shndx = read_u16(data, offset + 6, is_little_endian);
            let value = read_u64(data, offset + 8, is_little_endian);
            let size = read_u64(data, offset + 16, is_little_endian);

            let binding = match info >> 4 {
                0 => SymbolBinding::Local,
                1 => SymbolBinding::Global,
                2 => SymbolBinding::Weak,
                _ => return Err(ElfError::InvalidSymbol),
            };

            let symbol_type = match info & 0xf {
                0 => SymbolType::NoType,
                1 => SymbolType::Object,
                2 => SymbolType::Func,
                3 => SymbolType::Section,
                4 => SymbolType::File,
                5 => SymbolType::Common,
                6 => SymbolType::Tls,
                _ => return Err(ElfError::InvalidSymbol),
            };

            let visibility = match other & 0x3 {
                0 => SymbolVisibility::Default,
                1 => SymbolVisibility::Internal,
                2 => SymbolVisibility::Hidden,
                3 => SymbolVisibility::Protected,
                _ => return Err(ElfError::InvalidSymbol),
            };

            Ok(Symbol {
                name,
                binding,
                symbol_type,
                visibility,
                value,
                size,
                section: SymbolSection::from_raw(shndx),
            })
        } else {
            let name = read_u32(data, offset, is_little_endian);
            let value = read_u32(data, offset + 4, is_little_endian) as u64;
            let size = read_u32(data, offset + 8, is_little_endian) as u64;
            let info = data[offset + 12];
            let other = data[offset + 13];
            let shndx = read_u16(data, offset + 14, is_little_endian);

            let binding = match info >> 4 {
                0 => SymbolBinding::Local,
                1 => SymbolBinding::Global,
                2 => SymbolBinding::Weak,
                _ => return Err(ElfError::InvalidSymbol),
            };

            let symbol_type = match info & 0xf {
                0 => SymbolType::NoType,
                1 => SymbolType::Object,
                2 => SymbolType::Func,
                3 => SymbolType::Section,
                4 => SymbolType::File,
                5 => SymbolType::Common,
                6 => SymbolType::Tls,
                _ => return Err(ElfError::InvalidSymbol),
            };

            let visibility = match other & 0x3 {
                0 => SymbolVisibility::Default,
                1 => SymbolVisibility::Internal,
                2 => SymbolVisibility::Hidden,
                3 => SymbolVisibility::Protected,
                _ => return Err(ElfError::InvalidSymbol),
            };

            Ok(Symbol {
                name,
                binding,
                symbol_type,
                visibility,
                value,
                size,
                section: SymbolSection::from_raw(shndx),
            })
        }
    }

    /// Check if symbol is undefined
    pub fn is_undefined(&self) -> bool {
        self.section == SymbolSection::Undefined
    }

    /// Check if symbol is global
    pub fn is_global(&self) -> bool {
        self.binding == SymbolBinding::Global
    }

    /// Check if symbol is weak
    pub fn is_weak(&self) -> bool {
        self.binding == SymbolBinding::Weak
    }

    /// Check if symbol is a function
    pub fn is_function(&self) -> bool {
        self.symbol_type == SymbolType::Func
    }

    /// Check if symbol is a data object
    pub fn is_object(&self) -> bool {
        self.symbol_type == SymbolType::Object
    }
}

/// Symbol table iterator
#[derive(Debug)]
pub struct SymbolIter<'a> {
    data: &'a [u8],
    count: usize,
    current: usize,
    entry_size: usize,
    is_64bit: bool,
    is_little_endian: bool,
}

impl<'a> SymbolIter<'a> {
    /// Create a new symbol iterator from section data
    pub fn new(section_data: &'a [u8], is_64bit: bool, is_little_endian: bool) -> Result<Self> {
        let entry_size = if is_64bit { 24 } else { 16 };
        let count = section_data.len() / entry_size;

        if section_data.len() % entry_size != 0 {
            return Err(ElfError::InvalidSymbol);
        }

        Ok(SymbolIter {
            data: section_data,
            count,
            current: 0,
            entry_size,
            is_64bit,
            is_little_endian,
        })
    }

    /// Get symbol by index
    pub fn get(&self, index: usize) -> Result<Symbol> {
        if index >= self.count {
            return Err(ElfError::IndexOutOfBounds);
        }

        let offset = index * self.entry_size;
        Symbol::parse(self.data, offset, self.is_64bit, self.is_little_endian)
    }

    /// Get the number of symbols
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if there are no symbols
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl<'a> Iterator for SymbolIter<'a> {
    type Item = Result<Symbol>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        let offset = self.current * self.entry_size;
        let result = Symbol::parse(self.data, offset, self.is_64bit, self.is_little_endian);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for SymbolIter<'a> {}

/// Symbol table with string table for name resolution
#[derive(Debug)]
pub struct SymbolTable<'a> {
    symbols: SymbolIter<'a>,
    string_table: Option<StringTable<'a>>,
}

impl<'a> SymbolTable<'a> {
    /// Create a new symbol table
    pub fn new(
        symbol_data: &'a [u8],
        string_data: Option<&'a [u8]>,
        is_64bit: bool,
        is_little_endian: bool,
    ) -> Result<Self> {
        let symbols = SymbolIter::new(symbol_data, is_64bit, is_little_endian)?;
        let string_table = string_data.map(StringTable::new);

        Ok(SymbolTable {
            symbols,
            string_table,
        })
    }

    /// Get symbol by index
    pub fn get_symbol(&self, index: usize) -> Result<Symbol> {
        self.symbols.get(index)
    }

    /// Get symbol name by index
    pub fn get_symbol_name(&self, index: usize) -> Result<Option<&str>> {
        let symbol = self.get_symbol(index)?;
        if symbol.name == 0 {
            return Ok(None);
        }

        match &self.string_table {
            Some(strtab) => strtab.get_string(symbol.name).map(Some),
            None => Ok(None),
        }
    }

    /// Find symbol by name
    pub fn find_symbol(&self, name: &str) -> Result<Option<(usize, Symbol)>> {
        for i in 0..self.symbols.len() {
            let symbol = self.get_symbol(i)?;
            if let Some(symbol_name) = self.get_symbol_name(i)? {
                if symbol_name == name {
                    return Ok(Some((i, symbol)));
                }
            }
        }
        Ok(None)
    }

    /// Get all global symbols
    pub fn global_symbols(&self) -> Result<Vec<(usize, Symbol)>> {
        let mut globals = Vec::new();
        for i in 0..self.symbols.len() {
            let symbol = self.get_symbol(i)?;
            if symbol.is_global() {
                globals.push((i, symbol));
            }
        }
        Ok(globals)
    }

    /// Get number of symbols
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if symbol table is empty
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

/// Symbol resolver for handling symbol lookup across multiple tables
#[derive(Debug)]
pub struct SymbolResolver<'a> {
    symbol_table: Option<SymbolTable<'a>>,
    dynamic_table: Option<SymbolTable<'a>>,
}

impl<'a> SymbolResolver<'a> {
    /// Create a new symbol resolver
    pub fn new() -> Self {
        Self {
            symbol_table: None,
            dynamic_table: None,
        }
    }

    /// Set the main symbol table
    pub fn set_symbol_table(&mut self, table: SymbolTable<'a>) {
        self.symbol_table = Some(table);
    }

    /// Set the dynamic symbol table
    pub fn set_dynamic_table(&mut self, table: SymbolTable<'a>) {
        self.dynamic_table = Some(table);
    }

    /// Resolve a symbol by name
    pub fn resolve(&self, name: &str) -> Result<Option<Symbol>> {
        // First try dynamic symbol table
        if let Some(ref dyntab) = self.dynamic_table {
            if let Some((_, symbol)) = dyntab.find_symbol(name)? {
                return Ok(Some(symbol));
            }
        }

        // Then try main symbol table
        if let Some(ref symtab) = self.symbol_table {
            if let Some((_, symbol)) = symtab.find_symbol(name)? {
                return Ok(Some(symbol));
            }
        }

        Ok(None)
    }

    /// Get all undefined symbols that need resolution
    pub fn undefined_symbols(&self) -> Result<Vec<Symbol>> {
        let mut undefined = Vec::new();

        if let Some(ref dyntab) = self.dynamic_table {
            for i in 0..dyntab.len() {
                let symbol = dyntab.get_symbol(i)?;
                if symbol.is_undefined() {
                    undefined.push(symbol);
                }
            }
        }

        if let Some(ref symtab) = self.symbol_table {
            for i in 0..symtab.len() {
                let symbol = symtab.get_symbol(i)?;
                if symbol.is_undefined() {
                    undefined.push(symbol);
                }
            }
        }

        Ok(undefined)
    }
}

impl<'a> Default for SymbolResolver<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for reading values
fn read_u16(data: &[u8], offset: usize, little_endian: bool) -> u16 {
    let bytes = [data[offset], data[offset + 1]];
    if little_endian {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    }
}

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