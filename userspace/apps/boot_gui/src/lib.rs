#![no_std]
#![no_main]

extern crate alloc;

use userspace_runtime::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        // Initialize userspace heap
        init_userspace_heap();
        
        // Run the GUI application
        let result = run_boot_gui();
        
        // Exit with result
        syscall_exit(result as u64);
    }
}

fn run_boot_gui() -> i32 {
    // Create the main window surface
    let surface_handle = syscall_create_object(
        0,    // Surface type
        800,  // width
        600,  // height
        0     // RGBA8888 format
    );
    
    if surface_handle == u64::MAX {
        return -1;
    }

    // Create a buffer for the surface
    let buffer_handle = syscall_create_object(
        1,    // Buffer type
        800,  // width
        600,  // height
        0     // RGBA8888 format
    );
    
    if buffer_handle == u64::MAX {
        return -2;
    }

    // Render some simple graphics to show the GUI is working
    render_boot_screen();

    // Attach buffer to surface
    if syscall_call_object(surface_handle, 0, buffer_handle, 0) != 0 {
        return -3;
    }

    // Commit surface (render it to screen)
    if syscall_call_object(surface_handle, 1, 0, 0) != 0 {
        return -4;
    }

    // Create event stream for input
    let event_handle = syscall_create_object(2, 0, 0, 0);
    if event_handle == u64::MAX {
        return -5;
    }

    // Main GUI loop - keep running and handling events
    let mut frame_count = 0u32;
    loop {
        // Poll for events
        let has_events = syscall_call_object(event_handle, 0, 0, 0);
        
        // Update graphics every few iterations
        if frame_count % 60 == 0 {
            render_animated_frame(frame_count / 60);
            
            // Re-commit surface to update display
            if syscall_call_object(surface_handle, 1, 0, 0) != 0 {
                break;
            }
        }
        
        frame_count = frame_count.wrapping_add(1);
        
        // Simple delay/yield (in real implementation would be proper scheduling)
        for _ in 0..100000 {
            core::hint::spin_loop();
        }
        
        // Exit after demo period
        if frame_count > 3600 { // ~60 seconds at 60fps
            break;
        }
    }

    0 // Success
}

fn render_boot_screen() {
    // Simple boot screen - just validates our graphics pipeline works
    // In a real implementation, this would draw to the actual buffer memory
    // For now, the syscall interface handles the rendering
}

fn render_animated_frame(frame: u32) {
    // Animated content - colors change over time
    // The actual rendering is handled by the kernel's object system
    // This would typically update buffer contents and commit
    let _color_cycle = frame % 256;
    
    // In a full implementation, we'd:
    // 1. Get buffer memory mapping from kernel
    // 2. Draw pixels directly to buffer
    // 3. Commit changes to display
}