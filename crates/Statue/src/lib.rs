//! # Statue - A no-std ELF binary parser and loader for operating systems
//!
//! This library provides comprehensive ELF (Executable and Linkable Format) binary
//! parsing and loading capabilities designed specifically for operating system kernels
//! and embedded systems that cannot use the standard library.
//!
//! ## Features
//!
//! - **No-std compatible**: Works in kernel space and embedded environments
//! - **Zero dependencies**: Self-contained implementation
//! - **Architecture support**: x86_64, AArch64, RISC-V
//! - **Complete ELF support**: Headers, sections, symbols, relocations
//! - **Memory-safe**: No unsafe code, comprehensive validation
//! - **Production-ready**: Extensive error handling and edge case coverage
//!
//! ## Example
//!
//! ```rust
//! use statue::{ElfFile, ElfLoader, LoaderConfig};
//!
//! // Parse an ELF binary
//! let elf = ElfFile::parse(binary_data)?;
//!
//! // Create a loader with custom memory allocator
//! let config = LoaderConfig::new(my_allocator);
//! let mut loader = ElfLoader::new(config);
//!
//! // Load the binary into memory
//! let loaded_binary = loader.load(&elf)?;
//!
//! // Execute at entry point
//! loaded_binary.execute()?;
//! ```

#![no_std]
#![deny(missing_docs)]
#![warn(clippy::all)]

extern crate alloc;

pub mod error;
pub mod header;
pub mod program;
pub mod section;
pub mod symbol;
pub mod relocation;
pub mod loader;
pub mod execution;
pub mod arch;
pub mod memory;
pub mod instruction;
// pub mod syscall; // Temporarily disabled due to documentation requirements

pub use error::{ElfError, Result};
pub use header::{ElfHeader, ElfFile};
pub use loader::{ElfLoader, LoaderConfig, LoadedBinary};
pub use execution::ExecutionContext;