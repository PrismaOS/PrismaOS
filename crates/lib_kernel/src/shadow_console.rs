//! Shadow buffer console - Linux fbcon-style implementation
//!
//! Uses a static character cell buffer in RAM and only updates the
//! framebuffer when necessary, avoiding slow framebuffer writes.

use crate::font::PsfFont;
use core::ptr;

const MAX_ROWS: usize = 60;
const MAX_COLS: usize = 120;

#[derive(Copy, Clone)]
struct CharCell {
    ch: u8,
    dirty: bool,
}

impl CharCell {
    const fn new() -> Self {
        Self { ch: b' ', dirty: false }
    }
}

/// Shadow buffer console that minimizes framebuffer writes
pub struct ShadowConsole {
    // Character cell buffer in fast RAM
    cells: [[CharCell; MAX_COLS]; MAX_ROWS],

    // Display dimensions
    rows: usize,
    cols: usize,

    // Cursor position
    cursor_row: usize,
    cursor_col: usize,

    // Framebuffer info
    fb_addr: *mut u8,
    fb_pitch: usize,
    fb_width: usize,
    fb_height: usize,

    // Font
    font_charsize: usize,
    color: u32,
}

impl ShadowConsole {
    /// Create a new shadow console
    pub const fn new() -> Self {
        Self {
            cells: [[CharCell::new(); MAX_COLS]; MAX_ROWS],
            rows: 0,
            cols: 0,
            cursor_row: 0,
            cursor_col: 0,
            fb_addr: ptr::null_mut(),
            fb_pitch: 0,
            fb_width: 0,
            fb_height: 0,
            font_charsize: 16,
            color: 0xFFFFFFFF,
        }
    }

    /// Initialize the console with framebuffer parameters
    pub fn init(&mut self, fb_addr: *mut u8, fb_width: usize, fb_height: usize, fb_pitch: usize, font: &PsfFont) {
        self.fb_addr = fb_addr;
        self.fb_width = fb_width;
        self.fb_height = fb_height;
        self.fb_pitch = fb_pitch;
        self.font_charsize = font.charsize;

        // Calculate console dimensions
        self.cols = (fb_width / 8).min(MAX_COLS);
        self.rows = (fb_height / font.charsize).min(MAX_ROWS);

        // Clear framebuffer once
        self.clear_framebuffer();
    }

    /// Write a string to the console
    pub fn write_str(&mut self, s: &str, font: &PsfFont) {
        for byte in s.bytes() {
            self.write_byte(byte, font);
        }
    }

    /// Write a single byte
    fn write_byte(&mut self, byte: u8, font: &PsfFont) {
        match byte {
            b'\n' => self.newline(font),
            b'\r' => self.cursor_col = 0,
            b'\t' => {
                let spaces = 8 - (self.cursor_col % 8);
                for _ in 0..spaces {
                    self.put_char(b' ', font);
                }
            }
            _ => self.put_char(byte, font),
        }
    }

    /// Put a character at current cursor position
    fn put_char(&mut self, ch: u8, font: &PsfFont) {
        if self.cursor_col >= self.cols {
            self.newline(font);
        }

        // Update cell buffer
        self.cells[self.cursor_row][self.cursor_col] = CharCell { ch, dirty: true };

        // Immediately render this character to framebuffer
        unsafe {
            self.render_char(self.cursor_row, self.cursor_col, font);
        }

        self.cursor_col += 1;
    }

    /// Move to next line
    fn newline(&mut self, font: &PsfFont) {
        self.cursor_col = 0;
        self.cursor_row += 1;

        if self.cursor_row >= self.rows {
            self.scroll_up(font);
            self.cursor_row = self.rows - 1;
        }
    }

    /// Scroll the display up by one line
    fn scroll_up(&mut self, font: &PsfFont) {
        // Move cell buffer up
        for row in 1..self.rows {
            self.cells[row - 1] = self.cells[row];
        }

        // Clear bottom line in buffer
        for col in 0..self.cols {
            self.cells[self.rows - 1][col] = CharCell::new();
        }

        // Scroll framebuffer (bulk copy - fast!)
        unsafe {
            let line_height = self.font_charsize;
            let scroll_pixels = line_height;
            let copy_rows = self.fb_height - scroll_pixels;

            let src = self.fb_addr.add(scroll_pixels * self.fb_pitch);
            let dst = self.fb_addr;
            let copy_bytes = copy_rows * self.fb_pitch;

            ptr::copy(src, dst, copy_bytes);

            // Clear bottom line
            let clear_start = self.fb_addr.add(copy_rows * self.fb_pitch);
            let clear_bytes = scroll_pixels * self.fb_pitch;
            ptr::write_bytes(clear_start, 0, clear_bytes);
        }
    }

    /// Render a single character cell to framebuffer
    unsafe fn render_char(&self, row: usize, col: usize, font: &PsfFont) {
        if self.fb_addr.is_null() { return; }

        let cell = &self.cells[row][col];
        let glyph = font.glyph(cell.ch);

        let x = col * 8;
        let y = row * self.font_charsize;

        // Fast character rendering - only write foreground pixels
        for glyph_row in 0..self.font_charsize {
            let bits = glyph[glyph_row];
            if bits == 0 { continue; }

            let dst = self.fb_addr.add((y + glyph_row) * self.fb_pitch + x * 4) as *mut u32;

            if bits & 0x80 != 0 { *dst.add(0) = self.color; }
            if bits & 0x40 != 0 { *dst.add(1) = self.color; }
            if bits & 0x20 != 0 { *dst.add(2) = self.color; }
            if bits & 0x10 != 0 { *dst.add(3) = self.color; }
            if bits & 0x08 != 0 { *dst.add(4) = self.color; }
            if bits & 0x04 != 0 { *dst.add(5) = self.color; }
            if bits & 0x02 != 0 { *dst.add(6) = self.color; }
            if bits & 0x01 != 0 { *dst.add(7) = self.color; }
        }
    }

    /// Clear the framebuffer
    fn clear_framebuffer(&self) {
        if self.fb_addr.is_null() { return; }

        unsafe {
            let total_bytes = self.fb_height * self.fb_pitch;
            ptr::write_bytes(self.fb_addr, 0, total_bytes);
        }
    }
}

/// Global shadow console instance
static mut GLOBAL_SHADOW_CONSOLE: ShadowConsole = ShadowConsole::new();

/// Initialize the global shadow console
pub fn init_shadow_console(fb_addr: *mut u8, fb_width: usize, fb_height: usize, fb_pitch: usize, font: &PsfFont) {
    unsafe {
        GLOBAL_SHADOW_CONSOLE.init(fb_addr, fb_width, fb_height, fb_pitch, font);
    }
}

/// Write to the global shadow console
pub fn shadow_console_write(s: &str, font: &PsfFont) {
    unsafe {
        GLOBAL_SHADOW_CONSOLE.write_str(s, font);
    }
}
