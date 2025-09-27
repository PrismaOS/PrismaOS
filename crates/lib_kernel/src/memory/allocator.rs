//! REPLACED: This file is replaced by allocator_correct.rs
//! The old implementation had critical race conditions and double-initialization bugs.

// Re-export the correct implementation
pub use crate::memory::allocator_correct::*;

// Use the correct global allocator
#[global_allocator]
static ALLOCATOR: crate::memory::allocator_correct::GlobalAllocator =
    crate::memory::allocator_correct::GlobalAllocator;