#![no_std]
#![no_main]

extern crate alloc;
extern crate runtime;

use runtime::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize userspace heap
    unsafe {
        init_userspace_heap();
    }

    // Test basic syscalls
    test_syscalls();

    // Exit with success
    syscall_exit(0);
}

fn test_syscalls() {
    // Test 1: Create a surface object
    let surface_handle = syscall_create_object(0, 640, 480, 32); // 640x480 RGBA32
    if surface_handle != u64::MAX {
        // Success - test calling a method on it
        let result = syscall_call_object(surface_handle, 1, 0, 0); // commit method
    }

    // Test 2: Create a buffer object
    let buffer_handle = syscall_create_object(1, 640, 480, 32); // 640x480 RGBA32
    if buffer_handle != u64::MAX {
        // Success - attach it to the surface
        if surface_handle != u64::MAX {
            let _result = syscall_call_object(surface_handle, 0, buffer_handle, 0); // attach_buffer
        }
    }

    // Test 3: Create an event stream
    let event_handle = syscall_create_object(2, 0, 0, 0);
    if event_handle != u64::MAX {
        // Check for events
        let _has_events = syscall_call_object(event_handle, 0, 0, 0);
    }

    // Test 4: Process creation
    let new_process = syscall_create_process();
    if new_process != u64::MAX {
        // Success - we created a new process
    }

    // Test 5: Memory mapping
    let mapped_addr = syscall_mmap(0, 4096, 3, 1); // 4KB, RW, private
    if mapped_addr != u64::MAX {
        // Success - we got a virtual address
        let _result = syscall_munmap(mapped_addr, 4096);
    }

    // Test 6: File operations
    let test_path = b"/test/file.txt";
    let fd = syscall_open(test_path.as_ptr(), test_path.len() as u64, 0);
    if fd != u64::MAX {
        // File opened successfully
        let mut buffer = [0u8; 64];
        let _bytes_read = syscall_read(fd, buffer.as_mut_ptr(), buffer.len() as u64);
        let _result = syscall_close(fd);
    }

    // All tests completed
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    syscall_exit(1);
}