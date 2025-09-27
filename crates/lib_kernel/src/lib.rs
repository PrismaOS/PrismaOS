#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![no_std]

extern crate alloc;

pub mod scrolling_text;
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
pub mod gdt_correct;
pub mod api;
