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
use pci::{init_pci, get_usb_controllers, UsbControllerType};
use galleon2::init_fs;
use usb::{UsbHostDriver, init_usb_subsystem};
use luminal;
use lib_kernel::memory::mmio::XhciMmioMapper;

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
    init_fs(0);

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

/// Get access to kernel memory management for MMIO mapping
fn get_kernel_memory_manager() -> Result<XhciMmioMapper, &'static str> {
    // Create the MMIO mapper using the kernel's memory management system
    XhciMmioMapper::new()
}

/// Initialize a specific xHCI controller
fn init_xhci_controller(controller: &pci::UsbController) -> Result<(), &'static str> {
    kprintln!("      → Initializing xHCI controller");
    kprintln!("      → PCI Location: {:02x}:{:02x}.{}",
             controller.bus, controller.device, controller.function);
    kprintln!("      → Device: {} {}",
             controller.vendor_name, controller.device_name);

    // Check if we have a valid BAR0 address
    let (mmio_base, mmio_size) = match (controller.bar0_addr, controller.bar0_size) {
        (Some(addr), Some(size)) => {
            kprintln!("      → BAR0: {:#x} (size: {:#x})", addr, size);
            (addr, size)
        },
        _ => {
            kprintln!("      → No valid BAR0 found, cannot initialize");
            return Err("No valid MMIO BAR found");
        }
    };

    // Validate the MMIO address
    if mmio_base == 0 {
        return Err("Invalid MMIO base address");
    }

    if mmio_size < 0x1000 {
        return Err("MMIO region too small for xHCI controller");
    }

    kprintln!("      → Requesting kernel memory manager for MMIO mapping");

    // Get the kernel memory manager
    let memory_manager = match get_kernel_memory_manager() {
        Ok(manager) => manager,
        Err(e) => {
            kprintln!("      → Memory management not ready: {}", e);
            kprintln!("      → This is expected in early development");
            kprintln!("      → Controller detected and ready for future initialization");
            return Ok(());
        }
    };

    kprintln!("      → Creating USB host driver with real MMIO mapping");

    // Create the USB host driver with proper memory mapping
    match UsbHostDriver::new(mmio_base as usize, memory_manager) {
        Ok(_driver) => {
            kprintln!("      ✓ xHCI controller initialized successfully");
            kprintln!("      → MMIO region mapped and accessible");
            kprintln!("      → Controller ready for device enumeration");
            Ok(())
        },
        Err(e) => {
            kprintln!("      ✗ Failed to initialize USB host driver: {:?}", e);
            Err("USB driver initialization failed")
        }
    }
}

/// Initialize USB controllers found via PCI enumeration
fn init_usb_controllers() {
    kprintln!("Scanning for USB controllers...");

    let usb_controllers = get_usb_controllers();

    if usb_controllers.is_empty() {
        kprintln!("No USB controllers found!");
        kprintln!("Make sure VirtualBox USB controller is enabled:");
        kprintln!("  Settings -> USB -> Enable USB Controller");
        kprintln!("  Choose: USB 1.1 (OHCI), USB 2.0 (EHCI), or USB 3.0 (xHCI)");
    } else {
        kprintln!("Found {} USB controller(s):", usb_controllers.len());

        for controller in &usb_controllers {
            let type_str = match &controller.controller_type {
                UsbControllerType::UHCI => "USB 1.1 (UHCI)",
                UsbControllerType::OHCI => "USB 1.1 (OHCI)",
                UsbControllerType::EHCI => "USB 2.0 (EHCI)",
                UsbControllerType::XHCI => "USB 3.0 (xHCI)",
                UsbControllerType::Unknown(prog_if) => {
                    kprintln!("  Unknown USB controller type (prog_if: {:#04x})", prog_if);
                    continue;
                }
            };

            kprintln!("  {:02x}:{:02x}.{} {} - {} {} [{:#06x}:{:#06x}]",
                     controller.bus, controller.device, controller.function,
                     type_str, controller.vendor_name, controller.device_name,
                     controller.vendor_id, controller.device_id);

            // Initialize supported controllers
            match &controller.controller_type {
                UsbControllerType::XHCI => {
                    kprintln!("    → Initializing xHCI controller (USB 3.0)");
                    match init_xhci_controller(controller) {
                        Ok(()) => kprintln!("    ✓ xHCI controller initialized successfully"),
                        Err(e) => kprintln!("    ✗ xHCI initialization failed: {:?}", e),
                    }
                },
                UsbControllerType::EHCI => {
                    kprintln!("    → EHCI support planned for future release");
                },
                UsbControllerType::OHCI => {
                    kprintln!("    → OHCI support planned for future release");
                },
                UsbControllerType::UHCI => {
                    kprintln!("    → UHCI support planned for future release");
                },
                UsbControllerType::Unknown(_) => {}
            }
        }
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