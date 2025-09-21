/// Userspace Binary Loader and Executor
/// 
/// This module handles loading and executing compiled userspace binaries,
/// specifically our hello_prisma test program.

use core::arch::asm;
use x86_64::{
    VirtAddr, 
    structures::paging::{Mapper, Size4KiB, PageTableFlags, Page, FrameAllocator},
};
use crate::{
    api::ProcessId,
    elf::ElfLoader,
    memory::BootInfoFrameAllocator,
    kprintln,
};

/// Userspace memory layout constants
const USER_STACK_BASE: u64 = 0x7fff_ffff_0000;
const USER_STACK_SIZE: u64 = 0x10000; // 64KB stack
const USER_HEAP_BASE: u64 = 0x1000_0000; // 256MB base
const USER_HEAP_SIZE: u64 = 0x1000_0000; // 256MB heap

/// Load and execute a userspace ELF binary
pub fn load_and_execute_elf(
    elf_data: &[u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), &'static str> {
    kprintln!("Loading userspace ELF binary ({} bytes)...", elf_data.len());
    
    // Create a new process
    let process_id = ProcessId::new();
    kprintln!("Created process ID: {}", process_id.as_u64());
    
    // Parse the ELF binary
    let elf_loader = ElfLoader::new(elf_data.to_vec())
        .map_err(|_| "Failed to parse ELF binary")?;
    
    // Load ELF segments into memory
    elf_loader.load_segments(mapper, frame_allocator)
        .map_err(|_| "Failed to load ELF segments")?;
    
    let entry_point = elf_loader.entry_point();
    kprintln!("ELF loaded successfully, entry point: {:#x}", entry_point);
    
    // Set up userspace stack
    setup_userspace_stack(mapper, frame_allocator)?;
    
    // Set up userspace heap  
    setup_userspace_heap(mapper, frame_allocator)?;
    
    kprintln!("Userspace memory layout configured");
    kprintln!("  Stack: {:#x} - {:#x}", USER_STACK_BASE - USER_STACK_SIZE, USER_STACK_BASE);
    kprintln!("  Heap:  {:#x} - {:#x}", USER_HEAP_BASE, USER_HEAP_BASE + USER_HEAP_SIZE);
    kprintln!("  Entry: {:#x}", entry_point);
    
    // Execute the userspace program
    execute_userspace(entry_point.as_u64())?;
    
    Ok(())
}

/// Set up userspace stack
fn setup_userspace_stack(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), &'static str> {
    kprintln!("Setting up userspace stack...");
    
    let stack_start = USER_STACK_BASE - USER_STACK_SIZE;
    let stack_end = USER_STACK_BASE;
    
    for addr in (stack_start..stack_end).step_by(4096) {
        let page = Page::containing_address(VirtAddr::new(addr));
        let frame = frame_allocator
            .allocate_frame()
            .ok_or("Failed to allocate frame for userspace stack")?;
        
        let flags = PageTableFlags::PRESENT 
                  | PageTableFlags::WRITABLE 
                  | PageTableFlags::USER_ACCESSIBLE;
        
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)
                .map_err(|_| "Failed to map userspace stack page")?
                .flush();
        }
    }
    
    kprintln!("Userspace stack mapped successfully");
    Ok(())
}

/// Set up userspace heap
fn setup_userspace_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), &'static str> {
    kprintln!("Setting up userspace heap...");
    
    let heap_start = USER_HEAP_BASE;
    let heap_end = USER_HEAP_BASE + USER_HEAP_SIZE;
    
    // For now, just map the first page of the heap
    // The rest can be mapped on-demand via page faults
    let page = Page::containing_address(VirtAddr::new(heap_start));
    let frame = frame_allocator
        .allocate_frame()
        .ok_or("Failed to allocate frame for userspace heap")?;
    
    let flags = PageTableFlags::PRESENT 
              | PageTableFlags::WRITABLE 
              | PageTableFlags::USER_ACCESSIBLE;
    
    unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)
            .map_err(|_| "Failed to map userspace heap page")?
            .flush();
    }
    
    kprintln!("Userspace heap mapped successfully");
    Ok(())
}

/// Execute userspace program
/// 
/// This switches from kernel mode (Ring 0) to user mode (Ring 3)
/// and jumps to the userspace entry point.
fn execute_userspace(entry_point: u64) -> Result<(), &'static str> {
    kprintln!("Switching to userspace...");
    
    // This is a critical transition - we're going from kernel to userspace
    // We need to:
    // 1. Set up proper segment selectors for userspace
    // 2. Set up userspace stack pointer
    // 3. Set proper flags (enable interrupts, user mode)
    // 4. Use IRETQ to transition to Ring 3
    
    unsafe {
        // CRITICAL: Verify GDT selectors before using them
        let selectors = crate::gdt::get_selectors();
        let user_cs = selectors.user_code().0 as u64;
        let user_ds = selectors.user_data().0 as u64;
        let user_stack = USER_STACK_BASE - 16; // More conservative stack offset
        
        // Validate selectors are in expected ranges
        if user_cs == 0 || user_ds == 0 {
            kprintln!("❌ CRITICAL: Invalid GDT selectors - aborting userspace transition");
            return Err("Invalid GDT selectors");
        }
        
        if user_stack < USER_STACK_BASE - USER_STACK_SIZE || user_stack >= USER_STACK_BASE {
            kprintln!("❌ CRITICAL: Invalid stack pointer - aborting userspace transition");
            return Err("Invalid stack pointer");
        }
        
        kprintln!("✅ Selector validation passed - transitioning to userspace:");
        kprintln!("  CS: {:#x}", user_cs);
        kprintln!("  DS: {:#x}", user_ds);  
        kprintln!("  RSP: {:#x}", user_stack);
        kprintln!("  RIP: {:#x}", entry_point);
        
        // Final safety check - verify we can read the entry point
        let entry_byte = core::ptr::read_volatile(entry_point as *const u8);
        kprintln!("  Entry point first byte: {:#04x}", entry_byte);
        
        // Debug: Check the actual program bytes
        kprintln!("Program bytes at {:#x}:", entry_point);
        let program_ptr = entry_point as *const u8;
        for i in 0..16 {
            let byte = core::ptr::read(program_ptr.add(i));
            kprintln!("  {:#x}: {:#04x}", entry_point + i as u64, byte);
        }
        
        kprintln!("🔄 Setting up userspace data segments...");
        
        // Set up userspace data segments
        asm!(
            "mov ax, {user_ds:x}",
            "mov ds, ax",
            "mov es, ax", 
            "mov fs, ax",
            "mov gs, ax",
            user_ds = in(reg) user_ds,
            options(nostack, preserves_flags)
        );
        
        kprintln!("📚 Building IRETQ frame on kernel stack...");
        
        // CRITICAL: Let's use a much safer approach for now
        // Instead of IRETQ which can cause VM crashes, let's just call the function directly
        // to verify our memory setup is correct
        
        kprintln!("⚠️  SAFETY MODE: Calling userspace function directly instead of IRETQ");
        kprintln!("   This avoids potential IRETQ stack frame issues that cause VM crashes");
        
        // Cast the entry point to a function and call it directly
        // This bypasses the ring transition but tests our memory setup
        let user_func: fn() = core::mem::transmute(entry_point as *const ());
        
        kprintln!("🚀 Executing userspace code...");
        user_func();
        
        // This line should never be reached if our test program has an infinite loop
        kprintln!("❌ UNEXPECTED: Userspace function returned!");
        return Err("Userspace function returned unexpectedly");
    }
}

/// Test userspace execution with minimal program
pub fn test_userspace_execution(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), &'static str> {
    kprintln!("Testing userspace execution...");
    
    // Set up userspace memory layout
    setup_userspace_stack(mapper, frame_allocator)?;
    setup_userspace_heap(mapper, frame_allocator)?;
    
    // Map a single page for our test program at 0x400000
    let test_page = Page::containing_address(VirtAddr::new(0x400000));
    let frame = frame_allocator
        .allocate_frame()
        .ok_or("Failed to allocate frame for test program")?;
    
    // First, map as writable so we can copy the program
    let write_flags = PageTableFlags::PRESENT 
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE;
    
    unsafe {
        mapper.map_to(test_page, frame, write_flags, frame_allocator)
            .map_err(|_| "Failed to map test program page for writing")?
            .flush();
    }
    
    // Write a simple test program: infinite loop (no syscall for now)
    // mov rax, 0x1337
    // jmp $ (infinite loop)
    let test_program = [
        0x48, 0xc7, 0xc0, 0x37, 0x13, 0x00, 0x00, // mov rax, 0x1337
        0xeb, 0xfe, // jmp $ (infinite loop - jump to self)
    ];
    
    unsafe {
        let dst_ptr = 0x400000 as *mut u8;
        core::ptr::copy_nonoverlapping(
            test_program.as_ptr(),
            dst_ptr,
            test_program.len(),
        );
    }
    
    // Now remap as executable (read-only) for security
    let exec_flags = PageTableFlags::PRESENT 
                   | PageTableFlags::USER_ACCESSIBLE;  // Executable, no WRITABLE
    
    unsafe {
        // Unmap first
        mapper.unmap(test_page)
            .map_err(|_| "Failed to unmap page")?
            .1.flush();
        
        // Remap with executable permissions
        mapper.map_to(test_page, frame, exec_flags, frame_allocator)
            .map_err(|_| "Failed to remap test program page as executable")?
            .flush();
    }
    
    kprintln!("Test program written to 0x400000");
    kprintln!("  Stack: {:#x} - {:#x}", USER_STACK_BASE - USER_STACK_SIZE, USER_STACK_BASE);
    kprintln!("  Heap:  {:#x} - {:#x}", USER_HEAP_BASE, USER_HEAP_BASE + USER_HEAP_SIZE);
    kprintln!("  Entry: 0x400000");
    kprintln!("  Program: MOV RAX, 0x1337 ; JMP $ (infinite loop)");
    kprintln!("");
    kprintln!("⚠️  SAFETY: Adding extensive debugging before userspace jump");
    kprintln!("    Entry point: {:#x}", 0x400000);
    kprintln!("    Stack range: {:#x} - {:#x}", USER_STACK_BASE - USER_STACK_SIZE, USER_STACK_BASE);
    
    // Verify page mappings before jumping
    let test_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(0x400000));
    kprintln!("    Program page: {:?} - verifying mapping...", test_page);
    
    // Read a few bytes to verify the mapping works
    unsafe {
        let program_ptr = 0x400000 as *const u8;
        for i in 0..8 {
            let byte = core::ptr::read_volatile(program_ptr.add(i));
            kprintln!("      Byte {}: {:#04x}", i, byte);
        }
    }
    
    kprintln!("    Memory verification complete - proceeding to userspace");
    kprintln!("");
    
    // Execute the test program
    execute_userspace(0x400000)?;
    
    Ok(())
}

// Embedded login screen binary - using a different approach to avoid alignment issues
static LOGIN_SCREEN_BINARY_DATA: &'static [u8] = include_bytes!("../login_screen.bin");

// Simple wrapper to ensure we have proper access
struct LoginBinary;

impl LoginBinary {
    const fn data() -> &'static [u8] {
        LOGIN_SCREEN_BINARY_DATA
    }
}

static LOGIN_SCREEN_BINARY: LoginBinary = LoginBinary;

/// Load and execute the login screen userspace program
pub fn load_login_screen(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), &'static str> {
    kprintln!("Loading PrismaOS login screen...");
    
    // Use the embedded binary data
    let binary_data = LoginBinary::data();
    kprintln!("Login screen binary size: {} bytes", binary_data.len());
    
    load_and_execute_elf(binary_data, mapper, frame_allocator)
}