use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::registers::control::Cr2;
use lazy_static::lazy_static;
use crate::gdt;


lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        
        // Hardware interrupt handlers using proper range indexing
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
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

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    //println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    // Check if this came from userspace
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3; // Ring 3
    
    if is_user_mode {
        //println!("🚨 USERSPACE DOUBLE FAULT - Process will be terminated");
        //println!("   RIP: {:#x}", stack_frame.instruction_pointer.as_u64());
        //println!("   Error: {:#x}", error_code);
        //println!("   → Kernel remains stable");
        
        // In a real implementation: terminate process and continue
        // For now, we'll still halt but with better messaging
        //println!("   → Halting system (would normally just kill process)");
    } else {
        //println!("💥 KERNEL DOUBLE FAULT - Critical system error");
    }
    
    panic!("DOUBLE FAULT\nRIP: {:#x}\nError: {:#x}", 
           stack_frame.instruction_pointer.as_u64(), error_code);
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

/// Keyboard interrupt handler - reads scancode and adds to async queue
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // Read scancode from PS/2 keyboard port
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    // Add to async keyboard queue for processing
    crate::executor::keyboard::add_scancode(scancode);

    // Notify PIC that interrupt has been handled
    unsafe {
        crate::consts::PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

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
    
    crate::kprintln!("🚨 PAGE FAULT:");
    crate::kprintln!("   Address: {:#x}", fault_address.as_u64());
    crate::kprintln!("   User Mode: {}", is_user_mode);
    crate::kprintln!("   Caused by Write: {}", error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE));
    crate::kprintln!("   Page Present: {}", error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION));
    
    if is_user_mode {
        crate::kprintln!("   → Userspace page fault - would terminate process");
        crate::kprintln!("   → Kernel memory remains protected");
        // In a real implementation: terminate process, don't panic kernel
    } else {
        //println!("   → Kernel page fault - this is a kernel bug!");
    }
    
    panic!("Page fault at {:#x}", fault_address.as_u64());
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    // Check if this came from userspace
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3; // Ring 3
    
    crate::kprintln!("🚨 GENERAL PROTECTION FAULT:");
    crate::kprintln!("   Error Code: {:#x}", error_code);
    crate::kprintln!("   RIP: {:#x}", stack_frame.instruction_pointer.as_u64());
    crate::kprintln!("   User Mode: {}", is_user_mode);
    
    if is_user_mode {
        crate::kprintln!("   → Userspace privilege violation - would terminate process");
        crate::kprintln!("   → Kernel remains protected");
        // In a real implementation: terminate process, don't panic kernel
    } else {
        //println!("   → Kernel privilege violation - kernel bug!");
    }
    
    panic!("General protection fault with error code {:#x}", error_code);
}