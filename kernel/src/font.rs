// Minimal PSF1 font renderer for framebuffer

#[link_section = ".rodata"]
#[no_mangle]
pub static FONT_PSF: [u8; include_bytes!("../../lat9-16.psf").len()] = *include_bytes!("../../lat9-16.psf");

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
    for row in 0..font.charsize {
        let py = y + row;
        if py >= height { continue; }
        let bits = glyph[row];
        for col in 0..8 {
            let px = x + col;
            if px >= width { continue; }
            if (bits >> (7 - col)) & 1 != 0 {
                let offset = py * pitch + px * 4;
                fb_addr.add(offset).cast::<u32>().write(color);
            }
        }
    }
}

// Draw a string at (x, y) with bounds checking
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
    for (i, &c) in s.iter().enumerate() {
        draw_char(fb_addr, pitch, x + i * 8, y, color, font, c, width, height);
    }
}