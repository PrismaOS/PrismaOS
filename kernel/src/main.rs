//! Kernel entrypoint and minimal runtime utilities
//!
//! This file contains the absolute minimum the kernel needs at boot time:
//! - `kmain`: the entrypoint called from the assembly bootstrap. It performs
//!   a small sanity check and delegates the full initialization to
//!   `crate::init::init_kernel()` so the majority of boot logic is kept in
//!   `kernel/src/init/*` modules.
//! - `panic_handler`: the kernel panic handler that renders a BSOD via the
//!   framebuffer renderer when available, or falls back to VGA text.
//! - `test_runner`: the custom test harness entry used when compiling tests.
//!
//! All functions in this file are intentionally small and well-documented so
//! the platform-specific startup path remains easy to audit.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![test_runner(crate::test_runner)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![allow(warnings)]

use core::panic::PanicInfo;
extern crate alloc;

mod font;
mod scrolling_text;
mod consts;
pub use consts::*;

mod userspace_isolation;
mod boot_userspace;
mod userspace_test;
mod interrupts;
mod scheduler;
mod executor;
mod process;
mod drivers;
mod syscall;
mod events;
mod memory;
mod utils;
mod time;
mod elf;
mod gdt;
pub mod api;
mod usb;
mod init;

use drivers::{ahci::probe_port, ahci::consts::*};

use drivers::pci::init_pci;

// NOTE: speaker and other modules are available as `crate::speaker` if
/// needed; avoid glob imports here to keep the top-level clean.

/// Kernel entrypoint called after assembly bootstrap completes.
///
/// Responsibilities and behavior:
/// - Performs a single sanity check that the Limine protocol revision is
///   supported by this kernel binary. This ensures the Limine request
///   objects are compatible with the code below.
/// - Delegates the rest of the boot sequence to `init::init_kernel()` to
///   keep this function small and auditable.
/// - On failure of the initialization function this routine prints an error
///   and halts the machine (consistent with the prior behavior).
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    // Verify Limine revision; a panic here indicates the bootstrap/limine
    // environment is incompatible with this kernel image.
    assert!(BASE_REVISION.is_supported());

    // Perform the heavy lifting in the `init` module so this entrypoint
    // remains compact and easy to audit. We keep behavior identical to the
    // previous implementation: if initialization fails, halt the system.
    match init::init_kernel() {
        Ok(()) => { /* Initialization completed successfully. */ }
        Err(e) => {
            kprintln!("ERROR: Kernel initialization failed: {}", e);
            crate::utils::system::halt_system();
        }
    }

    kprintln!("PciAccess {:?}", init_pci());

    // When compiling tests the harness re-exports `test_main()`; run it here.
    #[cfg(test)]
    test_main();

    kprintln!("System idle. Halting CPU...");

    // Enter an infinite HALT loop.
    loop {
        core::arch::asm!("hlt");
    }
}

/// Kernel panic handler
///
/// Disables interrupts immediately and attempts to render a BSOD using the
/// framebuffer renderer if available. If no framebuffer renderer exists the
/// handler falls back to a VGA-style blue screen. The handler never returns
/// — it halts the machine after rendering diagnostics.
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    // Disable interrupts to avoid re-entrancy while rendering the panic
    // screen.
    x86_64::instructions::interrupts::disable();

    // Try to render a framebuffer BSOD, otherwise use VGA fallback. The
    // global renderer is an Option in `scrolling_text` so we check it
    // unsafely here (same behavior as before).
    unsafe {
        if let Some(ref mut renderer) = scrolling_text::GLOBAL_RENDERER {
            crate::utils::bsod::render_framebuffer_bsod(renderer, info);
        } else {
            crate::utils::bsod::render_vga_bsod(info);
        }
    }

    crate::utils::system::halt_system();
}

/// Custom test runner required by `#![test_runner]`.
///
/// Executes each test function provided by the test harness. Kept intentionally
/// tiny: real test reporting is handled by the test framework and logging
/// done elsewhere.
pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}