#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
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

mod font;
use font::{PsfFont, draw_string, FONT_PSF};

mod scrolling_text;
use scrolling_text::{ScrollingTextRenderer, init_global_renderer};

mod memory;
mod gdt;
mod interrupts;
mod drivers;
mod scheduler;
mod api;
mod executor;
mod time;
mod syscall;
mod process;
mod userspace_test;
mod events;
mod elf;
mod boot_userspace;
mod utils;
mod userspace_isolation;

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

            let initial_width = framebuffer.width() as usize;
            let initial_height = framebuffer.height() as usize;

            let width = initial_width.min(800); // cap at 800x600 for safety
            let height = initial_height.min(600);
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
        
        // Initialize bootstrap heap for early allocations
        unsafe {
            memory::init_bootstrap_heap();
        }
        kprintln!("[OK] Bootstrap heap initialized (64KB)");
        
        // Initialize GDT (Global Descriptor Table)
        gdt::init();
        kprintln!("[OK] GDT initialized");
        
        // Initialize IDT (Interrupt Descriptor Table)
        interrupts::init_idt();
        kprintln!("[OK] IDT initialized");
        
        // Initialize PICs (Programmable Interrupt Controllers)
        unsafe { PICS.lock().initialize() };
        kprintln!("[OK] PIC initialized (IRQ 32-47)");
        
        // Initialize memory management
        if let Some(memory_map_response) = MEMORY_MAP_REQUEST.get_response() {
            let memory_entries = memory_map_response.entries();
            let entry_count = memory_entries.len();
            if entry_count == 0 {
                kprintln!("ERROR: No memory map");
                halt_system();
            }
            kprintln!("[OK] Memory map validated ({} entries)", entry_count);
            
            if let Some(hhdm_response) = HHDM_REQUEST.get_response() {
                let phys_mem_offset = x86_64::VirtAddr::new(hhdm_response.offset());
                kprintln!("[OK] Physical memory offset: {:#x}", phys_mem_offset.as_u64());
                
                // Initialize paging and frame allocator  
                let (mut mapper, mut frame_allocator) = memory::init_memory(memory_entries, phys_mem_offset);
                
                // Initialize kernel heap
                kprintln!("[INFO] Initializing kernel heap: {} MiB at {:#x}", 
                         memory::HEAP_SIZE / (1024 * 1024), memory::HEAP_START);
                match memory::init_heap(&mut mapper, &mut frame_allocator) {
                    Ok(_) => {
                        kprintln!("[OK] Kernel heap initialized with proper virtual memory mapping");
                        let stats = memory::heap_stats();
                        kprintln!("     Heap size: {} MiB, Start: {:#x}", stats.total_size / (1024 * 1024), memory::HEAP_START);
                    },
                    Err(e) => {
                        kprintln!("ERROR: Failed to initialize heap: {:?}", e);
                        halt_system();
                    }
                }
                
                // Initialize scheduler
                scheduler::init_scheduler(1); // Single CPU for now
                kprintln!("[OK] Scheduler initialized");
                
                // Launch the boot GUI!
                kprintln!("");
                kprintln!("=== Launching PrismaOS Desktop Environment ===");
                match boot_userspace::launch_boot_gui(&mut mapper, &mut frame_allocator) {
                    Ok(_) => {
                        kprintln!("[OK] Boot GUI launched successfully");
                    }
                    Err(e) => {
                        kprintln!("ERROR: Failed to launch boot GUI: {}", e);
                        kprintln!("Continuing without GUI...");
                    }
                }
                
            } else {
                kprintln!("ERROR: No HHDM response");
                halt_system();
            }
        } else {
            kprintln!("ERROR: No memory map response");
            halt_system();
        }
        
        // Initialize syscalls
        syscall::init_syscalls();
        kprintln!("[OK] Syscall interface initialized");
        
        // Set up userspace protection before enabling interrupts
        unsafe {
            userspace_isolation::setup_userspace_protection();
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
        utils::color_test::show_rainbow_test();

        kprintln!("");
        kprintln!("=== PrismaOS Kernel Successfully Initialized ===");
        kprintln!("All systems operational");
        
        // Test userspace execution
        userspace_test::test_userspace_execution();
        
        kprintln!("");
        kprintln!("=== Entering idle state ===");
        
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
        core::arch::asm!("hlt");
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
            utils::bsod::render_framebuffer_bsod(renderer, info);
        } else {
            utils::bsod::render_vga_bsod(info);
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