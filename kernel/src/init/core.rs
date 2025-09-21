//! Core subsystem initialization
//!
//! Initializes bootstrap heap, GDT, IDT and PICs. These are required early
//! in the boot flow and don't require the full virtual memory setup.

/// Initialize core kernel subsystems that do not depend on complex memory
/// layout. This includes bootstrap heap, GDT, IDT, PICs, and a very small
/// scheduler initialization.
pub fn init_core_subsystems() {
    // Bootstrap heap for early allocations
    unsafe { crate::memory::init_bootstrap_heap(); }
    crate::kprintln!("[OK] Bootstrap heap initialized (64KB)");

    // Initialize GDT and IDT
    crate::gdt::init();
    crate::kprintln!("[OK] GDT initialized");

    crate::interrupts::init_idt();
    crate::kprintln!("[OK] IDT initialized");

    // Initialize PICs
    unsafe { crate::PICS.lock().initialize() };
    crate::kprintln!("[OK] PIC initialized (IRQ 32-47)");
}
