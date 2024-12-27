#![no_std]
#![cfg_attr(test, no_main)]
#![feature(derive_default_enum)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]

pub mod vga_buffer;
pub mod interrupts;
pub mod gdt;
pub mod memory;

pub fn init() {
    gdt::init();  // Initialize GDT and TSS first
    interrupts::init();  // Then initialize the IDT which depends on GDT/TSS
//    unsafe { interrupts::PICS.lock().initialize() };  // Initialize the interrupt controller
    x86_64::instructions::interrupts::enable();  // Finally enable interrupts
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}