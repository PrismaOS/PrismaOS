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
    // Note: No heap needed yet - GDT/IDT/PICs use static structures
    // Heap will be initialized after memory mapping is set up
    
    // Initialize GDT and IDT
    gdt::init();

    interrupts::init_idt();

    // Initialize PICs
    unsafe { PICS.lock().initialize() };
}
