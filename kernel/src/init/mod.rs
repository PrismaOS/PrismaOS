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

pub use core::init_core_subsystems;
pub use framebuffer::init_framebuffer_and_renderer;
pub use lib_kernel::kprintln;
pub use memory::init_memory_and_heap;
pub use subsystems::init_higher_level_subsystems;
pub use userspace::launch_userspace_components;

/// Top-level orchestration function.
///
/// This function keeps `kmain` tiny and delegates to the smaller
/// initialization submodules. It returns `Ok(())` on success or an
/// `Err(&'static str)` on a fatal error that the caller should handle by
/// halting the machine.
pub fn init_kernel() -> Result<(), &'static str> {
    match init_framebuffer_and_renderer() {
        Ok(Some(mut fbctx)) => {
            fbctx.write_line("Initializing kernel subsystems...");

            init_core_subsystems();
            init_memory_and_heap()?;
            init_higher_level_subsystems();
            launch_userspace_components();

            kprintln!("");
            kprintln!("=== PrismaOS Kernel Successfully Initialized ===");
            kprintln!("All systems operational");
            kprintln!("");
            kprintln!("=== Entering idle state ===");
            Ok(())
        }
        Ok(None) => {
            kprintln!("Initializing kernel subsystems...");
            init_core_subsystems();

            unreachable!("CPU should have halted after framebuffer initialization failure");

            init_memory_and_heap()?;
            init_higher_level_subsystems();
            launch_userspace_components();

            kprintln!("");
            kprintln!("=== PrismaOS Kernel Successfully Initialized ===");
            kprintln!("All systems operational");
            kprintln!("");
            kprintln!("=== Entering idle state ===");
            Ok(())
        }
        Err(e) => Err(e),
    }
}
