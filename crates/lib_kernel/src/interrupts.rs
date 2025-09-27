use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::registers::control::Cr2;
use lazy_static::lazy_static;
use crate::gdt;
use crate::println;


lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // CPU Exception Handlers (0-31)
        idt.divide_error.set_handler_fn(divide_error_handler);
        idt.debug.set_handler_fn(debug_handler);
        idt.non_maskable_interrupt.set_handler_fn(nmi_handler);
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.overflow.set_handler_fn(overflow_handler);
        idt.bound_range_exceeded.set_handler_fn(bound_range_exceeded_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.device_not_available.set_handler_fn(device_not_available_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.invalid_tss.set_handler_fn(invalid_tss_handler);
        idt.segment_not_present.set_handler_fn(segment_not_present_handler);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.x87_floating_point.set_handler_fn(x87_floating_point_handler);
        idt.alignment_check.set_handler_fn(alignment_check_handler);
        idt.machine_check.set_handler_fn(machine_check_handler);
        idt.simd_floating_point.set_handler_fn(simd_floating_point_handler);
        idt.virtualization.set_handler_fn(virtualization_handler);
        idt.security_exception.set_handler_fn(security_exception_handler);
        
        // Hardware interrupt handlers using proper range indexing
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        // idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::Mouse.as_u8()].set_handler_fn(mouse_interrupt_handler);
            
        idt
    };
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 32,     // PIT Timer
    Keyboard = 33,  // PS/2 Keyboard
    Mouse = 44,     // PS/2 Mouse
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

pub fn init_idt() {
    IDT.load();
}

/// Initialize a minimal emergency IDT for early boot protection
/// This catches faults that occur before the full IDT is loaded
pub fn init_emergency_idt() {
    use x86_64::structures::idt::InterruptDescriptorTable;
    
    static mut EMERGENCY_IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();
    
    unsafe {
        // Set up only the most critical handlers
        EMERGENCY_IDT.double_fault.set_handler_fn(emergency_double_fault_handler);
        EMERGENCY_IDT.general_protection_fault.set_handler_fn(emergency_gpf_handler);
        EMERGENCY_IDT.page_fault.set_handler_fn(emergency_page_fault_handler);
        EMERGENCY_IDT.invalid_opcode.set_handler_fn(emergency_invalid_opcode_handler);
        
        // Load the emergency IDT
        EMERGENCY_IDT.load();
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    // Check if this came from userspace
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3; // Ring 3
    
    let details = if is_user_mode {
        "Double fault in userspace - process would be terminated"
    } else {
        "Critical double fault in kernel - system unstable"
    };
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "DOUBLE_FAULT",
        details,
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Increment system ticks
    crate::time::increment_tick();
    
    // Call scheduler tick for preemptive multitasking
    crate::scheduler::scheduler_tick(0); // TODO: Get actual CPU ID
    
    // Process pending events
    crate::events::event_dispatcher().process_pending_events();
    
    unsafe {
        crate::consts::PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

// TODO: Make More generic so as not to depend on a particular driver
// extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
//     // Let the device manager handle the keyboard interrupt
//     let handled = crate::drivers::device_manager().handle_interrupt(InterruptIndex::Keyboard.as_u8());
//     
//     if !handled {
//         // Fallback: directly add scancode to async queue if driver didn't handle it
//         use x86_64::instructions::port::Port;
//         let mut port = Port::new(0x60);
//         let scancode: u8 = unsafe { port.read() };
//         crate::executor::keyboard::add_scancode(scancode);
//     }
//     
//     unsafe {
//         crate::consts::PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
//     }
// }

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    
    // Read mouse data from PS/2 port
    let mut port = Port::new(0x60);
    let mouse_data: u8 = unsafe { port.read() };
    
    // This is a simplified mouse handler
    // Real PS/2 mouse protocol requires state machine and 3-byte packets
    static mut MOUSE_X: i32 = 0;
    static mut MOUSE_Y: i32 = 0;
    
    unsafe {
        // Simplified: treat data as relative movement
        let x_delta = (mouse_data as i8) as i32;
        MOUSE_X = (MOUSE_X + x_delta).clamp(0, 1024);
        MOUSE_Y = (MOUSE_Y + 1).clamp(0, 768); // Fake Y movement
        
        crate::events::dispatch_mouse_move(MOUSE_X, MOUSE_Y);
        
        crate::consts::PICS.lock().notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_address = Cr2::read().unwrap_or(x86_64::VirtAddr::new(0));
    
    // Check if this came from userspace
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3; // Ring 3
    
    let details = format!(
        "Page fault at address {:#x} - Write: {}, Present: {}", 
        fault_address.as_u64(),
        error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE),
        error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
    );
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "PAGE_FAULT",
        &details,
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code.bits() as u64)
    );
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    // Check if this came from userspace
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3; // Ring 3
    
    let details = format!("General protection fault - Error code: {:#x}", error_code);
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "GENERAL_PROTECTION_FAULT",
        &details,
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

// Additional fault handlers to catch all possible CPU exceptions

extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "DIVIDE_BY_ZERO_ERROR",
        &format!("Division by zero at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn debug_handler(stack_frame: InterruptStackFrame) {
    println!("DEBUG EXCEPTION at RIP: {:#x}", stack_frame.instruction_pointer.as_u64());
    // Debug exceptions are usually non-fatal, just log them
}

extern "x86-interrupt" fn nmi_handler(stack_frame: InterruptStackFrame) {
    crate::utils::bsod::trigger_comprehensive_bsod(
        "NON_MASKABLE_INTERRUPT", 
        "Critical hardware error - Non-maskable interrupt received",
        false, // NMI is always in kernel context
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn overflow_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "INTEGER_OVERFLOW",
        &format!("Arithmetic overflow at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn bound_range_exceeded_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "BOUND_RANGE_EXCEEDED",
        &format!("Array bounds exceeded at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "INVALID_OPCODE",
        &format!("Invalid instruction at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn device_not_available_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "DEVICE_NOT_AVAILABLE",
        &format!("FPU/SIMD device not available at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn invalid_tss_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    crate::utils::bsod::trigger_comprehensive_bsod(
        "INVALID_TSS",
        &format!("Invalid Task State Segment - Error: {:#x}, RIP: {:#x}", 
                error_code, stack_frame.instruction_pointer.as_u64()),
        false, // TSS errors are always kernel-level
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

extern "x86-interrupt" fn segment_not_present_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "SEGMENT_NOT_PRESENT",
        &format!("Segment not present - Selector: {:#x}, RIP: {:#x}", 
                error_code, stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

extern "x86-interrupt" fn stack_segment_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "STACK_SEGMENT_FAULT",
        &format!("Stack segment fault - Error: {:#x}, RIP: {:#x}", 
                error_code, stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

extern "x86-interrupt" fn x87_floating_point_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "X87_FLOATING_POINT_ERROR",
        &format!("x87 FPU floating point error at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn alignment_check_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "ALIGNMENT_CHECK",
        &format!("Memory alignment check failed - Error: {:#x}, RIP: {:#x}", 
                error_code, stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

extern "x86-interrupt" fn machine_check_handler(stack_frame: InterruptStackFrame) -> ! {
    // Machine check exceptions are always fatal
    crate::utils::bsod::trigger_comprehensive_bsod(
        "MACHINE_CHECK_EXCEPTION",
        "Critical hardware error detected by CPU",
        false, // Always kernel-level
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn simd_floating_point_handler(stack_frame: InterruptStackFrame) {
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3;
    
    crate::utils::bsod::trigger_comprehensive_bsod(
        "SIMD_FLOATING_POINT_ERROR",
        &format!("SIMD floating point error at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        is_user_mode,
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn virtualization_handler(stack_frame: InterruptStackFrame) {
    crate::utils::bsod::trigger_comprehensive_bsod(
        "VIRTUALIZATION_EXCEPTION",
        &format!("Virtualization exception at RIP: {:#x}", stack_frame.instruction_pointer.as_u64()),
        false, // Virtualization exceptions are kernel-level
        Some(stack_frame.instruction_pointer.as_u64()),
        None
    );
}

extern "x86-interrupt" fn security_exception_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    crate::utils::bsod::trigger_comprehensive_bsod(
        "SECURITY_EXCEPTION",
        &format!("Security exception - Error: {:#x}, RIP: {:#x}", 
                error_code, stack_frame.instruction_pointer.as_u64()),
        false, // Security exceptions are kernel-level
        Some(stack_frame.instruction_pointer.as_u64()),
        Some(error_code)
    );
}

// Emergency fault handlers for early boot protection
// These are used before the full IDT is loaded and must be very minimal

extern "x86-interrupt" fn emergency_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    // Very basic VGA output since nothing else may be initialized
    unsafe {
        let vga_buffer = 0xb8000 as *mut u16;
        // Clear screen with red background for emergency
        for i in 0..(80 * 25) {
            vga_buffer.add(i).write(0x4F00 | b' ' as u16); // White on red
        }
        
        let msg = b"EMERGENCY DOUBLE FAULT - EARLY BOOT";
        for (i, &byte) in msg.iter().enumerate() {
            if i < 80 {
                vga_buffer.add(i).write(0x4F00 | byte as u16);
            }
        }
        
        // Show RIP
        let rip_msg = b"RIP: ";
        let line2 = 80;
        for (i, &byte) in rip_msg.iter().enumerate() {
            vga_buffer.add(line2 + i).write(0x4F00 | byte as u16);
        }
    }
    
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn emergency_gpf_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    unsafe {
        let vga_buffer = 0xb8000 as *mut u16;
        for i in 0..(80 * 25) {
            vga_buffer.add(i).write(0x4F00 | b' ' as u16);
        }
        
        let msg = b"EMERGENCY GENERAL PROTECTION FAULT - EARLY BOOT";
        for (i, &byte) in msg.iter().enumerate() {
            if i < 80 {
                vga_buffer.add(i).write(0x4F00 | byte as u16);
            }
        }
    }
    
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn emergency_page_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    unsafe {
        let vga_buffer = 0xb8000 as *mut u16;
        for i in 0..(80 * 25) {
            vga_buffer.add(i).write(0x4F00 | b' ' as u16);
        }
        
        let msg = b"EMERGENCY PAGE FAULT - EARLY BOOT";
        for (i, &byte) in msg.iter().enumerate() {
            if i < 80 {
                vga_buffer.add(i).write(0x4F00 | byte as u16);
            }
        }
    }
    
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn emergency_invalid_opcode_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        let vga_buffer = 0xb8000 as *mut u16;
        for i in 0..(80 * 25) {
            vga_buffer.add(i).write(0x4F00 | b' ' as u16);
        }
        
        let msg = b"EMERGENCY INVALID OPCODE - EARLY BOOT";
        for (i, &byte) in msg.iter().enumerate() {
            if i < 80 {
                vga_buffer.add(i).write(0x4F00 | byte as u16);
            }
        }
    }
    
    loop {
        x86_64::instructions::hlt();
    }
}