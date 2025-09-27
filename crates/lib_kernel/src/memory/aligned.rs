//! Safe alignment wrapper utilities
//!
//! This module provides safe wrapper types for data that needs specific alignment.
//! These wrappers ensure that alignment is handled correctly and prevent the
//! "offset is not a multiple of 16" errors that can occur with direct array access.

/// Wrapper for 16-byte aligned data
/// Useful for SIMD operations, certain hardware structures, and memory allocators
#[repr(align(16))]
#[derive(Debug)]
pub struct Aligned16<T>(pub T);

impl<T> Aligned16<T> {
    /// Create a new 16-byte aligned wrapper
    pub const fn new(data: T) -> Self {
        Self(data)
    }

    /// Get a reference to the wrapped data
    pub const fn get(&self) -> &T {
        &self.0
    }

    /// Get a mutable reference to the wrapped data
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Unwrap and return the inner data
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Wrapper for 8-byte aligned data
/// Useful for 64-bit data structures and pointers
#[repr(align(8))]
#[derive(Debug)]
pub struct Aligned8<T>(pub T);

impl<T> Aligned8<T> {
    /// Create a new 8-byte aligned wrapper
    pub const fn new(data: T) -> Self {
        Self(data)
    }

    /// Get a reference to the wrapped data
    pub const fn get(&self) -> &T {
        &self.0
    }

    /// Get a mutable reference to the wrapped data
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Unwrap and return the inner data
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Wrapper for 4-byte aligned data
/// Useful for 32-bit data structures
#[repr(align(4))]
#[derive(Debug)]
pub struct Aligned4<T>(pub T);

impl<T> Aligned4<T> {
    /// Create a new 4-byte aligned wrapper
    pub const fn new(data: T) -> Self {
        Self(data)
    }

    /// Get a reference to the wrapped data
    pub const fn get(&self) -> &T {
        &self.0
    }

    /// Get a mutable reference to the wrapped data
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Unwrap and return the inner data
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Wrapper for page-aligned data (4096 bytes)
/// Useful for page tables, frame buffers, and large buffers
#[repr(align(4096))]
#[derive(Debug)]
pub struct PageAligned<T>(pub T);

impl<T> PageAligned<T> {
    /// Create a new page-aligned wrapper
    pub const fn new(data: T) -> Self {
        Self(data)
    }

    /// Get a reference to the wrapped data
    pub const fn get(&self) -> &T {
        &self.0
    }

    /// Get a mutable reference to the wrapped data
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Unwrap and return the inner data
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Type alias for 16-byte aligned byte arrays
pub type AlignedBytes16<const N: usize> = Aligned16<[u8; N]>;

/// Type alias for 8-byte aligned byte arrays
pub type AlignedBytes8<const N: usize> = Aligned8<[u8; N]>;

/// Type alias for 4-byte aligned byte arrays
pub type AlignedBytes4<const N: usize> = Aligned4<[u8; N]>;

/// Type alias for page-aligned byte arrays
pub type PageAlignedBytes<const N: usize> = PageAligned<[u8; N]>;

// Commonly used aligned stack sizes
pub type AlignedStack<const N: usize> = AlignedBytes16<N>;
pub type AlignedHeap<const N: usize> = AlignedBytes16<N>;

impl<const N: usize> AlignedBytes16<N> {
    /// Create a zeroed 16-byte aligned byte array
    pub const fn zeroed() -> Self {
        Self::new([0; N])
    }

    /// Get a pointer to the start of the array
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    /// Get a mutable pointer to the start of the array
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    /// Get the length of the array
    pub const fn len(&self) -> usize {
        N
    }
}

impl<const N: usize> AlignedBytes8<N> {
    /// Create a zeroed 8-byte aligned byte array
    pub const fn zeroed() -> Self {
        Self::new([0; N])
    }

    /// Get a pointer to the start of the array
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    /// Get a mutable pointer to the start of the array
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    /// Get the length of the array
    pub const fn len(&self) -> usize {
        N
    }
}

impl<const N: usize> PageAlignedBytes<N> {
    /// Create a zeroed page-aligned byte array
    pub const fn zeroed() -> Self {
        Self::new([0; N])
    }

    /// Get a pointer to the start of the array
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    /// Get a mutable pointer to the start of the array
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    /// Get the length of the array
    pub const fn len(&self) -> usize {
        N
    }
}