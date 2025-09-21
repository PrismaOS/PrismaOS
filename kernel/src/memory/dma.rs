use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{PhysFrame, Page, Size4KiB};

/// A DMA buffer that can be shared between kernel and userspace
/// or between processes with zero-copy semantics
#[derive(Debug)]
pub struct DmaBuffer {
    id: BufferId,
    physical_pages: Vec<PhysFrame<Size4KiB>>,
    size: usize,
    pinned: bool,
    ref_count: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(u64);

impl BufferId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        BufferId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl DmaBuffer {
    /// Create a new DMA buffer with the specified size in bytes
    pub fn new(size: usize) -> Result<Arc<Self>, DmaError> {
        if size == 0 {
            return Err(DmaError::InvalidSize);
        }

        // Round up to page boundaries
        let page_count = (size + 0xFFF) / 0x1000;
        let mut physical_pages = Vec::with_capacity(page_count);

        // Allocate physical pages
        // In a real implementation, this would use a frame allocator
        for i in 0..page_count {
            let phys_addr = PhysAddr::new(0x100000 + (i * 0x1000) as u64); // Placeholder
            let frame = PhysFrame::containing_address(phys_addr);
            physical_pages.push(frame);
        }

        Ok(Arc::new(DmaBuffer {
            id: BufferId::new(),
            physical_pages,
            size: page_count * 0x1000, // Actual allocated size
            pinned: true,
            ref_count: AtomicU64::new(1),
        }))
    }

    /// Get the buffer ID
    pub fn id(&self) -> BufferId {
        self.id
    }

    /// Get the physical pages backing this buffer
    pub fn physical_pages(&self) -> &[PhysFrame<Size4KiB>] {
        &self.physical_pages
    }

    /// Get the size in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if buffer is pinned in memory
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Map this buffer into a virtual address space
    pub unsafe fn map_to_virtual(
        &self,
        virt_addr: VirtAddr,
        mapper: &mut impl x86_64::structures::paging::Mapper<Size4KiB>,
        frame_allocator: &mut impl x86_64::structures::paging::FrameAllocator<Size4KiB>,
        writable: bool,
    ) -> Result<(), DmaError> {
        use x86_64::structures::paging::PageTableFlags;

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            flags |= PageTableFlags::WRITABLE;
        }

        for (i, &frame) in self.physical_pages.iter().enumerate() {
            let page_addr = virt_addr + (i * 0x1000) as u64;
            let page = Page::containing_address(page_addr);

            match mapper.map_to(page, frame, flags, frame_allocator) {
                Ok(flush) => flush.flush(),
                Err(_) => return Err(DmaError::MappingFailed),
            }
        }

        Ok(())
    }

    /// Increment reference count
    pub fn add_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count, returns true if buffer should be freed
    pub fn release(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Relaxed) == 1
    }

    /// Get current reference count
    pub fn ref_count(&self) -> u64 {
        self.ref_count.load(Ordering::Relaxed)
    }
}

/// Global DMA buffer registry for tracking shared buffers
pub struct DmaRegistry {
    buffers: RwLock<Vec<Option<Arc<DmaBuffer>>>>,
}

impl DmaRegistry {
    pub const fn new() -> Self {
        DmaRegistry {
            buffers: RwLock::new(Vec::new()),
        }
    }

    /// Register a new DMA buffer
    pub fn register_buffer(&self, buffer: Arc<DmaBuffer>) -> BufferId {
        let id = buffer.id();
        let mut buffers = self.buffers.write();
        
        // Find empty slot or expand registry
        //TODO: optimize with a free list rather than scanning
        if let Some(index) = buffers.iter().position(|b| b.is_none()) {
            buffers[index] = Some(buffer);
        } else {
            buffers.push(Some(buffer));
        }
        
        id
    }

    /// Get a buffer by ID
    pub fn get_buffer(&self, id: BufferId) -> Option<Arc<DmaBuffer>> {
        let buffers = self.buffers.read();
        for buffer_slot in buffers.iter() {
            if let Some(buffer) = buffer_slot {
                if buffer.id() == id {
                    buffer.add_ref();
                    return Some(buffer.clone());
                }
            }
        }
        None
    }

    /// Remove buffer from registry (called when all references are dropped)
    pub fn unregister_buffer(&self, id: BufferId) {
        let mut buffers = self.buffers.write();
        for buffer_slot in buffers.iter_mut() {
            if let Some(buffer) = buffer_slot {
                if buffer.id() == id {
                    *buffer_slot = None;
                    break;
                }
            }
        }
    }

    /// Get statistics about DMA buffers
    pub fn get_stats(&self) -> DmaStats {
        let buffers = self.buffers.read();
        let mut total_buffers = 0;
        let mut total_memory = 0;
        let mut pinned_buffers = 0;

        // TODO: optimize by maintaining counts during register/unregister
        for buffer_slot in buffers.iter() {
            if let Some(buffer) = buffer_slot {
                total_buffers += 1;
                total_memory += buffer.size();
                if buffer.is_pinned() {
                    pinned_buffers += 1;
                }
            }
        }

        DmaStats {
            total_buffers,
            total_memory_bytes: total_memory,
            pinned_buffers,
            registry_slots: buffers.len(),
        }
    }
}

#[derive(Debug)]
pub struct DmaStats {
    pub total_buffers: usize,
    pub total_memory_bytes: usize,
    pub pinned_buffers: usize,
    pub registry_slots: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    InvalidSize,
    OutOfMemory,
    MappingFailed,
    BufferNotFound,
    AccessDenied,
}

/// Global DMA registry instance
static DMA_REGISTRY: DmaRegistry = DmaRegistry::new();

pub fn dma_registry() -> &'static DmaRegistry {
    &DMA_REGISTRY
}

/// Create a new DMA buffer and register it
pub fn create_dma_buffer(size: usize) -> Result<BufferId, DmaError> {
    let buffer = DmaBuffer::new(size)?;
    let id = buffer.id();
    dma_registry().register_buffer(buffer);
    Ok(id)
}

/// Get a DMA buffer by ID
pub fn get_dma_buffer(id: BufferId) -> Option<Arc<DmaBuffer>> {
    dma_registry().get_buffer(id)
}

/// Map a DMA buffer to userspace virtual address
pub unsafe fn map_buffer_to_userspace(
    buffer_id: BufferId,
    process_vaddr: VirtAddr,
    writable: bool,
) -> Result<(), DmaError> {
    let buffer = get_dma_buffer(buffer_id).ok_or(DmaError::BufferNotFound)?;
    
    // In a real implementation, we would:
    // 1. Get the process's page table
    // 2. Map the buffer's physical pages to the virtual address
    // 3. Update the process's memory map
    
    // For now, just return success as a placeholder
    Ok(())
}

/// Zero-copy buffer sharing between processes
pub fn share_buffer_with_process(
    buffer_id: BufferId,
    target_process: crate::api::ProcessId,
    rights: crate::api::Rights,
) -> Result<crate::api::ObjectHandle, DmaError> {
    let buffer = get_dma_buffer(buffer_id).ok_or(DmaError::BufferNotFound)?;
    
    // Create a buffer object for the IPC system
    let buffer_object = crate::api::objects::Buffer::from_dma_buffer(buffer);
    
    // Register with object registry
    let registry = crate::api::get_registry();
    registry.write().register_object(
        alloc::sync::Arc::new(buffer_object),
        target_process,
        rights,
    ).map_err(|_| DmaError::AccessDenied)
}