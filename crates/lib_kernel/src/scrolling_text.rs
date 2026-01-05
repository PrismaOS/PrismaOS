#![no_std]
extern crate core;
extern crate alloc;

use core::ptr;
use core::cmp;
use core::fmt::Write;
use alloc::string::{String, ToString};
use alloc::format;

use crate::font::{draw_string, PsfFont};

/// Simple scrolling text renderer for a linear framebuffer.
/// - Uses draw_string from font.rs to render lines.
/// - Abstracts line/char spacing and basic scrolling behavior.
/// - Designed for kernels / no_std environments.
/// - Uses ring buffer approach for efficient scrolling without copying entire framebuffer
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

    // Ring buffer for efficient scrolling
    // Current logical line we're writing to (0 to total_lines-1)
    current_logical_line: usize,
    // Index of the first physical line that maps to logical line 0
    first_line_index: usize,
    // Total number of lines that fit in the framebuffer
    total_lines: usize,
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
        let actual_line_height = if line_height == 0 { 8 } else { line_height };
        let default_line_spacing = 2;

        // Calculate total lines that fit in the framebuffer
        // Each line occupies line_height + line_spacing pixels
        let available_height = if fb_height > top_margin { fb_height - top_margin } else { 0 };
        let line_total_height = actual_line_height + default_line_spacing;
        let total_lines = if line_total_height > 0 { available_height / line_total_height } else { 0 };

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
            line_height: actual_line_height,
            left_margin,
            top_margin,
            cursor_x,
            cursor_y,
            current_logical_line: 0,
            first_line_index: 0,
            total_lines,
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
    pub fn clear(&mut self) {
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

        // Reset ring buffer state
        self.current_logical_line = 0;
        self.first_line_index = 0;
    }

    /// Convert a logical line index to physical Y coordinate
    /// This handles the ring buffer wraparound
    fn logical_line_to_y(&self, logical_line: usize) -> usize {
        if self.total_lines == 0 {
            return self.top_margin;
        }
        let physical_line = (self.first_line_index + logical_line) % self.total_lines;
        let line_stride = self.line_height + self.line_spacing;
        self.top_margin + (physical_line * line_stride)
    }

    /// Get the current logical line number based on cursor_y
    fn y_to_logical_line(&self, y: usize) -> usize {
        if y < self.top_margin {
            return 0;
        }
        let line_stride = self.line_height + self.line_spacing;
        (y - self.top_margin) / line_stride
    }

    /// Clear a specific line in the framebuffer (by physical line index)
    /// Clears the full line height including spacing to remove all old content
    fn clear_physical_line(&self, physical_line: usize) {
        if self.fb_addr.is_null() || self.pitch == 0 || self.total_lines == 0 {
            return;
        }

        let line_stride = self.line_height + self.line_spacing;
        let y_start = self.top_margin + (physical_line * line_stride);
        // Clear the full line including spacing between lines
        let y_end = y_start + line_stride;

        let bytes_per_pixel = 4usize;
        let stride = self.pitch;

        for y in y_start..y_end {
            if y >= self.fb_height {
                break;
            }
            let row_base = unsafe { self.fb_addr.add(y * stride) };
            for x in 0..self.fb_width {
                let pixel_ptr = unsafe { row_base.add(x * bytes_per_pixel).cast::<u32>() };
                unsafe { pixel_ptr.write_volatile(self.bg_color) };
            }
        }
    }

    /// Internal: scroll framebuffer up by `pixels` vertical pixels.
    /// Uses ring buffer approach: instead of copying all framebuffer data,
    /// we just clear the top line and reuse it as the bottom line.
    fn scroll_up(&mut self, pixels: usize) {
        if self.fb_addr.is_null() || self.pitch == 0 || pixels == 0 || self.total_lines == 0 {
            return;
        }

        // Calculate how many lines to scroll
        let lines_to_scroll = (pixels + self.line_height - 1) / self.line_height;

        if lines_to_scroll >= self.total_lines {
            // Scrolling more than the entire screen - just clear everything
            self.clear();
            self.cursor_y = self.top_margin;
            self.first_line_index = 0;
            return;
        }

        // Ring buffer magic: for each line to scroll
        for _ in 0..lines_to_scroll {
            // Clear the physical line at first_line_index (the current top line)
            // This line will be reused as the new bottom line
            self.clear_physical_line(self.first_line_index);

            // Move the ring buffer forward
            // The line we just cleared is now logically at the bottom
            self.first_line_index = (self.first_line_index + 1) % self.total_lines;
        }

        // Don't change cursor_y - it should stay at the last line position
        // The physical line it points to is now a different logical line,
        // but that's the whole point of the ring buffer!
    }

    /// Write a single line (no newline handling). Draws the provided bytes
    /// at the current cursor (cursor_x, cursor_y). Advances cursor to next line.
    pub fn write_line(&mut self, line: &[u8]) {
        if self.fb_addr.is_null() || self.total_lines == 0 {
            return;
        }

        let line_stride = self.line_height + self.line_spacing;

        // Check if we need to scroll
        if self.current_logical_line >= self.total_lines {
            // Scroll: clear the top line and move first_line_index forward
            self.clear_physical_line(self.first_line_index);
            self.first_line_index = (self.first_line_index + 1) % self.total_lines;
            // Reset to last logical line
            self.current_logical_line = self.total_lines - 1;
        }

        // Convert logical line to physical line
        let physical_line = (self.first_line_index + self.current_logical_line) % self.total_lines;

        // Clear this physical line before writing
        self.clear_physical_line(physical_line);

        // Calculate physical Y position
        self.cursor_y = self.top_margin + (physical_line * line_stride);

        // Render using draw_string from font.rs
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

        // Advance to next logical line
        self.current_logical_line += 1;
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
        // Also reset ring buffer state
        self.current_logical_line = 0;
        self.first_line_index = 0;
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

            // After scrolling, reposition cursor to account for ring buffer
            if self.total_lines > 0 {
                let bottom_physical_line = (self.first_line_index + self.total_lines - 1) % self.total_lines;
                let line_stride = self.line_height + self.line_spacing;
                let bottom_y = self.top_margin + (bottom_physical_line * line_stride);
                // Adjust cursor_y to maintain relative position
                self.cursor_y = bottom_y - (overflow - line_stride);
                if self.cursor_y < self.top_margin {
                    self.cursor_y = self.top_margin;
                }
            }
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

        // Update logical line tracking to stay in sync
        let line_stride = self.line_height + self.line_spacing;
        if line_stride > 0 && self.cursor_y >= self.top_margin {
            let lines_consumed = (rows_to_draw + self.line_spacing + line_stride - 1) / line_stride;
            self.current_logical_line += lines_consumed;
        }
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

            // After scrolling, reposition cursor to account for ring buffer
            if self.total_lines > 0 {
                let bottom_physical_line = (self.first_line_index + self.total_lines - 1) % self.total_lines;
                let line_stride = self.line_height + self.line_spacing;
                let bottom_y = self.top_margin + (bottom_physical_line * line_stride);
                // Adjust cursor_y to maintain relative position
                self.cursor_y = bottom_y - (overflow - line_stride);
                if self.cursor_y < self.top_margin {
                    self.cursor_y = self.top_margin;
                }
            }
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

            // Update logical line tracking
            let line_stride = self.line_height + self.line_spacing;
            if line_stride > 0 && self.cursor_y >= self.top_margin {
                let lines_consumed = (rows_to_draw + self.line_spacing + line_stride - 1) / line_stride;
                self.current_logical_line += lines_consumed;
            }
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

        // Update logical line tracking to stay in sync
        let line_stride = self.line_height + self.line_spacing;
        if line_stride > 0 && self.cursor_y >= self.top_margin {
            let lines_consumed = (rows_to_draw + self.line_spacing + line_stride - 1) / line_stride;
            self.current_logical_line += lines_consumed;
        }
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
            let mut writer = $crate::scrolling_text::LineWriter::new();
            let _ = ::core::fmt::Write::write_fmt(&mut writer, format_args!($($arg)*));
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

/// Blocking interactive user prompt using keyboard driver and text rendering
/// Returns the user's input as a String when they press Enter
/// This version polls the keyboard directly without async/await
pub fn interactive_prompt_blocking(prompt_text: &str, max_length: usize) -> String {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use crate::api::commands::inb;
    
    // Display the prompt
    kprint!("{}", prompt_text);
    
    // Set up keyboard processing
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(), 
        layouts::Us104Key, 
        HandleControl::Ignore
    );
    
    // Use a fixed-size buffer to avoid heap allocation
    let mut input_buffer = [0u8; 256]; // Fixed buffer
    let mut buffer_pos = 0;
    let max_len = max_length.min(255); // Leave room for null terminator
    
    // Input loop - polls keyboard hardware directly
    loop {
        // Poll keyboard hardware directly
        unsafe {
            // Check if keyboard has data
            if (inb(0x64) & 0x01) != 0 {
                let scancode = inb(0x60);
                
                if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                    if let Some(key) = keyboard.process_keyevent(key_event) {
                        match key {
                            DecodedKey::Unicode(character) => {
                                match character {
                                    '\n' | '\r' => {
                                        // Enter pressed - finish input
                                        kprintln!(); // Move to next line
                                        // Convert buffer to String only at the end
                                        let result_str = core::str::from_utf8(&input_buffer[..buffer_pos])
                                            .unwrap_or("")
                                            .to_string();
                                        return result_str;
                                    }
                                    '\x08' => {
                                        // Backspace
                                        if buffer_pos > 0 {
                                            buffer_pos -= 1;
                                            // Clear and redraw line
                                            kprint!("\r{}", prompt_text);
                                            let current_str = core::str::from_utf8(&input_buffer[..buffer_pos])
                                                .unwrap_or("");
                                            kprint!("{} \r{}{}", current_str, prompt_text, current_str);
                                        }
                                    }
                                    '\t' => {
                                        // Tab - convert to spaces
                                        if buffer_pos + 4 <= max_len {
                                            for _ in 0..4 {
                                                if buffer_pos < max_len {
                                                    input_buffer[buffer_pos] = b' ';
                                                    buffer_pos += 1;
                                                }
                                            }
                                            kprint!("    ");
                                        }
                                    }
                                    c if c.is_ascii() && !c.is_control() => {
                                        // Regular character
                                        if buffer_pos < max_len {
                                            input_buffer[buffer_pos] = c as u8;
                                            buffer_pos += 1;
                                            kprint!("{}", c);
                                        }
                                    }
                                    _ => {
                                        // Ignore other characters
                                    }
                                }
                            }
                            DecodedKey::RawKey(_) => {
                                // Ignore raw keys for now
                            }
                        }
                    }
                }
            }
        }
        
        // Small delay to avoid spinning too fast
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
    }
}


/// Interactive user prompt using keyboard driver and text rendering
/// Returns the user's input as a String when they press Enter
pub async fn interactive_prompt(prompt_text: &str, max_length: usize) -> String {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use futures_util::stream::StreamExt;
    
    // Display the prompt
    kprint!("{}", prompt_text);
    
    // Set up keyboard processing
    let mut scancodes = crate::executor::keyboard::ScancodeStream::new();
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(), 
        layouts::Us104Key, 
        HandleControl::Ignore
    );
    
    let mut input_buffer = String::new();
    
    // Input loop
    loop {
        // Wait for keyboard input
        if let Some(scancode) = scancodes.next().await {
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => {
                            match character {
                                '\n' | '\r' => {
                                    // Enter pressed - finish input
                                    kprintln!(); // Move to next line
                                    return input_buffer;
                                }
                                '\x08' => {
                                    // Backspace
                                    if !input_buffer.is_empty() {
                                        input_buffer.pop();
                                        // Clear and redraw line
                                        kprint!("\r{}{}", prompt_text, input_buffer);
                                        kprint!(" \r{}{}", prompt_text, input_buffer); // Clear extra char
                                    }
                                }
                                '\t' => {
                                    // Tab - convert to spaces
                                    if input_buffer.len() + 4 <= max_length {
                                        input_buffer.push_str("    ");
                                        kprint!("    ");
                                    }
                                }
                                c if c.is_ascii_graphic() || c == ' ' => {
                                    // Printable character
                                    if input_buffer.len() < max_length {
                                        input_buffer.push(c);
                                        kprint!("{}", c);
                                    }
                                }
                                _ => {
                                    // Ignore other characters
                                }
                            }
                        }
                        DecodedKey::RawKey(raw_key) => {
                            // Handle special keys
                            match raw_key {
                                pc_keyboard::KeyCode::Escape => {
                                    // ESC pressed - cancel input
                                    kprintln!("\n[CANCELLED]");
                                    return String::new();
                                }
                                pc_keyboard::KeyCode::F1 => {
                                    // F1 - show help
                                    show_prompt_help();
                                    kprint!("{}{}", prompt_text, input_buffer); // Redraw prompt
                                }
                                _ => {
                                    // Ignore other raw keys
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Show help for the interactive prompt
fn show_prompt_help() {
    kprintln!();
    kprintln!("📋 Interactive Prompt Help:");
    kprintln!("  • Type your input normally");
    kprintln!("  • Press Enter to submit");
    kprintln!("  • Press Backspace to delete");
    kprintln!("  • Press Tab for 4 spaces");
    kprintln!("  • Press ESC to cancel");
    kprintln!("  • Press F1 for this help");
    kprintln!();
}

/// Simple user prompt for yes/no questions
pub async fn prompt_yes_no(question: &str) -> bool {
    loop {
        let response = interactive_prompt(&format!("{} (y/n): ", question), 10).await;
        let response = response.trim().to_lowercase();
        
        match response.as_str() {
            "y" | "yes" | "1" | "true" => return true,
            "n" | "no" | "0" | "false" => return false,
            "" => return false, // Default to no
            _ => {
                kprintln!("Please enter 'y' for yes or 'n' for no.");
            }
        }
    }
}

/// Prompt for a number within a range
pub async fn prompt_number(question: &str, min: i32, max: i32) -> i32 {
    loop {
        let prompt = format!("{} ({}-{}): ", question, min, max);
        let response = interactive_prompt(&prompt, 10).await;
        
        if let Ok(num) = response.trim().parse::<i32>() {
            if num >= min && num <= max {
                return num;
            } else {
                kprintln!("Number must be between {} and {}.", min, max);
            }
        } else {
            kprintln!("Please enter a valid number.");
        }
    }
}

/// Interactive menu selection
pub async fn interactive_menu(title: &str, options: &[&str]) -> usize {
    loop {
        kprintln!();
        kprintln!("📋 {}", title);
        kprintln!("{}", "═".repeat(title.len() + 4));
        
        for (i, option) in options.iter().enumerate() {
            kprintln!("  {}. {}", i + 1, option);
        }
        kprintln!();
        
        let choice = prompt_number(
            "Select an option", 
            1, 
            options.len() as i32
        ).await;
        
        return (choice - 1) as usize;
    }
}

/// Demonstration function showcasing the interactive prompt system
pub async fn demo_interactive_system() {
    kprintln!();
    kprintln!("🚀 Interactive System Demo");
    kprintln!("==========================");
    kprintln!();
    
    // Simple text input
    let name = interactive_prompt("What's your name? ", 50).await;
    if name.is_empty() {
        kprintln!("Hello, Anonymous!");
    } else {
        kprintln!("Hello, {}!", name);
    }
    
    // Yes/No question
    let likes_rust = prompt_yes_no("Do you like Rust programming").await;
    if likes_rust {
        kprintln!("Great! Rust is awesome for OS development! 🦀");
    } else {
        kprintln!("That's okay, maybe you'll learn to love it!");
    }
    
    // Number input
    let age = prompt_number("What's your age", 1, 150).await;
    kprintln!("Age {} is a great age for learning OS development!", age);
    
    // Menu selection
    let favorite_color = interactive_menu(
        "What's your favorite color?",
        &["Red", "Green", "Blue", "Yellow", "Purple", "Orange"]
    ).await;
    
    let colors = ["Red", "Green", "Blue", "Yellow", "Purple", "Orange"];
    kprintln!("Excellent choice! {} is a beautiful color.", colors[favorite_color]);
    
    // Final message
    kprintln!();
    kprintln!("🎉 Demo complete! The interactive keyboard system is working!");
    kprintln!("✨ Features demonstrated:");
    kprintln!("  • Text input with character processing");
    kprintln!("  • Backspace and special key handling");
    kprintln!("  • Input validation and prompts");
    kprintln!("  • Menu system with numbered options");
    kprintln!("  • Yes/No prompts with multiple valid inputs");
    kprintln!();
}

/// Test keyboard driver functionality
async fn test_keyboard_driver() {
    kprintln!();
    kprintln!("⌨️  Keyboard Driver Test");
    kprintln!("========================");
    kprintln!("Type some text to test the keyboard driver.");
    kprintln!("Press ESC when done, or type 'done' and press Enter.");
    kprintln!();
    
    let result = interactive_prompt("Test input: ", 200).await;
    
    if result.is_empty() {
        kprintln!("Test cancelled.");
    } else {
        kprintln!();
        kprintln!("✅ Keyboard test successful!");
        kprintln!("You typed: '{}'", result);
        kprintln!("Length: {} characters", result.len());
        
        // Character analysis
        let alphabetic = result.chars().filter(|c| c.is_alphabetic()).count();
        let numeric = result.chars().filter(|c| c.is_numeric()).count();
        let spaces = result.chars().filter(|c| c.is_whitespace()).count();
        let punctuation = result.len() - alphabetic - numeric - spaces;
        
        kprintln!("Analysis:");
        kprintln!("  • Alphabetic: {}", alphabetic);
        kprintln!("  • Numeric: {}", numeric);
        kprintln!("  • Spaces: {}", spaces);
        kprintln!("  • Other: {}", punctuation);
    }
    kprintln!();
}

/// Simple test function that can be called from main to demonstrate keyboard functionality
pub async fn test_interactive_keyboard() {
    kprintln!();
    kprintln!("🎯 Testing Interactive Keyboard System");
    kprintln!("======================================");
    kprintln!();
    kprintln!("This demonstrates the integration of:");
    kprintln!("  • PS/2 Keyboard Driver (hardware level)");
    kprintln!("  • Character processing and input handling");
    kprintln!("  • Text rendering and display system");
    kprintln!("  • Async keyboard event processing");
    kprintln!();
    
    // Simple test
    let test_input = interactive_prompt("Enter some text to test: ", 100).await;
    
    if test_input.is_empty() {
        kprintln!("❌ No input received (cancelled or empty)");
    } else {
        kprintln!("✅ Success! You entered: '{}'", test_input);
        kprintln!("   Length: {} characters", test_input.len());
        
        // Show character breakdown
        let uppercase = test_input.chars().filter(|c| c.is_uppercase()).count();
        let lowercase = test_input.chars().filter(|c| c.is_lowercase()).count();
        let digits = test_input.chars().filter(|c| c.is_numeric()).count();
        let spaces = test_input.chars().filter(|c| c.is_whitespace()).count();
        
        kprintln!("   Analysis: {} upper, {} lower, {} digits, {} spaces", 
                 uppercase, lowercase, digits, spaces);
    }
    
    kprintln!();
    kprintln!("🎉 Keyboard integration test complete!");
    kprintln!();
}