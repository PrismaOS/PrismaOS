#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![no_std]

extern crate alloc;

// Memory utility functions needed for compilation
#[no_mangle]
pub extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        for i in 0..n {
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

#[no_mangle]
pub extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        memcpy(dest, src, n)
    } else {
        unsafe {
            for i in (0..n).rev() {
                *dest.add(i) = *src.add(i);
            }
        }
        dest
    }
}

#[no_mangle]
pub extern "C" fn memset(dest: *mut u8, val: i32, n: usize) -> *mut u8 {
    let val = val as u8;
    unsafe {
        for i in 0..n {
            *dest.add(i) = val;
        }
    }
    dest
}

#[no_mangle]
pub extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a != b {
                return a as i32 - b as i32;
            }
        }
    }
    0
}

// Panic and unwinding symbols
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[no_mangle]
pub extern "C" fn _Unwind_Resume() {}

pub mod scrolling_text;
pub mod fast_console;
pub mod interrupts;
pub mod scheduler;
pub mod executor;
pub mod process;
pub mod syscall;
pub mod drivers;
pub mod events;
pub mod consts;
pub mod memory;
pub mod utils;
pub mod font;
pub mod time;
pub mod elf;
pub mod gdt;
pub mod api;
