#![no_std]
#![no_main]

use core::arch::asm;
use core::ptr;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};
use core::f32::consts::PI;

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

/// Define the stand and end markers for Limine requests.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            // Try to get refresh rate from framebuffer (Limine does not provide this, so fallback)
            fn get_refresh_rate() -> u32 {
                // If Limine ever provides refresh rate, use it here
                // For now, fallback to 60Hz for testing
                240
            }

            // Simple software delay for frame limiting
            fn wait_for_frame(refresh_rate: u32) {
                // Calibrate this value for your hardware; it's a rough delay
                let delay = 100_000 / refresh_rate;
                for _ in 0..delay {
                    unsafe { asm!("nop"); }
                }
            }
            // Simple HSV to RGB conversion
            fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
                let h = h % 360.0;
                let c = v * s;
                let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
                let m = v - c;
                let (r1, g1, b1) = match h as u32 {
                    0..=59 => (c, x, 0.0),
                    60..=119 => (x, c, 0.0),
                    120..=179 => (0.0, c, x),
                    180..=239 => (0.0, x, c),
                    240..=299 => (x, 0.0, c),
                    _ => (c, 0.0, x),
                };
                (
                    ((r1 + m) * 255.0) as u8,
                    ((g1 + m) * 255.0) as u8,
                    ((b1 + m) * 255.0) as u8,
                )
            }

            let width = framebuffer.width();
            let height = framebuffer.height();
            let pitch = framebuffer.pitch();
            let addr = framebuffer.addr();

            let mut frame = 0u32;
            let refresh_rate = get_refresh_rate();
            loop {
                for y in 0..height {
                    for x in 0..width {
                        // Moving rainbow gradient: hue depends on x, y, and frame
                        let hue = ((x as f32 + y as f32 + frame as f32 * 2.0) % 360.0) as f32;
                        let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
                        let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

                        let pixel_offset = y * pitch + x * 4;
                        unsafe {
                            addr.add(pixel_offset as usize)
                                .cast::<u32>()
                                .write(color);
                        }
                    }
                }
                frame = frame.wrapping_add(1);
                wait_for_frame(refresh_rate);
            }
        }
    }

    hcf();
}

#[cfg(not(test))]
#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    hcf();
}

fn hcf() -> ! {
    loop {
        unsafe {
            #[cfg(target_arch = "x86_64")]
            asm!("hlt");
            #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
            asm!("wfi");
            #[cfg(target_arch = "loongarch64")]
            asm!("idle 0");
        }
    }
}
