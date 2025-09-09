use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use lazy_static::lazy_static;
use crate::gdt;
use crate::println;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        
        // Hardware interrupt handlers  
        // Note: Direct indexing not available in current x86_64 crate version
        // These would be set up differently in production code
            
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
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Increment system ticks
    crate::time::increment_tick();
    
    // Call scheduler tick for preemptive multitasking
    crate::scheduler::scheduler_tick(0); // TODO: Get actual CPU ID
    
    // Process pending events
    crate::events::event_dispatcher().process_pending_events();
    
    unsafe {
        crate::PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    
    // Read scan code from keyboard data port
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    
    // Convert scancode to key event and dispatch
    // This is a simplified mapping - real implementation would use
    // proper scan code tables and handle key states
    let key_code = scancode as u32;
    
    if scancode & 0x80 == 0 {
        // Key press (scan code without break bit)
        crate::events::dispatch_key_press(key_code, 0);
    } else {
        // Key release (scan code with break bit set)
        crate::events::dispatch_key_release(key_code & 0x7F, 0);
    }
    
    unsafe {
        crate::PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
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
        
        crate::PICS.lock().notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
}