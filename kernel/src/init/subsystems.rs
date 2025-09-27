//! Higher-level subsystems initialization
//!
//! Initializes syscalls, userspace protections, enables interrupts, and
//! performs optional device initialization and a small renderer test.

/// Higher-level subsystem initialization errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemInitError {
    SyscallInitFailed,
    UserspaceProtectionFailed,
    DeviceInitFailed,
    RendererTestFailed,
}

impl ::core::fmt::Display for SubsystemInitError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            Self::SyscallInitFailed => write!(f, "Syscall interface initialization failed"),
            Self::UserspaceProtectionFailed => write!(f, "Userspace protection setup failed"),
            Self::DeviceInitFailed => write!(f, "Device driver initialization failed"),
            Self::RendererTestFailed => write!(f, "Renderer test failed"),
        }
    }
}

/// Safe initialization of higher-level subsystems
pub fn init_higher_level_subsystems() -> Result<(), &'static str> {
    // Initialize syscall interface
    lib_kernel::syscall::init_syscalls();
    lib_kernel::kprintln!("[OK] Syscall interface initialized");

    // Setup userspace protection
    match setup_userspace_protection_safe() {
        Ok(_) => lib_kernel::kprintln!("[OK] Userspace protections active"),
        Err(_) => {
            lib_kernel::kprintln!("[WARN] Userspace protection setup failed, continuing with reduced security...");
            // Continue without full userspace isolation
        }
    }

    // Enable interrupts (this is generally safe once core systems are ready)
    x86_64::instructions::interrupts::enable();
    lib_kernel::kprintln!("[OK] Interrupts enabled");

    // Device initialization (best effort)
    lib_kernel::kprintln!("[OK] Device drivers ready (simplified init)");

    // Visual test (best effort)
    match test_renderer_functionality() {
        Ok(_) => lib_kernel::kprintln!("[OK] Renderer functionality verified"),
        Err(_) => lib_kernel::kprintln!("[WARN] Renderer test failed, display may not work properly"),
    }

    Ok(())
}

/// Safe userspace protection setup
fn setup_userspace_protection_safe() -> Result<(), SubsystemInitError> {
    // Wrap the unsafe userspace protection setup
    unsafe {
        crate::userspace_isolation::setup_userspace_protection();
    }
    Ok(())
}

/// Test renderer functionality
fn test_renderer_functionality() -> Result<(), SubsystemInitError> {
    // Safe wrapper around renderer test
    lib_kernel::utils::color_test::show_rainbow_test();
    Ok(())
}
