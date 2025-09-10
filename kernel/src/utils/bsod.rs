use core::panic::PanicInfo;

use crate::scrolling_text;

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

/// Render BSOD using framebuffer renderer
pub unsafe fn render_framebuffer_bsod(renderer: &mut scrolling_text::ScrollingTextRenderer, info: &PanicInfo) {
    // Clear screen with blue background
    let blue = 0xFF0000AA; // Dark blue
    let pitch = renderer.get_pitch();
    let width = renderer.get_fb_width();
    let height = renderer.get_fb_height();
    let fb_addr = renderer.get_fb_addr();
    
    // Fill entire screen with blue
    for y in 0..height {
        for x in 0..width {
            let pixel_offset = y * pitch + x * 4;
            fb_addr.add(pixel_offset).cast::<u32>().write(blue);
        }
    }
    
    // Reset cursor to top with some margin
    renderer.set_cursor_y(50);
    
    // Write BSOD header
    renderer.write_line(b"");
    renderer.write_line(b"    ***  KERNEL PANIC - PrismaOS STOPPED  ***");
    renderer.write_line(b"");
    renderer.write_line(b"  A critical error has occurred and PrismaOS has been");
    renderer.write_line(b"  shut down to prevent damage to your system.");
    renderer.write_line(b"");
    
    // Write panic information
    renderer.write_line(b"  PANIC DETAILS:");
    renderer.write_line(b"  --------------");
    
    if let Some(location) = info.location() {
        // Use LineWriter to format the location info
        use core::fmt::Write;
        let mut writer = scrolling_text::LineWriter::new();
        let _ = write!(writer, "  File: {}", location.file());
        writer.write_line();
        
        let mut writer2 = scrolling_text::LineWriter::new();
        let _ = write!(writer2, "  Line: {}", location.line());
        writer2.write_line();
        
        let mut writer3 = scrolling_text::LineWriter::new();
        let _ = write!(writer3, "  Column: {}", location.column());
        writer3.write_line();
    } else {
        renderer.write_line(b"  Location: Unknown");
    }
    
    // Show panic message if available  
    let msg = info.message();
    renderer.write_line(b"");
    renderer.write_line(b"  PANIC MESSAGE:");
    use core::fmt::Write;
    let mut writer = scrolling_text::LineWriter::new();
    let _ = write!(writer, "  {}", msg);
    writer.write_line();
    
    renderer.write_line(b"");
    renderer.write_line(b"  SYSTEM STATE:");
    renderer.write_line(b"  * Interrupts disabled");
    renderer.write_line(b"  * System halted");
    renderer.write_line(b"  * Memory state preserved");
    renderer.write_line(b"  * All cores stopped");
    renderer.write_line(b"");
    renderer.write_line(b"  CPU REGISTERS:");
    renderer.write_line(b"  * Register dump not implemented");
    renderer.write_line(b"");
    renderer.write_line(b"  Please restart your computer.");
}