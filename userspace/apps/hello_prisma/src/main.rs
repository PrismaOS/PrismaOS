#![no_std]
#![no_main]

extern crate alloc;

use prisma_userspace::*;
use alloc::vec::Vec;

/// Entry point for PrismaOS userspace programs
/// 
/// This is called by the kernel after loading the ELF binary
/// and setting up the userspace environment.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize the userspace heap allocator
    unsafe {
        init_userspace_heap();
    }
    
    // Run our main application logic
    let exit_code = hello_prisma_main();
    
    // Exit the process with the return code
    syscall_exit(exit_code as u64);
}

/// Main application logic
fn hello_prisma_main() -> i32 {
    // Test 1: Basic memory allocation
    let mut test_vec = Vec::new();
    test_vec.push(42u32);
    test_vec.push(1337u32);
    
    if test_vec.len() != 2 || test_vec[0] != 42 || test_vec[1] != 1337 {
        return 1; // Memory allocation test failed
    }
    
    // Test 2: Create a graphics surface (400x300 RGBA)
    let surface = Surface::new(400, 300, 0);
    let surface_handle = match surface {
        Some(surf) => surf.handle(),
        None => return 2, // Surface creation failed
    };
    
    // Test 3: Create a buffer for the surface
    let buffer = Buffer::new(400, 300, 0);
    let buffer_handle = match buffer {
        Some(buf) => buf.handle(),
        None => return 3, // Buffer creation failed
    };
    
    // Test 4: Attach buffer to surface
    if let Some(surface) = surface {
        if !surface.attach_buffer(buffer_handle) {
            return 4; // Buffer attachment failed
        }
        
        // Test 5: Commit the surface to display
        if !surface.commit() {
            return 5; // Surface commit failed
        }
    }
    
    // Test 6: Create event stream for input
    let event_stream = EventStream::new();
    if event_stream.is_none() {
        return 6; // Event stream creation failed
    }
    
    let event_stream = event_stream.unwrap();
    
    // Test 7: Check for events (should be none initially)
    if event_stream.has_events() {
        return 7; // Unexpected events present
    }
    
    // Test 8: Advanced memory allocation test
    let large_vec: Vec<u64> = (0..1000).map(|i| i * i).collect();
    let sum: u64 = large_vec.iter().sum();
    let expected_sum = (0..1000u64).map(|i| i * i).sum::<u64>();
    
    if sum != expected_sum {
        return 8; // Advanced memory test failed
    }
    
    // All tests passed!
    0
}

/// Custom panic handler for userspace programs
/// 
/// When a userspace program panics, we should exit gracefully
/// rather than crashing the entire kernel.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In a real implementation, we might want to log the panic info
    // to the kernel for debugging purposes
    syscall_exit(255); // Exit with error code 255 for panics
}