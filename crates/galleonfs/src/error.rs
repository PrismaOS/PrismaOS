//! Error types for GalleonFS (no_std compatible)

// #![no_std] // Only at crate root

extern crate alloc;

use alloc::string::String;
use core::fmt;

/// GalleonFS error types (no_std compatible)
#[derive(Debug, Clone)]
pub enum GalleonError {
    /// I/O error during storage operation
    IoError(&'static str),
    /// I/O error with dynamic message (use sparingly)
    IoErrorDynamic(String),
    /// Permission denied
    PermissionDenied,
    /// File or directory not found
    NotFound,
    /// File or directory already exists
    AlreadyExists,
    /// Invalid argument provided
    InvalidArgument(&'static str),
    /// Invalid argument with dynamic message
    InvalidArgumentDynamic(String),
    /// Filesystem is full
    NoSpace,
    /// No more inodes available
    NoInodes,
    /// Filesystem corruption detected
    Corruption(&'static str),
    /// Filesystem corruption with dynamic message
    CorruptionDynamic(String),
    /// Network error during replication
    NetworkError(&'static str),
    /// Network error with dynamic message
    NetworkErrorDynamic(String),
    /// Replication conflict
    ReplicationConflict(&'static str),
    /// Replication conflict with dynamic message
    ReplicationConflictDynamic(String),
    /// Transaction error
    TransactionError(&'static str),
    /// Transaction error with dynamic message
    TransactionErrorDynamic(String),
    /// Encryption/decryption error
    CryptoError(&'static str),
    /// Encryption/decryption error with dynamic message
    CryptoErrorDynamic(String),
    /// Compression/decompression error
    CompressionError(&'static str),
    /// Compression/decompression error with dynamic message
    CompressionErrorDynamic(String),
    /// Invalid filesystem state
    InvalidState(&'static str),
    /// Invalid filesystem state with dynamic message
    InvalidStateDynamic(String),
    /// Operation not supported
    NotSupported,
    /// Quota exceeded
    QuotaExceeded,
    /// Deadlock detected
    Deadlock,
    /// Timeout occurred
    Timeout,
    /// Invalid path
    InvalidPath(&'static str),
    /// Invalid path with dynamic message
    InvalidPathDynamic(String),
    /// Cross-device link
    CrossDevice,
    /// Directory not empty
    DirectoryNotEmpty,
    /// Not a directory
    NotADirectory,
    /// Is a directory
    IsADirectory,
    /// Too many symbolic links
    TooManyLinks,
    /// Name too long
    NameTooLong,
    /// Read-only filesystem
    ReadOnlyFilesystem,
    /// Stale file handle
    StaleHandle,
}

impl fmt::Display for GalleonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GalleonError::IoError(msg) => write!(f, "I/O error: {}", msg),
            GalleonError::IoErrorDynamic(msg) => write!(f, "I/O error: {}", msg),
            GalleonError::PermissionDenied => write!(f, "Permission denied"),
            GalleonError::NotFound => write!(f, "File or directory not found"),
            GalleonError::AlreadyExists => write!(f, "File or directory already exists"),
            GalleonError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            GalleonError::InvalidArgumentDynamic(msg) => write!(f, "Invalid argument: {}", msg),
            GalleonError::NoSpace => write!(f, "No space left on device"),
            GalleonError::NoInodes => write!(f, "No inodes available"),
            GalleonError::Corruption(msg) => write!(f, "Filesystem corruption: {}", msg),
            GalleonError::CorruptionDynamic(msg) => write!(f, "Filesystem corruption: {}", msg),
            GalleonError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            GalleonError::NetworkErrorDynamic(msg) => write!(f, "Network error: {}", msg),
            GalleonError::ReplicationConflict(msg) => write!(f, "Replication conflict: {}", msg),
            GalleonError::ReplicationConflictDynamic(msg) => write!(f, "Replication conflict: {}", msg),
            GalleonError::TransactionError(msg) => write!(f, "Transaction error: {}", msg),
            GalleonError::TransactionErrorDynamic(msg) => write!(f, "Transaction error: {}", msg),
            GalleonError::CryptoError(msg) => write!(f, "Encryption error: {}", msg),
            GalleonError::CryptoErrorDynamic(msg) => write!(f, "Encryption error: {}", msg),
            GalleonError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            GalleonError::CompressionErrorDynamic(msg) => write!(f, "Compression error: {}", msg),
            GalleonError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            GalleonError::InvalidStateDynamic(msg) => write!(f, "Invalid state: {}", msg),
            GalleonError::NotSupported => write!(f, "Operation not supported"),
            GalleonError::QuotaExceeded => write!(f, "Quota exceeded"),
            GalleonError::Deadlock => write!(f, "Deadlock detected"),
            GalleonError::Timeout => write!(f, "Operation timed out"),
            GalleonError::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            GalleonError::InvalidPathDynamic(msg) => write!(f, "Invalid path: {}", msg),
            GalleonError::CrossDevice => write!(f, "Cross-device link"),
            GalleonError::DirectoryNotEmpty => write!(f, "Directory not empty"),
            GalleonError::NotADirectory => write!(f, "Not a directory"),
            GalleonError::IsADirectory => write!(f, "Is a directory"),
            GalleonError::TooManyLinks => write!(f, "Too many symbolic links"),
            GalleonError::NameTooLong => write!(f, "File name too long"),
            GalleonError::ReadOnlyFilesystem => write!(f, "Read-only filesystem"),
            GalleonError::StaleHandle => write!(f, "Stale file handle"),
        }
    }
}

/// Result type for GalleonFS operations
pub type Result<T> = core::result::Result<T, GalleonError>;

/// Convert from static string slice
impl From<&'static str> for GalleonError {
    fn from(msg: &'static str) -> Self {
        GalleonError::IoError(msg)
    }
}

/// Convert from String (use sparingly in no_std)
impl From<String> for GalleonError {
    fn from(msg: String) -> Self {
        GalleonError::IoErrorDynamic(msg)
    }
}

/// Error context for better error reporting (no_std compatible)
pub struct ErrorContext {
    pub operation: &'static str,
    pub path: Option<String>,
    pub object_id: Option<super::ObjectId>,
}

impl ErrorContext {
    pub const fn new(operation: &'static str) -> Self {
        Self {
            operation,
            path: None,
            object_id: None,
        }
    }

    pub fn with_path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_object_id(mut self, id: super::ObjectId) -> Self {
        self.object_id = Some(id);
        self
    }

    pub fn wrap_error(&self, error: GalleonError) -> GalleonError {
        // In a more sophisticated implementation, we'd wrap the error
        // with context information. For no_std, we keep it simple.
        error
    }
}

/// Macro for creating context-aware errors (no_std compatible)
#[macro_export]
macro_rules! galleon_error {
    ($op:expr, $err:expr) => {
        ErrorContext::new($op).wrap_error($err)
    };
    ($op:expr, $path:expr, $err:expr) => {
        ErrorContext::new($op).with_path($path).wrap_error($err)
    };
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Informational - operation succeeded with notes
    Info,
    /// Warning - operation succeeded but with issues
    Warning,
    /// Error - operation failed but system is stable
    Error,
    /// Critical - operation failed and system stability is compromised
    Critical,
    /// Fatal - operation failed and system must halt
    Fatal,
}

/// Extended error with severity and recovery suggestions
#[derive(Debug, Clone)]
pub struct ExtendedError {
    pub base_error: GalleonError,
    pub severity: ErrorSeverity,
    pub recovery_suggestion: Option<&'static str>,
    pub error_code: u32,
}

impl ExtendedError {
    pub fn new(base_error: GalleonError, severity: ErrorSeverity) -> Self {
        Self {
            base_error,
            severity,
            recovery_suggestion: None,
            error_code: 0,
        }
    }

    pub fn with_recovery_suggestion(mut self, suggestion: &'static str) -> Self {
        self.recovery_suggestion = Some(suggestion);
        self
    }

    pub fn with_error_code(mut self, code: u32) -> Self {
        self.error_code = code;
        self
    }

    pub fn is_recoverable(&self) -> bool {
        !matches!(self.severity, ErrorSeverity::Critical | ErrorSeverity::Fatal)
    }
}

impl fmt::Display for ExtendedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.severity, self.base_error)?;
        
        if self.error_code != 0 {
            write!(f, " (code: {})", self.error_code)?;
        }
        
        if let Some(suggestion) = self.recovery_suggestion {
            write!(f, " - Recovery: {}", suggestion)?;
        }
        
        Ok(())
    }
}

/// Error collection for batch operations
pub struct ErrorCollection {
    errors: alloc::vec::Vec<ExtendedError>,
    max_errors: usize,
}

impl ErrorCollection {
    pub fn new(max_errors: usize) -> Self {
        Self {
            errors: alloc::vec::Vec::new(),
            max_errors,
        }
    }

    pub fn add_error(&mut self, error: ExtendedError) -> bool {
        if self.errors.len() < self.max_errors {
            self.errors.push(error);
            true
        } else {
            false // Collection is full
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn errors(&self) -> &[ExtendedError] {
        &self.errors
    }

    pub fn has_critical_errors(&self) -> bool {
        self.errors.iter().any(|e| matches!(e.severity, ErrorSeverity::Critical | ErrorSeverity::Fatal))
    }

    pub fn clear(&mut self) {
        self.errors.clear();
    }
}

/// No-std compatible assertion macros for GalleonFS
#[macro_export]
macro_rules! galleon_assert {
    ($cond:expr, $err:expr) => {
        if !$cond {
            return Err($err);
        }
    };
}

#[macro_export]
macro_rules! galleon_assert_eq {
    ($left:expr, $right:expr, $err:expr) => {
        if $left != $right {
            return Err($err);
        }
    };
}

#[macro_export]
macro_rules! galleon_ensure {
    ($cond:expr, $err:expr) => {
        if !$cond {
            return Err($err.into());
        }
    };
}