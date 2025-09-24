use lib_kernel::kprintln;

/// Safe system halt
pub fn halt_system() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Show system information
fn show_system_info() {
    kprintln!();
    kprintln!("💻 PrismaOS System Information");
    kprintln!("==============================");
    kprintln!("  OS: PrismaOS");
    kprintln!("  Architecture: x86_64");
    kprintln!("  Kernel: Rust-based microkernel");
    kprintln!("  Uptime: {} ticks", lib_kernel::time::current_tick());
    kprintln!("  Timestamp: {} ms", lib_kernel::time::get_timestamp());
    
    // Show driver status
    let dm = lib_kernel::drivers::device_manager();
    let driver_count = dm.driver_count();
    kprintln!("  Drivers loaded: {}", driver_count);
    
    let driver_names = dm.list_drivers();
    for name in driver_names {
        kprintln!("    • {}", name);
    }
    kprintln!();
}