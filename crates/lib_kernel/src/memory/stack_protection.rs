//! Stack overflow protection and guard page implementation
//!
//! This module provides stack overflow protection for both kernel and userspace
//! stacks by implementing guard pages and stack canaries.

use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Stack protection configuration
pub struct StackProtection {
    /// Guard page size (typically 4KB)
    guard_size: usize,
    /// Stack canary value for detection
    canary_value: u64,
    /// Stack overflow counter for statistics
    overflow_count: AtomicUsize,
}

impl StackProtection {
    /// Create new stack protection with random canary
    pub const fn new() -> Self {
        Self {
            guard_size: 4096,
            canary_value: 0xDEADBEEFCAFEBABE, // In real implementation, this would be random
            overflow_count: AtomicUsize::new(0),
        }
    }

    /// Set up stack guard pages for a given stack region
    pub fn setup_stack_guards(
        &self,
        stack_bottom: VirtAddr,
        _stack_size: usize,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), &'static str> {
        // Guard page at the bottom of the stack (lowest address)
        let guard_page = Page::containing_address(stack_bottom);

        // Allocate frame for guard page
        let guard_frame = frame_allocator
            .allocate_frame()
            .ok_or("Failed to allocate guard page frame")?;

        // Map guard page as read-only (no write access)
        // Any write to this page will cause a page fault
        let guard_flags = PageTableFlags::PRESENT; // No WRITABLE flag

        unsafe {
            mapper.map_to(guard_page, guard_frame, guard_flags, frame_allocator)
                .map_err(|_| "Failed to map guard page")?
                .flush();
        }

        crate::kprintln!("[STACK_PROTECTION] Guard page mapped at 0x{:x}",
                        stack_bottom.as_u64());

        // Set up stack canary at the end of usable stack space
        let canary_addr = stack_bottom + self.guard_size as u64;
        unsafe {
            let canary_ptr = canary_addr.as_u64() as *mut u64;
            core::ptr::write_volatile(canary_ptr, self.canary_value);
        }

        crate::kprintln!("[STACK_PROTECTION] Stack canary placed at 0x{:x}",
                        canary_addr.as_u64());

        Ok(())
    }

    /// Check if stack canary is intact
    pub fn check_canary(&self, stack_bottom: VirtAddr) -> bool {
        let canary_addr = stack_bottom + self.guard_size as u64;
        unsafe {
            let canary_ptr = canary_addr.as_u64() as *const u64;
            let current_value = core::ptr::read_volatile(canary_ptr);
            current_value == self.canary_value
        }
    }

    /// Handle stack overflow detection
    pub fn handle_overflow(&self, fault_addr: VirtAddr, stack_bottom: VirtAddr) -> bool {
        let guard_page_start = stack_bottom.as_u64();
        let guard_page_end = guard_page_start + self.guard_size as u64;

        // Check if fault occurred in guard page
        if fault_addr.as_u64() >= guard_page_start && fault_addr.as_u64() < guard_page_end {
            self.overflow_count.fetch_add(1, Ordering::Relaxed);

            crate::kprintln!("[STACK_PROTECTION] ❌ Stack overflow detected!");
            crate::kprintln!("  Fault address: 0x{:x}", fault_addr.as_u64());
            crate::kprintln!("  Guard page: 0x{:x} - 0x{:x}", guard_page_start, guard_page_end);
            crate::kprintln!("  Total overflows: {}", self.overflow_count.load(Ordering::Relaxed));

            true // Stack overflow confirmed
        } else {
            false // Not a stack overflow
        }
    }

    /// Get stack overflow statistics
    pub fn get_stats(&self) -> usize {
        self.overflow_count.load(Ordering::Relaxed)
    }
}

/// Global stack protection instance
static STACK_PROTECTION: StackProtection = StackProtection::new();

/// Set up kernel stack protection
pub fn setup_kernel_stack_protection(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), &'static str> {
    crate::kprintln!("[STACK_PROTECTION] Setting up kernel stack protection...");

    // Kernel stacks are typically at fixed locations
    // For PrismaOS, we'll protect the main kernel stack

    // Get current stack pointer to determine stack location
    let current_rsp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) current_rsp);
    }

    // Assume 64KB kernel stack, find bottom
    const KERNEL_STACK_SIZE: usize = 64 * 1024;
    let stack_top = VirtAddr::new((current_rsp + 4095) & !4095); // Align up
    let stack_bottom = stack_top - KERNEL_STACK_SIZE as u64;

    crate::kprintln!("[STACK_PROTECTION] Kernel stack detected:");
    crate::kprintln!("  Current RSP: 0x{:x}", current_rsp);
    crate::kprintln!("  Stack bottom: 0x{:x}", stack_bottom.as_u64());
    crate::kprintln!("  Stack top: 0x{:x}", stack_top.as_u64());

    STACK_PROTECTION.setup_stack_guards(
        stack_bottom,
        KERNEL_STACK_SIZE,
        mapper,
        frame_allocator
    )
}

/// Set up userspace stack protection
pub fn setup_userspace_stack_protection(
    stack_bottom: VirtAddr,
    stack_size: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), &'static str> {
    crate::kprintln!("[STACK_PROTECTION] Setting up userspace stack protection...");
    crate::kprintln!("  Stack bottom: 0x{:x}", stack_bottom.as_u64());
    crate::kprintln!("  Stack size: {} KB", stack_size / 1024);

    STACK_PROTECTION.setup_stack_guards(
        stack_bottom,
        stack_size,
        mapper,
        frame_allocator
    )
}

/// Check if a page fault is due to stack overflow
pub fn is_stack_overflow(fault_addr: VirtAddr) -> bool {
    // This is a simplified check - in a real implementation, we'd track
    // all stack regions and check against each one

    // For now, check against common stack ranges
    let fault_u64 = fault_addr.as_u64();

    // Kernel stack range (approximate)
    if fault_u64 >= 0xFFFF_8000_0000_0000 && fault_u64 < 0xFFFF_FFFF_FFFF_FFFF {
        // Potential kernel stack overflow
        return true;
    }

    // Userspace stack range (typical location)
    if fault_u64 >= 0x7FFF_0000_0000 && fault_u64 < 0x8000_0000_0000 {
        // Potential userspace stack overflow
        return true;
    }

    false
}

/// Handle page fault that might be stack overflow
pub fn handle_potential_stack_overflow(fault_addr: VirtAddr) -> bool {
    if is_stack_overflow(fault_addr) {
        crate::kprintln!("[STACK_PROTECTION] ⚠️  Potential stack overflow at 0x{:x}",
                        fault_addr.as_u64());

        // In a real implementation, we would:
        // 1. Identify which stack this belongs to
        // 2. Check if it's actually the guard page
        // 3. Terminate the offending process/thread
        // 4. Log the incident for debugging

        true
    } else {
        false
    }
}

/// Periodic stack canary check (called from timer interrupt)
pub fn periodic_canary_check() {
    // In a real implementation, we'd check canaries for all active stacks
    // This is a placeholder for the concept

    static mut LAST_CHECK: usize = 0;
    static mut CHECK_COUNTER: usize = 0;

    unsafe {
        CHECK_COUNTER += 1;

        // Check every 1000 timer ticks
        if CHECK_COUNTER >= 1000 {
            CHECK_COUNTER = 0;

            // Get current stats
            let overflow_count = STACK_PROTECTION.get_stats();
            if overflow_count != LAST_CHECK {
                crate::kprintln!("[STACK_PROTECTION] New overflows detected: {}",
                               overflow_count - LAST_CHECK);
                LAST_CHECK = overflow_count;
            }
        }
    }
}

/// Stack safety utilities
pub mod utils {
    use super::*;

    /// Check remaining stack space
    pub fn check_stack_space() -> usize {
        let current_rsp: u64;
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) current_rsp);
        }

        // This is a rough estimate - in reality we'd need to track
        // the actual stack boundaries for accurate measurement
        const ESTIMATED_STACK_SIZE: u64 = 64 * 1024;
        let estimated_bottom = (current_rsp & !0xFFFF) + 0x1000; // Rough estimate

        if current_rsp > estimated_bottom {
            (current_rsp - estimated_bottom) as usize
        } else {
            0
        }
    }

    /// Macro for checking stack space before large allocations
    #[macro_export]
    macro_rules! check_stack_before_alloc {
        ($size:expr) => {
            {
                let remaining = $crate::stack_protection::utils::check_stack_space();
                if remaining < $size + 1024 { // 1KB safety margin
                    $crate::kprintln!("[STACK_PROTECTION] ⚠️  Low stack space: {} bytes remaining", remaining);
                }
            }
        };
    }

    /// Safe stack allocation with overflow check
    pub fn safe_stack_alloc<T, F>(size: usize, f: F) -> Result<T, &'static str>
    where
        F: FnOnce() -> T,
    {
        let remaining = check_stack_space();
        if remaining < size + 2048 { // 2KB safety margin
            return Err("Insufficient stack space for allocation");
        }

        Ok(f())
    }
}