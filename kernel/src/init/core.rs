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

/// Core initialization error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreInitError {
    CpuFeatureValidationFailed,
    BootstrapHeapInitFailed,
    BootstrapHeapTestFailed,
    GdtInitializationFailed,
    GdtValidationFailed,
    PicInitializationFailed,
}

impl ::core::fmt::Display for CoreInitError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            Self::CpuFeatureValidationFailed => write!(f, "CPU does not support required features"),
            Self::BootstrapHeapInitFailed => write!(f, "Bootstrap heap initialization failed"),
            Self::BootstrapHeapTestFailed => write!(f, "Bootstrap heap functionality test failed"),
            Self::GdtInitializationFailed => write!(f, "GDT initialization failed"),
            Self::GdtValidationFailed => write!(f, "GDT validation failed"),
            Self::PicInitializationFailed => write!(f, "PIC initialization failed"),
        }
    }
}

/// Safe core subsystem initialization with comprehensive error handling
pub fn init_core_subsystems() -> Result<(), CoreInitError> {
    // CRITICAL: Set up emergency fault handling FIRST
    // This catches faults that occur during early initialization
    interrupts::init_emergency_idt();
    
    // Add CPU feature validation
    if !validate_cpu_features() {
        return Err(CoreInitError::CpuFeatureValidationFailed);
    }
    
    // Bootstrap heap for early allocations (with safe wrapper)
    init_bootstrap_heap_safe()?;
    
    // Initialize unified GDT with safe error handling
    init_unified_gdt_safe()?;

    // Initialize full IDT (replaces emergency IDT)
    interrupts::init_idt();
    kprintln!("[OK] IDT initialized with comprehensive fault handling");

    // Test that fault handling is working
    test_fault_handling_active();

    // Initialize PICs safely
    init_pics_safe()?;
    
    kprintln!("[OK] All core subsystems initialized successfully");
    Ok(())
}

/// Safe bootstrap heap initialization
fn init_bootstrap_heap_safe() -> Result<(), CoreInitError> {
    // Safe wrapper around unsafe bootstrap heap initialization
    let result = unsafe { memory::init_bootstrap_heap() };
    
    match result {
        Ok(()) => {
            kprintln!("[OK] Bootstrap heap initialized (128KB)");
            
            // Test bootstrap heap immediately
            test_bootstrap_heap().map_err(|_| CoreInitError::BootstrapHeapTestFailed)?;
            kprintln!("[OK] Bootstrap heap functionality verified");
            
            Ok(())
        }
        Err(_) => Err(CoreInitError::BootstrapHeapInitFailed),
    }
}

/// Safe unified GDT initialization
fn init_unified_gdt_safe() -> Result<(), CoreInitError> {
    match unified_gdt::init() {
        Ok(()) => {
            kprintln!("[OK] Unified GDT initialized");
            
            // Validate GDT is actually working
            unified_gdt::validate_gdt().map_err(|_| CoreInitError::GdtValidationFailed)?;
            kprintln!("[OK] GDT validation passed");
            
            Ok(())
        }
        Err(_) => Err(CoreInitError::GdtInitializationFailed),
    }
}

/// Safe PIC initialization
fn init_pics_safe() -> Result<(), CoreInitError> {
    // PICs initialization is inherently unsafe but we can wrap it
    unsafe { 
        PICS.lock().initialize();
    }
    kprintln!("[OK] PIC initialized (IRQ 32-47)");
    Ok(())
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
fn test_fault_handling_active() -> Result<(), CoreInitError> {
    // We can't actually trigger a fault here, but we can verify IDT is loaded
    use x86_64::instructions::tables;
    let idt = tables::sidt();
    if idt.limit == 0 {
        return Err(CoreInitError::PicInitializationFailed); // Reuse for simplicity
    }
    kprintln!("[OK] Fault handling system active (IDT limit: {})", idt.limit);
    Ok(())
}
