//! Top-level `init` module
//!
//! This file only re-exports the submodules that contain the real
//! initialization logic. Each submodule is kept small and documented so the
//! boot sequence is easy to follow. The public `init_kernel` function simply
//! orchestrates the smaller building blocks.

pub mod core;
pub mod framebuffer;
pub mod memory;
pub mod subsystems;
pub mod usb;
pub mod userspace;

pub use self::core::{init_core_subsystems, CoreInitError};
pub use framebuffer::init_framebuffer_and_renderer;
pub use lib_kernel::kprintln;
pub use memory::{init_memory_and_heap, MemoryInitError};
pub use subsystems::init_higher_level_subsystems;
pub use userspace::launch_userspace_components;

/// Comprehensive kernel initialization error types
#[derive(Debug)]
pub enum KernelInitError {
    FramebufferInit(&'static str),
    CoreSubsystems(CoreInitError),
    Memory(MemoryInitError),
    HigherLevelSubsystems(&'static str),
    UserspaceComponents(&'static str),
}

impl ::core::fmt::Display for KernelInitError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            Self::FramebufferInit(e) => write!(f, "Framebuffer initialization failed: {}", e),
            Self::CoreSubsystems(e) => write!(f, "Core subsystem initialization failed: {}", e),
            Self::Memory(e) => write!(f, "Memory initialization failed: {}", e),
            Self::HigherLevelSubsystems(e) => write!(f, "Higher level subsystem initialization failed: {}", e),
            Self::UserspaceComponents(e) => write!(f, "Userspace component initialization failed: {}", e),
        }
    }
}

/// Safe, comprehensive kernel initialization with proper error handling
pub fn init_kernel() -> Result<(), KernelInitError> {
    // Phase 1: Initialize framebuffer and renderer
    match init_framebuffer_and_renderer() {
        Ok(Some(mut fbctx)) => {
            fbctx.write_line("Initializing kernel subsystems...");
        }
        Ok(None) => {
            kprintln!("Initializing kernel subsystems (no framebuffer)...");
        }
        Err(e) => return Err(KernelInitError::FramebufferInit(e)),
    }

    // Phase 2: Initialize core subsystems (emergency systems, GDT, IDT, PICs)
    init_core_subsystems().map_err(KernelInitError::CoreSubsystems)?;

    // Phase 3: Initialize memory management (frame allocator, heap, paging)
    init_memory_and_heap().map_err(KernelInitError::Memory)?;

    // Phase 4: Initialize higher-level subsystems
    match init_higher_level_subsystems() {
        Ok(()) => {},
        Err(e) => return Err(KernelInitError::HigherLevelSubsystems(e)),
    }

    // Phase 5: Launch userspace components
    match launch_userspace_components() {
        Ok(()) => {},
        Err(e) => return Err(KernelInitError::UserspaceComponents(e)),
    }

    kprintln!("");
    kprintln!("🎉 === PrismaOS Kernel Successfully Initialized === 🎉");
    kprintln!("✅ All critical systems operational");
    kprintln!("✅ Memory management ready for complex workloads");
    kprintln!("✅ Fault handling active and comprehensive");
    kprintln!("✅ Ready for Galleon2 filesystem and Luminal runtime");
    kprintln!("");
    kprintln!("=== Entering main kernel loop ===");
    
    Ok(())
}
