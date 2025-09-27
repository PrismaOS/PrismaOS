//! REPLACED: This file is replaced by gdt_correct.rs
//! The old implementation had critical SYSCALL compatibility issues.

// Re-export the correct implementation
pub use crate::gdt_correct::*;

/// Legacy constant for compatibility (maps to new IST system)
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;