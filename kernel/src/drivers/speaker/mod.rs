use crate::api::commands::{inb, outb};

pub fn play_sound(freq: u8) {
    let div: u32 = 1193180 / (freq as u32);

    unsafe { 
        outb(0x43, 0xb8);
        outb(0x42, div as u8);
        outb(0x42, (div >> 8) as u8);
        let tmp: u8 = inb(0x61);
        if tmp != (tmp | 3) {
            outb(0x61, tmp | 3);
        }
    };
}

pub fn stop_sound() {
    unsafe {
        let tmp: u8 = inb(0x61) & 0xFC;
        outb(0x61, tmp);
    }
}

fn delay_ms(ms: u64) {
    let cycles_per_ms = 1000000;
    
    for _ in 0..(ms * cycles_per_ms) {
        unsafe {
            core::arch::asm!("nop");
        }
    }
}

pub fn beep(freq: u8, duration_ms: u64) {
    play_sound(freq);
    delay_ms(duration_ms);
    stop_sound();
}