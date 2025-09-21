use crate::scheduler::process::ProcessContext;
use x86_64::VirtAddr;

/// Assembly context switch function
/// Saves current context, loads new context, switches page table
#[no_mangle]
pub unsafe extern "C" fn context_switch(
    old_context: *mut ProcessContext,
    new_context: *const ProcessContext,
) {
    // Save current context
    core::arch::asm!(
        // Save general purpose registers
        "mov [rdi + 0x00], rax",
        "mov [rdi + 0x08], rbx", 
        "mov [rdi + 0x10], rcx",
        "mov [rdi + 0x18], rdx",
        "mov [rdi + 0x20], rsi",
        "mov [rdi + 0x28], rdi",
        "mov [rdi + 0x30], rbp",
        "mov [rdi + 0x38], rsp",
        "mov [rdi + 0x40], r8",
        "mov [rdi + 0x48], r9",
        "mov [rdi + 0x50], r10",
        "mov [rdi + 0x58], r11",
        "mov [rdi + 0x60], r12",
        "mov [rdi + 0x68], r13",
        "mov [rdi + 0x70], r14",
        "mov [rdi + 0x78], r15",
        
        // Save RIP (return address)
        "lea rax, [rip + 2f]",
        "mov [rdi + 0x80], rax",
        
        // Save flags
        "pushfq",
        "pop rax",
        "mov [rdi + 0x88], rax",
        
        // Save CR3
        "mov rax, cr3",
        "mov [rdi + 0x90], rax",
        
        // Load new context
        // Switch page table first
        "mov rax, [rsi + 0x90]",
        "mov cr3, rax",
        
        // Load new registers
        "mov rax, [rsi + 0x00]",
        "mov rbx, [rsi + 0x08]",
        "mov rcx, [rsi + 0x10]",
        "mov rdx, [rsi + 0x18]",
        "mov rbp, [rsi + 0x30]",
        "mov rsp, [rsi + 0x38]",
        "mov r8,  [rsi + 0x40]",
        "mov r9,  [rsi + 0x48]",
        "mov r10, [rsi + 0x50]",
        "mov r11, [rsi + 0x58]",
        "mov r12, [rsi + 0x60]",
        "mov r13, [rsi + 0x68]",
        "mov r14, [rsi + 0x70]",
        "mov r15, [rsi + 0x78]",
        
        // Restore flags
        "mov r12, [rsi + 0x88]",
        "push r12",
        "popfq",
        
        // Save RIP before overwriting registers
        "mov r12, [rsi + 0x80]",
        
        // Load RSI and RDI last
        "mov rdi, [rsi + 0x28]",
        "mov r13, [rsi + 0x20]", // RSI value to r13 temporarily
        "mov rsi, r13",
        
        // Jump to new RIP
        "jmp r12",
        
        "2:",
        in("rdi") old_context,
        in("rsi") new_context,
        clobber_abi("C")
    );
}

/// Initialize context for a new process
pub fn init_process_context(
    entry_point: VirtAddr,
    stack_top: VirtAddr,
    page_table: x86_64::PhysAddr,
) -> ProcessContext {
    ProcessContext::new(entry_point, stack_top, page_table)
}

/// Save context on interrupt (called from interrupt handlers)
#[no_mangle] 
pub unsafe extern "C" fn save_context_on_interrupt(
    context: *mut ProcessContext,
    interrupt_frame: *const InterruptFrame,
) {
    // Save the interrupted context
    (*context).rip = (*interrupt_frame).instruction_pointer.as_u64();
    (*context).rsp = (*interrupt_frame).stack_pointer.as_u64();
    (*context).rflags = (*interrupt_frame).cpu_flags;
    
    // Note: General purpose registers need to be saved by the interrupt handler
    // before calling this function
}

#[repr(C)]
pub struct InterruptFrame {
    pub instruction_pointer: VirtAddr,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: VirtAddr,
    pub stack_segment: u64,
}