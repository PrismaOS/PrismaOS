//! Core subsystem initialization
//!
//! Initializes bootstrap heap, GDT, IDT and PICs. These are required early
//! in the boot flow and don't require the full virtual memory setup.

/// Initialize core kernel subsystems that do not depend on complex memory
/// layout. This includes bootstrap heap, GDT, IDT, PICs, and a very small
/// scheduler initialization.

use lib_kernel::{
    memory,
    gdt,
    interrupts::{self},
    consts::PICS,
    kprintln,
};

pub fn init_core_subsystems() {
    // Bootstrap heap for early allocations
    unsafe { memory::init_bootstrap_heap(); }
    kprintln!("[OK] Bootstrap heap initialized (64KB)");

    // Initialize GDT and IDT
    gdt::init();
    kprintln!("[OK] GDT initialized");

    interrupts::init_idt();
    kprintln!("[OK] IDT initialized");

    // Initialize PICs
    unsafe { PICS.lock().initialize() };
    kprintln!("[OK] PIC initialized (IRQ 32-47)");
}
