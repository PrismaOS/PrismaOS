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

extern crate alloc;

pub fn init_core_subsystems() {
    // CRITICAL: Set up emergency fault handling FIRST
    // This catches faults that occur during early initialization
    interrupts::init_emergency_idt();
    
    // Add CPU feature validation
    if !validate_cpu_features() {
        panic!("CPU does not support required features");
    }
    
    // Bootstrap heap for early allocations
    unsafe { 
        match memory::init_bootstrap_heap() {
            Ok(()) => {
                kprintln!("[OK] Bootstrap heap initialized (128KB)");
                
                // Test bootstrap heap immediately
                if let Err(e) = test_bootstrap_heap() {
                    panic!("Bootstrap heap test failed: {:?}", e);
                }
            }
            Err(e) => panic!("Failed to initialize bootstrap heap: {:?}", e),
        }
    }

    // Initialize unified GDT
    match unified_gdt::init() {
        Ok(()) => {
            kprintln!("[OK] Unified GDT initialized");
            
            // Validate GDT is actually working
            if let Err(e) = unified_gdt::validate_gdt() {
                panic!("GDT validation failed: {}", e);
            }
        }
        Err(e) => panic!("Failed to initialize GDT: {}", e),
    }

    // Initialize full IDT (replaces emergency IDT)
    interrupts::init_idt();
    kprintln!("[OK] IDT initialized with comprehensive fault handling");

    // Test that fault handling is working
    test_fault_handling_active();

    // Initialize PICs
    unsafe { PICS.lock().initialize() };
    kprintln!("[OK] PIC initialized (IRQ 32-47)");
}

/// Validate that the CPU supports required features
fn validate_cpu_features() -> bool {
    use x86_64::registers::control::Cr0Flags;
    use x86_64::registers::control::{Cr0, Cr4, Cr4Flags};
    
    // Check that paging is enabled
    let cr0 = Cr0::read();
    if !cr0.contains(Cr0Flags::PAGING) {
        kprintln!("[ERROR] Paging not enabled");
        return false;
    }
    
    // Check that necessary CR4 features are available
    let cr4 = Cr4::read();
    if !cr4.contains(Cr4Flags::PAGE_SIZE_EXTENSION) {
        kprintln!("[WARN] PSE not available, may affect performance");
    }
    
    kprintln!("[OK] CPU feature validation passed");
    true
}

/// Test that bootstrap heap is working
fn test_bootstrap_heap() -> Result<(), &'static str> {
    use alloc::{vec::Vec, boxed::Box};
    
    // Test basic allocation
    let test_box = Box::new(0xDEADBEEFu32);
    if *test_box != 0xDEADBEEF {
        return Err("Box allocation test failed");
    }
    
    // Test Vec allocation
    let mut test_vec = Vec::new();
    for i in 0..10 {
        test_vec.push(i);
    }
    if test_vec.len() != 10 {
        return Err("Vec allocation test failed");
    }
    
    Ok(())
}

/// Test that fault handling is properly active
fn test_fault_handling_active() {
    // We can't actually trigger a fault here, but we can verify IDT is loaded
    use x86_64::instructions::tables;
    let idt = tables::sidt();
    if idt.limit == 0 {
        panic!("IDT not properly loaded");
    }
    kprintln!("[OK] Fault handling system active (IDT limit: {})", idt.limit);
}
