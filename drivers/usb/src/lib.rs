#![no_std]

mod consts;
use consts::*;
use lib_kernel::api::commands::{inb, outb};

pub fn init_xhci() {
    // reset XHCI controller
    unsafe { 
        outb(USB_CMD as u16, 0x02);
        while (inb(USB_STS as u16) & 0x01) == 0 {}
        outb(USB_CMD as u16, 0x00);
    };
}