use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use spin::Mutex;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

// Kernel heap configuration
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 256 * 1024 * 1024; // 256 MiB

// Buddy allocator configuration
const MIN_BLOCK_SIZE: usize = 32; // Minimum allocation size (must fit BlockHeader)
const MAX_ORDER: usize = 26; // 2^26 = 64 MiB max single allocation
const MIN_ORDER: usize = 5; // 2^5 = 32 bytes min allocation

// Block header stored at the beginning of each free block
#[repr(C)]
struct BlockHeader {
    next: Option<NonNull<BlockHeader>>,
    prev: Option<NonNull<BlockHeader>>,
}

impl BlockHeader {
    const fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }
}

/// Production-grade buddy allocator with anti-fragmentation and statistics
pub struct BuddyAllocator {
    heap_start: usize,
    heap_size: usize,
    // Free lists for each order (2^order sized blocks)
    free_lists: [Option<NonNull<BlockHeader>>; MAX_ORDER + 1],
    // Statistics
    stats: AllocatorStats,
}

#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub active_allocations: u64,
    pub bytes_allocated: usize,
    pub bytes_freed: usize,
    pub bytes_in_use: usize,
    pub failed_allocations: u64,
}

impl AllocatorStats {
    const fn new() -> Self {
        Self {
            total_allocations: 0,
            total_deallocations: 0,
            active_allocations: 0,
            bytes_allocated: 0,
            bytes_freed: 0,
            bytes_in_use: 0,
            failed_allocations: 0,
        }
    }
}

// SAFETY: BuddyAllocator is only accessed through a Mutex, ensuring single-threaded access
unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_size: 0,
            free_lists: [None; MAX_ORDER + 1],
            stats: AllocatorStats::new(),
        }
    }

    /// Initialize the allocator with a memory region
    unsafe fn init(&mut self, heap_start: *mut u8, heap_size: usize) {
        // CRITICAL: Store these values FIRST
        let new_start = heap_start as usize;
        let new_size = heap_size;
        
        self.heap_start = new_start;
        self.heap_size = new_size;

        // Clear all free lists
        for list in &mut self.free_lists {
            *list = None;
        }

        // IMPORTANT: Reset statistics when reinitializing
        // This prevents corruption from previous heap allocations
        self.stats = AllocatorStats::new();

        // Add free blocks in descending order size, ensuring proper alignment
        // This builds the free list from the entire heap
        let mut current_addr = new_start;
        let end_addr = new_start + new_size;
        let mut blocks_added = 0;

        while current_addr < end_addr {
            // Find the largest block we can create at this address
            let remaining = end_addr - current_addr;
            let mut order = MAX_ORDER;

            // Find the largest properly-aligned block that fits
            let mut added = false;
            while order >= MIN_ORDER {
                let block_size = 1 << order;

                // Check if block fits and address is properly aligned for this order
                if block_size <= remaining && (current_addr & (block_size - 1)) == 0 {
                    if self.add_free_block(current_addr, order) {
                        current_addr += block_size;
                        blocks_added += 1;
                        added = true;
                        break;
                    }
                    // If add failed, try smaller block size
                }

                order -= 1;
            }

            // If we couldn't add any block at this address, skip minimum amount
            if !added {
                current_addr += MIN_BLOCK_SIZE;
            }
        }
        
        // Store blocks_added in stats for reporting outside the lock
        // Reuse failed_allocations temporarily as a communication channel
        self.stats.failed_allocations = blocks_added as u64;
    }

    /// Convert size to order (log2 rounded up)
    fn size_to_order(size: usize) -> usize {
        let size = size.max(MIN_BLOCK_SIZE);
        let mut order = 0;
        let mut block_size = 1;

        while block_size < size && order < MAX_ORDER {
            block_size <<= 1;
            order += 1;
        }

        order.max(MIN_ORDER)
    }

    /// Get buddy address for a block
    fn get_buddy(&self, addr: usize, order: usize) -> usize {
        let block_size = 1 << order;
        addr ^ block_size
    }

    /// Check if address is within heap bounds
    fn is_valid_address(&self, addr: usize) -> bool {
        addr >= self.heap_start && addr < self.heap_start + self.heap_size
    }

    /// Add a free block to the appropriate free list
    unsafe fn add_free_block(&mut self, addr: usize, order: usize) -> bool {
        if order > MAX_ORDER {
            return false;
        }
        
        if !self.is_valid_address(addr) {
            return false;
        }
        
        // Check alignment
        let block_size = 1 << order;
        if addr & (block_size - 1) != 0 {
            return false;
        }

        // Try to write to the address to ensure it's mapped and writable
        let header = addr as *mut BlockHeader;
        
        // Initialize the block header
        core::ptr::write(header, BlockHeader::new());
        
        (*header).next = self.free_lists[order];
        (*header).prev = None;

        if let Some(mut next) = self.free_lists[order] {
            let next_addr = next.as_ptr() as usize;
            if !self.is_valid_address(next_addr) {
                return false;
            }
            next.as_mut().prev = NonNull::new(header);
        }

        self.free_lists[order] = NonNull::new(header);
        true
    }

    /// Remove a specific block from its free list
    unsafe fn remove_free_block(&mut self, addr: usize, order: usize) {
        if order > MAX_ORDER {
            return;
        }

        let header = addr as *mut BlockHeader;
        let prev = (*header).prev;
        let next = (*header).next;

        if let Some(mut prev_block) = prev {
            prev_block.as_mut().next = next;
        } else {
            // This was the head of the list
            self.free_lists[order] = next;
        }

        if let Some(mut next_block) = next {
            next_block.as_mut().prev = prev;
        }
    }

    /// Find a free block in the free list
    unsafe fn find_free_block(&self, order: usize) -> Option<usize> {
        if order > MAX_ORDER {
            return None;
        }

        self.free_lists[order].map(|ptr| ptr.as_ptr() as usize)
    }

    /// Allocate a block of the given order
    unsafe fn allocate_order(&mut self, order: usize) -> Option<usize> {
        if order > MAX_ORDER {
            crate::kprintln!("[ALLOC] Order {} exceeds MAX_ORDER {}", order, MAX_ORDER);
            self.stats.failed_allocations += 1;
            return None;
        }

        // Try to find a block of this order
        if let Some(addr) = self.find_free_block(order) {
            self.remove_free_block(addr, order);
            return Some(addr);
        }

        // No block of this order, try to split a larger block
        if order < MAX_ORDER {
            if let Some(larger_block) = self.allocate_order(order + 1) {
                // Split the larger block
                let buddy = larger_block + (1 << order);
                self.add_free_block(buddy, order);
                return Some(larger_block);
            }
        }

        // Out of memory - log which order failed
        crate::kprintln!("[ALLOC] Failed to allocate order {} (size: {} bytes)", order, 1 << order);
        self.stats.failed_allocations += 1;
        None
    }

    /// Deallocate a block and coalesce with buddy if possible
    unsafe fn deallocate_order(&mut self, addr: usize, order: usize) {
        if order > MAX_ORDER || !self.is_valid_address(addr) {
            return;
        }

        // If we're at MAX_ORDER, we can't coalesce further
        if order == MAX_ORDER {
            self.add_free_block(addr, order);
            return;
        }

        // Try to coalesce with buddy
        let buddy_addr = self.get_buddy(addr, order);

        if self.is_valid_address(buddy_addr) && self.is_free(buddy_addr, order) {
            // Buddy is free, coalesce
            self.remove_free_block(buddy_addr, order);
            let coalesced_addr = addr.min(buddy_addr);
            self.deallocate_order(coalesced_addr, order + 1);
        } else {
            // Cannot coalesce, add to free list
            self.add_free_block(addr, order);
        }
    }

    /// Check if a block is in the free list
    unsafe fn is_free(&self, addr: usize, order: usize) -> bool {
        if order > MAX_ORDER {
            return false;
        }

        let mut current = self.free_lists[order];
        while let Some(block) = current {
            if block.as_ptr() as usize == addr {
                return true;
            }
            current = block.as_ref().next;
        }
        false
    }

    /// Allocate memory with proper alignment
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align()).max(MIN_BLOCK_SIZE);
        let order = Self::size_to_order(size);
        let block_size = 1 << order;

        if let Some(addr) = self.allocate_order(order) {
            // Update statistics
            self.stats.total_allocations += 1;
            self.stats.active_allocations += 1;
            self.stats.bytes_allocated += block_size;
            self.stats.bytes_in_use += block_size;

            addr as *mut u8
        } else {
            // Store the failed size in bytes_allocated temporarily for retrieval outside lock
            self.stats.bytes_allocated = size;
            self.stats.failed_allocations += 1;
            ptr::null_mut()
        }
    }

    /// Deallocate memory
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let addr = ptr as usize;
        if !self.is_valid_address(addr) {
            return;
        }

        let size = layout.size().max(layout.align()).max(MIN_BLOCK_SIZE);
        let order = Self::size_to_order(size);
        let block_size = 1 << order;

        // Update statistics
        self.stats.total_deallocations += 1;
        self.stats.active_allocations = self.stats.active_allocations.saturating_sub(1);
        self.stats.bytes_freed += block_size;
        self.stats.bytes_in_use = self.stats.bytes_in_use.saturating_sub(block_size);

        self.deallocate_order(addr, order);
    }

    pub fn get_stats(&self) -> AllocatorStats {
        self.stats
    }

    /// Count how many free blocks exist (for debugging)
    pub fn count_free_blocks(&self) -> usize {
        let mut count = 0;
        for order in MIN_ORDER..=MAX_ORDER {
            let mut current = self.free_lists[order];
            while let Some(block) = current {
                count += 1;
                unsafe {
                    current = block.as_ref().next;
                }
            }
        }
        count
    }
}

// Global allocator instance
struct LockedAllocator {
    inner: Mutex<BuddyAllocator>,
}

impl LockedAllocator {
    const fn new() -> Self {
        Self {
            inner: Mutex::new(BuddyAllocator::new()),
        }
    }

    unsafe fn init(&self, heap_start: *mut u8, heap_size: usize) {
        self.inner.lock().init(heap_start, heap_size);
    }
}

unsafe impl GlobalAlloc for LockedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = self.inner.lock().alloc(layout);
        if result.is_null() {
            // Get the failed size that was stored
            let failed_size = self.inner.lock().stats.bytes_allocated;
            crate::kprintln!("[ALLOC ERROR] Failed to allocate {} bytes (align {})", 
                failed_size, layout.align());
        }
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: LockedAllocator = LockedAllocator::new();

// Bootstrap heap for early allocations
#[repr(C, align(16))]
struct BootstrapHeap {
    data: [u8; 64 * 1024], // 64KB bootstrap heap
}

static mut BOOTSTRAP_HEAP: BootstrapHeap = BootstrapHeap { data: [0; 64 * 1024] };
static mut BOOTSTRAP_ACTIVE: bool = false;

/// Initialize bootstrap heap for early kernel allocations  
/// Uses a static buffer that will later be included in the main heap
pub unsafe fn init_bootstrap_heap() {
    // Use the static bootstrap buffer temporarily
    // This is in kernel data segment, so it's already mapped
    ALLOCATOR.inner.lock().init(
        core::ptr::addr_of_mut!(BOOTSTRAP_HEAP.data).cast(),
        64 * 1024
    );
    BOOTSTRAP_ACTIVE = true;
    crate::kprintln!("[HEAP] Bootstrap heap initialized at {:#x} (64KB)", 
        core::ptr::addr_of!(BOOTSTRAP_HEAP.data) as usize);
}

/// Initialize the main kernel heap with proper virtual memory mapping
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    crate::kprintln!("[HEAP] ===== STARTING MAIN HEAP INIT =====");
    
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    let total_pages = ((HEAP_SIZE + 4095) / 4096) as u64;
    crate::kprintln!("[HEAP] Allocating {} pages ({} MiB) for heap...",
        total_pages, HEAP_SIZE / (1024 * 1024));

    let mut allocated_pages = 0u64;

    // Map each page of the heap to physical memory
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or_else(|| {
                crate::kprintln!("[HEAP ERROR] Failed to allocate frame after {} pages!", allocated_pages);
                crate::kprintln!("[HEAP ERROR] Needed {} total pages, got {} pages",
                    total_pages, allocated_pages);
                MapToError::FrameAllocationFailed
            })?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            let flush_result = mapper.map_to(page, frame, flags, frame_allocator)?;
            flush_result.flush();
        };
        allocated_pages += 1;

        // Progress indicator every 1000 pages
        if allocated_pages % 1000 == 0 {
            crate::kprintln!("[HEAP] Allocated {} / {} pages...", allocated_pages, total_pages);
        }
    }

    crate::kprintln!("[HEAP] Successfully allocated all {} pages", allocated_pages);

    // Test write to verify pages are actually writable
    crate::kprintln!("[HEAP] Testing heap write access...");
    unsafe {
        let test_ptr = HEAP_START as *mut u64;
        core::ptr::write_volatile(test_ptr, 0xDEADBEEFCAFEBABE);
        let readback = core::ptr::read_volatile(test_ptr);
        if readback != 0xDEADBEEFCAFEBABE {
            crate::kprintln!("[HEAP ERROR] Heap write test failed! Wrote 0xDEADBEEFCAFEBABE, read back {:#x}", readback);
            return Err(MapToError::FrameAllocationFailed);
        }
        crate::kprintln!("[HEAP] Heap write test passed");
    }

    // Initialize the main heap - this is the FIRST and ONLY heap initialization
    crate::kprintln!("[HEAP] Initializing main heap at {:#x} with {} bytes", HEAP_START, HEAP_SIZE);
    
    let blocks_added = unsafe {
        let mut allocator = ALLOCATOR.inner.lock();
        
        // Initialize with full heap (first and only init)
        allocator.init(HEAP_START as *mut u8, HEAP_SIZE);
        
        // Get blocks_added from the temporary storage
        let blocks = allocator.stats.failed_allocations;
        
        // Now reset failed_allocations to 0
        allocator.stats.failed_allocations = 0;
        
        blocks
    };
    
    crate::kprintln!("[HEAP] Added {} free blocks during initialization", blocks_added);
    
    if blocks_added == 0 {
        crate::kprintln!("[HEAP ERROR] No free blocks were added! Heap initialization failed!");
        return Err(MapToError::FrameAllocationFailed);
    }
    
    let block_count = ALLOCATOR.inner.lock().count_free_blocks();
    crate::kprintln!("[HEAP] Buddy allocator initialized: {} MiB, {} free blocks",
        HEAP_SIZE / (1024 * 1024), block_count);

    Ok(())
}

/// Get heap usage statistics
pub fn heap_stats() -> HeapStats {
    let stats = ALLOCATOR.inner.lock().get_stats();

    HeapStats {
        total_size: HEAP_SIZE,
        used_size: stats.bytes_in_use,
        free_size: HEAP_SIZE.saturating_sub(stats.bytes_in_use),
        fragmentation_ratio: 0.0, // Disable fragmentation calc to avoid FPU issues
        total_allocations: stats.total_allocations,
        total_deallocations: stats.total_deallocations,
        active_allocations: stats.active_allocations,
        failed_allocations: stats.failed_allocations,
    }
}

#[derive(Debug)]
pub struct HeapStats {
    pub total_size: usize,
    pub used_size: usize,
    pub free_size: usize,
    pub fragmentation_ratio: f32,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub active_allocations: u64,
    pub failed_allocations: u64,
}

/// Custom allocation error handling
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    // DO NOT try to lock the allocator here - we're already in an allocation failure
    // and the lock is likely already held
    panic!(
        "Allocation error: failed to allocate {} bytes with alignment {}",
        layout.size(),
        layout.align()
    );
}
