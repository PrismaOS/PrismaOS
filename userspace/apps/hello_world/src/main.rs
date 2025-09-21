#![no_std]
#![no_main]

extern crate alloc;

use userspace_runtime::*;
use alloc::{format, vec::Vec};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        // Initialize userspace heap
        init_userspace_heap();
        
        // Run the hello world application
        let result = hello_world_main();
        
        // Exit with result
        syscall_exit(result as u64);
    }
}

fn hello_world_main() -> i32 {
    // Test heap allocation
    let mut message = Vec::new();
    message.extend_from_slice(b"Hello from PrismaOS userspace!");
    
    // Create a simple surface to test graphics
    let surface = Surface::new(400, 300, 0); // 400x300 RGBA8888
    match surface {
        Some(surf) => {
            // Success! We have graphics capability
            let buffer = Buffer::new(400, 300, 0);
            if let Some(buf) = buffer {
                // Attach buffer to surface
                if surf.attach_buffer(buf.handle()) {
                    // Commit to display
                    if surf.commit() {
                        // Graphics pipeline working!
                        return 0; // Success
                    }
                }
            }
        }
        None => {
            // Graphics not available, but that's ok for a hello world
        }
    }
    
    // Test event system
    let event_stream = EventStream::new();
    if let Some(_stream) = event_stream {
        // Event system is working
    }
    
    // Even if graphics fail, we succeeded in:
    // 1. Loading as userspace program
    // 2. Initializing heap
    // 3. Making syscalls
    // 4. Allocating memory
    
    0 // Success
}

// Custom panic handler for this userspace program
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In userspace, panics should not crash the kernel
    // Just exit the process
    syscall_exit(1);
}