//! USB Memory Management

use alloc::{vec, vec::Vec, collections::BTreeMap};
use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
    ptr::NonNull,
};
use spin::Mutex;
use crate::{Result, UsbDriverError};

/// USB Memory Pool for DMA-coherent allocations
pub struct UsbMemoryPool {
    /// Base address of the pool
    base: usize,
    /// Size of the pool
    size: usize,
    /// Free blocks (offset -> size)
    free_blocks: BTreeMap<usize, usize>,
    /// Allocated blocks (offset -> size)
    allocated_blocks: BTreeMap<usize, usize>,
    /// Total allocated bytes
    allocated_bytes: AtomicUsize,
}

impl UsbMemoryPool {
    /// Create a new memory pool
    pub fn new(base: usize, size: usize) -> Self {
        let mut free_blocks = BTreeMap::new();
        free_blocks.insert(0, size);

        Self {
            base,
            size,
            free_blocks,
            allocated_blocks: BTreeMap::new(),
            allocated_bytes: AtomicUsize::new(0),
        }
    }

    /// Allocate memory with alignment
    pub fn allocate(&mut self, size: usize, align: usize) -> Result<NonNull<u8>> {
        if size == 0 {
            return Err(UsbDriverError::InvalidParameter);
        }

        let aligned_size = (size + align - 1) & !(align - 1);

        // Find a suitable free block
        let mut best_block = None;
        for (&offset, &block_size) in &self.free_blocks {
            let aligned_offset = (self.base + offset + align - 1) & !(align - 1);
            let alignment_waste = aligned_offset - (self.base + offset);

            if block_size >= aligned_size + alignment_waste {
                best_block = Some((offset, block_size, aligned_offset, alignment_waste));
                break;
            }
        }

        if let Some((offset, block_size, aligned_offset, alignment_waste)) = best_block {
            // Remove the free block
            self.free_blocks.remove(&offset);

            // Add back any unused space before the aligned allocation
            if alignment_waste > 0 {
                self.free_blocks.insert(offset, alignment_waste);
            }

            // Add back any unused space after the allocation
            let remaining = block_size - aligned_size - alignment_waste;
            if remaining > 0 {
                let remaining_offset = offset + alignment_waste + aligned_size;
                self.free_blocks.insert(remaining_offset, remaining);
            }

            // Record the allocation
            let alloc_offset = offset + alignment_waste;
            self.allocated_blocks.insert(alloc_offset, aligned_size);
            self.allocated_bytes.fetch_add(aligned_size, Ordering::Relaxed);

            let ptr = NonNull::new(aligned_offset as *mut u8)
                .ok_or(UsbDriverError::MemoryError)?;

            Ok(ptr)
        } else {
            Err(UsbDriverError::MemoryError)
        }
    }

    /// Deallocate memory
    pub fn deallocate(&mut self, ptr: NonNull<u8>) -> Result<()> {
        let addr = ptr.as_ptr() as usize;
        let offset = addr - self.base;

        if let Some(size) = self.allocated_blocks.remove(&offset) {
            self.allocated_bytes.fetch_sub(size, Ordering::Relaxed);

            // Add back to free blocks
            self.free_blocks.insert(offset, size);

            // Coalesce adjacent free blocks
            self.coalesce_free_blocks();

            Ok(())
        } else {
            Err(UsbDriverError::InvalidParameter)
        }
    }

    /// Coalesce adjacent free blocks
    fn coalesce_free_blocks(&mut self) {
        let mut coalesced = BTreeMap::new();
        let mut current_offset = None;
        let mut current_size = 0;

        for (&offset, &size) in &self.free_blocks {
            if let Some(curr_offset) = current_offset {
                if curr_offset + current_size == offset {
                    // Adjacent block, merge it
                    current_size += size;
                } else {
                    // Non-adjacent, store previous and start new
                    coalesced.insert(curr_offset, current_size);
                    current_offset = Some(offset);
                    current_size = size;
                }
            } else {
                // First block
                current_offset = Some(offset);
                current_size = size;
            }
        }

        // Store the last block
        if let Some(offset) = current_offset {
            coalesced.insert(offset, current_size);
        }

        self.free_blocks = coalesced;
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            total_size: self.size,
            allocated_bytes: self.allocated_bytes.load(Ordering::Relaxed),
            free_bytes: self.size - self.allocated_bytes.load(Ordering::Relaxed),
            free_blocks: self.free_blocks.len(),
            allocated_blocks: self.allocated_blocks.len(),
        }
    }
}

/// Memory pool statistics
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub total_size: usize,
    pub allocated_bytes: usize,
    pub free_bytes: usize,
    pub free_blocks: usize,
    pub allocated_blocks: usize,
}

/// USB Memory Allocator
pub struct UsbMemoryAllocator {
    /// DMA-coherent memory pool
    dma_pool: Mutex<UsbMemoryPool>,
    /// Regular heap allocations
    allocations: Mutex<BTreeMap<usize, usize>>,
}

impl UsbMemoryAllocator {
    /// Create a new USB memory allocator
    pub fn new() -> Result<Self> {
        // In a real implementation, you'd allocate DMA-coherent memory from the kernel
        // For now, we'll simulate it
        let dma_base = 0x80000000; // Simulated DMA base
        let dma_size = 1024 * 1024; // 1MB DMA pool

        let dma_pool = UsbMemoryPool::new(dma_base, dma_size);

        Ok(Self {
            dma_pool: Mutex::new(dma_pool),
            allocations: Mutex::new(BTreeMap::new()),
        })
    }

    /// Allocate DMA-coherent memory
    pub fn allocate_dma(&self, size: usize, align: usize) -> Result<NonNull<u8>> {
        let mut pool = self.dma_pool.lock();
        pool.allocate(size, align)
    }

    /// Deallocate DMA-coherent memory
    pub fn deallocate_dma(&self, ptr: NonNull<u8>) -> Result<()> {
        let mut pool = self.dma_pool.lock();
        pool.deallocate(ptr)
    }

    /// Allocate regular memory (falls back to heap)
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>> {
        use alloc::alloc::{alloc, Layout};

        let layout = Layout::from_size_align(size, 8)
            .map_err(|_| UsbDriverError::MemoryError)?;

        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(UsbDriverError::MemoryError);
        }

        // Record the allocation
        let mut allocations = self.allocations.lock();
        allocations.insert(ptr as usize, size);

        NonNull::new(ptr).ok_or(UsbDriverError::MemoryError)
    }

    /// Deallocate regular memory
    pub fn deallocate(&self, ptr: NonNull<u8>) -> Result<()> {
        use alloc::alloc::{dealloc, Layout};

        let mut allocations = self.allocations.lock();
        let size = allocations.remove(&(ptr.as_ptr() as usize))
            .ok_or(UsbDriverError::InvalidParameter)?;

        let layout = Layout::from_size_align(size, 8)
            .map_err(|_| UsbDriverError::MemoryError)?;

        unsafe { dealloc(ptr.as_ptr(), layout) };
        Ok(())
    }

    /// Get allocator statistics
    pub fn stats(&self) -> AllocatorStats {
        let pool_stats = self.dma_pool.lock().stats();
        let heap_allocations = self.allocations.lock().len();

        AllocatorStats {
            dma_pool: pool_stats,
            heap_allocations,
        }
    }
}

/// Allocator statistics
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    pub dma_pool: PoolStats,
    pub heap_allocations: usize,
}

/// USB Buffer for DMA operations
pub struct UsbBuffer {
    /// Pointer to the buffer
    ptr: NonNull<u8>,
    /// Size of the buffer
    size: usize,
    /// Whether this is DMA-coherent memory
    is_dma: bool,
    /// Reference to allocator for cleanup
    allocator: Option<*const UsbMemoryAllocator>,
}

impl UsbBuffer {
    /// Create a new DMA buffer
    pub fn new_dma(allocator: &UsbMemoryAllocator, size: usize) -> Result<Self> {
        let ptr = allocator.allocate_dma(size, 64)?; // 64-byte alignment for xHCI

        Ok(Self {
            ptr,
            size,
            is_dma: true,
            allocator: Some(allocator as *const _),
        })
    }

    /// Create a new regular buffer
    pub fn new(allocator: &UsbMemoryAllocator, size: usize) -> Result<Self> {
        let ptr = allocator.allocate(size)?;

        Ok(Self {
            ptr,
            size,
            is_dma: false,
            allocator: Some(allocator as *const _),
        })
    }

    /// Create from existing memory (not owned)
    pub unsafe fn from_raw(ptr: *mut u8, size: usize) -> Result<Self> {
        let ptr = NonNull::new(ptr).ok_or(UsbDriverError::InvalidParameter)?;

        Ok(Self {
            ptr,
            size,
            is_dma: false,
            allocator: None,
        })
    }

    /// Get buffer as slice
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.size) }
    }

    /// Get buffer as mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.size) }
    }

    /// Get buffer pointer
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    /// Get mutable buffer pointer
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Get buffer size
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Check if this is DMA-coherent memory
    pub fn is_dma(&self) -> bool {
        self.is_dma
    }

    /// Get physical address (simplified)
    pub fn physical_address(&self) -> usize {
        // In a real implementation, you'd translate virtual to physical
        self.ptr.as_ptr() as usize
    }

    /// Zero the buffer
    pub fn zero(&mut self) {
        let slice = self.as_mut_slice();
        slice.fill(0);
    }

    /// Copy data into buffer
    pub fn copy_from_slice(&mut self, src: &[u8]) -> Result<()> {
        if src.len() > self.size {
            return Err(UsbDriverError::BufferOverflow);
        }

        let dst = &mut self.as_mut_slice()[..src.len()];
        dst.copy_from_slice(src);
        Ok(())
    }

    /// Copy data from buffer
    pub fn copy_to_slice(&self, dst: &mut [u8]) -> Result<usize> {
        let copy_len = core::cmp::min(self.size, dst.len());
        let src = &self.as_slice()[..copy_len];
        dst[..copy_len].copy_from_slice(src);
        Ok(copy_len)
    }
}

impl Drop for UsbBuffer {
    fn drop(&mut self) {
        if let Some(allocator_ptr) = self.allocator {
            let allocator = unsafe { &*allocator_ptr };

            if self.is_dma {
                let _ = allocator.deallocate_dma(self.ptr);
            } else {
                let _ = allocator.deallocate(self.ptr);
            }
        }
    }
}

impl fmt::Debug for UsbBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsbBuffer")
            .field("ptr", &self.ptr)
            .field("size", &self.size)
            .field("is_dma", &self.is_dma)
            .finish()
    }
}

unsafe impl Send for UsbBuffer {}
unsafe impl Sync for UsbBuffer {}

/// Memory alignment helpers
pub mod alignment {
    /// Align value up to alignment boundary
    pub const fn align_up(value: usize, align: usize) -> usize {
        (value + align - 1) & !(align - 1)
    }

    /// Align value down to alignment boundary
    pub const fn align_down(value: usize, align: usize) -> usize {
        value & !(align - 1)
    }

    /// Check if value is aligned
    pub const fn is_aligned(value: usize, align: usize) -> bool {
        value & (align - 1) == 0
    }

    /// Common alignment values
    pub const BYTE: usize = 1;
    pub const WORD: usize = 2;
    pub const DWORD: usize = 4;
    pub const QWORD: usize = 8;
    pub const CACHE_LINE: usize = 64;
    pub const PAGE: usize = 4096;
}

/// Scatter-gather list for large transfers
pub struct ScatterGatherList {
    /// List of buffer segments
    segments: Vec<UsbBuffer>,
    /// Total size
    total_size: usize,
}

impl ScatterGatherList {
    /// Create a new scatter-gather list
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            total_size: 0,
        }
    }

    /// Add a buffer segment
    pub fn add_segment(&mut self, buffer: UsbBuffer) {
        self.total_size += buffer.len();
        self.segments.push(buffer);
    }

    /// Get total size
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Get number of segments
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Get segment by index
    pub fn segment(&self, index: usize) -> Option<&UsbBuffer> {
        self.segments.get(index)
    }

    /// Get mutable segment by index
    pub fn segment_mut(&mut self, index: usize) -> Option<&mut UsbBuffer> {
        self.segments.get_mut(index)
    }

    /// Iterator over segments
    pub fn segments(&self) -> impl Iterator<Item = &UsbBuffer> {
        self.segments.iter()
    }

    /// Copy data into the scatter-gather list
    pub fn copy_from_slice(&mut self, mut src: &[u8]) -> Result<usize> {
        let mut copied = 0;

        for segment in &mut self.segments {
            if src.is_empty() {
                break;
            }

            let to_copy = core::cmp::min(src.len(), segment.len());
            segment.copy_from_slice(&src[..to_copy])?;
            src = &src[to_copy..];
            copied += to_copy;
        }

        Ok(copied)
    }

    /// Copy data from the scatter-gather list
    pub fn copy_to_slice(&self, mut dst: &mut [u8]) -> Result<usize> {
        let mut copied = 0;

        for segment in &self.segments {
            if dst.is_empty() {
                break;
            }

            let to_copy = core::cmp::min(dst.len(), segment.len());
            segment.copy_to_slice(&mut dst[..to_copy])?;
            dst = &mut dst[to_copy..];
            copied += to_copy;
        }

        Ok(copied)
    }
}

impl fmt::Debug for ScatterGatherList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScatterGatherList")
            .field("segment_count", &self.segments.len())
            .field("total_size", &self.total_size)
            .finish()
    }
}