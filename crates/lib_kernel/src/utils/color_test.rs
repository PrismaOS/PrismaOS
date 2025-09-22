use crate::{ kprintln, scrolling_text };

// Constants for rainbow test canvas
const RAINBOW_W: usize = 160;
const RAINBOW_H: usize = 48;

/// Display rainbow test canvas using global renderer
pub fn show_rainbow_test() {
    kprintln!("[OK] Graphics test: Rendering rainbow canvas...");
    
    // Build a small rainbow test canvas and draw it using the renderer.
    // Canvas size is chosen modestly to avoid large stack usage.
    // Use heap allocation instead of stack to avoid stack overflow
    let mut pixels = alloc::vec![0u32; RAINBOW_W * RAINBOW_H];

    // Define rainbow color stops (ARGB)
    const STOPS: [u32; 8] = [
        0xFFFF0000, // red
        0xFFFF7F00, // orange
        0xFFFFFF00, // yellow
        0xFF00FF00, // green
        0xFF00FFFF, // cyan
        0xFF0000FF, // blue
        0xFFFF00FF, // magenta
        0xFFFF0000, // back to red (loop)
    ];
    let segments = STOPS.len() - 1;

    // Fill pixels with horizontally interpolated rainbow, slightly darken by row for vertical variation.
    for y in 0..RAINBOW_H {
        for x in 0..RAINBOW_W {
            // position along width in [0, segments)
            let pos = (x * segments) as usize * 256 / (RAINBOW_W.max(1));
            let seg = (pos / 256).min(segments - 1);
            let t = (pos % 256) as u32; // 0..255

            let c0 = STOPS[seg];
            let c1 = STOPS[seg + 1];

            let a0 = ((c0 >> 24) & 0xFF) as u32;
            let r0 = ((c0 >> 16) & 0xFF) as u32;
            let g0 = ((c0 >> 8) & 0xFF) as u32;
            let b0 = (c0 & 0xFF) as u32;

            let a1 = ((c1 >> 24) & 0xFF) as u32;
            let r1 = ((c1 >> 16) & 0xFF) as u32;
            let g1 = ((c1 >> 8) & 0xFF) as u32;
            let b1 = (c1 & 0xFF) as u32;

            // linear interpolation
            let a = ((a0 * (256 - t) + a1 * t) >> 8) as u32;
            let rr = ((r0 * (256 - t) + r1 * t) >> 8) as u32;
            let gg = ((g0 * (256 - t) + g1 * t) >> 8) as u32;
            let bb = ((b0 * (256 - t) + b1 * t) >> 8) as u32;

            // slight vertical darkening to give a banded look
            let dark = 220u32.saturating_sub(y as u32 * 120 / RAINBOW_H as u32);
            let rr = (rr * dark) / 255;
            let gg = (gg * dark) / 255;
            let bb = (bb * dark) / 255;

            pixels[y * RAINBOW_W + x] = (a << 24) | (rr << 16) | (gg << 8) | bb;
        }
    }

    // Draw the generated rainbow canvas using global renderer
    scrolling_text::kdraw_canvas(&pixels, RAINBOW_W, RAINBOW_H);
}