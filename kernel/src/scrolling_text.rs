// filepath: c:\Users\redst\OneDrive\Documents\GitHub\PrismaOS\kernel\src\scrolling_text.rs
use core::ptr;
use core::cmp;
use core::fmt::Write;

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
        let _pixel_bytes = stride;
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

    /// Get framebuffer address (unsafe for BSOD use)
    pub unsafe fn get_fb_addr(&self) -> *mut u8 {
        self.fb_addr
    }

    /// Get framebuffer pitch  
    pub fn get_pitch(&self) -> usize {
        self.pitch
    }

    /// Get framebuffer width
    pub fn get_fb_width(&self) -> usize {
        self.fb_width
    }

    /// Get framebuffer height
    pub fn get_fb_height(&self) -> usize {
        self.fb_height
    }

    /// Set cursor Y position (unsafe for BSOD use)
    pub unsafe fn set_cursor_y(&mut self, y: usize) {
        self.cursor_y = y;
    }

    /// Draw a rectangular canvas of pixels (each pixel is u32 0xAARRGGBB) at the current
    /// cursor position (cursor_x is ignored; canvas is drawn starting at left_margin).
    ///
    /// - pixels: row-major pixel data (length must be at least src_width * src_height; if shorter it's clipped)
    /// - src_width/src_height: dimensions of the source canvas
    ///
    /// After drawing the canvas the cursor is advanced below it by src_height + line_spacing so
    /// subsequent text will render below the canvas with spacing.
    pub fn draw_canvas(&mut self, pixels: &[u32], src_width: usize, src_height: usize) {
        let minimal_rows = if src_width == 0 { 0 } else { pixels.len() / src_width };
        let rows_to_draw = cmp::min(src_height, minimal_rows);

        if rows_to_draw == 0 {
            return;
        }

        // Ensure canvas will fit vertically; if not, scroll up by the overflow amount.
        if self.cursor_y + rows_to_draw > self.fb_height {
            let overflow = self.cursor_y + rows_to_draw - self.fb_height;
            self.scroll_up(overflow);
        }

        // If after scrolling it still doesn't fit, clamp the number of rows.
        let max_rows_available = if self.cursor_y >= self.fb_height {
            0
        } else {
            self.fb_height - self.cursor_y
        };
        let rows_to_draw = cmp::min(rows_to_draw, max_rows_available);

        if rows_to_draw == 0 {
            return;
        }

        let bytes_per_pixel = 4usize;
        let stride = self.pitch;
        let dest_left = self.left_margin;
        let max_cols = if dest_left >= self.fb_width {
            0
        } else {
            self.fb_width - dest_left
        };
        let cols_to_draw = cmp::min(src_width, max_cols);
        if cols_to_draw == 0 {
            // Nothing to draw horizontally
            // Still advance cursor below canvas height + spacing.
            self.cursor_y += rows_to_draw + self.line_spacing;
            return;
        }

        let row_bytes = cols_to_draw * bytes_per_pixel;

        // Copy row by row, honoring framebuffer stride.
        for r in 0..rows_to_draw {
            let dest_y = self.cursor_y + r;
            if dest_y >= self.fb_height {
                break;
            }
            let row_base = unsafe { self.fb_addr.add(dest_y * stride).add(dest_left * bytes_per_pixel) };
            let src_index = r * src_width;
            if src_index >= pixels.len() {
                break;
            }
            let src_ptr = unsafe { pixels.as_ptr().add(src_index) } as *const u8;
            unsafe {
                ptr::copy_nonoverlapping(src_ptr, row_base, row_bytes);
            }
        }

        // Advance cursor below the canvas
        self.cursor_y += rows_to_draw + self.line_spacing;
    }

    /// Unsafe variant that draws a canvas from a raw pointer to u32 pixels (row-major).
    /// Caller must ensure the pointer is valid for src_width * src_height elements.
    pub unsafe fn draw_canvas_raw(&mut self, pixels_ptr: *const u32, src_width: usize, src_height: usize) {
        if pixels_ptr.is_null() || src_width == 0 || src_height == 0 {
            return;
        }

        // Ensure canvas will fit vertically; if not, scroll up by the overflow amount.
        if self.cursor_y + src_height > self.fb_height {
            let overflow = self.cursor_y + src_height - self.fb_height;
            self.scroll_up(overflow);
        }

        let max_rows_available = if self.cursor_y >= self.fb_height {
            0
        } else {
            self.fb_height - self.cursor_y
        };
        let rows_to_draw = cmp::min(src_height, max_rows_available);
        if rows_to_draw == 0 {
            return;
        }

        let bytes_per_pixel = 4usize;
        let stride = self.pitch;
        let dest_left = self.left_margin;
        let max_cols = if dest_left >= self.fb_width {
            0
        } else {
            self.fb_width - dest_left
        };
        let cols_to_draw = cmp::min(src_width, max_cols);
        if cols_to_draw == 0 {
            self.cursor_y += rows_to_draw + self.line_spacing;
            return;
        }

        let row_bytes = cols_to_draw * bytes_per_pixel;

        for r in 0..rows_to_draw {
            let dest_y = self.cursor_y + r;
            if dest_y >= self.fb_height {
                break;
            }
            let row_base = self.fb_addr.add(dest_y * stride).add(dest_left * bytes_per_pixel);
            let src_row_ptr = (pixels_ptr.add(r * src_width)) as *const u8;
            ptr::copy_nonoverlapping(src_row_ptr, row_base, row_bytes);
        }

        self.cursor_y += rows_to_draw + self.line_spacing;
    }
}

/// Global renderer storage for macro access
pub static mut GLOBAL_RENDERER: Option<ScrollingTextRenderer<'static>> = None;

/// Initialize the global renderer for use with macros
pub unsafe fn init_global_renderer(
    fb_addr: *mut u8,
    pitch: usize,
    fb_width: usize,
    fb_height: usize,
    font: &'static PsfFont,
    line_height: usize,
    left_margin: usize,
    top_margin: usize,
) {
    GLOBAL_RENDERER = Some(ScrollingTextRenderer::new(
        fb_addr,
        pitch,
        fb_width,
        fb_height,
        font,
        line_height,
        left_margin,
        top_margin,
    ));
}

/// Internal helper for formatted writing
pub struct GlobalWriteProxy;

impl Write for GlobalWriteProxy {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            if let Some(ref mut renderer) = GLOBAL_RENDERER {
                renderer.write_text(s.as_bytes());
            }
        }
        Ok(())
    }
}

/// Write a line to the global renderer (no formatting)
pub fn kwrite_line(text: &str) {
    unsafe {
        if let Some(ref mut renderer) = GLOBAL_RENDERER {
            renderer.write_line(text.as_bytes());
        }
    }
}

/// Write text to the global renderer (handles newlines)
pub fn kwrite_text(text: &str) {
    unsafe {
        if let Some(ref mut renderer) = GLOBAL_RENDERER {
            renderer.write_text(text.as_bytes());
        }
    }
}

/// Draw canvas to the global renderer at current cursor position
pub fn kdraw_canvas(pixels: &[u32], src_width: usize, src_height: usize) {
    unsafe {
        if let Some(ref mut renderer) = GLOBAL_RENDERER {
            renderer.draw_canvas(pixels, src_width, src_height);
        }
    }
}

/// Writer that accumulates formatted output and then writes it as a line
pub struct LineWriter {
    buffer: [u8; 512],
    pos: usize,
}

impl LineWriter {
    pub fn new() -> Self {
        LineWriter {
            buffer: [0; 512],
            pos: 0,
        }
    }

    pub fn write_line(self) {
        let text = unsafe { 
            core::str::from_utf8_unchecked(&self.buffer[..self.pos]) 
        };
        kwrite_line(text);
    }

    pub fn write_text(self) {
        let text = unsafe { 
            core::str::from_utf8_unchecked(&self.buffer[..self.pos]) 
        };
        kwrite_text(text);
    }
}

impl core::fmt::Write for LineWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        for &byte in bytes {
            if self.pos < self.buffer.len() - 1 {
                self.buffer[self.pos] = byte;
                self.pos += 1;
            }
        }
        Ok(())
    }
}

/// Macro for writing a line with full formatting support
#[macro_export]
macro_rules! kprintln {
    () => {
        $crate::scrolling_text::kwrite_line("")
    };
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let mut writer = $crate::scrolling_text::LineWriter::new();
            let _ = write!(writer, $($arg)*);
            writer.write_line();
        }
    };
}

/// Macro for writing text without automatic newline with full formatting support
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let mut writer = $crate::scrolling_text::LineWriter::new();
            let _ = write!(writer, $($arg)*);
            writer.write_text();
        }
    };
}

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