//! Core subsystem initialization
//!
//! Initializes bootstrap heap, unified GDT, IDT and PICs. These are required early
//! in the boot flow and don't require the full virtual memory setup.

use lib_kernel::{
    memory::{self, unified_gdt},
    interrupts::{self},
    consts::PICS,
    kprintln,
};

pub fn init_core_subsystems() {
    // Bootstrap heap for early allocations
    unsafe { 
        if let Err(e) = memory::init_bootstrap_heap() {
            panic!("Failed to initialize bootstrap heap: {:?}", e);
        }
    }
    kprintln!("[OK] Bootstrap heap initialized (128KB)");

    // Initialize unified GDT
    match unified_gdt::init() {
        Ok(()) => kprintln!("[OK] Unified GDT initialized"),
        Err(e) => panic!("Failed to initialize GDT: {}", e),
    }

    // Initialize IDT
    interrupts::init_idt();
    kprintln!("[OK] IDT initialized");

    // Initialize PICs
    unsafe { PICS.lock().initialize() };
    kprintln!("[OK] PIC initialized (IRQ 32-47)");
}
