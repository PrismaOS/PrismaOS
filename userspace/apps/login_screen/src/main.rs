#![no_std]
#![no_main]

extern crate alloc;

use prisma_userspace::*;
use alloc::vec::Vec;

/// Entry point for PrismaOS login screen
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize the userspace heap allocator
    unsafe {
        init_userspace_heap();
    }
    
    // Run the login screen
    let exit_code = login_screen_main();
    
    // Exit the process
    syscall_exit(exit_code as u64);
}

/// Main login screen logic
fn login_screen_main() -> i32 {
    // Screen dimensions
    const SCREEN_WIDTH: u32 = 1024;
    const SCREEN_HEIGHT: u32 = 768;
    const RGBA_FORMAT: u32 = 0; // RGBA8888
    
    // Create main display surface
    let surface = Surface::new(SCREEN_WIDTH, SCREEN_HEIGHT, RGBA_FORMAT);
    if surface.is_none() {
        return 1; // Failed to create surface
    }
    let surface = surface.unwrap();
    
    // Create framebuffer
    let buffer = Buffer::new(SCREEN_WIDTH, SCREEN_HEIGHT, RGBA_FORMAT);
    if buffer.is_none() {
        return 2; // Failed to create buffer
    }
    let buffer = buffer.unwrap();
    
    // Attach buffer to surface
    if !surface.attach_buffer(buffer.handle()) {
        return 3; // Failed to attach buffer
    }
    
    // Render the login screen
    render_login_screen(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT);
    
    // Commit the surface to display
    if !surface.commit() {
        return 4; // Failed to commit surface
    }
    
    // Create event stream for user input
    let event_stream = EventStream::new();
    if event_stream.is_none() {
        return 5; // Failed to create event stream
    }
    let event_stream = event_stream.unwrap();
    
    // Main event loop
    loop {
        // Check for events (keyboard/mouse input)
        if event_stream.has_events() {
            // In a real implementation, we'd process login events here
            // For now, just continue the loop
        }
        
        // Yield CPU to other processes
        // In a real implementation, we'd have a proper yield syscall
        for _ in 0..1000 {
            unsafe { core::arch::asm!("pause"); }
        }
    }
}

/// Render the login screen to the framebuffer
fn render_login_screen(buffer: &Buffer, width: u32, height: u32) {
    // Get framebuffer pointer (this would be provided by the buffer object)
    // For now, we'll simulate rendering by making syscalls to draw components
    
    // Clear screen with gradient background
    render_background(width, height);
    
    // Draw PrismaOS logo/title
    render_title(width, height);
    
    // Draw login box
    render_login_box(width, height);
    
    // Draw status text
    render_status_text(width, height);
}

/// Render gradient background
fn render_background(width: u32, height: u32) {
    // Create a gradient from dark blue to light blue
    // This would typically involve direct framebuffer access
    // For now, we'll use syscalls to create colored rectangles
    
    let gradient_steps = 32;
    let step_height = height / gradient_steps;
    
    for i in 0..gradient_steps {
        let y = i * step_height;
        let intensity = (i * 255) / gradient_steps;
        let color = 0xFF000000 | (intensity << 16) | (intensity << 8) | intensity;
        
        // Draw gradient strip (would be a syscall to draw rectangle)
        draw_rectangle(0, y, width, step_height, color);
    }
}

/// Render PrismaOS title
fn render_title(width: u32, height: u32) {
    let title_y = height / 6;
    let title_color = 0xFFFFFFFF; // White
    
    // Draw title background
    draw_rectangle(width / 4, title_y - 20, width / 2, 80, 0x80000000); // Semi-transparent black
    
    // Draw title text "PrismaOS" (simulated with colored rectangles)
    draw_text_simulation("PrismaOS", width / 2 - 100, title_y, title_color);
}

/// Render login input box
fn render_login_box(width: u32, height: u32) {
    let box_width = 400;
    let box_height = 300;
    let box_x = (width - box_width) / 2;
    let box_y = (height - box_height) / 2;
    
    // Draw login box background
    draw_rectangle(box_x, box_y, box_width, box_height, 0xE0F0F0F0); // Light gray with transparency
    
    // Draw border
    draw_border(box_x, box_y, box_width, box_height, 0xFF808080); // Gray border
    
    // Draw username field
    let field_y = box_y + 80;
    draw_rectangle(box_x + 20, field_y, box_width - 40, 40, 0xFFFFFFFF); // White input field
    draw_border(box_x + 20, field_y, box_width - 40, 40, 0xFF000000); // Black border
    draw_text_simulation("Username:", box_x + 30, field_y - 25, 0xFF000000);
    
    // Draw password field  
    let pass_y = field_y + 80;
    draw_rectangle(box_x + 20, pass_y, box_width - 40, 40, 0xFFFFFFFF); // White input field
    draw_border(box_x + 20, pass_y, box_width - 40, 40, 0xFF000000); // Black border
    draw_text_simulation("Password:", box_x + 30, pass_y - 25, 0xFF000000);
    
    // Draw login button
    let btn_y = pass_y + 80;
    draw_rectangle(box_x + 20, btn_y, box_width - 40, 50, 0xFF4080FF); // Blue button
    draw_text_simulation("Login", box_x + (box_width / 2) - 30, btn_y + 15, 0xFFFFFFFF);
}

/// Render status text
fn render_status_text(width: u32, height: u32) {
    let status_y = height - 100;
    draw_text_simulation("Welcome to PrismaOS", width / 2 - 120, status_y, 0xFFFFFFFF);
    draw_text_simulation("Press any key to continue", width / 2 - 140, status_y + 30, 0xFFCCCCCC);
}

/// Simulate drawing a rectangle (would be a syscall)
fn draw_rectangle(x: u32, y: u32, width: u32, height: u32, color: u32) {
    // In a real implementation, this would be a graphics syscall
    // For now, we'll just make a syscall to the graphics subsystem
    syscall_create_object(100, // Graphics draw command
                         ((x as u64) << 32) | (y as u64),
                         ((width as u64) << 32) | (height as u64),
                         color as u64);
}

/// Simulate drawing a border
fn draw_border(x: u32, y: u32, width: u32, height: u32, color: u32) {
    // Draw four rectangles for the border
    draw_rectangle(x, y, width, 2, color); // Top
    draw_rectangle(x, y + height - 2, width, 2, color); // Bottom  
    draw_rectangle(x, y, 2, height, color); // Left
    draw_rectangle(x + width - 2, y, 2, height, color); // Right
}

/// Simulate text rendering with colored rectangles
fn draw_text_simulation(text: &str, x: u32, y: u32, color: u32) {
    // This is a very basic text simulation using rectangles
    // In a real implementation, we'd have proper font rendering
    
    let char_width = 12;
    let char_height = 16;
    
    for (i, _ch) in text.chars().enumerate() {
        let char_x = x + (i as u32 * char_width);
        
        // Draw a simple rectangle pattern to simulate each character
        // This is just for demonstration - real text would use proper fonts
        draw_rectangle(char_x, y, char_width - 2, char_height, color);
        draw_rectangle(char_x + 2, y + 2, char_width - 6, char_height - 4, 0xFF000000); // Create "hollow" effect
    }
}

/// Custom panic handler for the login screen
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Exit with error code for panics
    syscall_exit(255);
}