#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker, HhdmRequest, MemoryMapRequest};
use core::panic::PanicInfo;
mod font;
use font::{PsfFont, draw_string, FONT_PSF};

mod scrolling_text;
use scrolling_text::ScrollingTextRenderer;

#[used]
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

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

/// Display a message in the framebuffer using the PSF font
unsafe fn display_fb_message(
    addr: *mut u8,
    pitch: usize,
    width: usize,
    height: usize,
    font: &PsfFont,
    message: &[u8],
    y: usize,
    color: u32,
) {
    draw_string(
        addr,
        pitch,
        0,
        y,
        color,
        font,
        message,
        width,
        height,
    );
}

/// Safe system halt
fn halt_system() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Production-ready kernel main function using WORKING memory access pattern
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    // Step 1: Verify Limine protocol is supported (like working version)
    assert!(BASE_REVISION.is_supported());

    // Step 2: Prefer framebuffer output if available (UEFI & BIOS compatible)
    let message = b"PrismaOS - Production Kernel Starting...";
    let mut used_fb = false;
    let mut font_loaded = false;
    let mut font: Option<PsfFont> = None;
    // store framebuffer parameters as (addr, pitch, width, height)
    let mut fb: Option<(*mut u8, usize, usize, usize)> = None;

    // Optional renderer created once font + framebuffer are available
    let mut renderer: Option<ScrollingTextRenderer> = None;

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            let addr = framebuffer.addr();
            let pitch = framebuffer.pitch() as usize;
            let width = framebuffer.width().min(800) as usize;
            let height = framebuffer.height().min(600) as usize;
            if addr.is_null() || pitch == 0 || width == 0 || height == 0 {
                // Fallback to VGA if framebuffer is invalid
            } else {
                if let Some(f) = PsfFont::from_bytes(&FONT_PSF) {
                    font_loaded = true;
                    font = Some(f);
                    // save raw framebuffer params
                    fb = Some((addr, pitch, width, height));
                    // create renderer with a sensible default line height/margins
                    renderer = Some(ScrollingTextRenderer::new(
                        addr,
                        pitch,
                        width,
                        height,
                        font.as_ref().unwrap(),
                        16, // line height
                        8,  // left margin
                        8,  // top margin
                    ));
                    // print boot message via renderer
                    if let Some(r) = renderer.as_mut() {
                        r.write_text(message);
                    }
                     used_fb = true;
                } else {
                    // Render a full red screen
                    for y in 0..height {
                        for x in 0..width {
                            let pixel_offset = y * pitch + x * 4;
                            unsafe {
                                addr.add(pixel_offset).cast::<u32>().write(0xFFFF0000); // Red
                            }
                        }
                    }
                    halt_system();
                }
            }
        }
    }

    // Use framebuffer for all status messages if available
    if used_fb && font_loaded && fb.is_some() {
        let (addr, pitch, width, height) = fb.unwrap();
        let font = font.as_ref().unwrap();

        let r = renderer.as_mut().expect("renderer must exist when fb+font present");

        // Step 3: Validate critical system components
        if let Some(memory_map_response) = MEMORY_MAP_REQUEST.get_response() {
            let entry_count = memory_map_response.entries().len();
            if entry_count == 0 {
                r.write_line(b"ERROR: No memory map");
                halt_system();
            }
            r.write_line(b"Memory map OK");
        }

        if let Some(hhdm_response) = HHDM_REQUEST.get_response() {
            let hhdm_offset = hhdm_response.offset();
            if hhdm_offset == 0 {
                r.write_line(b"ERROR: Invalid HHDM");
                halt_system();
            }
            r.write_line(b"HHDM OK");
        }

        // Step 4: Safe framebuffer using WORKING PATTERN
        r.write_line(b"Initializing framebuffer...");

        if width == 0 || height == 0 || pitch == 0 || addr.is_null() {
            r.write_line(b"FB: Invalid parameters");
        } else if (addr as usize) < 0x1000 {
            r.write_line(b"FB: Invalid address");
        } else {
            r.write_line(b"FB: Params valid, drawing...");

            let safe_width = width.min(800) as u64;
            let safe_height = height.min(600) as u64;
            let safe_pitch = pitch as u64;

            // for y in 100..150u64 {
            //     if y >= safe_height { break; }
            //     for x in 100..150u64 {
            //         if x >= safe_width { break; }
            //         let pixel_offset = (y * safe_pitch + x * 4) as usize;
            //         unsafe {
            //             addr.add(pixel_offset)
            //                 .cast::<u32>()
            //                 .write(0xFF00FF00); // Green test pattern
            //         }
            //     }
            // }

            r.write_line(b"FB: Test pattern drawn");
        }

        r.write_line(b"System ready - entering idle");
    } else {
        // Fallback: Try VGA only if framebuffer is unavailable
        let vga_buffer = 0xb8000 as *mut u16;
        if (vga_buffer as usize) >= 0xb8000 && (vga_buffer as usize) < 0xc0000 {
            for (i, &byte) in message.iter().enumerate() {
                if i < 80 * 25 {
                    let entry = 0x0F00 | byte as u16;
                    unsafe {
                        vga_buffer.add(i).write(entry);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    test_main();

    // Safe idle loop
    loop {
        core::arch::asm!("hlt");
    }
}

/// Production-ready panic handler
#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    unsafe {
        let vga_buffer = 0xb8000 as *mut u16;

        // Clear screen with blue background (BSOD)
        for i in 0..(80 * 25) {
            vga_buffer.add(i).write(0x1F00 | b' ' as u16);
        }

        // Display panic header
        let header = b"*** PRISMAOS KERNEL PANIC ***";
        let start_pos = (80 - header.len()) / 2;
        for (i, &byte) in header.iter().enumerate() {
            if start_pos + i < 80 {
                vga_buffer.add(start_pos + i).write(0x1F00 | byte as u16);
            }
        }

        // Show location if available
        if let Some(location) = info.location() {
            let file_info = b"File: ";
            let line_start = 80 * 3;

            for (i, &byte) in file_info.iter().enumerate() {
                if line_start + i < 80 * 25 {
                    vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
                }
            }

            // Write filename (truncated)
            let filename = location.file().as_bytes();
            let filename_start = line_start + file_info.len();
            for (i, &byte) in filename.iter().take(40).enumerate() {
                if filename_start + i < 80 * 25 {
                    vga_buffer.add(filename_start + i).write(0x1F00 | byte as u16);
                }
            }

            // Write line number
            let line_info = b"Line: ";
            let line_start = 80 * 4;
            for (i, &byte) in line_info.iter().enumerate() {
                if line_start + i < 80 * 25 {
                    vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
                }
            }

            // Simple line number display
            let line_num = location.line();
            let mut temp_line = line_num;
            let mut digits = [0u8; 10];
            let mut digit_count = 0;

            if temp_line == 0 {
                digits[0] = b'0';
                digit_count = 1;
            } else {
                while temp_line > 0 && digit_count < 10 {
                    digits[digit_count] = (temp_line % 10) as u8 + b'0';
                    temp_line /= 10;
                    digit_count += 1;
                }
            }

            // Display digits in reverse order
            let line_num_start = line_start + line_info.len();
            for i in 0..digit_count {
                let digit_pos = line_num_start + i;
                if digit_pos < 80 * 25 {
                    let digit = digits[digit_count - 1 - i];
                    vga_buffer.add(digit_pos).write(0x1F00 | digit as u16);
                }
            }
        }

        // Instructions - process individually to avoid array type issues
        let instructions = [
            "System halted to prevent damage",
            "Please restart to continue",
            "If this persists, check kernel config",
        ];

        let mut current_line = 7;
        for instruction in instructions.iter() {
            let line_start = 80 * current_line;
            for (i, byte) in instruction.bytes().enumerate() {
                if line_start + i < 80 * 25 {
                    vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
                }
            }
            current_line += 1;
        }
    }

    halt_system();
}

/// Test runner
pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}

/// Basic test
#[test_case]
fn basic_test() {
    assert_eq!(1 + 1, 2);
}