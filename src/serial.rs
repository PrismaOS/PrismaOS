use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

// Test utilities for serial port
#[cfg(test)]
pub fn test_print(s: &str) {
    serial_print!("[test] {}", s);
}

#[cfg(test)]
pub fn test_println(s: &str) {
    serial_println!("[test] {}", s);
}

#[test_case]
fn test_serial_output() {
    serial_println!("Testing serial output...");
    assert!(true); // If we get here, serial output worked
}

// Helper functions for serial port configuration
pub fn init() {
    unsafe {
        let mut port = SerialPort::new(0x3F8);
        port.init();
    }
}

pub fn set_baud_rate(divisor: u16) {
    unsafe {
        let port = &mut *SERIAL1.lock();
        port.set_baud_divisor(divisor);
    }
}

// Advanced serial port configuration options
pub mod config {
    use super::*;
    use uart_16550::LineStatus;

    pub fn set_word_length(length: u8) {
        if !(7..=8).contains(&length) {
            return;
        }
        unsafe {
            let port = &mut *SERIAL1.lock();
            let lcr = port.line_control_register();
            port.write_line_control(lcr | ((length - 5) as u8) & 0x03);
        }
    }

    pub fn set_parity(enable: bool, even: bool) {
        unsafe {
            let port = &mut *SERIAL1.lock();
            let mut lcr = port.line_control_register();
            if enable {
                lcr |= 0x08; // Enable parity
                if even {
                    lcr |= 0x10; // Even parity
                } else {
                    lcr &= !0x10; // Odd parity
                }
            } else {
                lcr &= !0x08; // Disable parity
            }
            port.write_line_control(lcr);
        }
    }

    pub fn set_stop_bits(two_bits: bool) {
        unsafe {
            let port = &mut *SERIAL1.lock();
            let mut lcr = port.line_control_register();
            if two_bits {
                lcr |= 0x04; // Two stop bits
            } else {
                lcr &= !0x04; // One stop bit
            }
            port.write_line_control(lcr);
        }
    }

    pub fn is_transmit_empty() -> bool {
        unsafe {
            let port = &mut *SERIAL1.lock();
            port.line_status().contains(LineStatus::OUTPUT_EMPTY)
        }
    }

    pub fn wait_for_transmit() {
        while !is_transmit_empty() {
            core::hint::spin_loop();
        }
    }

    pub fn send_break() {
        unsafe {
            let port = &mut *SERIAL1.lock();
            let lcr = port.line_control_register();
            port.write_line_control(lcr | 0x40);
            // Wait a bit
            for _ in 0..100000 {
                core::hint::spin_loop();
            }
            port.write_line_control(lcr);
        }
    }
}