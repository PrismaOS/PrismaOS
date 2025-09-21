//! Platform abstraction layer for no_std GalleonFS
use core::{sync::atomic::{AtomicU64, Ordering}, time::Duration, cell::UnsafeCell};

/// Platform-specific random number generation
pub trait PlatformRng {
    fn next_u64(&mut self) -> u64;
    fn fill_bytes(&mut self, dest: &mut [u8]);
}

/// Simple linear congruential generator for no_std environments
pub struct SimpleRng {
    state: AtomicU64,
}

impl SimpleRng {
    pub const fn new(seed: u64) -> Self {
        Self { 
            state: AtomicU64::new(seed),
        }
    }
    
    pub fn from_entropy() -> Self {
        // In a real no_std implementation, you'd get entropy from:
        // - Hardware random number generator
        // - Timer values
        // - ADC noise
        // - Other platform-specific entropy sources
        let entropy = get_platform_entropy();
        Self::new(entropy)
    }
}

impl PlatformRng for SimpleRng {
    fn next_u64(&mut self) -> u64 {
        let current = self.state.load(Ordering::Relaxed);
        let next = current.wrapping_mul(1103515245).wrapping_add(12345);
        self.state.store(next, Ordering::Relaxed);
        next
    }
    
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let random_bytes = self.next_u64().to_le_bytes();
            let len = chunk.len().min(8);
            chunk[..len].copy_from_slice(&random_bytes[..len]);
        }
    }
}

/// Global RNG instance
struct GlobalRngCell(UnsafeCell<SimpleRng>);

// SAFETY: We guarantee single-threaded or controlled access in no_std
unsafe impl Sync for GlobalRngCell {}

static GLOBAL_RNG: GlobalRngCell = GlobalRngCell(UnsafeCell::new(SimpleRng::new(0x123456789ABCDEF0)));

/// Get the global RNG instance
pub fn get_rng() -> &'static mut SimpleRng {
    unsafe { &mut *GLOBAL_RNG.0.get() }
}

/// Timestamp for no_std environments
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    pub seconds: u64,
    pub nanoseconds: u32,
}

impl Timestamp {
    pub fn now() -> Self {
        // In no_std, you need to provide time through platform-specific means
        get_platform_time()
    }

    pub const fn zero() -> Self {
        Self {
            seconds: 0,
            nanoseconds: 0,
        }
    }

    pub fn elapsed_since(&self, earlier: Timestamp) -> Duration {
        let seconds_diff = self.seconds.saturating_sub(earlier.seconds);
        let nanos_diff = if self.nanoseconds >= earlier.nanoseconds {
            self.nanoseconds - earlier.nanoseconds
        } else {
            1_000_000_000 + self.nanoseconds - earlier.nanoseconds
        };

        Duration::new(seconds_diff, nanos_diff)
    }
}

/// Platform-specific time source trait
pub trait PlatformTimeSource {
    fn get_time(&self) -> Timestamp;
    fn get_monotonic_time(&self) -> u64; // Monotonic time in nanoseconds
}

/// Default time source using a simple counter (for testing)
pub struct CounterTimeSource {
    counter: AtomicU64,
}

impl CounterTimeSource {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }
}

impl PlatformTimeSource for CounterTimeSource {
    fn get_time(&self) -> Timestamp {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        Timestamp {
            seconds: count / 1_000_000_000,
            nanoseconds: (count % 1_000_000_000) as u32,
        }
    }

    fn get_monotonic_time(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }
}

/// Global time source
static TIME_SOURCE: CounterTimeSource = CounterTimeSource::new();

/// Platform-specific entropy gathering
fn get_platform_entropy() -> u64 {
    // In a real implementation, this would gather entropy from:
    // - Hardware RNG if available
    // - Timer jitter
    // - Stack addresses
    // - Memory addresses
    // - Hardware-specific sources (ADC noise, etc.)
    
    // For now, use a simple combination of sources
    let stack_addr = &get_platform_entropy as *const _ as u64;
    let time = TIME_SOURCE.get_monotonic_time();
    
    stack_addr.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(time)
}

/// Get current platform time
pub fn get_platform_time() -> Timestamp {
    TIME_SOURCE.get_time()
}

/// Get monotonic time for performance measurements
pub fn get_monotonic_time() -> u64 {
    TIME_SOURCE.get_monotonic_time()
}

/// Platform-specific memory allocation tracking
pub struct MemoryTracker {
    allocated: AtomicU64,
    peak: AtomicU64,
}

impl MemoryTracker {
    pub const fn new() -> Self {
        Self {
            allocated: AtomicU64::new(0),
            peak: AtomicU64::new(0),
        }
    }

    pub fn allocate(&self, size: u64) {
        let current = self.allocated.fetch_add(size, Ordering::Relaxed) + size;
        
        // Update peak if necessary
        let mut peak = self.peak.load(Ordering::Relaxed);
        while current > peak {
            match self.peak.compare_exchange_weak(peak, current, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    pub fn deallocate(&self, size: u64) {
        self.allocated.fetch_sub(size, Ordering::Relaxed);
    }

    pub fn current_usage(&self) -> u64 {
        self.allocated.load(Ordering::Relaxed)
    }

    pub fn peak_usage(&self) -> u64 {
        self.peak.load(Ordering::Relaxed)
    }
}

/// Global memory tracker
static MEMORY_TRACKER: MemoryTracker = MemoryTracker::new();

/// Get the global memory tracker
pub fn get_memory_tracker() -> &'static MemoryTracker {
    &MEMORY_TRACKER
}

/// Platform-specific atomic operations helper
pub struct AtomicHelper;

impl AtomicHelper {
    /// Atomic compare and swap for pointers
    pub fn compare_and_swap_ptr<T>(
        ptr: &core::sync::atomic::AtomicPtr<T>,
        current: *mut T,
        new: *mut T,
    ) -> Result<*mut T, *mut T> {
        ptr.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
    }

    /// Atomic fetch and increment with wraparound
    pub fn fetch_add_wrapping(atomic: &AtomicU64, val: u64) -> u64 {
        atomic.fetch_add(val, Ordering::Relaxed)
    }
}

// Platform yield/sleep moved to kernel::platform (see kernel/src/platform/mod.rs)

/// Platform capabilities detection
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    pub has_hardware_rng: bool,
    pub has_hardware_crypto: bool,
    pub has_rtc: bool,
    pub has_dma: bool,
    pub cache_line_size: usize,
    pub page_size: usize,
    pub max_atomic_width: usize,
}

impl Default for PlatformCapabilities {
    fn default() -> Self {
        Self {
            has_hardware_rng: false,
            has_hardware_crypto: false,
            has_rtc: false,
            has_dma: false,
            cache_line_size: 64,
            page_size: 4096,
            max_atomic_width: 8,
        }
    }
}

/// Get platform capabilities
pub fn get_platform_capabilities() -> PlatformCapabilities {
    // In a real implementation, this would detect hardware features
    PlatformCapabilities::default()
}

/// Platform-specific cache management
pub struct CacheManager;

impl CacheManager {
    /// Flush data cache for the given address range
    pub fn flush_dcache(_addr: *const u8, _len: usize) {
        // Platform-specific cache flush implementation
        #[cfg(target_arch = "arm")]
        {
            // ARM cache flush operations would go here
        }
        
        #[cfg(target_arch = "x86_64")]
        {
            // x86 cache flush operations would go here
        }
    }

    /// Invalidate data cache for the given address range
    pub fn invalidate_dcache(_addr: *const u8, _len: usize) {
        // Platform-specific cache invalidate implementation
    }

    /// Flush and invalidate data cache
    pub fn flush_invalidate_dcache(addr: *const u8, len: usize) {
        Self::flush_dcache(addr, len);
        Self::invalidate_dcache(addr, len);
    }
}

/// Platform-specific interrupt management
pub struct InterruptManager;

impl InterruptManager {
    /// Disable interrupts and return previous state
    pub fn disable() -> bool {
        // Platform-specific interrupt disable
        // Return previous interrupt state
        false
    }

    /// Restore interrupt state
    pub fn restore(_state: bool) {
        // Platform-specific interrupt restore
    }

    /// Critical section helper
    pub fn with_interrupts_disabled<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let state = Self::disable();
        let result = f();
        Self::restore(state);
        result
    }
}

/// Hardware abstraction for storage devices
pub trait StorageDevice {
    /// Read data from the device
    fn read(&self, offset: u64, buffer: &mut [u8]) -> Result<usize, &'static str>;
    
    /// Write data to the device
    fn write(&self, offset: u64, buffer: &[u8]) -> Result<usize, &'static str>;
    
    /// Flush pending writes
    fn flush(&self) -> Result<(), &'static str>;
    
    /// Get device capacity in bytes
    fn capacity(&self) -> u64;
    
    /// Get device block size
    fn block_size(&self) -> u32;
    
    /// Check if device is read-only
    fn is_read_only(&self) -> bool;
}

/// Mock storage device for testing
pub struct MockStorageDevice {
    data: spin::Mutex<alloc::vec::Vec<u8>>,
    capacity: u64,
    block_size: u32,
}

impl MockStorageDevice {
    pub fn new(capacity: u64, block_size: u32) -> Self {
        Self {
            data: spin::Mutex::new(alloc::vec![0u8; capacity as usize]),
            capacity,
            block_size,
        }
    }
}

impl StorageDevice for MockStorageDevice {
    fn read(&self, offset: u64, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let data = self.data.lock();
        
        if offset >= self.capacity {
            return Err("Offset beyond device capacity");
        }
        
        let start = offset as usize;
        let len = buffer.len().min((self.capacity - offset) as usize);
        let end = start + len;
        
        if end <= data.len() {
            buffer[..len].copy_from_slice(&data[start..end]);
            Ok(len)
        } else {
            Err("Read beyond device capacity")
        }
    }
    
    fn write(&self, offset: u64, buffer: &[u8]) -> Result<usize, &'static str> {
        let mut data = self.data.lock();
        
        if offset >= self.capacity {
            return Err("Offset beyond device capacity");
        }
        
        let start = offset as usize;
        let len = buffer.len().min((self.capacity - offset) as usize);
        let end = start + len;
        
        if end <= data.len() {
            data[start..end].copy_from_slice(&buffer[..len]);
            Ok(len)
        } else {
            Err("Write beyond device capacity")
        }
    }
    
    fn flush(&self) -> Result<(), &'static str> {
        // Mock device is always flushed
        Ok(())
    }
    
    fn capacity(&self) -> u64 {
        self.capacity
    }
    
    fn block_size(&self) -> u32 {
        self.block_size
    }
    
    fn is_read_only(&self) -> bool {
        false
    }
}