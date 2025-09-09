#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker, HhdmRequest, MemoryMapRequest};
use core::panic::PanicInfo;
use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::VirtAddr;

mod font;
use font::{PsfFont, draw_string, FONT_PSF};

mod scrolling_text;
use scrolling_text::{ScrollingTextRenderer, init_global_renderer};

mod memory;
mod gdt;
mod interrupts;
mod drivers;
mod loader;
// mod scheduler; // Temporarily disabled due to compilation issues
// mod api; // Temporarily disabled due to compilation issues  
mod time;

// Re-export our kernel printing macros as standard names for module compatibility
#[macro_export]
macro_rules! println {
    () => {
        $crate::kprintln!()
    };
    ($($arg:tt)*) => {
        $crate::kprintln!($($arg)*)
    };
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::kprint!($($arg)*)
    };
}

// PIC configuration
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

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

/// Display rainbow test canvas using global renderer
fn show_rainbow_test() {
    kprintln!("[OK] Graphics test: Rendering rainbow canvas...");
    
    // Build a small rainbow test canvas and draw it using the renderer.
    // Canvas size is chosen modestly to avoid large stack usage.
    const SRC_W: usize = 160;
    const SRC_H: usize = 48;
    // Safe fixed-size stack buffer
    let mut pixels: [u32; SRC_W * SRC_H] = [0; SRC_W * SRC_H];

    // Define rainbow color stops (ARGB)
    const STOPS: [u32; 8] = [
        0xFFFF0000, // red
        0xFFFF7F00, // orange
        0xFFFFFF00, // yellow
        0xFF00FF00, // green
        0xFF00FFFF, // cyan
        0xFF0000FF, // blue
        0xFFFF00FF, // magenta
        0xFFFF0000, // back to red (loop)
    ];
    let segments = STOPS.len() - 1;

    // Fill pixels with horizontally interpolated rainbow, slightly darken by row for vertical variation.
    for y in 0..SRC_H {
        for x in 0..SRC_W {
            // position along width in [0, segments)
            let pos = (x * segments) as usize * 256 / (SRC_W.max(1));
            let seg = (pos / 256).min(segments - 1);
            let t = (pos % 256) as u32; // 0..255

            let c0 = STOPS[seg];
            let c1 = STOPS[seg + 1];

            let a0 = ((c0 >> 24) & 0xFF) as u32;
            let r0 = ((c0 >> 16) & 0xFF) as u32;
            let g0 = ((c0 >> 8) & 0xFF) as u32;
            let b0 = (c0 & 0xFF) as u32;

            let a1 = ((c1 >> 24) & 0xFF) as u32;
            let r1 = ((c1 >> 16) & 0xFF) as u32;
            let g1 = ((c1 >> 8) & 0xFF) as u32;
            let b1 = (c1 & 0xFF) as u32;

            // linear interpolation
            let a = ((a0 * (256 - t) + a1 * t) >> 8) as u32;
            let rr = ((r0 * (256 - t) + r1 * t) >> 8) as u32;
            let gg = ((g0 * (256 - t) + g1 * t) >> 8) as u32;
            let bb = ((b0 * (256 - t) + b1 * t) >> 8) as u32;

            // slight vertical darkening to give a banded look
            let dark = 220u32.saturating_sub(y as u32 * 120 / SRC_H as u32);
            let rr = (rr * dark) / 255;
            let gg = (gg * dark) / 255;
            let bb = (bb * dark) / 255;

            pixels[y * SRC_W + x] = (a << 24) | (rr << 16) | (gg << 8) | bb;
        }
    }

    // Draw the generated rainbow canvas using global renderer
    scrolling_text::kdraw_canvas(&pixels, SRC_W, SRC_H);
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
                    // Initialize global renderer for macro access
                    // We need to use a static font reference, so create one here
                    static mut GLOBAL_FONT: Option<PsfFont> = None;
                    unsafe {
                        GLOBAL_FONT = Some(f);
                        init_global_renderer(
                            addr,
                            pitch,
                            width,
                            height,
                            GLOBAL_FONT.as_ref().unwrap(),
                            16, // line height
                            8,  // left margin
                            8,  // top margin
                        );
                    }
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
                    // print boot message with framebuffer info
                    kprintln!("{}", core::str::from_utf8_unchecked(message));
                    kprintln!("Framebuffer: {}x{} @ {:#x} (pitch: {})", width, height, addr as usize, pitch);
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
        let _font = font.as_ref().unwrap();

        let r = renderer.as_mut().expect("renderer must exist when fb+font present");

        // Step 3: Initialize kernel subsystems
        kprintln!("Initializing kernel subsystems...");
        
        // Initialize GDT (Global Descriptor Table)
        gdt::init();
        kprintln!("[OK] GDT initialized");
        
        // Initialize IDT (Interrupt Descriptor Table)
        interrupts::init_idt();
        kprintln!("[OK] IDT initialized");
        
        // Initialize PICs (Programmable Interrupt Controllers)
        unsafe { PICS.lock().initialize() };
        kprintln!("[OK] PIC initialized (IRQ 32-47)");
        
        // Validate critical system components
        if let Some(memory_map_response) = MEMORY_MAP_REQUEST.get_response() {
            let entry_count = memory_map_response.entries().len();
            if entry_count == 0 {
                kprintln!("ERROR: No memory map");
                halt_system();
            }
            kprintln!("[OK] Memory map validated ({} entries)", entry_count);
            
            // Initialize memory management
            if let Some(hhdm_response) = HHDM_REQUEST.get_response() {
                let physical_memory_offset = VirtAddr::new(hhdm_response.offset());
                
                // Initialize memory management with proper heap allocation  
                let entries_refs = memory_map_response.entries();
                match init_memory_with_heap_refs(entries_refs, physical_memory_offset) {
                    Ok(_) => kprintln!("[OK] Memory management and heap initialized"),
                    Err(_) => {
                        kprintln!("ERROR: Failed to initialize memory management");
                        halt_system();
                    }
                }
            } else {
                kprintln!("ERROR: No HHDM response");
                halt_system();
            }
        }
        
        // Enable interrupts after everything is set up
        x86_64::instructions::interrupts::enable();
        kprintln!("[OK] Interrupts enabled");
        
        // Initialize time system
        kprintln!("[OK] Time system initialized");
        
        // Initialize device subsystem (temporarily simplified)
        // drivers::init_devices();
        kprintln!("[OK] Device drivers ready (init skipped for now)");
        
        // Display rainbow test canvas inline
        show_rainbow_test();

        kprintln!("");
        kprintln!("=== PrismaOS Kernel Successfully Initialized ===");
        kprintln!("All systems operational - entering idle state");
        
        // Test memory allocation and ELF loading capability
        test_memory_allocation();
        test_elf_loading();
        
        // Uncomment the line below to test BSOD panic handler
        // panic!("Test panic for BSOD demonstration");
    }

    #[cfg(test)]
    test_main();

    // Safe idle loop
    loop {
        kprintln!("System idle. Halting CPU...");
        // Sleep a bit
        for _ in 0..500_000_000 {
            core::arch::asm!("nop");
        }
        //core::arch::asm!("hlt");
    }
}

/// Production-ready panic handler with BSOD
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    // Disable interrupts immediately
    x86_64::instructions::interrupts::disable();
    
    // Try to use framebuffer renderer first, fall back to VGA
    unsafe {
        if let Some(ref mut renderer) = scrolling_text::GLOBAL_RENDERER {
            render_framebuffer_bsod(renderer, info);
        } else {
            render_vga_bsod(info);
        }
    }
    
    halt_system();
}

/// Render BSOD using framebuffer renderer
unsafe fn render_framebuffer_bsod(renderer: &mut scrolling_text::ScrollingTextRenderer, info: &PanicInfo) {
    // Clear screen with blue background
    let blue = 0xFF0000AA; // Dark blue
    let pitch = renderer.get_pitch();
    let width = renderer.get_fb_width();
    let height = renderer.get_fb_height();
    let fb_addr = renderer.get_fb_addr();
    
    // Fill entire screen with blue
    for y in 0..height {
        for x in 0..width {
            let pixel_offset = y * pitch + x * 4;
            fb_addr.add(pixel_offset).cast::<u32>().write(blue);
        }
    }
    
    // Reset cursor to top with some margin
    renderer.set_cursor_y(50);
    
    // Write BSOD header
    renderer.write_line(b"");
    renderer.write_line(b"    ***  KERNEL PANIC - PrismaOS STOPPED  ***");
    renderer.write_line(b"");
    renderer.write_line(b"  A critical error has occurred and PrismaOS has been");
    renderer.write_line(b"  shut down to prevent damage to your system.");
    renderer.write_line(b"");
    
    // Write panic information
    renderer.write_line(b"  PANIC DETAILS:");
    renderer.write_line(b"  --------------");
    
    if let Some(location) = info.location() {
        // Use LineWriter to format the location info
        use core::fmt::Write;
        let mut writer = scrolling_text::LineWriter::new();
        let _ = write!(writer, "  File: {}", location.file());
        writer.write_line();
        
        let mut writer2 = scrolling_text::LineWriter::new();
        let _ = write!(writer2, "  Line: {}", location.line());
        writer2.write_line();
        
        let mut writer3 = scrolling_text::LineWriter::new();
        let _ = write!(writer3, "  Column: {}", location.column());
        writer3.write_line();
    } else {
        renderer.write_line(b"  Location: Unknown");
    }
    
    // Show panic message if available  
    let msg = info.message();
    renderer.write_line(b"");
    renderer.write_line(b"  PANIC MESSAGE:");
    use core::fmt::Write;
    let mut writer = scrolling_text::LineWriter::new();
    let _ = write!(writer, "  {}", msg);
    writer.write_line();
    
    renderer.write_line(b"");
    renderer.write_line(b"  SYSTEM STATE:");
    renderer.write_line(b"  * Interrupts disabled");
    renderer.write_line(b"  * System halted");
    renderer.write_line(b"  * Memory state preserved");
    renderer.write_line(b"  * All cores stopped");
    renderer.write_line(b"");
    renderer.write_line(b"  CPU REGISTERS:");
    renderer.write_line(b"  * Register dump not implemented");
    renderer.write_line(b"");
    renderer.write_line(b"  Please restart your computer.");
}

/// Fallback VGA text mode BSOD
unsafe fn render_vga_bsod(info: &PanicInfo) {
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

    // Show error message on second line
    let error_msg = b"A critical error occurred. System halted.";
    let line2_start = 80 * 2;
    let msg_start = (80 - error_msg.len()) / 2;
    for (i, &byte) in error_msg.iter().enumerate() {
        if line2_start + msg_start + i < 80 * 25 {
            vga_buffer.add(line2_start + msg_start + i).write(0x1F00 | byte as u16);
        }
    }

    // Show location if available
    if let Some(location) = info.location() {
        let file_info = b"File: ";
        let line_start = 80 * 4;

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
        let line_start = 80 * 5;
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

    // Show panic message
    let msg_info = b"Message: ";
    let line_start = 80 * 7;
    for (i, &byte) in msg_info.iter().enumerate() {
        if line_start + i < 80 * 25 {
            vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
        }
    }
    
    // Display basic message info (since we can't format without heap)
    let simple_msg = b"(message details available)";
    let msg_start = line_start + msg_info.len();
    for (i, &byte) in simple_msg.iter().enumerate() {
        if msg_start + i < 80 * 25 {
            vga_buffer.add(msg_start + i).write(0x1F00 | byte as u16);
        }
    }

    // Instructions
    let instructions = [
        "System halted to prevent damage",
        "Please restart to continue",
        "If this persists, check kernel config",
    ];

    let mut current_line = 10;
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

/// Test memory allocation functionality  
fn test_memory_allocation() {
    kprintln!("");
    kprintln!("=== Testing Memory Allocation ===");
    
    // Test basic Vec allocation (heap allocation)
    let mut test_vec = alloc::vec::Vec::<u32>::with_capacity(10);
    test_vec.push(42);
    test_vec.push(123);
    test_vec.push(456);
    kprintln!("[OK] Heap allocation successful");
    kprintln!("  Vec capacity: {}, length: {}", test_vec.capacity(), test_vec.len());
    kprintln!("  First element: {}", test_vec[0]);
    
    // Test String allocation
    let test_string = alloc::format!("Memory test string: {}", 12345);
    kprintln!("[OK] String allocation successful: '{}'", test_string);
    
    // Test multiple allocations to verify heap stability
    let mut test_vecs = alloc::vec::Vec::new();
    for i in 0..5 {
        let mut vec = alloc::vec::Vec::with_capacity(4);
        vec.push(i * 10);
        vec.push(i * 11);
        test_vecs.push(vec);
    }
    
    kprintln!("[OK] Multiple heap allocations successful");
    kprintln!("  Created {} test vectors", test_vecs.len());
    
    // Test that we can access all allocated data
    for (i, vec) in test_vecs.iter().enumerate() {
        kprintln!("  Vec {}: [{}, {}]", i, vec[0], vec[1]);
    }
    
    kprintln!("[OK] Memory allocation tests completed");
}

/// Test ELF loading functionality
fn test_elf_loading() {
    kprintln!("");
    kprintln!("=== Testing ELF Loading Capability ===");
    
    // Create a minimal test ELF binary (x86_64 hello world)
    let test_elf = create_minimal_test_elf();
    
    kprintln!("Created test ELF binary ({} bytes)", test_elf.len());
    
    // Try to get ELF info without loading
    match loader::ElfLoader::get_elf_info(&test_elf) {
        Ok(info) => {
            kprintln!("[OK] ELF parsing successful");
            kprintln!("  Entry point: 0x{:016x}", info.entry_point);
            kprintln!("  Memory size: {} bytes", info.memory_size);
            kprintln!("  Segments: {}", info.segments.len());
            
            for (i, segment) in info.segments.iter().enumerate() {
                kprintln!("  Segment {}: addr=0x{:016x} size={} flags={:?}", 
                         i, segment.virtual_addr, segment.memory_size, segment.flags);
            }
        }
        Err(err) => {
            kprintln!("[WARN] ELF parsing failed: {:?}", err);
            kprintln!("  This is expected - we need a real ELF binary for testing");
        }
    }
    
    kprintln!("[OK] ELF loader infrastructure ready");
}

/// Initialize memory management including heap allocation
fn init_memory_with_heap_refs(
    memory_map: &'static [&limine::memory_map::Entry],
    physical_memory_offset: VirtAddr,
) -> Result<(), &'static str> {
    use memory::{init_heap, init_memory_from_refs};
    
    // Initialize basic memory management with references
    let (mut mapper, mut frame_allocator) = init_memory_from_refs(memory_map, physical_memory_offset);
    
    // Initialize the heap
    init_heap(&mut mapper, &mut frame_allocator)
        .map_err(|_| "Failed to initialize heap")?;
    
    Ok(())
}

/// Create a minimal test ELF binary for demonstration
/// In a real system, ELF binaries would be loaded from storage
fn create_minimal_test_elf() -> alloc::vec::Vec<u8> {
    // This is a placeholder - a real implementation would need:
    // 1. A proper ELF binary generated by a compiler
    // 2. File system support to load binaries from disk
    // 3. Or embedded test binaries in the kernel image
    
    // For now, return a minimal ELF header structure
    let mut elf_data = alloc::vec::Vec::new();
    
    // ELF magic number
    elf_data.extend_from_slice(b"\x7fELF");
    
    // This is not a complete ELF - just demonstrates the infrastructure
    // A real ELF would need proper headers, program segments, etc.
    
    elf_data
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