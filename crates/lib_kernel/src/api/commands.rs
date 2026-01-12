//! # x86_commands: Architecture-Specific Low-Level x86_64 Operations
//!
//! This crate provides direct access to low-level x86/x86_64 hardware commands, intended for use in OS kernels, bootloaders, or other bare-metal environments. All functions are highly architecture-specific and use inline assembly to interact with hardware directly. These commands are not portable and will only work on x86-family CPUs.
//!
//! ## Architecture-Specific Notes
//!
//! - All functions in this crate assume an x86 or x86_64 environment. They use I/O port instructions (`in`, `out`) and other CPU-specific features that are not available on other architectures (such as ARM, RISC-V, etc).
//! - Attempting to use these functions on non-x86 hardware will result in a compile-time or runtime error.
//! - These routines are typically only safe to use in kernel or bootloader code, not in userspace or general application code.
//!
//! ## Example Usage
//!
//! ```no_run
//! use polished_x86_commands::disable_pic;
//! disable_pic();
//! ```
//!
//! This will mask all interrupts from the legacy Programmable Interrupt Controller (PIC), which is a common step in modern x86_64 kernels that use the APIC instead.

use core::arch::asm;

/// Disables the legacy Programmable Interrupt Controller (PIC) on x86/x86_64 systems.
///
/// # Architecture
/// This function is specific to x86-family CPUs. It uses the `out` instruction to write to the PIC's I/O ports (0x21 for the master PIC, 0xA1 for the slave PIC).
///
/// - On modern x86_64 systems, the legacy PIC is often replaced by the APIC, but the PIC must still be masked to prevent spurious interrupts.
/// - This function is a no-op on non-x86 architectures and will not compile there.
///
/// # Safety
/// This function uses inline assembly to directly access hardware ports. It should only be called in a privileged (kernel or bootloader) context.
///
/// # Example
/// ```no_run
/// polished_x86_commands::disable_pic();
/// ```
pub fn disable_pic() {
    unsafe {
        // Mask all interrupts on both PICs by writing 0xFF to their data ports.
        // 0x21: Master PIC data port
        // 0xA1: Slave PIC data port
        // This disables all IRQs from the legacy PIC, which is required before enabling the APIC.
        asm!(
            "mov al, 0xFF", // Set AL register to 0xFF (all bits set)
            "out 0xA1, al", // Write AL to slave PIC data port (0xA1)
            "out 0x21, al", // Write AL to master PIC data port (0x21)
            options(nostack, nomem, preserves_flags)
        );
    }
}

/// Reads an 8-bit value from the specified I/O port.
///
/// # Safety
/// This function is unsafe because it performs a raw hardware port read.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Writes an 8-bit value to the specified I/O port.
///
/// # Safety
/// This function is unsafe because it performs a raw hardware port write.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Reads a 32-bit value from the specified I/O port.
///
/// # Safety
/// This function is unsafe because it performs a raw hardware port read.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    unsafe {
        asm!(
            "in eax, dx",
            out("eax") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Writes a 32-bit value to the specified I/O port.
///
/// # Safety
/// This function is unsafe because it performs a raw hardware port write.
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Reads `count` 32-bit values from the specified I/O port into the buffer.
///
/// # Safety
/// - The caller must ensure that `buffer` points to valid writable memory for at least `count` `u32` values.
/// - This function performs raw hardware port reads and is only safe in privileged (kernel or bootloader) contexts.
#[inline]
pub unsafe fn insl(port: u16, buffer: *mut u32, count: u32) {
    for i in 0..count {
        unsafe { *buffer.offset(i as isize) = inl(port) };
    }
}

/// Reads a 16-bit value from the specified I/O port.
///
/// # Safety
/// This function is unsafe because it performs a raw hardware port read.
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    unsafe {
        asm!(
            "in ax, dx",
            out("ax") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Reads `count` 16-bit values from the specified I/O port into the buffer.
///
/// # Safety
/// - The caller must ensure that `buffer` points to valid writable memory for at least `count` `u16` values.
/// - This function performs raw hardware port reads and is only safe in privileged (kernel or bootloader) contexts.
#[inline]
pub unsafe fn insw(port: u16, buffer: *mut u16, count: u32) {
    for i in 0..count {
        unsafe { *buffer.offset(i as isize) = inw(port) };
    }
}

/// Safe wrapper for reading words into a byte buffer with bounds checking
pub fn read_port_words_safe(port: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
    if buffer.len() % 2 != 0 {
        return Err("Buffer length must be even for 16-bit reads");
    }

    if buffer.len() > u32::MAX as usize * 2 {
        return Err("Buffer too large");
    }

    for chunk in buffer.chunks_exact_mut(2) {
        let word = unsafe { inw(port) };
        chunk[0] = (word & 0xFF) as u8;
        chunk[1] = (word >> 8) as u8;
    }

    Ok(())
}

/// Writes a 16-bit value to the specified I/O port.
///
/// # Safety
/// This function is unsafe because it performs a raw hardware port write.
#[inline]
pub unsafe fn outw(port: u16, value: u16) {
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Writes `count` 16-bit values from the buffer to the specified I/O port.
///
/// # Safety
/// - The caller must ensure that `buffer` points to valid readable memory for at least `count` `u16` values.
/// - This function performs raw hardware port writes and is only safe in privileged (kernel or bootloader) contexts.
#[inline]
pub unsafe fn outsw(port: u16, buffer: *const u16, count: u32) {
    unsafe {
        asm!(
            "rep outsw",
            in("dx") port,
            in("rsi") buffer,
            in("rcx") count as u64,
            options(nostack, preserves_flags)
        );
    }
}