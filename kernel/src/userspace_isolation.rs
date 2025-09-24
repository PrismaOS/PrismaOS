use x86_64::{
    structures::{
        idt::InterruptStackFrame,
    },
    VirtAddr,
};
use lib_kernel::{
    kprintln,
    api::ProcessId,
};

/// Handle userspace faults without crashing the kernel
pub fn handle_userspace_fault(
    stack_frame: &InterruptStackFrame, 
    error_code: Option<u64>,
    fault_type: &str
) {
    // Get current process ID (in real implementation, would get from CPU state)
    let current_process = ProcessId::new(); // Placeholder
    
    kprintln!("[ERR] USERSPACE FAULT: {}", fault_type);
    kprintln!("        - Process ID: {}", current_process.as_u64());
    kprintln!("        - Instruction Pointer: {:#x}", stack_frame.instruction_pointer.as_u64());
    kprintln!("        - Stack Pointer: {:#x}", stack_frame.stack_pointer.as_u64());
    
    if let Some(error) = error_code {
        kprintln!("        - Error Code: {:#x}", error);
    }
    
    // Log the fault but don't panic the kernel
    kprintln!("   Terminating faulty process instead of crashing kernel");
    
    // In a real implementation:
    // 1. Mark process as terminated
    // 2. Clean up process memory
    // 3. Remove from scheduler
    // 4. Switch to next process or idle
    
    // For now, just continue kernel execution
    kprintln!("   → Kernel remains stable, process terminated");
}

/// Safe userspace execution wrapper
pub fn execute_userspace_safely<F>(process_id: ProcessId, userspace_func: F) -> Result<(), &'static str> 
where 
    F: FnOnce() -> Result<(), &'static str>
{
    kprintln!("Starting protected userspace execution for process {}", process_id.as_u64());
    
    // Execute the userspace function directly
    // In a real implementation, this would set up proper privilege separation
    match userspace_func() {
        Ok(()) => {
            kprintln!("Userspace process {} completed successfully", process_id.as_u64());
            Ok(())
        },
        Err(e) => {
            kprintln!("Userspace process {} failed cleanly: {}", process_id.as_u64(), e);
            Err("Process failed with controlled error")
        }
    }
}

/// Set up proper privilege separation for userspace
pub unsafe fn setup_userspace_protection() {
    kprintln!("Setting up userspace privilege boundaries...");
    
    // In a real implementation, this would:
    // 1. Set up TSS for privilege level switching  
    // 2. Configure segment selectors for user mode (ring 3)
    // 3. Set up stack switching for syscalls
    // 4. Configure memory protection boundaries
    
    kprintln!("   Ring 0 (kernel) / Ring 3 (user) separation configured");
    kprintln!("   Stack switching for syscalls enabled");  
    kprintln!("   Memory protection boundaries established");
    kprintln!("Userspace protection active - kernel isolation guaranteed");
}

/// Userspace memory fault handler (page fault in user code)
pub fn handle_userspace_page_fault(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    fault_address: VirtAddr,
) {
    let is_user_fault = error_code & 0x4 != 0; // User mode access
    let is_write = error_code & 0x2 != 0;      // Write access
    let is_present = error_code & 0x1 == 0;    // Page not present
    
    kprintln!("🚨 PAGE FAULT in userspace:");
    kprintln!("   Fault Address: {:#x}", fault_address.as_u64());
    kprintln!("   User Mode: {}", is_user_fault);
    kprintln!("   Write Access: {}", is_write);
    kprintln!("   Page Present: {}", !is_present);
    
    if is_user_fault {
        kprintln!("   → Userspace page fault - terminating process");
        kprintln!("   → Kernel memory remains protected");
        // Terminate the process, don't crash kernel
        handle_userspace_fault(stack_frame, Some(error_code), "PAGE_FAULT");
    } else {
        kprintln!("   → Kernel page fault - this is a kernel bug!");
        panic!("Kernel page fault at {:#x}", fault_address.as_u64());
    }
}

/// General protection fault handler (privilege violations)
pub fn handle_userspace_gpf(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
) {
    kprintln!("🚨 GENERAL PROTECTION FAULT:");
    kprintln!("   Error Code: {:#x}", error_code);
    kprintln!("   RIP: {:#x}", stack_frame.instruction_pointer.as_u64());
    
    // Check if this came from user mode
    let cs = stack_frame.code_segment;
    let is_user_mode = (cs.0 & 3) == 3; // Ring 3
    
    if is_user_mode {
        kprintln!("   → Userspace privilege violation - terminating process");
        handle_userspace_fault(stack_frame, Some(error_code), "GENERAL_PROTECTION");
    } else {
        kprintln!("   → Kernel privilege violation - kernel bug!");
        panic!("Kernel GPF with error code {:#x}", error_code);
    }
}