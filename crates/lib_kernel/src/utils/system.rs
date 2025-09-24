
/// Safe system halt
pub fn halt_system() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
