#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker, HhdmRequest, MemoryMapRequest};

// Simplified modules for initial working build
// mod gdt;
// mod interrupts; 
// mod memory;
// mod executor;
// mod api;
// mod scheduler;
// mod drivers;
// mod time;

/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
/// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

/// Define the stand and end markers for Limine requests.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    assert!(BASE_REVISION.is_supported());

    // Basic VGA text mode output for initial demo
    let vga_buffer = 0xb8000 as *mut u8;
    let message = b"PrismaOS - Object-Based Operating System Booting...";
    
    unsafe {
        for (i, &byte) in message.iter().enumerate() {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0x0F; // White on black
        }
    }

    #[cfg(test)]
    test_main();
    
    // Hardware-adaptive framebuffer initialization
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            // Get hardware-provided parameters
            let width = framebuffer.width() as u64;
            let height = framebuffer.height() as u64; 
            let pitch = framebuffer.pitch() as u64;
            let bpp = framebuffer.bpp() as u64;
            let addr = framebuffer.addr() as *mut u8;
            let red_mask_size = framebuffer.red_mask_size();
            let red_mask_shift = framebuffer.red_mask_shift();
            let green_mask_size = framebuffer.green_mask_size();
            let green_mask_shift = framebuffer.green_mask_shift(); 
            let blue_mask_size = framebuffer.blue_mask_size();
            let blue_mask_shift = framebuffer.blue_mask_shift();
            
            // Display detected hardware parameters
            let info_msg = b"FB: ";
            unsafe {
                for (i, &byte) in info_msg.iter().enumerate() {
                    *vga_buffer.offset((160 + i) as isize * 2) = byte;
                    *vga_buffer.offset((160 + i) as isize * 2 + 1) = 0x0E; // Yellow
                }
            }
            
            // Validate all parameters are sane
            if width == 0 || height == 0 || pitch == 0 || bpp < 8 || bpp > 32 || addr.is_null() {
                let error_msg = b"Invalid framebuffer - VGA fallback";
                unsafe {
                    for (i, &byte) in error_msg.iter().enumerate() {
                        *vga_buffer.offset((170 + i) as isize * 2) = byte;
                        *vga_buffer.offset((170 + i) as isize * 2 + 1) = 0x0C; // Red
                    }
                }
            } else {
                let bytes_per_pixel = (bpp + 7) / 8;
                let max_line_size = width.saturating_mul(bytes_per_pixel);
                let max_framebuffer_size = height.saturating_mul(pitch);
                
                // Additional hardware-specific validation
                let pitch_valid = pitch >= max_line_size; // Pitch must be at least line width
                let size_reasonable = max_framebuffer_size > 0 && max_framebuffer_size < (256u64 << 20); // < 256MB
                let masks_valid = u64::from(red_mask_size + green_mask_size + blue_mask_size) <= bpp;
                
                if !pitch_valid || !size_reasonable || !masks_valid {
                    let warn_msg = b"FB params suspicious - proceeding cautiously";
                    unsafe {
                        for (i, &byte) in warn_msg.iter().enumerate() {
                            *vga_buffer.offset((170 + i) as isize * 2) = byte;
                            *vga_buffer.offset((170 + i) as isize * 2 + 1) = 0x0E; // Yellow warning
                        }
                    }
                }
                
                // Only draw if we have at least RGB components
                if bytes_per_pixel >= 2 && red_mask_size > 0 && green_mask_size > 0 && blue_mask_size > 0 {
                    // Adapt drawing to hardware capabilities
                    let safe_width = width.min(1024); // Conservative limit
                    let safe_height = height.min(768);
                    
                    // Create color values using detected color masks
                    let dark_blue = (0x33u64 << blue_mask_shift) | (0x00u64 << green_mask_shift) | (0x00u64 << red_mask_shift);
                    let light_green = (0x00u64 << blue_mask_shift) | (0xCCu64 << green_mask_shift) | (0x66u64 << red_mask_shift);
                    
                    // Clear screen with hardware-appropriate method
                    for y in 0..safe_height {
                        let line_offset = y * pitch;
                        if line_offset < max_framebuffer_size {
                            for x in 0..safe_width {
                                let pixel_offset = line_offset + x * bytes_per_pixel;
                                if pixel_offset + bytes_per_pixel <= max_framebuffer_size {
                                    // Write pixel data based on detected format
                                    match bytes_per_pixel {
                                        2 => unsafe { // 16-bit RGB/BGR
                                            *(addr.add(pixel_offset as usize) as *mut u16) = dark_blue as u16;
                                        },
                                        3 => unsafe { // 24-bit RGB/BGR
                                            let color_bytes = dark_blue.to_le_bytes();
                                            *addr.add(pixel_offset as usize) = color_bytes[0];
                                            *addr.add(pixel_offset as usize + 1) = color_bytes[1]; 
                                            *addr.add(pixel_offset as usize + 2) = color_bytes[2];
                                        },
                                        4 => unsafe { // 32-bit RGBA/BGRA
                                            *(addr.add(pixel_offset as usize) as *mut u32) = dark_blue as u32;
                                        },
                                        _ => {} // Skip unsupported formats
                                    }
                                }
                            }
                        }
                    }
                    
                    // Draw test pattern adapted to hardware
                    let pattern_w = (safe_width / 4).min(100);
                    let pattern_h = (safe_height / 4).min(75);
                    let pattern_x = safe_width / 3;
                    let pattern_y = safe_height / 3;
                    
                    for y in pattern_y..pattern_y + pattern_h {
                        let line_offset = y * pitch;
                        if line_offset < max_framebuffer_size {
                            for x in pattern_x..pattern_x + pattern_w {
                                let pixel_offset = line_offset + x * bytes_per_pixel;
                                if pixel_offset + bytes_per_pixel <= max_framebuffer_size {
                                    match bytes_per_pixel {
                                        2 => unsafe {
                                            *(addr.add(pixel_offset as usize) as *mut u16) = light_green as u16;
                                        },
                                        3 => unsafe {
                                            let color_bytes = light_green.to_le_bytes();
                                            *addr.add(pixel_offset as usize) = color_bytes[0];
                                            *addr.add(pixel_offset as usize + 1) = color_bytes[1];
                                            *addr.add(pixel_offset as usize + 2) = color_bytes[2];
                                        },
                                        4 => unsafe {
                                            *(addr.add(pixel_offset as usize) as *mut u32) = light_green as u32;
                                        },
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Write success message to VGA text mode
            let success_msg = b"PrismaOS Framebuffer Active - System Ready!";
            unsafe {
                for (i, &byte) in success_msg.iter().enumerate() {
                    *vga_buffer.offset((80 + i) as isize * 2) = byte;
                    *vga_buffer.offset((80 + i) as isize * 2 + 1) = 0x0A; // Green on black
                }
            }
        }
    }

    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(not(test))]
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    // Display BSOD using VGA text mode for maximum compatibility
    let vga_buffer = 0xb8000 as *mut u16;
    let width = 80;
    let height = 25;
    
    unsafe {
        // Clear screen with blue background
        for i in 0..(width * height) {
            *vga_buffer.add(i) = 0x1F00 | b' ' as u16; // White on blue
        }
        
        // Draw BSOD header
        let header = b"*** PRISMA OS KERNEL PANIC ***";
        let start_pos = (width - header.len()) / 2;
        for (i, &byte) in header.iter().enumerate() {
            *vga_buffer.add(start_pos + i) = 0x1F00 | byte as u16;
        }
        
        // Error message
        let error_msg = match info.message() {
            Some(msg) => {
                // Try to format the message (limited without std)
                b"A critical error occurred in the kernel"
            },
            None => b"Unknown kernel panic occurred"
        };
        
        let error_start = width * 3;
        for (i, &byte) in error_msg.iter().enumerate() {
            if i < width - 1 {
                *vga_buffer.add(error_start + i) = 0x1F00 | byte as u16;
            }
        }
        
        // Show location if available
        if let Some(location) = info.location() {
            let file_line = b"File: ";
            let mut pos = width * 5;
            
            // Write "File: " prefix
            for (i, &byte) in file_line.iter().enumerate() {
                *vga_buffer.add(pos + i) = 0x1F00 | byte as u16;
            }
            pos += file_line.len();
            
            // Write filename (truncated)
            let filename = location.file().as_bytes();
            let max_filename_len = 40;
            for (i, &byte) in filename.iter().take(max_filename_len).enumerate() {
                *vga_buffer.add(pos + i) = 0x1F00 | byte as u16;
            }
            
            // Write line number info on next line
            let line_info = b"Line: ";
            pos = width * 6;
            for (i, &byte) in line_info.iter().enumerate() {
                *vga_buffer.add(pos + i) = 0x1F00 | byte as u16;
            }
            
            // Simple number display for line (limited without std)
            let line_num = location.line();
            let line_str = format_number(line_num);
            pos += line_info.len();
            for (i, byte) in line_str.iter().enumerate() {
                if pos + i < width * height {
                    *vga_buffer.add(pos + i) = 0x1F00 | *byte as u16;
                }
            }
        }
        
        // Instructions
        let instructions = [
            b"",
            b"The system has been halted to prevent damage.",
            b"",
            b"* Check your kernel code for memory safety issues",
            b"* Verify framebuffer access is within bounds", 
            b"* Ensure all hardware initialization is valid",
            b"",
            b"System will remain halted. Restart to continue.",
        ];
        
        let mut line = 9;
        for instruction in instructions.iter() {
            let start_pos = width * line + 2;
            for (i, &byte) in instruction.iter().enumerate() {
                if start_pos + i < width * height {
                    *vga_buffer.add(start_pos + i) = 0x1F00 | byte as u16;
                }
            }
            line += 1;
        }
        
        // Draw a border for visual effect
        for x in 0..width {
            *vga_buffer.add(x) = 0x1F00 | b'=' as u16; // Top border
            *vga_buffer.add(width * (height - 1) + x) = 0x1F00 | b'=' as u16; // Bottom border
        }
    }
    
    // Halt the CPU safely
    loop {
        x86_64::instructions::hlt();
    }
}

// Helper function to format numbers without std
fn format_number(mut num: u32) -> [u8; 10] {
    let mut result = [b' '; 10];
    let mut pos = 9;
    
    if num == 0 {
        result[pos] = b'0';
        return result;
    }
    
    while num > 0 && pos > 0 {
        result[pos] = (num % 10) as u8 + b'0';
        num /= 10;
        pos -= 1;
    }
    
    result
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    // Simple test runner without println
    for test in tests {
        test();
    }
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
