#![no_std]

#[allow(dead_code)]

use core::fmt::{self, Write};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

const PORT: u16 = 0x3F8; // COM1

// Low-level port I/O functions
#[inline]
unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}

pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn init(&self) -> Result<(), ()> {
        unsafe {
            outb(self.port + 1, 0x00); // Disable all interrupts
            outb(self.port + 3, 0x80); // Enable DLAB (set baud rate divisor)
            outb(self.port + 0, 0x03); // Set divisor to 3 (lo byte) 38400 baud
            outb(self.port + 1, 0x00); // (hi byte)
            outb(self.port + 3, 0x03); // 8 bits, no parity, one stop bit
            outb(self.port + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
            outb(self.port + 4, 0x0B); // IRQs enabled, RTS/DSR set
            outb(self.port + 4, 0x1E); // Set in loopback mode, test the serial chip
            outb(self.port + 0, 0xAE); // Test serial chip (send byte 0xAE and check if serial returns same byte)

            // Check if serial is faulty (i.e: not same byte as sent)
            if inb(self.port + 0) != 0xAE {
                return Err(());
            }

            // If serial is not faulty set it in normal operation mode
            // (not-loopback with IRQs enabled and OUT#1 and OUT#2 bits enabled)
            outb(self.port + 4, 0x0F);
            Ok(())
        }
    }

    fn is_transmit_empty(&self) -> bool {
        unsafe { inb(self.port + 5) & 0x20 != 0 }
    }

    pub fn write_char(&self, ch: u8) {
        while !self.is_transmit_empty() {
            // Busy wait
        }
        unsafe {
            outb(self.port, ch);
        }
    }

    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            self.write_char(byte);
        }
    }
}

impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        SerialPort::write_str(self, s);
        Ok(())
    }
}

// Option 1: Using static with atomic initialization flag (Recommended)
static SERIAL: SerialPort = SerialPort::new(PORT);
static SERIAL_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init_serial() -> Result<(), ()> {
    let result = SERIAL.init();
    if result.is_ok() {
        SERIAL_INITIALIZED.store(true, Ordering::Release);
    }
    result
}

fn ensure_serial_init() {
    if !SERIAL_INITIALIZED.load(Ordering::Acquire) {
        // In kernel context, you might want to panic or handle this differently
        init_serial().expect("Failed to initialize serial port");
    }
}

// Helper function to safely access serial port
fn with_serial<F, R>(f: F) -> R
where
    F: FnOnce(&SerialPort) -> R,
{
    ensure_serial_init();
    f(&SERIAL)
}

// Custom writer that implements formatting
struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        with_serial(|serial| serial.write_str(s));
        Ok(())
    }
}

// Main printf macro - now safer
#[macro_export]
macro_rules! serial_printf {
    ($fmt:expr) => {
        $crate::with_serial(|serial| serial.write_str($fmt))
    };
    ($fmt:expr, $($arg:expr),*) => {{
        use core::fmt::Write;
        let mut writer = $crate::SerialWriter;
        write!(writer, $fmt, $($arg),*).ok();
    }};
}

// Convenience macros
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial_printf!($($arg)*);
    };
}

#[macro_export]
macro_rules! serial_println {
    () => {
        $crate::with_serial(|serial| serial.write_str("\n"));
    };
    ($($arg:tt)*) => {{
        $crate::serial_printf!($($arg)*);
        $crate::with_serial(|serial| serial.write_str("\n"));
    }};
}

// Buffer-based snprintf equivalent using a simple buffer
pub struct FixedBuffer<const N: usize> {
    buf: [u8; N],
    pos: usize,
}

impl<const N: usize> FixedBuffer<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            pos: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.pos
    }

    pub fn clear(&mut self) {
        self.pos = 0;
        if self.buf.len() > 0 {
            self.buf[0] = 0;
        }
    }
}

impl<const N: usize> Write for FixedBuffer<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let available = N.saturating_sub(self.pos).saturating_sub(1); // Reserve space for null terminator
        let to_copy = bytes.len().min(available);
        
        if to_copy > 0 {
            self.buf[self.pos..self.pos + to_copy].copy_from_slice(&bytes[..to_copy]);
            self.pos += to_copy;
        }
        
        // Ensure null termination
        if self.pos < N {
            self.buf[self.pos] = 0;
        }
        
        Ok(())
    }
}

// kvsnprintf equivalent macro
#[macro_export]
macro_rules! ksnprintf {
    ($buf:expr, $fmt:expr, $($arg:expr),*) => {{
        use core::fmt::Write;
        $buf.clear();
        write!($buf, $fmt, $($arg),*).ok();
        $buf.len()
    }};
    ($buf:expr, $fmt:expr) => {{
        use core::fmt::Write;
        $buf.clear();
        $buf.write_str($fmt).ok();
        $buf.len()
    }};
}

// Public helper functions for backwards compatibility
pub fn write_serial_char(ch: char) {
    with_serial(|serial| serial.write_char(ch as u8));
}

pub fn write_serial(s: &str) {
    with_serial(|serial| serial.write_str(s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_formatting() {
        let mut buf = FixedBuffer::<64>::new();
        
        // Test basic formatting
        let len = ksnprintf!(buf, "Hello, {}!", "world");
        assert_eq!(buf.as_str(), "Hello, world!");
        assert_eq!(len, 13);
        
        // Test number formatting
        let _len = ksnprintf!(buf, "Number: {}", 42);
        assert_eq!(buf.as_str(), "Number: 42");
        
        // Test hex formatting
        let _len = ksnprintf!(buf, "Hex: 0x{:x}", 255);
        assert_eq!(buf.as_str(), "Hex: 0xff");
    }

    #[test]
    fn test_serial_operations() {
        // These tests would need to be run in an environment with actual serial hardware
        // or mocked I/O operations
        write_serial("Test message\n");
        write_serial_char('X');
    }
}