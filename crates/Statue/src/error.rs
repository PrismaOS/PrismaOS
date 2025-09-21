//! Error types and result handling for the Statue ELF library.

/// Result type alias for Statue operations.
pub type Result<T> = core::result::Result<T, ElfError>;

/// Comprehensive error types for ELF parsing and loading operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    /// Invalid or corrupted ELF magic number
    InvalidMagic,
    /// Unsupported ELF class (32-bit/64-bit)
    UnsupportedClass,
    /// Unsupported data encoding (endianness)
    UnsupportedEncoding,
    /// Unsupported ELF version
    UnsupportedVersion,
    /// Unsupported target architecture
    UnsupportedArchitecture,
    /// Invalid or corrupted ELF header
    InvalidHeader,
    /// Invalid program header
    InvalidProgramHeader,
    /// Invalid section header
    InvalidSectionHeader,
    /// Invalid symbol table entry
    InvalidSymbol,
    /// Invalid relocation entry
    InvalidRelocation,
    /// Insufficient buffer size for parsing
    BufferTooSmall,
    /// Invalid file offset or size
    InvalidOffset,
    /// Invalid virtual address
    InvalidAddress,
    /// Invalid alignment requirements
    InvalidAlignment,
    /// Memory allocation failure
    AllocationFailed,
    /// Memory mapping failure
    MappingFailed,
    /// Permission denied for memory operation
    PermissionDenied,
    /// Unsupported relocation type
    UnsupportedRelocation,
    /// Missing required section
    MissingSection,
    /// Missing required symbol
    MissingSymbol,
    /// Circular dependency detected
    CircularDependency,
    /// Dynamic linking error
    DynamicLinkingFailed,
    /// Execution setup failure
    ExecutionSetupFailed,
    /// Invalid string table
    InvalidStringTable,
    /// String not found in string table
    StringNotFound,
    /// Index out of bounds
    IndexOutOfBounds,
    /// Arithmetic overflow
    ArithmeticOverflow,
    /// Generic parsing error
    ParseError,
}

impl ElfError {
    /// Returns a human-readable description of the error.
    pub const fn description(self) -> &'static str {
        match self {
            ElfError::InvalidMagic => "Invalid ELF magic number",
            ElfError::UnsupportedClass => "Unsupported ELF class",
            ElfError::UnsupportedEncoding => "Unsupported data encoding",
            ElfError::UnsupportedVersion => "Unsupported ELF version",
            ElfError::UnsupportedArchitecture => "Unsupported target architecture",
            ElfError::InvalidHeader => "Invalid ELF header",
            ElfError::InvalidProgramHeader => "Invalid program header",
            ElfError::InvalidSectionHeader => "Invalid section header",
            ElfError::InvalidSymbol => "Invalid symbol table entry",
            ElfError::InvalidRelocation => "Invalid relocation entry",
            ElfError::BufferTooSmall => "Buffer too small for parsing",
            ElfError::InvalidOffset => "Invalid file offset or size",
            ElfError::InvalidAddress => "Invalid virtual address",
            ElfError::InvalidAlignment => "Invalid alignment requirements",
            ElfError::AllocationFailed => "Memory allocation failure",
            ElfError::MappingFailed => "Memory mapping failure",
            ElfError::PermissionDenied => "Permission denied",
            ElfError::UnsupportedRelocation => "Unsupported relocation type",
            ElfError::MissingSection => "Missing required section",
            ElfError::MissingSymbol => "Missing required symbol",
            ElfError::CircularDependency => "Circular dependency detected",
            ElfError::DynamicLinkingFailed => "Dynamic linking failed",
            ElfError::ExecutionSetupFailed => "Execution setup failed",
            ElfError::InvalidStringTable => "Invalid string table",
            ElfError::StringNotFound => "String not found in string table",
            ElfError::IndexOutOfBounds => "Index out of bounds",
            ElfError::ArithmeticOverflow => "Arithmetic overflow",
            ElfError::ParseError => "Generic parsing error",
        }
    }
}