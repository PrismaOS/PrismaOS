#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec, format};

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker, HhdmRequest, MemoryMapRequest};

mod gdt;
mod interrupts;
mod memory;
mod executor;
mod api;
mod scheduler;
mod drivers;
mod time;

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

    println!("PrismaOS Kernel Starting...");

    gdt::init();
    interrupts::init_idt();

    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();

    let physical_memory_offset = HHDM_REQUEST
        .get_response()
        .expect("Failed to get HHDM response")
        .offset();

    let memory_map = MEMORY_MAP_REQUEST
        .get_response()
        .expect("Failed to get memory map")
        .entries();

    let (mut mapper, mut frame_allocator) = memory::init_memory(
        memory_map,
        x86_64::VirtAddr::new(physical_memory_offset)
    );

    memory::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    #[cfg(test)]
    test_main();

    // Initialize SMP scheduler
    scheduler::smp::init_smp();
    scheduler::init_scheduler(scheduler::smp::cpu_count());

    // Initialize device subsystem
    drivers::init_devices();

    // Initialize framebuffer driver with Limine response
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            if let Err(e) = drivers::framebuffer::init_global_framebuffer(framebuffer) {
                println!("Failed to initialize framebuffer: {:?}", e);
            }
        }
    }

    println!("Initializing compositor and demo application...");
    
    let mut executor = executor::Executor::new();

    executor.spawn(executor::task::Task::new(example_task()));
    executor.spawn(executor::task::Task::new(
        executor::keyboard::print_keypresses(),
    ));

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            executor.spawn(executor::task::Task::new(modern_compositor_task(
                framebuffer.addr() as *mut u8,
                framebuffer.width() as u32,
                framebuffer.height() as u32,
                framebuffer.pitch() as u32,
            )));
        }
    }

    println!("Starting executor...");
    executor.run();
}

#[cfg(not(test))]
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

async fn async_number() -> u32 {
    42
}

async fn modern_compositor_task(
    framebuffer: *mut u8,
    width: u32,
    height: u32,
    stride: u32,
) {
    use api::objects::*;
    use api::*;
    use alloc::sync::Arc;

    println!("Starting modern compositor with double-buffering...");

    // Create display object for the framebuffer
    let display = Arc::new(Display::new(width, height, 60, framebuffer));
    let display_handle = get_registry()
        .register_object(display.clone(), ProcessId::new(), Rights::ALL)
        .expect("Failed to register display");

    // Create a demo surface
    let surface = Arc::new(Surface::new(400, 300, PixelFormat::Rgba8888));
    let surface_handle = get_registry()
        .register_object(surface.clone(), ProcessId::new(), Rights::ALL)
        .expect("Failed to register surface");

    // Create a buffer for the surface
    let buffer = Arc::new(Buffer::new(400, 300, PixelFormat::Rgba8888));
    let buffer_handle = get_registry()
        .register_object(buffer.clone(), ProcessId::new(), Rights::ALL)
        .expect("Failed to register buffer");

    // Fill buffer with a gradient
    {
        let buffer_data = buffer.data.as_ptr() as *mut u32;
        for y in 0..300u32 {
            for x in 0..400u32 {
                let r = (x * 255 / 400) as u8;
                let g = (y * 255 / 300) as u8;
                let b = 128;
                let pixel = (r as u32) << 16 | (g as u32) << 8 | (b as u32);
                unsafe {
                    *buffer_data.add((y * 400 + x) as usize) = pixel;
                }
            }
        }
    }

    // Attach buffer to surface and commit
    surface.attach_buffer(buffer_handle);
    surface.commit();

    println!("Compositor initialized with demo surface!");

    // Simple compositor loop with vsync simulation
    let mut frame_count = 0u32;
    loop {
        // Simulate vsync timing (60 FPS)
        executor::task::sleep(Duration::from_millis(16)).await;

        // Composite the surface onto the framebuffer
        if *surface.committed.read() {
            composite_surface_to_framebuffer(
                &surface,
                &buffer,
                framebuffer,
                width,
                height,
                stride,
                100, // x offset
                100, // y offset
            );
        }

        frame_count = frame_count.wrapping_add(1);
        if frame_count % 60 == 0 {
            println!("Compositor: {} frames rendered", frame_count);
        }
    }
}

fn composite_surface_to_framebuffer(
    surface: &Surface,
    buffer: &Buffer,
    framebuffer: *mut u8,
    fb_width: u32,
    fb_height: u32,
    fb_stride: u32,
    dst_x: u32,
    dst_y: u32,
) {
    let src_data = buffer.data.as_ptr() as *const u32;
    let dst_data = framebuffer as *mut u32;

    for y in 0..surface.height {
        let dst_line_y = dst_y + y;
        if dst_line_y >= fb_height {
            break;
        }

        for x in 0..surface.width {
            let dst_line_x = dst_x + x;
            if dst_line_x >= fb_width {
                break;
            }

            let src_idx = (y * surface.width + x) as usize;
            let dst_idx = (dst_line_y * (fb_stride / 4) + dst_line_x) as usize;

            unsafe {
                let pixel = *src_data.add(src_idx);
                *dst_data.add(dst_idx) = pixel;
            }
        }
    }
}

use core::time::Duration;

pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
