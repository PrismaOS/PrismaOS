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
use lib_kernel::{
    kprintln,
    consts::BASE_REVISION,
    scrolling_text,
};

extern crate alloc;

use lib_kernel::consts::*;
use x86_64;

pub mod userspace_isolation;
pub mod boot_userspace;
pub mod userspace_test;

use ide::ide_initialize;

mod init;
mod utils;

use ahci;
use pci::init_pci;
use galleon2::{read_boot_block, validate_boot_block, write_boot_block};
use usb;
use luminal_rt;

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


    // +--------------------------------+
    // |                                |
    // | Temporary IDE testing code     |
    // |                                |
    // +--------------------------------+
    ide_initialize();
    kprintln!("If this doesnt work cry: {}", validate_boot_block(0));
    read_boot_block(0);



    // Initialize USB subsystem
    init_usb_controllers();



    #[cfg(test)]
    test_main();

    kprintln!("System idle. Halting CPU...");

    // Enter an infinite HALT loop.
    loop {
        core::arch::asm!("hlt");
    }
}

/// Initialize USB controllers found via PCI enumeration
fn init_usb_controllers() {
    use ez_pci::PciAccess;

    kprintln!("Scanning for USB controllers...");

    let mut pci = unsafe { PciAccess::new_pci() };
    let busses = pci.known_buses();
    let mut usb_controllers_found = 0;

    for bus in busses {
        let mut specific_bus = pci.bus(bus);
        if let Some(mut device) = specific_bus.device(bus) {
            let functions = device.possible_functions();

            for function in functions {
                if let Some(mut pci_fn) = device.function(function) {
                    let class_code = pci_fn.class_code();
                    let subclass = pci_fn.sub_class();
                    let prog_if = pci_fn.prog_if();

                    // USB controllers have class code 0x0C (Serial Bus Controller)
                    // and subclass 0x03 (USB controller)
                    if class_code == 0x0C && subclass == 0x03 {
                        let vendor_id = pci_fn.vendor_id();
                        let device_id = pci_fn.device_id();

                        kprintln!("Found USB controller: Bus {}, Function {}", bus, function);
                        kprintln!("  Vendor: {:#06x}, Device: {:#06x}", vendor_id, device_id);
                        kprintln!("  Class: {:#04x}, Subclass: {:#04x}, Prog IF: {:#04x}",
                                 class_code, subclass, prog_if);

                        // Check if this is an xHCI controller (prog_if 0x30)
                        if prog_if == 0x30 {
                            kprintln!("  Type: xHCI (USB 3.0) controller");

                            // Get BAR0 for MMIO base address
                            // Note: BAR access method depends on ez_pci crate API
                            // For now, we'll use a placeholder address
                            let _mmio_base = 0xFE000000; // Placeholder MMIO base
                            // For now, we'll just log the USB controller discovery
                            // Full USB initialization requires async runtime setup
                            kprintln!("  USB controller found - initialization deferred");
                            kprintln!("  Note: USB initialization will be added in future kernel updates");
                            usb_controllers_found += 1;
                        } else if prog_if == 0x20 {
                            kprintln!("  Type: EHCI (USB 2.0) controller - not supported yet");
                        } else if prog_if == 0x10 {
                            kprintln!("  Type: OHCI (USB 1.1) controller - not supported yet");
                        } else if prog_if == 0x00 {
                            kprintln!("  Type: UHCI (USB 1.1) controller - not supported yet");
                        } else {
                            kprintln!("  Type: Unknown USB controller (prog_if: {:#04x})", prog_if);
                        }
                    }
                }
            }
        }
    }

    if usb_controllers_found == 0 {
        kprintln!("No supported USB controllers found");
    } else {
        kprintln!("Initialized {} USB controller(s)", usb_controllers_found);
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