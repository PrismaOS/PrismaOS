//! Higher-level subsystems initialization
//!
//! Initializes syscalls, userspace protections, enables interrupts, and
//! performs optional device initialization and a small renderer test.

/// Initialize higher-level subsystems: syscalls, userspace protections,
/// interrupts enabling, device drivers (best effort), and display a small
/// color test. This function assumes memory and core subsystems are ready.
pub fn init_higher_level_subsystems() {
    crate::syscall::init_syscalls();
    crate::kprintln!("[OK] Syscall interface initialized");

    unsafe {
        crate::userspace_isolation::setup_userspace_protection();
    }

    // Now enable interrupts once userspace protections are in place
    x86_64::instructions::interrupts::enable();
    crate::kprintln!("[OK] Interrupts enabled");

    // Device initialization is currently simplified/optional
    // crate::drivers::init_devices();
    crate::kprintln!("[OK] Device drivers ready (init skipped for now)");

    // Simple visual test to ensure the renderer is functional
    crate::utils::color_test::show_rainbow_test();
}
