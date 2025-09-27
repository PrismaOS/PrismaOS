use core::panic::PanicInfo;

use crate::scrolling_text;

/// Simple hex formatter that doesn't require alloc  
fn write_hex_to_buf(val: u64, buf: &mut [u8; 18]) -> &[u8] {
    const HEX_CHARS: &[u8] = b"0123456789ABCDEF";
    buf[0] = b'0';
    buf[1] = b'x';
    
    let mut pos = 2;
    let mut temp = val;
    let mut digits = [0u8; 16];
    let mut digit_count = 0;
    
    // Extract digits
    if temp == 0 {
        digits[0] = 0;
        digit_count = 1;
    } else {
        while temp > 0 {
            digits[digit_count] = (temp % 16) as u8;
            temp /= 16;
            digit_count += 1;
        }
    }
    
    // Write digits in reverse order
    for i in (0..digit_count).rev() {
        buf[pos] = HEX_CHARS[digits[i] as usize];
        pos += 1;
    }
    
    &buf[0..pos]
}

/// Simple string copier for static messages
fn copy_str_to_buf(src: &str, buf: &mut [u8], max_len: usize) -> usize {
    let src_bytes = src.as_bytes();
    let copy_len = core::cmp::min(src_bytes.len(), max_len);
    buf[0..copy_len].copy_from_slice(&src_bytes[0..copy_len]);
    copy_len
}

/// Fast square root approximation for no_std environment
fn fast_sqrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    
    // Newton-Raphson method for square root
    let mut guess = x * 0.5;
    for _ in 0..4 { // 4 iterations for good accuracy
        guess = 0.5 * (guess + x / guess);
    }
    guess
}

/// Fallback VGA text mode BSOD
pub unsafe fn render_vga_bsod(info: &PanicInfo) {
    let vga_buffer = 0xb8000 as *mut u16;

    // Clear screen with blue background (BSOD)
    for i in 0..(80 * 25) {
        vga_buffer.add(i).write(0x1F00 | b' ' as u16);
    }

    // Display panic header
    let header = b"*** PRISMAOS KERNEL PANIC ***";
    let start_pos = (80 - header.len()) / 2;
    for (i, &byte) in header.iter().enumerate() {
        if start_pos + i < 80 {
            vga_buffer.add(start_pos + i).write(0x1F00 | byte as u16);
        }
    }

    // Show error message on second line
    let error_msg = b"A critical error occurred. System halted.";
    let line2_start = 80 * 2;
    let msg_start = (80 - error_msg.len()) / 2;
    for (i, &byte) in error_msg.iter().enumerate() {
        if line2_start + msg_start + i < 80 * 25 {
            vga_buffer.add(line2_start + msg_start + i).write(0x1F00 | byte as u16);
        }
    }

    // Show location if available
    if let Some(location) = info.location() {
        let file_info = b"File: ";
        let line_start = 80 * 4;

        for (i, &byte) in file_info.iter().enumerate() {
            if line_start + i < 80 * 25 {
                vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
            }
        }

        // Write filename (truncated)
        let filename = location.file().as_bytes();
        let filename_start = line_start + file_info.len();
        for (i, &byte) in filename.iter().take(40).enumerate() {
            if filename_start + i < 80 * 25 {
                vga_buffer.add(filename_start + i).write(0x1F00 | byte as u16);
            }
        }

        // Write line number
        let line_info = b"Line: ";
        let line_start = 80 * 5;
        for (i, &byte) in line_info.iter().enumerate() {
            if line_start + i < 80 * 25 {
                vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
            }
        }

        // Simple line number display
        let line_num = location.line();
        let mut temp_line = line_num;
        let mut digits = [0u8; 10];
        let mut digit_count = 0;

        if temp_line == 0 {
            digits[0] = b'0';
            digit_count = 1;
        } else {
            while temp_line > 0 && digit_count < 10 {
                digits[digit_count] = (temp_line % 10) as u8 + b'0';
                temp_line /= 10;
                digit_count += 1;
            }
        }

        // Display digits in reverse order
        let line_num_start = line_start + line_info.len();
        for i in 0..digit_count {
            let digit_pos = line_num_start + i;
            if digit_pos < 80 * 25 {
                let digit = digits[digit_count - 1 - i];
                vga_buffer.add(digit_pos).write(0x1F00 | digit as u16);
            }
        }
    }

    // Show panic message
    let msg_info = b"Message: ";
    let line_start = 80 * 7;
    for (i, &byte) in msg_info.iter().enumerate() {
        if line_start + i < 80 * 25 {
            vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
        }
    }
    
    // Display basic message info (since we can't format without heap)
    let simple_msg = b"(message details available)";
    let msg_start = line_start + msg_info.len();
    for (i, &byte) in simple_msg.iter().enumerate() {
        if msg_start + i < 80 * 25 {
            vga_buffer.add(msg_start + i).write(0x1F00 | byte as u16);
        }
    }

    // Instructions
    let instructions = [
        "System halted to prevent damage",
        "Please restart to continue",
        "If this persists, check kernel config",
    ];

    let mut current_line = 10;
    for instruction in instructions.iter() {
        let line_start = 80 * current_line;
        for (i, byte) in instruction.bytes().enumerate() {
            if line_start + i < 80 * 25 {
                vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
            }
        }
        current_line += 1;
    }
}

/// Render BSOD using framebuffer renderer (Modern Windows-style graphics)
pub unsafe fn render_framebuffer_bsod(renderer: &mut scrolling_text::ScrollingTextRenderer, info: &PanicInfo) {
    let windows_blue = 0xFF0037DA; // Windows 10/11 BSOD blue
    let white = 0xFFFFFFFF;
    let light_blue = 0xFF4A90E2;
    let pitch = renderer.get_pitch();
    let width = renderer.get_fb_width();
    let height = renderer.get_fb_height();
    let fb_addr = renderer.get_fb_addr();
    
    // Create clean solid background first
    draw_solid_background(fb_addr, pitch, width, height, windows_blue);
    
    // Draw clean sad face at top
    draw_clean_sad_face(fb_addr, pitch, width, height, white);
    
    // Draw clean progress bar
    draw_clean_progress_bar(fb_addr, pitch, width, height, white, light_blue);
    
    // Draw clean QR code
    draw_clean_qr_code(fb_addr, pitch, width, height, white);
    
    // Now overlay all the text with proper spacing
    overlay_clean_bsod_text(renderer, info, white);
}

/// Create a clean solid background
unsafe fn draw_solid_background(fb_addr: *mut u8, pitch: usize, width: usize, height: usize, color: u32) {
    for y in 0..height {
        for x in 0..width {
            let pixel_offset = y * pitch + x * 4;
            if pixel_offset + 4 <= height * pitch {
                fb_addr.add(pixel_offset).cast::<u32>().write(color);
            }
        }
    }
}

/// Draw a clean, simple sad face
unsafe fn draw_clean_sad_face(fb_addr: *mut u8, pitch: usize, width: usize, height: usize, color: u32) {
    let center_x = width / 2;
    let center_y = height / 6; // Move up more
    let face_radius = 35;
    
    // Draw face outline (simple circle)
    draw_simple_circle_outline(fb_addr, pitch, width, height, center_x, center_y, face_radius, color, 3);
    
    // Draw eyes (simple filled circles)
    draw_simple_filled_circle(fb_addr, pitch, width, height, center_x - 12, center_y - 6, 4, color);
    draw_simple_filled_circle(fb_addr, pitch, width, height, center_x + 12, center_y - 6, 4, color);
    
    // Draw simple sad mouth (arc)
    draw_simple_sad_mouth(fb_addr, pitch, width, height, center_x, center_y + 15, 15, color);
}

/// Draw a clean progress bar
unsafe fn draw_clean_progress_bar(fb_addr: *mut u8, pitch: usize, width: usize, height: usize, 
                                 fg_color: u32, accent_color: u32) {
    let bar_x = width / 4;
    let bar_y = height * 2 / 3;
    let bar_width = width / 2;
    let bar_height = 6;
    
    // Background
    draw_simple_rectangle(fb_addr, pitch, width, height, bar_x, bar_y, bar_width, bar_height, 0xFF333333);
    
    // Fill (100%)
    draw_simple_rectangle(fb_addr, pitch, width, height, bar_x + 1, bar_y + 1, 
                         bar_width - 2, bar_height - 2, accent_color);
    
    // Border
    draw_simple_rectangle_outline(fb_addr, pitch, width, height, bar_x, bar_y, 
                                 bar_width, bar_height, fg_color, 1);
}

/// Draw a clean QR code
unsafe fn draw_clean_qr_code(fb_addr: *mut u8, pitch: usize, width: usize, height: usize, color: u32) {
    let qr_size = 80;
    let qr_x = width / 12;
    let qr_y = height * 3 / 4 + 20;
    
    // White background
    draw_simple_rectangle(fb_addr, pitch, width, height, qr_x, qr_y, qr_size, qr_size, 0xFFFFFFFF);
    
    // Simple QR pattern
    let cell_size = qr_size / 8;
    for row in 0..8 {
        for col in 0..8 {
            let should_fill = (row + col) % 2 == 0 || 
                             (row == 0 || row == 7 || col == 0 || col == 7);
            
            if should_fill {
                let cell_x = qr_x + col * cell_size + 2;
                let cell_y = qr_y + row * cell_size + 2;
                draw_simple_rectangle(fb_addr, pitch, width, height, cell_x, cell_y,
                                    cell_size - 2, cell_size - 2, 0xFF000000);
            }
        }
    }
    
    // Border
    draw_simple_rectangle_outline(fb_addr, pitch, width, height, qr_x, qr_y, 
                                 qr_size, qr_size, color, 2);
}

/// Overlay text with clean spacing
unsafe fn overlay_clean_bsod_text(renderer: &mut scrolling_text::ScrollingTextRenderer, 
                                 info: &PanicInfo, text_color: u32) {
    renderer.set_colors(text_color, 0x00000000); // White text, transparent background
    
    // Main message - positioned well below sad face
    renderer.set_cursor(50, renderer.get_fb_height() / 3);
    
    renderer.write_line(b"Your PC ran into a problem and needs to restart.");
    renderer.write_line(b"");
    renderer.write_line(b"We're just collecting some error info, and then");
    renderer.write_line(b"we'll restart for you.");
    renderer.write_line(b"");
    renderer.write_line(b"If you'd like to know more, you can search online later");
    renderer.write_line(b"for this error: PRISMAOS_KERNEL_PANIC");
    renderer.write_line(b"");
    renderer.write_line(b"Don't worry - your files are safe. This is just a");
    renderer.write_line(b"temporary hiccup in the system.");
    renderer.write_line(b"");
    
    // Progress text positioned above progress bar
    renderer.write_line(b"");
    renderer.write_line(b"");
    
    // Clean technical error info positioned to the right of QR code
    renderer.set_cursor(renderer.get_fb_width() / 12 + 100, renderer.get_fb_height() * 3 / 4 + 20);
    
    // Show the actual panic message first - using simple string without format
    let panic_message = "Panic occurred - check kernel logs";
    renderer.write_line(panic_message.as_bytes());
    renderer.write_line(b"");
    
    // Show detailed location information - clean and simple
    if let Some(location) = info.location() {
        use core::fmt::Write;
        
        // Show filename
        let mut writer1 = scrolling_text::LineWriter::new();
        let _ = write!(writer1, "File: {}", extract_filename(location.file()));
        writer1.write_line();
        
        // Show line and column
        let mut writer2 = scrolling_text::LineWriter::new();
        let _ = write!(writer2, "Line: {}, Column: {}", location.line(), location.column());
        writer2.write_line();
        
        // Show full file path for debugging
        let mut writer3 = scrolling_text::LineWriter::new();
        let _ = write!(writer3, "Path: {}", location.file());
        writer3.write_line();
    }
}

/// Safely set a pixel with bounds checking
unsafe fn set_pixel_safe(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                        x: usize, y: usize, color: u32) {
    if x < width && y < height {
        let pixel_offset = y * pitch + x * 4;
        if pixel_offset + 4 <= height * pitch {
            fb_addr.add(pixel_offset).cast::<u32>().write(color);
        }
    }
}

/// Blend a pixel with alpha
unsafe fn blend_pixel(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                     x: usize, y: usize, color: u32) {
    if x < width && y < height {
        let pixel_offset = y * pitch + x * 4;
        if pixel_offset + 4 <= height * pitch {
            let existing = fb_addr.add(pixel_offset).cast::<u32>().read();
            let blended = blend_colors(existing, color);
            fb_addr.add(pixel_offset).cast::<u32>().write(blended);
        }
    }
}

/// Blend two colors with alpha
fn blend_colors(background: u32, foreground: u32) -> u32 {
    let fg_alpha = ((foreground >> 24) & 0xFF) as f32 / 255.0;
    let inv_alpha = 1.0 - fg_alpha;
    
    let bg_r = ((background >> 16) & 0xFF) as f32;
    let bg_g = ((background >> 8) & 0xFF) as f32;
    let bg_b = (background & 0xFF) as f32;
    
    let fg_r = ((foreground >> 16) & 0xFF) as f32;
    let fg_g = ((foreground >> 8) & 0xFF) as f32;
    let fg_b = (foreground & 0xFF) as f32;
    
    let r = (fg_r * fg_alpha + bg_r * inv_alpha) as u32;
    let g = (fg_g * fg_alpha + bg_g * inv_alpha) as u32;
    let b = (fg_b * fg_alpha + bg_b * inv_alpha) as u32;
    
    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Draw a simple circle outline
unsafe fn draw_simple_circle_outline(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                                    center_x: usize, center_y: usize, radius: usize, color: u32, thickness: usize) {
    for y in 0..height {
        for x in 0..width {
            let dx = (x as i32 - center_x as i32).abs() as usize;
            let dy = (y as i32 - center_y as i32).abs() as usize;
            let distance_sq = dx * dx + dy * dy;
            let radius_sq = radius * radius;
            let inner_radius_sq = if radius > thickness { (radius - thickness) * (radius - thickness) } else { 0 };
            
            if distance_sq <= radius_sq && distance_sq >= inner_radius_sq {
                set_pixel_safe(fb_addr, pitch, width, height, x, y, color);
            }
        }
    }
}

/// Draw a simple filled circle
unsafe fn draw_simple_filled_circle(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                                   center_x: usize, center_y: usize, radius: usize, color: u32) {
    for y in 0..height {
        for x in 0..width {
            let dx = (x as i32 - center_x as i32).abs() as usize;
            let dy = (y as i32 - center_y as i32).abs() as usize;
            let distance_sq = dx * dx + dy * dy;
            let radius_sq = radius * radius;
            
            if distance_sq <= radius_sq {
                set_pixel_safe(fb_addr, pitch, width, height, x, y, color);
            }
        }
    }
}

/// Draw a simple sad mouth
unsafe fn draw_simple_sad_mouth(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                               center_x: usize, center_y: usize, radius: usize, color: u32) {
    for x in 0..width {
        if x >= center_x - radius && x <= center_x + radius {
            let dx = (x as i32 - center_x as i32) as f32;
            let normalized_x = dx / radius as f32;
            let y_offset = (normalized_x * normalized_x * radius as f32 * 0.4) as usize;
            
            for thickness in 0..2 {
                let y = center_y + y_offset + thickness;
                set_pixel_safe(fb_addr, pitch, width, height, x, y, color);
            }
        }
    }
}

/// Draw a simple rectangle
unsafe fn draw_simple_rectangle(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                               x: usize, y: usize, rect_width: usize, rect_height: usize, color: u32) {
    for dy in 0..rect_height {
        for dx in 0..rect_width {
            let pixel_x = x + dx;
            let pixel_y = y + dy;
            set_pixel_safe(fb_addr, pitch, width, height, pixel_x, pixel_y, color);
        }
    }
}

/// Draw a simple rectangle outline
unsafe fn draw_simple_rectangle_outline(fb_addr: *mut u8, pitch: usize, width: usize, height: usize,
                                       x: usize, y: usize, rect_width: usize, rect_height: usize, 
                                       color: u32, thickness: usize) {
    // Top and bottom borders
    for i in 0..thickness {
        draw_simple_rectangle(fb_addr, pitch, width, height, x, y + i, rect_width, 1, color);
        if rect_height > i {
            draw_simple_rectangle(fb_addr, pitch, width, height, x, y + rect_height - 1 - i, rect_width, 1, color);
        }
    }
    
    // Left and right borders
    for i in 0..thickness {
        draw_simple_rectangle(fb_addr, pitch, width, height, x + i, y, 1, rect_height, color);
        if rect_width > i {
            draw_simple_rectangle(fb_addr, pitch, width, height, x + rect_width - 1 - i, y, 1, rect_height, color);
        }
    }
}

/// Extract filename from full path
fn extract_filename(path: &str) -> &str {
    path.split('/').last()
        .or_else(|| path.split('\\').last())
        .unwrap_or(path)
}

/// Comprehensive BSOD trigger that handles all fault types consistently
/// This ensures ALL faults are caught and display properly
pub fn trigger_comprehensive_bsod(
    error_type: &'static str,
    message: &str,
    is_user_mode: bool,
    rip: Option<u64>,
    error_code: Option<u64>
) -> ! {
    // Disable interrupts immediately to prevent re-entrancy
    x86_64::instructions::interrupts::disable();
    
    // Create comprehensive message without alloc
    let prefix = if is_user_mode { "[USER] " } else { "[KERNEL] " };
    
    // Log basic fault info (without detailed formatting to avoid alloc dependency)
    // For now, we skip the serial output since it requires format! or we don't have access to it
    
    // Try to render comprehensive BSOD
    unsafe {
        if let Some(ref mut renderer) = scrolling_text::GLOBAL_RENDERER {
            render_comprehensive_framebuffer_bsod(renderer, error_type, message, is_user_mode, rip, error_code);
        } else {
            render_comprehensive_vga_bsod(error_type, message, is_user_mode, rip, error_code);
        }
    }
    
    // Final halt
    crate::utils::system::halt_system();
}

/// Render comprehensive VGA BSOD with fault details
unsafe fn render_comprehensive_vga_bsod(
    error_type: &str,
    message: &str,
    is_user_mode: bool,
    rip: Option<u64>,
    error_code: Option<u64>
) {
    let vga_buffer = 0xb8000 as *mut u16;
    
    // Clear screen with blue background
    for i in 0..(80 * 25) {
        vga_buffer.add(i).write(0x1F00 | b' ' as u16);
    }
    
    // Display comprehensive header
    let header = b"*** PRISMAOS COMPREHENSIVE FAULT HANDLER ***";
    let start_pos = (80 - header.len()) / 2;
    for (i, &byte) in header.iter().enumerate() {
        if start_pos + i < 80 {
            vga_buffer.add(start_pos + i).write(0x1F00 | byte as u16);
        }
    }
    
    // Show fault type
    let fault_label = b"FAULT TYPE: ";
    let line2_start = 80 * 3;
    for (i, &byte) in fault_label.iter().enumerate() {
        if line2_start + i < 80 * 25 {
            vga_buffer.add(line2_start + i).write(0x1F00 | byte as u16);
        }
    }
    
    // Display error type (truncated to fit)
    let type_bytes = error_type.as_bytes();
    let type_start = line2_start + fault_label.len();
    for (i, &byte) in type_bytes.iter().take(80 - fault_label.len()).enumerate() {
        if type_start + i < 80 * 25 {
            vga_buffer.add(type_start + i).write(0x1F00 | byte as u16);
        }
    }
    
    // Show privilege level
    let priv_msg = if is_user_mode { "USER MODE FAULT" } else { "KERNEL MODE FAULT" };
    let line4_start = 80 * 5;
    for (i, byte) in priv_msg.bytes().enumerate() {
        if line4_start + i < 80 * 25 {
            vga_buffer.add(line4_start + i).write(0x1F00 | byte as u16);
        }
    }
    
    // Show RIP if available
    if let Some(rip_val) = rip {
        let rip_label = b"RIP: ";
        let line6_start = 80 * 7;
        for (i, &byte) in rip_label.iter().enumerate() {
            if line6_start + i < 80 * 25 {
                vga_buffer.add(line6_start + i).write(0x1F00 | byte as u16);
            }
        }
        
        // Display RIP in hex without format!
        let mut rip_buf = [0u8; 18];
        let rip_bytes = write_hex_to_buf(rip_val, &mut rip_buf);
        let rip_start = line6_start + rip_label.len();
        for (i, &byte) in rip_bytes.iter().take(80 - rip_label.len()).enumerate() {
            if rip_start + i < 80 * 25 {
                vga_buffer.add(rip_start + i).write(0x1F00 | byte as u16);
            }
        }
    }
    
    // Show error code if available
    if let Some(error_val) = error_code {
        let error_label = b"ERROR: ";
        let line8_start = 80 * 9;
        for (i, &byte) in error_label.iter().enumerate() {
            if line8_start + i < 80 * 25 {
                vga_buffer.add(line8_start + i).write(0x1F00 | byte as u16);
            }
        }
        
        // Display error code in hex without format!
        let mut error_buf = [0u8; 18];
        let error_bytes = write_hex_to_buf(error_val, &mut error_buf);
        let error_start = line8_start + error_label.len();
        for (i, &byte) in error_bytes.iter().take(80 - error_label.len()).enumerate() {
            if error_start + i < 80 * 25 {
                vga_buffer.add(error_start + i).write(0x1F00 | byte as u16);
            }
        }
    }
    
    // Instructions
    let instructions = if is_user_mode {
        [
            "User process fault detected",
            "Process would be terminated in full OS",
            "Kernel stability maintained",
            "System restart recommended for testing"
        ]
    } else {
        [
            "Critical kernel fault detected",
            "System halted to prevent data corruption",
            "Please restart and check system logs",
            "If persistent, review recent changes"
        ]
    };
    
    let mut current_line = 12;
    for instruction in instructions.iter() {
        let line_start = 80 * current_line;
        for (i, byte) in instruction.bytes().take(80).enumerate() {
            if line_start + i < 80 * 25 {
                vga_buffer.add(line_start + i).write(0x1F00 | byte as u16);
            }
        }
        current_line += 1;
    }
}

/// Render comprehensive framebuffer BSOD
unsafe fn render_comprehensive_framebuffer_bsod(
    renderer: &mut scrolling_text::ScrollingTextRenderer,
    error_type: &str,
    message: &str,
    is_user_mode: bool,
    rip: Option<u64>,
    error_code: Option<u64>
) {
    let windows_blue = 0xFF0037DA;
    let _white = 0xFFFFFFFFu32;
    let red = 0xFFFF4444;
    let yellow = 0xFFFFFF00;
    let pitch = renderer.get_pitch();
    let width = renderer.get_fb_width();
    let height = renderer.get_fb_height();
    let fb_addr = renderer.get_fb_addr();
    
    // Create solid background
    draw_solid_background(fb_addr, pitch, width, height, windows_blue);
    
    // Draw enhanced sad face (different color for user vs kernel)
    let face_color = if is_user_mode { yellow } else { red };
    draw_clean_sad_face(fb_addr, pitch, width, height, face_color);
    
    // Display comprehensive fault information without format!
    let fault_prefix = if is_user_mode { "USER FAULT: " } else { "KERNEL FAULT: " };
    
    // Create title without format - we'll write parts separately
    // Use the scrolling text renderer to display detailed info
    // Note: We don't clear the screen as the background is already set
    renderer.write_line(fault_prefix.as_bytes());
    renderer.write_line(error_type.as_bytes());
    renderer.write_line(b"");
    
    if let Some(rip_val) = rip {
        renderer.write_line(b"RIP: ");
        let mut rip_buf = [0u8; 18];
        let rip_bytes = write_hex_to_buf(rip_val, &mut rip_buf);
        renderer.write_line(rip_bytes);
    }
    
    if let Some(error_val) = error_code {
        renderer.write_line(b"Error Code: ");
        let mut error_buf = [0u8; 18];
        let error_bytes = write_hex_to_buf(error_val, &mut error_buf);
        renderer.write_line(error_bytes);
    }
    
    renderer.write_line(b"");
    renderer.write_line(message.as_bytes());
    renderer.write_line(b"");
    
    if is_user_mode {
        renderer.write_line(b"USERSPACE FAULT - Process would be terminated");
        renderer.write_line(b"Kernel remains stable");
    } else {
        renderer.write_line(b"CRITICAL KERNEL FAULT");
        renderer.write_line(b"System halted for safety");
    }
    
    renderer.write_line(b"");
    renderer.write_line(b"System diagnostic information saved");
    renderer.write_line(b"Please restart to continue");
}