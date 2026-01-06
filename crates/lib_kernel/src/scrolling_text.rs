//! Linux-style character buffer console for kernel logging
//!
//! This implements a framebuffer console similar to Linux's fbcon:
//! - Characters are stored in a FIXED-SIZE text buffer (not pixels, no heap allocation)
//! - Scrolling is instant (just move line pointers)
//! - Only dirty lines are re-rendered to framebuffer
//! - Supports scrollback buffer for history

use core::ptr;
use crate::font::{draw_string, PsfFont};

extern crate alloc;
use alloc::boxed::Box;

// Fixed buffer dimensions (heap-allocated)
// Keep small for bootstrap heap compatibility (renderer created before main heap init)
const MAX_COLS: usize = 80;   // Maximum characters per line
const MAX_LINES: usize = 30;   // Small buffer - fits in 64KB bootstrap heap

/// A single character cell with color attributes
#[derive(Copy, Clone, Debug)]
pub struct ConsoleChar {
    pub ch: u8,
    pub fg_color: u32,
    pub bg_color: u32,
}

impl ConsoleChar {
    pub const fn new(ch: u8, fg_color: u32, bg_color: u32) -> Self {
        Self { ch, fg_color, bg_color }
    }

    pub const fn blank(bg_color: u32) -> Self {
        Self {
            ch: b' ',
            fg_color: 0xFFFFFFFF,
            bg_color,
        }
    }
}

/// A single line of console text with fixed-size array (no allocation)
#[derive(Copy, Clone)]
pub struct ConsoleLine {
    chars: [ConsoleChar; MAX_COLS],
    width: usize,  // Actual width used
    dirty: bool,
}

impl ConsoleLine {
    pub const fn new(bg_color: u32) -> Self {
        Self {
            chars: [ConsoleChar::blank(bg_color); MAX_COLS],
            width: MAX_COLS,
            dirty: false,
        }
    }

    pub fn set_width(&mut self, width: usize) {
        self.width = width.min(MAX_COLS);
    }

    pub fn clear(&mut self, bg_color: u32) {
        for i in 0..self.width {
            self.chars[i] = ConsoleChar::blank(bg_color);
        }
        self.dirty = true;
    }

    pub fn set_char(&mut self, col: usize, ch: ConsoleChar) {
        if col < self.width {
            self.chars[col] = ch;
            self.dirty = true;
        }
    }

    pub fn get_char(&self, col: usize) -> Option<ConsoleChar> {
        if col < self.width {
            Some(self.chars[col])
        } else {
            None
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

/// Linux-style framebuffer console with character buffer (heap-allocated to avoid stack overflow)
pub struct ScrollingTextRenderer<'a> {
    // FIXED-SIZE CHARACTER BUFFER (heap-allocated via Box::leak to avoid stack overflow)
    lines: &'a mut [ConsoleLine; MAX_LINES],
    line_count: usize,          // Number of lines currently in buffer
    start_line: usize,          // Index of first line (ring buffer)
    visible_lines: usize,       // Lines visible on screen

    // CURSOR STATE
    cursor_col: usize,          // Column position (0 to cols-1)
    cursor_line: usize,         // Logical line number (0 to line_count-1)

    // DISPLAY DIMENSIONS
    cols: usize,                // Characters per line
    rows: usize,                // Visible rows on screen

    // FRAMEBUFFER
    fb_addr: *mut u8,
    pitch: usize,
    fb_width: usize,
    fb_height: usize,

    // RENDERING
    font: &'a PsfFont<'a>,
    line_height: usize,
    char_width: usize,

    // COLORS
    fg_color: u32,
    bg_color: u32,

    // MARGINS
    left_margin: usize,
    top_margin: usize,
    line_spacing: usize,
}

impl<'a> ScrollingTextRenderer<'a> {
    /// Create a new character-buffered console (NO ALLOCATION)
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
        let actual_line_height = if line_height == 0 { 16 } else { line_height };
        let line_spacing = 2;
        let line_stride = actual_line_height + line_spacing;

        // Calculate display dimensions
        let available_height = if fb_height > top_margin { fb_height - top_margin } else { 0 };
        let rows = if line_stride > 0 { available_height / line_stride } else { 0 };

        let char_width = 8; // Typical PSF font width
        let available_width = if fb_width > left_margin { fb_width - left_margin } else { 0 };
        let cols = if char_width > 0 { available_width / char_width } else { 80 };

        let bg_color = 0x00000000;

        // Allocate lines buffer on HEAP to avoid stack overflow during early boot
        // The allocator MUST be initialized before calling this function!
        let mut lines = Box::new([ConsoleLine::new(bg_color); MAX_LINES]);
        for line in lines.iter_mut() {
            line.set_width(cols.min(MAX_COLS));
        }
        // Leak the box to get a 'static reference (never freed, lives forever)
        let lines: &'static mut [ConsoleLine; MAX_LINES] = Box::leak(lines);

        // Start with one visible screen of lines
        let initial_lines = rows.min(MAX_LINES);

        Self {
            lines,
            line_count: initial_lines,
            start_line: 0,
            visible_lines: rows.min(MAX_LINES),
            cursor_col: 0,
            cursor_line: 0,
            cols: cols.min(MAX_COLS),
            rows: rows.min(MAX_LINES),
            fb_addr,
            pitch,
            fb_width,
            fb_height,
            font,
            line_height: actual_line_height,
            char_width,
            fg_color: 0xFFFFFFFF,
            bg_color,
            left_margin,
            top_margin,
            line_spacing,
        }
    }

    /// Get the physical index in the ring buffer
    fn physical_index(&self, logical_line: usize) -> usize {
        (self.start_line + logical_line) % MAX_LINES
    }

    /// Get a line by logical index
    fn get_line(&self, logical_line: usize) -> Option<&ConsoleLine> {
        if logical_line < self.line_count {
            Some(&self.lines[self.physical_index(logical_line)])
        } else {
            None
        }
    }

    /// Get a mutable line by logical index
    fn get_line_mut(&mut self, logical_line: usize) -> Option<&mut ConsoleLine> {
        if logical_line < self.line_count {
            let idx = self.physical_index(logical_line);
            Some(&mut self.lines[idx])
        } else {
            None
        }
    }

    /// Write a single character to the buffer
    pub fn write_char(&mut self, ch: u8) {
        match ch {
            b'\n' => {
                // Move to next line
                self.cursor_col = 0;
                self.cursor_line += 1;

                // If we've filled the buffer, scroll
                if self.cursor_line >= self.line_count {
                    self.scroll_up();
                }
            }
            b'\r' => {
                self.cursor_col = 0;
            }
            b'\t' => {
                // Tab = 4 spaces
                for _ in 0..4 {
                    self.write_char(b' ');
                }
            }
            _ => {
                // Normal character - capture values before borrowing
                let fg = self.fg_color;
                let bg = self.bg_color;
                let col = self.cursor_col;
                let cols = self.cols;

                if let Some(line) = self.get_line_mut(self.cursor_line) {
                    let console_char = ConsoleChar::new(ch, fg, bg);
                    line.set_char(col, console_char);
                }

                self.cursor_col += 1;

                // Wrap to next line if needed
                if self.cursor_col >= cols {
                    self.cursor_col = 0;
                    self.cursor_line += 1;

                    if self.cursor_line >= self.line_count {
                        self.scroll_up();
                    }
                }
            }
        }
    }

    /// Write multiple bytes to the buffer
    pub fn write_text(&mut self, text: &[u8]) {
        for &byte in text {
            self.write_char(byte);
        }
        self.render_dirty();
    }

    /// Scroll up by one line (instant operation - just move pointer!)
    pub fn scroll_up(&mut self) {
        if self.line_count < MAX_LINES {
            // Buffer not full yet, just add a new line
            self.line_count += 1;
            let new_line_idx = self.physical_index(self.line_count - 1);
            self.lines[new_line_idx].clear(self.bg_color);
            self.cursor_line = self.line_count - 1;
        } else {
            // Buffer is full, use ring buffer
            // Clear the line we're about to reuse
            let old_start = self.start_line;
            self.lines[old_start].clear(self.bg_color);

            // Move start pointer forward
            self.start_line = (self.start_line + 1) % MAX_LINES;
            self.cursor_line = self.line_count - 1;
        }

        // Mark all visible lines dirty for re-render
        let visible_count = self.visible_lines.min(self.line_count);
        let display_start = if self.line_count > visible_count {
            self.line_count - visible_count
        } else {
            0
        };

        for i in display_start..self.line_count {
            if let Some(line) = self.get_line_mut(i) {
                line.mark_dirty();
            }
        }
    }

    /// Render only the lines that have been modified
    pub fn render_dirty(&mut self) {
        let visible_count = self.visible_lines.min(self.line_count);
        let display_start = if self.line_count > visible_count {
            self.line_count - visible_count
        } else {
            0
        };

        for (screen_row, logical_line) in (display_start..self.line_count).enumerate() {
            // Copy the line first (it's Copy), then render it
            let line_copy = if let Some(line) = self.get_line(logical_line) {
                if line.is_dirty() {
                    Some(*line)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(line) = line_copy {
                self.render_line(screen_row, &line);
                // Mark the original line as clean
                if let Some(line_mut) = self.get_line_mut(logical_line) {
                    line_mut.mark_clean();
                }
            }
        }
    }

    /// Render a single line to the framebuffer
    fn render_line(&self, screen_row: usize, line: &ConsoleLine) {
        let y = self.top_margin + screen_row * (self.line_height + self.line_spacing);

        // Clear line background
        for py in 0..self.line_height {
            let row_y = y + py;
            if row_y >= self.fb_height {
                break;
            }

            let row_offset = row_y * self.pitch;
            for px in 0..self.cols * self.char_width {
                let x = self.left_margin + px;
                if x >= self.fb_width {
                    break;
                }

                let pixel_offset = row_offset + x * 4;
                unsafe {
                    ptr::write_volatile(
                        (self.fb_addr as usize + pixel_offset) as *mut u32,
                        self.bg_color,
                    );
                }
            }
        }

        // Render each character
        for col in 0..self.cols {
            if let Some(console_char) = line.get_char(col) {
                if console_char.ch != b' ' {
                    let x = self.left_margin + col * self.char_width;
                    self.draw_char(x, y, console_char.ch, console_char.fg_color);
                }
            }
        }
    }

    /// Draw a single character
    fn draw_char(&self, x: usize, y: usize, ch: u8, color: u32) {
        let glyph = self.font.glyph(ch);
        let glyph_height = self.font.charsize;
        let glyph_width = self.char_width;

        for gy in 0..glyph_height {
            if y + gy >= self.fb_height {
                break;
            }

            let row_offset = (y + gy) * self.pitch;
            let byte = glyph[gy];

            for gx in 0..glyph_width {
                if x + gx >= self.fb_width {
                    break;
                }

                let bit = (byte >> (7 - gx)) & 1;
                if bit != 0 {
                    let pixel_offset = row_offset + (x + gx) * 4;
                    unsafe {
                        ptr::write_volatile(
                            (self.fb_addr as usize + pixel_offset) as *mut u32,
                            color,
                        );
                    }
                }
            }
        }
    }

    // Utility methods for compatibility
    pub fn get_pitch(&self) -> usize {
        self.pitch
    }

    pub fn get_fb_width(&self) -> usize {
        self.fb_width
    }

    pub fn get_fb_height(&self) -> usize {
        self.fb_height
    }

    pub fn get_fb_addr(&self) -> *mut u8 {
        self.fb_addr
    }

    pub fn set_colors(&mut self, fg: u32, bg: u32) {
        self.fg_color = fg;
        self.bg_color = bg;
    }

    pub fn set_cursor(&mut self, col: usize, line: usize) {
        // Clamp to valid ranges to prevent out-of-bounds access
        self.cursor_col = col.min(self.cols.saturating_sub(1));
        self.cursor_line = line.min(self.line_count.saturating_sub(1));
    }

    pub fn write_line(&mut self, text: &[u8]) {
        self.write_text(text);
        self.write_char(b'\n');
    }

    /// Create a canvas for inline drawing
    pub fn create_canvas(&mut self, width: usize, height: usize) -> Canvas {
        let y = self.top_margin
            + self.cursor_line * (self.line_height + self.line_spacing)
            + self.cursor_col * self.char_width / self.cols;

        Canvas {
            fb_addr: self.fb_addr,
            pitch: self.pitch,
            x: self.left_margin,
            y,
            width: width.min(self.fb_width - self.left_margin),
            height: height.min(self.fb_height - y),
        }
    }
}

/// Inline canvas for drawing graphics
pub struct Canvas {
    fb_addr: *mut u8,
    pitch: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl Canvas {
    pub fn draw_pixel(&self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let fb_x = self.x + x;
            let fb_y = self.y + y;
            let offset = fb_y * self.pitch + fb_x * 4;
            unsafe {
                ptr::write_volatile((self.fb_addr as usize + offset) as *mut u32, color);
            }
        }
    }

    pub fn draw_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.draw_pixel(x + dx, y + dy, color);
            }
        }
    }

    pub fn fill(&self, color: u32) {
        self.draw_rect(0, 0, self.width, self.height, color);
    }
}

// GLOBAL RENDERER (static, no allocation!)
pub static mut GLOBAL_RENDERER: Option<ScrollingTextRenderer<'static>> = None;

/// Initialize the global renderer
pub fn init_global_renderer(renderer: ScrollingTextRenderer<'static>) {
    unsafe {
        GLOBAL_RENDERER = Some(renderer);
    }
}

/// Write to the global renderer
pub fn write_global(text: &[u8]) {
    unsafe {
        if let Some(ref mut renderer) = GLOBAL_RENDERER {
            renderer.write_text(text);
        }
    }
}

/// Draw a pixel buffer canvas using the global renderer
pub fn kdraw_canvas(pixels: &[u32], width: usize, height: usize) {
    unsafe {
        if let Some(ref mut renderer) = GLOBAL_RENDERER {
            let canvas = renderer.create_canvas(width, height);
            for y in 0..height.min(canvas.height) {
                for x in 0..width.min(canvas.width) {
                    let pixel = pixels[y * width + x];
                    canvas.draw_pixel(x, y, pixel);
                }
            }
            // Account for the drawn canvas in cursor positioning
            let lines_used = (height + renderer.line_height - 1) / renderer.line_height;
            renderer.cursor_line += lines_used;
            if renderer.cursor_line >= renderer.line_count {
                for _ in 0..(renderer.cursor_line - renderer.line_count + 1) {
                    renderer.scroll_up();
                }
            }
        }
    }
}

/// Helper struct for formatting text before writing
pub struct LineWriter {
    buffer: [u8; 512],
    pos: usize,
}

impl LineWriter {
    pub fn new() -> Self {
        Self {
            buffer: [0; 512],
            pos: 0,
        }
    }

    pub fn finish(&self) -> &[u8] {
        &self.buffer[..self.pos]
    }

    pub fn write_line(&mut self) {
        if self.pos < self.buffer.len() {
            self.buffer[self.pos] = b'\n';
            self.pos += 1;
        }
        write_global(&self.buffer[..self.pos]);
    }
}

impl core::fmt::Write for LineWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buffer.len() - self.pos;
        let to_copy = bytes.len().min(remaining);
        self.buffer[self.pos..self.pos + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.pos += to_copy;
        Ok(())
    }
}

/// Print with newline
#[macro_export]
macro_rules! kprintln {
    ($($arg:tt)*) => {{
        {
            //for _ in 1..10 {
            //    x86_64::instructions::hlt(); //delay
            //}
        }
        let mut writer = $crate::scrolling_text::LineWriter::new();
        use ::core::fmt::Write;
        let _ = ::core::writeln!(&mut writer, $($arg)*);
        $crate::scrolling_text::write_global(&writer.finish());
    }};
}

/// Print without newline
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        let mut writer = $crate::scrolling_text::LineWriter::new();
        use ::core::fmt::Write;
        let _ = ::core::write!(&mut writer, $($arg)*);
        $crate::scrolling_text::write_global(&writer.finish());
    }};
}

/// Alias for kprintln
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::kprintln!($($arg)*)
    };
}

/// Alias for kprint
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::kprint!($($arg)*)
    };
}
