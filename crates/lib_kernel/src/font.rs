use core::ptr;

#[link_section = ".rodata"]
#[no_mangle]
pub static FONT_PSF: [u8; include_bytes!("../../../assets/fonts/lat9-16.psf").len()] = *include_bytes!("../../../assets/fonts/lat9-16.psf");

#[derive(Clone, Copy)]
pub struct PsfFont<'a> {
    pub glyphs: &'a [u8],
    pub charsize: usize,
}

impl<'a> PsfFont<'a> {
    pub fn from_bytes(data: &'a [u8]) -> Option<Self> {
        // PSF1 header: 2 bytes magic, 1 byte mode, 1 byte charsize
        if data.len() < 4 || data[0] != 0x36 || data[1] != 0x04 {
            return None;
        }
        let charsize = data[3] as usize;
        // PSF1 fonts always have 256 glyphs
        let expected_size = 4 + 256 * charsize;
        if data.len() < expected_size {
            return None;
        }
        let glyphs = &data[4..(4 + 256 * charsize)];
        Some(PsfFont { glyphs, charsize })
    }

    pub fn glyph(&self, c: u8) -> &[u8] {
        let idx = c as usize * self.charsize;
        &self.glyphs[idx..idx + self.charsize]
    }
}

// Draw a single ASCII character at (x, y) in framebuffer with bounds checking
// Highly optimized: only writes foreground pixels, skips background entirely
pub unsafe fn draw_char(
    fb_addr: *mut u8,
    pitch: usize,
    x: usize,
    y: usize,
    color: u32,
    font: &PsfFont,
    c: u8,
    width: usize,
    height: usize,
) {
    let glyph = font.glyph(c);

    // Fast path: if character fits entirely within framebuffer bounds
    if x + 8 <= width && y + font.charsize <= height {
        // Process entire character without bounds checking each pixel
        for row in 0..font.charsize {
            let py = y + row;
            let bits = glyph[row];

            // Skip empty rows entirely
            if bits == 0 { continue; }

            let row_base = fb_addr.add(py * pitch + x * 4) as *mut u32;

            // Only write foreground pixels - skip background for massive speedup
            if (bits & 0x80) != 0 { *row_base.add(0) = color; }
            if (bits & 0x40) != 0 { *row_base.add(1) = color; }
            if (bits & 0x20) != 0 { *row_base.add(2) = color; }
            if (bits & 0x10) != 0 { *row_base.add(3) = color; }
            if (bits & 0x08) != 0 { *row_base.add(4) = color; }
            if (bits & 0x04) != 0 { *row_base.add(5) = color; }
            if (bits & 0x02) != 0 { *row_base.add(6) = color; }
            if (bits & 0x01) != 0 { *row_base.add(7) = color; }
        }
    } else {
        // Slow path: character near edge, need bounds checking
        for row in 0..font.charsize {
            let py = y + row;
            if py >= height { continue; }
            let bits = glyph[row];
            if bits == 0 { continue; }

            let row_base = fb_addr.add(py * pitch + x * 4) as *mut u32;

            for col in 0..8 {
                let px = x + col;
                if px >= width { continue; }
                if (bits >> (7 - col)) & 1 != 0 {
                    *row_base.add(col) = color;
                }
            }
        }
    }
}

// Draw a string at (x, y) with bounds checking
// Optimized: minimal branching, only writes foreground pixels
pub unsafe fn draw_string(
    fb_addr: *mut u8,
    pitch: usize,
    x: usize,
    y: usize,
    color: u32,
    font: &PsfFont,
    s: &[u8],
    width: usize,
    height: usize,
) {
    if s.is_empty() { return; }

    let charsize = font.charsize;

    // Fast path: string fits entirely within bounds
    if y + charsize <= height && x + s.len() * 8 <= width {
        // Process each character
        for (char_idx, &ch) in s.iter().enumerate() {
            let char_x = x + char_idx * 8;
            let glyph = font.glyph(ch);

            // Draw each row of the character
            for row in 0..charsize {
                let bits = glyph[row];
                if bits == 0 { continue; } // Skip empty rows

                let dst = fb_addr.add((y + row) * pitch + char_x * 4) as *mut u32;

                // Unrolled, branchless pixel writes
                // Only write if bit is set (no background writes)
                if bits & 0x80 != 0 { *dst.add(0) = color; }
                if bits & 0x40 != 0 { *dst.add(1) = color; }
                if bits & 0x20 != 0 { *dst.add(2) = color; }
                if bits & 0x10 != 0 { *dst.add(3) = color; }
                if bits & 0x08 != 0 { *dst.add(4) = color; }
                if bits & 0x04 != 0 { *dst.add(5) = color; }
                if bits & 0x02 != 0 { *dst.add(6) = color; }
                if bits & 0x01 != 0 { *dst.add(7) = color; }
            }
        }
    } else {
        // Slow path with bounds checking
        for (i, &c) in s.iter().enumerate() {
            draw_char(fb_addr, pitch, x + i * 8, y, color, font, c, width, height);
        }
    }
}