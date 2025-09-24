use crate::{surface::Surface, PixelFormat};

pub struct SoftwareRenderer {
    framebuffer: *mut u8,
    width: u32,
    height: u32,
    stride: u32,
}

unsafe impl Send for SoftwareRenderer {}
unsafe impl Sync for SoftwareRenderer {}

impl SoftwareRenderer {
    pub fn new(framebuffer: *mut u8, width: u32, height: u32) -> Self {
        let stride = width * 4; // Assume RGBA8888 for framebuffer
        SoftwareRenderer {
            framebuffer,
            width,
            height,
            stride,
        }
    }

    pub fn clear(&mut self, color: u32) {
        let pixel_count = (self.width * self.height) as usize;
        let fb_pixels = self.framebuffer as *mut u32;

        for i in 0..pixel_count {
            unsafe {
                *fb_pixels.add(i) = color;
            }
        }
    }

    pub fn blit_surface(&mut self, surface: &Surface, dst_x: i32, dst_y: i32) {
        let buffer = match surface.get_buffer() {
            Some(buf) => buf,
            None => return,
        };

        let src_width = surface.width();
        let src_height = surface.height();
        let src_format = surface.format();

        // Calculate clipping bounds
        let clip_x = dst_x.max(0) as u32;
        let clip_y = dst_y.max(0) as u32;
        let clip_width = (dst_x + src_width as i32).min(self.width as i32) - clip_x as i32;
        let clip_height = (dst_y + src_height as i32).min(self.height as i32) - clip_y as i32;

        if clip_width <= 0 || clip_height <= 0 {
            return;
        }

        let src_x_offset = if dst_x < 0 { (-dst_x) as u32 } else { 0 };
        let src_y_offset = if dst_y < 0 { (-dst_y) as u32 } else { 0 };

        match src_format {
            PixelFormat::Rgba8888 => {
                self.blit_rgba8888(&buffer, src_width, src_x_offset, src_y_offset,
                                  clip_x, clip_y, clip_width as u32, clip_height as u32);
            }
            PixelFormat::Bgra8888 => {
                self.blit_bgra8888(&buffer, src_width, src_x_offset, src_y_offset,
                                  clip_x, clip_y, clip_width as u32, clip_height as u32);
            }
            PixelFormat::Rgb888 => {
                self.blit_rgb888(&buffer, src_width, src_x_offset, src_y_offset,
                                clip_x, clip_y, clip_width as u32, clip_height as u32);
            }
            PixelFormat::Bgr888 => {
                self.blit_bgr888(&buffer, src_width, src_x_offset, src_y_offset,
                                clip_x, clip_y, clip_width as u32, clip_height as u32);
            }
        }
    }

    fn blit_rgba8888(&mut self, src: &[u8], src_width: u32, 
                    src_x: u32, src_y: u32,
                    dst_x: u32, dst_y: u32, width: u32, height: u32) {
        let dst_pixels = self.framebuffer as *mut u32;
        let src_pixels = src.as_ptr() as *const u32;

        for y in 0..height {
            let src_row = ((src_y + y) * src_width + src_x) as usize;
            let dst_row = ((dst_y + y) * self.width + dst_x) as usize;

            for x in 0..width {
                unsafe {
                    let src_pixel = *src_pixels.add(src_row + x as usize);
                    let alpha = (src_pixel >> 24) & 0xFF;
                    
                    if alpha == 0xFF {
                        // Fully opaque, direct copy
                        *dst_pixels.add(dst_row + x as usize) = src_pixel;
                    } else if alpha > 0 {
                        // Alpha blending
                        let dst_pixel = *dst_pixels.add(dst_row + x as usize);
                        let blended = self.alpha_blend(src_pixel, dst_pixel);
                        *dst_pixels.add(dst_row + x as usize) = blended;
                    }
                }
            }
        }
    }

    fn blit_bgra8888(&mut self, src: &[u8], src_width: u32,
                    src_x: u32, src_y: u32,
                    dst_x: u32, dst_y: u32, width: u32, height: u32) {
        let dst_pixels = self.framebuffer as *mut u32;
        let src_pixels = src.as_ptr() as *const u32;

        for y in 0..height {
            let src_row = ((src_y + y) * src_width + src_x) as usize;
            let dst_row = ((dst_y + y) * self.width + dst_x) as usize;

            for x in 0..width {
                unsafe {
                    let src_pixel = *src_pixels.add(src_row + x as usize);
                    // Convert BGRA to RGBA
                    let b = (src_pixel >> 24) & 0xFF;
                    let g = (src_pixel >> 16) & 0xFF;
                    let r = (src_pixel >> 8) & 0xFF;
                    let a = src_pixel & 0xFF;
                    let rgba_pixel = (a << 24) | (r << 16) | (g << 8) | b;
                    
                    if a == 0xFF {
                        *dst_pixels.add(dst_row + x as usize) = rgba_pixel;
                    } else if a > 0 {
                        let dst_pixel = *dst_pixels.add(dst_row + x as usize);
                        let blended = self.alpha_blend(rgba_pixel, dst_pixel);
                        *dst_pixels.add(dst_row + x as usize) = blended;
                    }
                }
            }
        }
    }

    fn blit_rgb888(&mut self, src: &[u8], src_width: u32,
                  src_x: u32, src_y: u32,
                  dst_x: u32, dst_y: u32, width: u32, height: u32) {
        let dst_pixels = self.framebuffer as *mut u32;

        for y in 0..height {
            let src_row_start = ((src_y + y) * src_width + src_x) as usize * 3;
            let dst_row = ((dst_y + y) * self.width + dst_x) as usize;

            for x in 0..width {
                let src_idx = src_row_start + (x as usize * 3);
                unsafe {
                    let r = src[src_idx] as u32;
                    let g = src[src_idx + 1] as u32;
                    let b = src[src_idx + 2] as u32;
                    let pixel = 0xFF000000 | (r << 16) | (g << 8) | b;
                    *dst_pixels.add(dst_row + x as usize) = pixel;
                }
            }
        }
    }

    fn blit_bgr888(&mut self, src: &[u8], src_width: u32,
                  src_x: u32, src_y: u32,
                  dst_x: u32, dst_y: u32, width: u32, height: u32) {
        let dst_pixels = self.framebuffer as *mut u32;

        for y in 0..height {
            let src_row_start = ((src_y + y) * src_width + src_x) as usize * 3;
            let dst_row = ((dst_y + y) * self.width + dst_x) as usize;

            for x in 0..width {
                let src_idx = src_row_start + (x as usize * 3);
                unsafe {
                    let b = src[src_idx] as u32;
                    let g = src[src_idx + 1] as u32;
                    let r = src[src_idx + 2] as u32;
                    let pixel = 0xFF000000 | (r << 16) | (g << 8) | b;
                    *dst_pixels.add(dst_row + x as usize) = pixel;
                }
            }
        }
    }

    fn alpha_blend(&self, src: u32, dst: u32) -> u32 {
        let src_a = (src >> 24) & 0xFF;
        let src_r = (src >> 16) & 0xFF;
        let src_g = (src >> 8) & 0xFF;
        let src_b = src & 0xFF;

        let dst_r = (dst >> 16) & 0xFF;
        let dst_g = (dst >> 8) & 0xFF;
        let dst_b = dst & 0xFF;

        let inv_alpha = 255 - src_a;
        
        let out_r = (src_r * src_a + dst_r * inv_alpha) / 255;
        let out_g = (src_g * src_a + dst_g * inv_alpha) / 255;
        let out_b = (src_b * src_a + dst_b * inv_alpha) / 255;

        0xFF000000 | (out_r << 16) | (out_g << 8) | out_b
    }

    pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        let dst_pixels = self.framebuffer as *mut u32;

        for row in y..(y + height).min(self.height) {
            let row_start = (row * self.width) as usize;
            for col in x..(x + width).min(self.width) {
                unsafe {
                    *dst_pixels.add(row_start + col as usize) = color;
                }
            }
        }
    }
}