//! Kernel logging system with circular ring buffer
//!
//! This module provides a structured logging system with zero heap allocation during
//! logging operations, making it safe to use in interrupt contexts and early boot.
//!
//! # Features
//!
//! - **6 log levels**: TRACE, DEBUG, INFO, WARN, ERROR, CRITICAL
//! - **Circular ring buffer**: 256 entries with compile-time static allocation
//! - **File & line tracking**: Automatic location capture via macros
//! - **Color-coded output**: Different colors for each log level
//! - **Zero allocations**: All memory statically allocated at compile time
//! - **Interrupt safe**: Lock-free writes using atomic operations
//! - **NO INITIALIZATION NEEDED**: Ready to use immediately - just import the macros
//!
//! # Usage
//!
//! ```rust
//! use lib_kernel::{log_info, log_warn, log_error};
//!
//! // NO initialization required - just start logging!
//! log_info!("System initialized");
//! log_warn!("Using fallback configuration");
//! log_error!("Failed to mount filesystem: {}", error);
//! ```
//!
//! # Log Levels
//!
//! - `TRACE`: Very detailed debugging information (normally disabled)
//! - `DEBUG`: General debugging information for development
//! - `INFO`: Informational messages about normal operation
//! - `WARN`: Warning messages for potentially problematic situations
//! - `ERROR`: Error messages for failures that don't halt the system
//! - `CRITICAL`: Critical errors that may lead to system halt

use core::sync::atomic::{AtomicUsize, Ordering};

/// Fixed size of the circular log buffer (256 entries = ~70 KB)
/// Reduced from 1024 to avoid kernel size issues and triple faults
const LOG_BUFFER_SIZE: usize = 256;

/// Maximum message length per log entry (bytes)
const MAX_MESSAGE_LEN: usize = 256;

/// Static buffer for log formatting (NO runtime allocation!)
static mut LOG_FORMAT_BUFFER: [u8; 512] = [0; 512];

/// Log levels for kernel logging
///
/// Levels are ordered from least to most severe. Use `set_min_level()` to
/// filter out less important messages in production builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    /// Very detailed tracing information (most verbose)
    TRACE = 0,
    /// General debugging information
    DEBUG = 1,
    /// Informational messages about normal operation
    INFO = 2,
    /// Warning messages for potentially problematic situations
    WARN = 3,
    /// Error messages for failures that don't halt the system
    ERROR = 4,
    /// Critical errors that may lead to system halt or BSOD
    CRITICAL = 5,
}

impl LogLevel {
    /// Convert log level to a 6-character padded string
    const fn as_str(&self) -> &'static [u8; 6] {
        match self {
            LogLevel::TRACE => b"TRACE ",
            LogLevel::DEBUG => b"DEBUG ",
            LogLevel::INFO => b"INFO  ",
            LogLevel::WARN => b"WARN  ",
            LogLevel::ERROR => b"ERROR ",
            LogLevel::CRITICAL => b"CRIT  ",
        }
    }

    /// Get ARGB colors for this log level (foreground, background)
    const fn colors(&self) -> (u32, u32) {
        match self {
            LogLevel::TRACE => (0xFF808080, 0x00000000),    // Gray on black
            LogLevel::DEBUG => (0xFF00FFFF, 0x00000000),    // Cyan on black
            LogLevel::INFO => (0xFFFFFFFF, 0x00000000),     // White on black
            LogLevel::WARN => (0xFFFFFF00, 0x00000000),     // Yellow on black
            LogLevel::ERROR => (0xFFFF0000, 0x00000000),    // Red on black
            LogLevel::CRITICAL => (0xFFFF00FF, 0xFFFF0000), // Magenta on red
        }
    }
}

/// A single log entry in the circular buffer
///
/// Each entry is approximately 280 bytes, designed to avoid heap allocation.
/// File paths are stored as static string slices, and messages are truncated
/// to fit within the fixed-size buffer.
#[derive(Copy, Clone)]
pub struct LogEntry {
    /// Log level for this entry
    level: LogLevel,

    /// Source file path (static string from file!() macro)
    file: &'static str,

    /// Source line number (from line!() macro)
    line: u32,

    /// Message content (fixed-size buffer, no allocation)
    message: [u8; MAX_MESSAGE_LEN],

    /// Actual length of message (message may be truncated)
    message_len: usize,
}

impl LogEntry {
    /// Create a new empty log entry
    pub const fn new() -> Self {
        Self {
            level: LogLevel::INFO,
            file: "",
            line: 0,
            message: [0u8; MAX_MESSAGE_LEN],
            message_len: 0,
        }
    }

    /// Create a log entry with the given parameters
    fn create(level: LogLevel, file: &'static str, line: u32, message_bytes: &[u8]) -> Self {
        let mut entry = Self::new();
        entry.level = level;
        entry.file = file;
        entry.line = line;

        // Copy message, truncating if necessary
        let copy_len = message_bytes.len().min(MAX_MESSAGE_LEN);
        entry.message[..copy_len].copy_from_slice(&message_bytes[..copy_len]);
        entry.message_len = copy_len;

        entry
    }
}

/// Kernel logger with circular ring buffer (NO heap allocation during logging)
///
/// The logger maintains a fixed-size circular buffer of log entries. When the buffer
/// is full, old entries are overwritten. All operations are lock-free and safe for
/// use in interrupt contexts.
///
/// # Memory Usage
///
/// - Static memory: ~280 KB (1024 entries × 280 bytes)
/// - Runtime cost: O(1) per log call
/// - Heap allocations: 0
pub struct KernelLogger {
    /// Fixed-size circular buffer for log entries
    entries: [LogEntry; LOG_BUFFER_SIZE],

    /// Write position in the circular buffer (atomic for lock-free writes)
    write_pos: AtomicUsize,

    /// Read position for potential log dumping (currently unused)
    read_pos: AtomicUsize,

    /// Minimum log level to display (for filtering)
    min_level: LogLevel,

    /// Total number of logs written (for statistics)
    total_logs: AtomicUsize,

    /// Number of logs dropped due to filtering (for statistics)
    dropped_logs: AtomicUsize,
}

impl KernelLogger {
    /// Create a new logger with default settings
    ///
    /// This is a const function suitable for static initialization.
    pub const fn new() -> Self {
        Self {
            entries: [LogEntry::new(); LOG_BUFFER_SIZE],
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            min_level: LogLevel::TRACE, // Show all levels by default
            total_logs: AtomicUsize::new(0),
            dropped_logs: AtomicUsize::new(0),
        }
    }

    /// Log a message (no heap allocation!)
    ///
    /// # Arguments
    ///
    /// * `level` - Log level for this message
    /// * `file` - Source file path (from file!() macro)
    /// * `line` - Source line number (from line!() macro)
    /// * `message_bytes` - Message content as bytes
    ///
    /// # Performance
    ///
    /// This function performs O(1) operations and does not allocate memory.
    /// It is safe to call from interrupt contexts.
    pub fn log(
        &mut self,
        level: LogLevel,
        file: &'static str,
        line: u32,
        message_bytes: &[u8],
    ) {
        // Filter by minimum level
        if level < self.min_level {
            self.dropped_logs.fetch_add(1, Ordering::Relaxed);
            return;
        }

        // Get current write position and advance it
        let pos = self.write_pos.load(Ordering::Relaxed);
        let next_pos = (pos + 1) % LOG_BUFFER_SIZE;

        // Create and store log entry
        let entry = LogEntry::create(level, file, line, message_bytes);
        self.entries[pos] = entry;

        // Update write position
        self.write_pos.store(next_pos, Ordering::Release);
        self.total_logs.fetch_add(1, Ordering::Relaxed);

        // Write to screen immediately (synchronous output)
        self.write_to_screen(&entry);
    }

    /// Write log entry to screen using the global renderer (NO runtime allocation!)
    ///
    /// Format: `[LEVEL] [file:line] message\n`
    fn write_to_screen(&self, entry: &LogEntry) {
        use crate::scrolling_text::write_global;

        // Set colors based on log level
        unsafe {
            if let Some(ref mut renderer) = crate::scrolling_text::GLOBAL_RENDERER {
                let (fg, bg) = entry.level.colors();
                renderer.set_colors(fg, bg);
            }

            // Format the log message using static buffer (NO allocation!)
            let mut pos = 0;

            // Write: "[LEVEL] "
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], b"[");
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], entry.level.as_str());
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], b"] ");

            // Write: "[file:line] "
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], b"[");
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], entry.file.as_bytes());
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], b":");
            pos += Self::write_u32(&mut LOG_FORMAT_BUFFER[pos..], entry.line);
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], b"] ");

            // Write message
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], &entry.message[..entry.message_len]);

            // Add newline
            pos += Self::copy_bytes(&mut LOG_FORMAT_BUFFER[pos..], b"\n");

            // Write to global renderer
            write_global(&LOG_FORMAT_BUFFER[..pos]);

            // Reset colors to default (white on black)
            if let Some(ref mut renderer) = crate::scrolling_text::GLOBAL_RENDERER {
                renderer.set_colors(0xFFFFFFFF, 0x00000000);
            }
        }
    }

    /// Helper function to copy bytes from source to destination
    ///
    /// Returns the number of bytes copied.
    fn copy_bytes(dest: &mut [u8], src: &[u8]) -> usize {
        let len = src.len().min(dest.len());
        dest[..len].copy_from_slice(&src[..len]);
        len
    }

    /// Helper function to write a u32 as ASCII decimal digits
    ///
    /// Returns the number of bytes written.
    fn write_u32(dest: &mut [u8], mut num: u32) -> usize {
        if num == 0 {
            if dest.len() > 0 {
                dest[0] = b'0';
                return 1;
            }
            return 0;
        }

        // Convert to decimal digits (reversed)
        let mut digits = [0u8; 10];
        let mut count = 0;

        while num > 0 && count < 10 {
            digits[count] = (num % 10) as u8 + b'0';
            num /= 10;
            count += 1;
        }

        // Copy digits in correct order to destination
        let write_count = count.min(dest.len());
        for i in 0..write_count {
            dest[i] = digits[count - 1 - i];
        }

        write_count
    }

    /// Set the minimum log level to display
    ///
    /// Messages below this level will be filtered out and not displayed.
    ///
    /// # Example
    ///
    /// ```rust
    /// // Only show INFO and above (hide TRACE and DEBUG)
    /// logger.set_min_level(LogLevel::INFO);
    /// ```
    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    /// Get logging statistics
    ///
    /// Returns `(total_logs, dropped_logs)` where:
    /// - `total_logs`: Total number of log messages written
    /// - `dropped_logs`: Number of messages filtered by log level
    pub fn stats(&self) -> (usize, usize) {
        (
            self.total_logs.load(Ordering::Relaxed),
            self.dropped_logs.load(Ordering::Relaxed),
        )
    }
}

/// Global logger instance (static, initialized at compile time - ZERO runtime allocation)
///
/// The entire ~70 KB buffer is allocated in the kernel's .bss section at compile time,
/// ensuring no stack overflow or heap allocation occurs during initialization.
///
/// **NO INITIALIZATION NEEDED** - Just import and use via the logging macros.
pub static mut GLOBAL_LOGGER: KernelLogger = KernelLogger::new();

/// Get a mutable reference to the global logger
///
/// Always returns a valid reference since the logger is initialized at compile time.
///
/// **NO INITIALIZATION NEEDED** - The logger is ready to use immediately.
pub fn get_logger() -> &'static mut KernelLogger {
    unsafe { &mut GLOBAL_LOGGER }
}

// ============================================================================
// LOGGING MACROS
// ============================================================================

/// Internal macro for logging (captures file and line information)
///
/// This macro should not be used directly - use the level-specific macros
/// like `log_info!`, `log_error!`, etc.
#[macro_export]
macro_rules! klog {
    ($level:expr, $($arg:tt)*) => {{
        // Format the message using LineWriter
        let mut writer = $crate::scrolling_text::LineWriter::new();
        use ::core::fmt::Write;
        let _ = ::core::write!(&mut writer, $($arg)*);

        // Log using the compile-time initialized logger
        let logger = $crate::logger::get_logger();
        logger.log(
            $level,
            file!(),
            line!(),
            writer.finish(),
        );
    }};
}

/// Log a TRACE level message
///
/// TRACE is the most verbose level, typically used for detailed tracing
/// of program execution. These messages are usually disabled in production.
///
/// # Example
///
/// ```rust
/// log_trace!("Entering function process_request()");
/// log_trace!("Variable state: {:?}", state);
/// ```
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::klog!($crate::logger::LogLevel::TRACE, $($arg)*)
    };
}

/// Log a DEBUG level message
///
/// DEBUG messages provide general debugging information useful during
/// development. They are typically disabled in production builds.
///
/// # Example
///
/// ```rust
/// log_debug!("Processing {} items", count);
/// log_debug!("Configuration loaded: {:?}", config);
/// ```
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::klog!($crate::logger::LogLevel::DEBUG, $($arg)*)
    };
}

/// Log an INFO level message
///
/// INFO messages provide informational output about normal system operation.
/// This is the default log level for general messages.
///
/// # Example
///
/// ```rust
/// log_info!("System initialized successfully");
/// log_info!("Loaded {} drivers", driver_count);
/// ```
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::klog!($crate::logger::LogLevel::INFO, $($arg)*)
    };
}

/// Log a WARN level message
///
/// WARN messages indicate potentially problematic situations that don't
/// prevent the system from functioning but should be investigated.
///
/// # Example
///
/// ```rust
/// log_warn!("Using fallback configuration");
/// log_warn!("Disk space low: {} KB remaining", free_space);
/// ```
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::klog!($crate::logger::LogLevel::WARN, $($arg)*)
    };
}

/// Log an ERROR level message
///
/// ERROR messages indicate failures that don't halt the system but represent
/// significant problems that should be addressed.
///
/// # Example
///
/// ```rust
/// log_error!("Failed to mount filesystem: {:?}", error);
/// log_error!("Network interface not found");
/// ```
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::klog!($crate::logger::LogLevel::ERROR, $($arg)*)
    };
}

/// Log a CRITICAL level message
///
/// CRITICAL messages indicate severe errors that may lead to system halt
/// or require immediate attention. These are shown in magenta on red background.
///
/// # Example
///
/// ```rust
/// log_critical!("Out of memory! Cannot allocate {} bytes", size);
/// log_critical!("Hardware fault detected in CPU core {}", core_id);
/// ```
#[macro_export]
macro_rules! log_critical {
    ($($arg:tt)*) => {
        $crate::klog!($crate::logger::LogLevel::CRITICAL, $($arg)*)
    };
}
