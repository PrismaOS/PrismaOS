// filepath: c:\Users\redst\OneDrive\Documents\GitHub\PrismaOS\kernel\src\scrolling_text.rs
#![no_std]

use core::ptr;

use crate::font::{draw_string, PsfFont};

/// Simple scrolling text renderer for a linear framebuffer.
/// - Uses draw_string from font.rs to render lines.
/// - Abstracts line/char spacing and basic scrolling behavior.
/// - Designed for kernels / no_std environments.
///
/// Notes:
/// - Colors are 0xAARRGGBB packed into u32 and written directly to framebuffer pixels.
/// - pitch is the framebuffer stride in bytes per scanline.
/// - This renderer assumes 32-bit pixels (4 bytes per pixel).
pub struct ScrollingTextRenderer<'a> {
    fb_addr: *mut u8,
    pitch: usize,
    fb_width: usize,
    fb_height: usize,

    font: &'a PsfFont<'a>,

    fg_color: u32,
    bg_color: u32,

    line_spacing: usize,
    char_spacing: usize,
    line_height: usize,

    left_margin: usize,
    top_margin: usize,

    cursor_x: usize,
    cursor_y: usize,
}

impl<'a> ScrollingTextRenderer<'a> {
    /// Create a new renderer.
    /// - fb_addr: framebuffer base pointer
    /// - pitch: bytes per scanline
    /// - fb_width/fb_height: framebuffer pixel dimensions (width/height)
    /// - font: reference to a loaded PSF font
    /// - line_height: pixel height reserved per logical line (typically font height + extra)
    /// - left_margin/top_margin: pixel offsets for rendering area origin
    pub fn new(
        fb_addr: *mut u8,
        pitch: usize,
        fb_width: usize,
        fb_height: usize,
        font: &'a PsfFont,
        line_height: usize,
        left_margin: usize,
        top_margin: usize,
    ) -> Self {
        let cursor_x = left_margin;
        let cursor_y = top_margin;

        Self {
            fb_addr,
            pitch,
            fb_width,
            fb_height,
            font,
            fg_color: 0xFFFFFFFF, // default white
            bg_color: 0x00000000, // default transparent/black
            line_spacing: 2,
            char_spacing: 1,
            line_height: if line_height == 0 { 8 } else { line_height },
            left_margin,
            top_margin,
            cursor_x,
            cursor_y,
        }
    }

    /// Set foreground and background colors (0xAARRGGBB).
    pub fn set_colors(&mut self, fg: u32, bg: u32) {
        self.fg_color = fg;
        self.bg_color = bg;
    }

    /// Configure spacing in pixels.
    pub fn set_spacing(&mut self, line_spacing: usize, char_spacing: usize) {
        self.line_spacing = line_spacing;
        self.char_spacing = char_spacing;
    }

    /// Clear the whole framebuffer region with the background color.
    pub fn clear(&self) {
        if self.fb_addr.is_null() || self.pitch == 0 {
            return;
        }

        let bytes_per_pixel = 4usize;
        let stride = self.pitch;
        let width = self.fb_width;
        let height = self.fb_height;

        for y in 0..height {
            let row_base = unsafe { self.fb_addr.add(y * stride) };
            for x in 0..width {
                let pixel_ptr = unsafe { row_base.add(x * bytes_per_pixel).cast::<u32>() };
                unsafe { pixel_ptr.write_volatile(self.bg_color) };
            }
        }
    }

    /// Internal: scroll framebuffer up by `pixels` vertical pixels.
    fn scroll_up(&mut self, pixels: usize) {
        if self.fb_addr.is_null() || self.pitch == 0 || pixels == 0 {
            return;
        }

        let stride = self.pitch;
        let pixel_bytes = stride;
        let h = self.fb_height;

        if pixels >= h {
            // Clear entire area
            self.clear();
            self.cursor_y = self.top_margin;
            return;
        }

        let copy_rows = h - pixels;
        let src_offset = pixels * stride;
        let src = unsafe { self.fb_addr.add(src_offset) };
        let dst = self.fb_addr;
        let copy_bytes = copy_rows * stride;

        // Move visible area up
        unsafe {
            ptr::copy(src, dst, copy_bytes);
        }

        // Clear the freed bottom area
        let start_clear_row = copy_rows;
        for y in start_clear_row..h {
            let row_base = unsafe { self.fb_addr.add(y * stride) };
            for x in 0..self.fb_width {
                let pixel_ptr = unsafe { row_base.add(x * 4).cast::<u32>() };
                unsafe { pixel_ptr.write_volatile(self.bg_color) };
            }
        }

        // Adjust cursor
        if self.cursor_y >= pixels {
            self.cursor_y -= pixels;
        } else {
            self.cursor_y = self.top_margin;
        }
    }

    /// Write a single line (no newline handling). Draws the provided bytes
    /// at the current cursor (cursor_x, cursor_y). Advances cursor to next line.
    pub fn write_line(&mut self, line: &[u8]) {
        if self.fb_addr.is_null() {
            return;
        }

        // Ensure we don't render out-of-bounds vertically
        if self.cursor_y + self.line_height > self.fb_height {
            // Scroll up by one logical line
            let scroll_pixels = self.line_height + self.line_spacing;
            self.scroll_up(scroll_pixels);
        }

        // Render using draw_string from font.rs
        // draw_string(addr, pitch, x, y, color, font, message, width, height)
        unsafe {
            draw_string(
                self.fb_addr,
                self.pitch,
                self.cursor_x,
                self.cursor_y,
                self.fg_color,
                self.font,
                line,
                self.fb_width,
                self.fb_height,
            );
        }

        // Advance cursor to next line
        self.cursor_y += self.line_height + self.line_spacing;
    }

    /// Write text handling '\n' as newlines. Splits on newline and writes each line.
    pub fn write_text(&mut self, text: &[u8]) {
        let mut start = 0usize;
        for (i, &b) in text.iter().enumerate() {
            if b == b'\n' {
                let slice = &text[start..i];
                self.write_line(slice);
                start = i + 1;
            }
        }
        if start < text.len() {
            self.write_line(&text[start..]);
        }
    }

    /// Move cursor to top-left of the rendering area.
    pub fn reset_cursor(&mut self) {
        self.cursor_x = self.left_margin;
        self.cursor_y = self.top_margin;
    }

    /// Set absolute cursor (x, y) in pixels.
    pub fn set_cursor(&mut self, x: usize, y: usize) {
        self.cursor_x = x;
        self.cursor_y = y;
    }
}