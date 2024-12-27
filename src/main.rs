#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(prisma_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::fmt;
use lazy_static::lazy_static;
use prisma_os::println;
use prisma_os::task::{executor::Executor, Task};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use spin::Mutex;
use volatile::Volatile;

// Import the VGA buffer
mod vga_buffer;

lazy_static! {
    pub static ref WRITER: Mutex<vga_buffer::Writer> = Mutex::new(vga_buffer::Writer {
        column_position: 0,
        color_code: vga_buffer::ColorCode::new(vga_buffer::Color::Yellow, vga_buffer::Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut vga_buffer::Buffer) },
    });
}

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use prisma_os::allocator;
    use prisma_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("Welcome to Prisma OS!");
    prisma_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.run();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    prisma_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    prisma_os::test_panic_handler(info)
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}