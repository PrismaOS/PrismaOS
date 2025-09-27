//! Fast framebuffer console implementation
//!
//! This implements a high-performance console similar to Linux's fbcon,
//! using character cell buffering and batched rendering for optimal performance.

use core::ptr;
use alloc::boxed::Box;
use alloc::vec;
use crate::font::PsfFont;
use crate::memory::aligned::Aligned16;

/// Character cell representing one console character
#[derive(Clone, Copy, Debug)]
struct CharCell {
    character: u8,
    fg_color: u32,
    bg_color: u32,
}

impl Default for CharCell {
    fn default() -> Self {
        Self {
            character: b' ',
            fg_color: 0xFFFFFFFF, // White
            bg_color: 0x00000000, // Black
        }
    }
}

/// Pre-rendered glyph cache for fast character rendering
struct GlyphCache {
    /// Pre-rendered character bitmaps (256 chars * 16 rows * 8 cols)
    glyphs: Aligned16<[u8; 256 * 16 * 8]>,
    char_width: usize,
    char_height: usize,
}

impl GlyphCache {
    fn new(font: &PsfFont) -> Self {
        let mut cache = Self {
            glyphs: Aligned16::zeroed(),
            char_width: 8,
            char_height: font.charsize,
        };

        // Pre-render all 256 possible characters
        for c in 0..256 {
            let glyph = font.glyph(c as u8);
            let base_offset = c * 16 * 8;

            for row in 0..font.charsize.min(16) {
                let bits = glyph[row];
                for col in 0..8 {
                    let pixel_set = (bits >> (7 - col)) & 1 != 0;
                    let offset = base_offset + row * 8 + col;
                    cache.glyphs.get_mut()[offset] = if pixel_set { 1 } else { 0 };
                }
            }
        }

        cache
    }

    #[inline]
    fn get_glyph_pixel(&self, character: u8, row: usize, col: usize) -> bool {
        if row >= 16 || col >= 8 { return false; }
        let offset = (character as usize) * 16 * 8 + row * 8 + col;
        self.glyphs.get()[offset] != 0
    }
}

/// Dirty region tracker for optimized rendering
#[derive(Default)]
struct DirtyRegion {
    min_row: usize,
    max_row: usize,
    min_col: usize,
    max_col: usize,
    has_changes: bool,
}

impl DirtyRegion {
    fn mark_dirty(&mut self, row: usize, col: usize) {
        if !self.has_changes {
            self.min_row = row;
            self.max_row = row;
            self.min_col = col;
            self.max_col = col;
            self.has_changes = true;
        } else {
            self.min_row = self.min_row.min(row);
            self.max_row = self.max_row.max(row);
            self.min_col = self.min_col.min(col);
            self.max_col = self.max_col.max(col);
        }
    }

    fn mark_line_dirty(&mut self, row: usize, cols: usize) {
        self.mark_dirty(row, 0);
        if cols > 0 {
            self.mark_dirty(row, cols - 1);
        }
    }

    fn clear(&mut self) {
        self.has_changes = false;
    }
}

/// High-performance framebuffer console
pub struct FastConsole {
    // Hardware framebuffer
    fb_addr: *mut u8,
    fb_width: usize,
    fb_height: usize,
    fb_pitch: usize,

    // Character cell buffer
    char_buffer: Box<[CharCell]>,
    rows: usize,
    cols: usize,

    // Rendering state
    glyph_cache: GlyphCache,
    dirty_region: DirtyRegion,

    // Cursor position
    cursor_row: usize,
    cursor_col: usize,

    // Default colors
    default_fg: u32,
    default_bg: u32,
}

impl FastConsole {
    pub fn new(
        fb_addr: *mut u8,
        fb_width: usize,
        fb_height: usize,
        fb_pitch: usize,
        font: &PsfFont,
    ) -> Result<Self, &'static str> {
        if fb_addr.is_null() {
            return Err("Invalid framebuffer address");
        }

        let char_width = 8;
        let char_height = font.charsize;
        let cols = fb_width / char_width;
        let rows = fb_height / char_height;

        if cols == 0 || rows == 0 {
            return Err("Framebuffer too small for console");
        }

        let char_buffer = vec![CharCell::default(); rows * cols].into_boxed_slice();
        let glyph_cache = GlyphCache::new(font);

        let mut console = Self {
            fb_addr,
            fb_width,
            fb_height,
            fb_pitch,
            char_buffer,
            rows,
            cols,
            glyph_cache,
            dirty_region: DirtyRegion::default(),
            cursor_row: 0,
            cursor_col: 0,
            default_fg: 0xFFFFFFFF,
            default_bg: 0x00000000,
        };

        // Initial clear
        console.clear();
        Ok(console)
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_char(byte);
        }
        // Batch render all changes
        self.flush();
    }

    pub fn write_char(&mut self, c: u8) {
        match c {
            b'\n' => self.newline(),
            b'\r' => self.cursor_col = 0,
            b'\t' => {
                let spaces = 8 - (self.cursor_col % 8);
                for _ in 0..spaces {
                    self.put_char(b' ');
                }
            },
            _ => self.put_char(c),
        }
    }

    fn put_char(&mut self, c: u8) {
        if self.cursor_col >= self.cols {
            self.newline();
        }

        let index = self.cursor_row * self.cols + self.cursor_col;
        self.char_buffer[index] = CharCell {
            character: c,
            fg_color: self.default_fg,
            bg_color: self.default_bg,
        };

        self.dirty_region.mark_dirty(self.cursor_row, self.cursor_col);
        self.cursor_col += 1;
    }

    fn newline(&mut self) {
        self.cursor_col = 0;
        self.cursor_row += 1;

        if self.cursor_row >= self.rows {
            self.scroll_up();
            self.cursor_row = self.rows - 1;
        }
    }

    fn scroll_up(&mut self) {
        // Move character buffer up by one line
        let chars_per_line = self.cols;
        let src_start = chars_per_line;
        let dst_start = 0;
        let copy_lines = self.rows - 1;
        let copy_chars = copy_lines * chars_per_line;

        // Fast memory move
        self.char_buffer.copy_within(src_start..src_start + copy_chars, dst_start);

        // Clear bottom line
        let bottom_line_start = copy_lines * chars_per_line;
        for i in bottom_line_start..self.char_buffer.len() {
            self.char_buffer[i] = CharCell::default();
        }

        // Mark entire screen dirty for scroll
        self.dirty_region.min_row = 0;
        self.dirty_region.max_row = self.rows - 1;
        self.dirty_region.min_col = 0;
        self.dirty_region.max_col = self.cols - 1;
        self.dirty_region.has_changes = true;
    }

    pub fn clear(&mut self) {
        for cell in self.char_buffer.iter_mut() {
            *cell = CharCell::default();
        }
        self.cursor_row = 0;
        self.cursor_col = 0;

        // Mark entire screen dirty
        self.dirty_region.min_row = 0;
        self.dirty_region.max_row = self.rows - 1;
        self.dirty_region.min_col = 0;
        self.dirty_region.max_col = self.cols - 1;
        self.dirty_region.has_changes = true;
    }

    /// Render all dirty regions to the framebuffer
    pub fn flush(&mut self) {
        if !self.dirty_region.has_changes {
            return;
        }

        // Render only the dirty region
        for row in self.dirty_region.min_row..=self.dirty_region.max_row.min(self.rows - 1) {
            for col in self.dirty_region.min_col..=self.dirty_region.max_col.min(self.cols - 1) {
                self.render_char(row, col);
            }
        }

        self.dirty_region.clear();
    }

    /// Render a single character cell to the framebuffer (optimized)
    #[inline]
    fn render_char(&self, row: usize, col: usize) {
        let cell_index = row * self.cols + col;
        let cell = &self.char_buffer[cell_index];

        let char_width = 8;
        let char_height = self.glyph_cache.char_height;
        let fb_x = col * char_width;
        let fb_y = row * char_height;

        // Bounds check
        if fb_x + char_width > self.fb_width || fb_y + char_height > self.fb_height {
            return;
        }

        // Fast character rendering using pre-cached glyphs
        for char_row in 0..char_height {
            let fb_row_y = fb_y + char_row;
            let row_addr = unsafe { self.fb_addr.add(fb_row_y * self.fb_pitch + fb_x * 4) };

            // Render 8 pixels at once as u32 array
            let pixel_row = unsafe { core::slice::from_raw_parts_mut(row_addr as *mut u32, char_width) };

            for char_col in 0..char_width {
                let pixel_set = self.glyph_cache.get_glyph_pixel(cell.character, char_row, char_col);
                pixel_row[char_col] = if pixel_set { cell.fg_color } else { cell.bg_color };
            }
        }
    }
}

/// Global fast console instance
static mut FAST_CONSOLE: Option<FastConsole> = None;

/// Initialize the global fast console
pub fn init_fast_console(
    fb_addr: *mut u8,
    fb_width: usize,
    fb_height: usize,
    fb_pitch: usize,
    font: &PsfFont,
) -> Result<(), &'static str> {
    let console = FastConsole::new(fb_addr, fb_width, fb_height, fb_pitch, font)?;
    unsafe {
        FAST_CONSOLE = Some(console);
    }
    Ok(())
}

/// Write a string to the fast console
pub fn fast_console_write(s: &str) {
    unsafe {
        if let Some(console) = &mut FAST_CONSOLE {
            console.write_string(s);
        }
    }
}

/// Fast kernel print macros
#[macro_export]
macro_rules! fast_kprint {
    ($($arg:tt)*) => {
        {
            use alloc::format;
            let s = format!($($arg)*);
            $crate::fast_console::fast_console_write(&s);
        }
    };
}

#[macro_export]
macro_rules! fast_kprintln {
    () => { $crate::fast_kprint!("\n") };
    ($($arg:tt)*) => {
        {
            use alloc::format;
            let s = format!($($arg)*);
            $crate::fast_console::fast_console_write(&s);
            $crate::fast_console::fast_console_write("\n");
        }
    };
}