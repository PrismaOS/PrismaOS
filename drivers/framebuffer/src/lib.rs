#![no_std]

use core::any::Any;
use spin::Mutex;

use lib_kernel::drivers::{Driver, DriverError};

pub struct FramebufferDriver {
    base_addr: Option<*mut u8>,
    width: u32,
    height: u32,
    pitch: u32,
    bytes_per_pixel: u32,
    initialized: bool,
}

impl FramebufferDriver {
    pub fn new() -> Self {
        FramebufferDriver {
            base_addr: None,
            width: 0,
            height: 0,
            pitch: 0,
            bytes_per_pixel: 4,
            initialized: false,
        }
    }

    /// Initialize framebuffer with Limine response
    pub fn init_with_limine(&mut self, framebuffer: &limine::framebuffer::Framebuffer) -> Result<(), DriverError> {
        self.base_addr = Some(framebuffer.addr() as *mut u8);
        self.width = framebuffer.width() as u32;
        self.height = framebuffer.height() as u32;
        self.pitch = framebuffer.pitch() as u32;
        self.bytes_per_pixel = (framebuffer.bpp() as u32 + 7) / 8;
        self.initialized = true;

        lib_kernel::println!("Framebuffer initialized: {}x{} @ {:p}", 
                       self.width, self.height, self.base_addr.unwrap());
        
        // Clear framebuffer to black
        self.clear(0x000000);
        
        Ok(())
    }

    /// Clear framebuffer with specified color
    pub fn clear(&self, color: u32) {
        if !self.initialized {
            return;
        }

        let fb_addr = match self.base_addr {
            Some(addr) => addr,
            None => return,
        };

        let pixel_count = (self.width * self.height) as usize;
        let fb_pixels = fb_addr as *mut u32;

        for i in 0..pixel_count {
            unsafe {
                *fb_pixels.add(i) = color;
            }
        }
    }

    /// Set pixel at coordinates
    pub fn set_pixel(&self, x: u32, y: u32, color: u32) {
        if !self.initialized || x >= self.width || y >= self.height {
            return;
        }

        let fb_addr = match self.base_addr {
            Some(addr) => addr,
            None => return,
        };

        let pixel_offset = (y * (self.pitch / 4) + x) as usize;
        let fb_pixels = fb_addr as *mut u32;

        unsafe {
            *fb_pixels.add(pixel_offset) = color;
        }
    }

    /// Get pixel at coordinates
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if !self.initialized || x >= self.width || y >= self.height {
            return 0;
        }

        let fb_addr = match self.base_addr {
            Some(addr) => addr,
            None => return 0,
        };

        let pixel_offset = (y * (self.pitch / 4) + x) as usize;
        let fb_pixels = fb_addr as *const u32;

        unsafe {
            *fb_pixels.add(pixel_offset)
        }
    }

    /// Draw rectangle
    pub fn draw_rect(&self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        for row in y..(y + height).min(self.height) {
            for col in x..(x + width).min(self.width) {
                self.set_pixel(col, row, color);
            }
        }
    }

    /// Copy buffer to framebuffer
    pub fn copy_from_buffer(&self, buffer: &[u32], src_x: u32, src_y: u32, 
                           src_width: u32, dst_x: u32, dst_y: u32, 
                           copy_width: u32, copy_height: u32) {
        if !self.initialized {
            return;
        }

        for row in 0..copy_height {
            let src_row = src_y + row;
            let dst_row = dst_y + row;
            
            if dst_row >= self.height {
                break;
            }

            for col in 0..copy_width {
                let src_col = src_x + col;
                let dst_col = dst_x + col;
                
                if dst_col >= self.width {
                    break;
                }

                let src_idx = (src_row * src_width + src_col) as usize;
                if src_idx < buffer.len() {
                    self.set_pixel(dst_col, dst_row, buffer[src_idx]);
                }
            }
        }
    }

    /// Get framebuffer info
    pub fn get_info(&self) -> FramebufferInfo {
        FramebufferInfo {
            width: self.width,
            height: self.height,
            pitch: self.pitch,
            bytes_per_pixel: self.bytes_per_pixel,
            initialized: self.initialized,
        }
    }

    /// Get raw framebuffer pointer (unsafe)
    pub unsafe fn get_raw_ptr(&self) -> Option<*mut u8> {
        self.base_addr
    }
}

unsafe impl Send for FramebufferDriver {}
unsafe impl Sync for FramebufferDriver {}

impl Driver for FramebufferDriver {
    fn name(&self) -> &'static str {
        "framebuffer"
    }

    fn init(&mut self) -> Result<(), DriverError> {
        // Framebuffer initialization is handled separately via Limine
        // This is just a placeholder
        if !self.initialized {
            return Err(DriverError::InitializationFailed);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        self.base_addr = None;
        self.initialized = false;
        Ok(())
    }

    fn interrupt_handler(&mut self, _irq: u8) -> bool {
        // Framebuffer doesn't typically use interrupts
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bytes_per_pixel: u32,
    pub initialized: bool,
}

// Global framebuffer driver instance for easy access
static FRAMEBUFFER: Mutex<Option<FramebufferDriver>> = Mutex::new(None);

pub fn init_global_framebuffer(framebuffer: &limine::framebuffer::Framebuffer) -> Result<(), DriverError> {
    let mut fb = FramebufferDriver::new();
    fb.init_with_limine(framebuffer)?;
    *FRAMEBUFFER.lock() = Some(fb);
    Ok(())
}

pub fn with_framebuffer<R>(f: impl FnOnce(&FramebufferDriver) -> R) -> Option<R> {
    let fb_guard = FRAMEBUFFER.lock();
    if let Some(ref fb) = *fb_guard {
        Some(f(fb))
    } else {
        None
    }
}

pub fn with_framebuffer_mut<R>(f: impl FnOnce(&mut FramebufferDriver) -> R) -> Option<R> {
    // Note: This is not actually mutable due to the static, but provides the interface
    let mut fb_guard = FRAMEBUFFER.lock();
    if let Some(ref mut fb) = *fb_guard {
        Some(f(fb))
    } else {
        None
    }
}