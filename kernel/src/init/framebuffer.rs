//! Framebuffer and renderer initialization
//!
//! Responsible for probing Limine framebuffer, loading the PSF font, and
//! initializing both a global renderer (for macros) and an explicit
//! `ScrollingTextRenderer` returned to the caller for direct use.

use crate::font::{PsfFont, FONT_PSF};
use crate::scrolling_text::{init_global_renderer, ScrollingTextRenderer};
use crate::utils;
use alloc::format;

// Keep a single font instance alive for the global renderer needing a
// `'static` reference.
static mut INIT_GLOBAL_FONT: Option<PsfFont> = None;

/// Minimal framebuffer context returned when rendering is available.
pub struct FbContext<'a> {
    pub addr: *mut u8,
    pub pitch: usize,
    pub width: usize,
    pub height: usize,
    pub renderer: ScrollingTextRenderer<'a>,
}

impl<'a> FbContext<'a> {
    /// Helper to write a string using the renderer.
    pub fn write_line(&mut self, text: &str) {
        self.renderer.write_text(text.as_bytes());
    }
}

/// Probe Limine framebuffer and initialize a renderer if possible.
pub fn init_framebuffer_and_renderer() -> Result<Option<FbContext<'static>>, &'static str> {
    if let Some(framebuffer_response) = crate::FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            let addr = framebuffer.addr();
            let pitch = framebuffer.pitch() as usize;
            let initial_width = framebuffer.width() as usize;
            let initial_height = framebuffer.height() as usize;
            let width = initial_width.min(800);
            let height = initial_height.min(600);
            
            if addr.is_null() || pitch == 0 || width == 0 || height == 0 {
                return Ok(None);
            }
            
            
            
            if let Some(font) = PsfFont::from_bytes(&FONT_PSF) {
                unsafe {
                    INIT_GLOBAL_FONT = Some(font);
                    let font_ref = INIT_GLOBAL_FONT.as_ref().unwrap();
                    init_global_renderer(addr, pitch, width, height, font_ref, 16, 8, 8);
                }
                
                unsafe {
                    let renderer = ScrollingTextRenderer::new(
                        addr,
                        pitch,
                        width,
                        height,
                        INIT_GLOBAL_FONT.as_ref().unwrap(),
                        16,
                        8,
                        8,
                    );
                    
                    let mut context = FbContext { 
                        addr: addr as *mut u8, 
                        pitch, 
                        width, 
                        height, 
                        renderer 
                    };
                    
                    context.renderer.write_text("PrismaOS - Production Kernel Starting...\n".as_bytes());
                    // TODO: We need to make sure the allocator is initialized before using format!
                    // context.renderer.write_text(format!("Framebuffer: {}x{} @ {:#x} (pitch: {})\n", width, height, addr as usize, pitch).as_bytes());
                    
                    return Ok(Some(context));
                }
            } else {
                for y in 0..height {
                    for x in 0..width {
                        let pixel_offset = y * pitch + x * 4;
                        unsafe {
                            (addr as *mut u32).add(pixel_offset / 4).write_volatile(0xFFFF0000);
                        }
                    }
                }
                return Err("Framebuffer present but PSF font cannot be loaded");
            }
        }
    }
    
    Ok(None)
}