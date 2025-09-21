/// System Call Entry Point
///
/// This module handles the low-level syscall entry and exit, including
/// setting up the SYSCALL/SYSRET mechanism and managing register state.

use core::arch::global_asm;
use x86_64::registers::model_specific::{Star, LStar, SFMask, Efer, EferFlags};
use x86_64::VirtAddr;

use crate::{kprintln, scheduler::get_current_process_id};
use super::{dispatch_syscall, SyscallArgs};

// Define the saved-register frame layout used by the assembly entry stub.
// This MUST match the exact order registers are pushed in the assembly code.
#[repr(C)]
#[derive(Debug)]
pub struct SyscallFrame {
    // Pushed in this exact order by assembly stub
    pub rax: u64,    // Last pushed (offset 0 from RSP)
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,    // First pushed (highest offset)
}

extern "C" {
    // Symbol defined by the assembly thunk below; LSTAR will point here.
    fn syscall_entry_asm();
}

// Assembly thunk for SYSCALL entry.
// This is the most critical part - any mistakes here will cause page faults!
global_asm!(r#"
    .global syscall_entry_asm
    .type syscall_entry_asm, @function
syscall_entry_asm:
    // SYSCALL instruction has already:
    // - Saved user RIP in RCX
    // - Saved user RFLAGS in R11
    // - Set CS to kernel code segment
    // - Set SS to kernel data segment (RSP unchanged)

    // We need to save user registers and set up kernel stack
    // CRITICAL: Push order must match SyscallFrame struct exactly!

    // Save callee-saved and argument registers in reverse struct order
    push r15
    push r14
    push r13
    push r12
    push r11        // Contains user RFLAGS
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx        // Contains user RIP
    push rbx
    push rax        // Contains syscall number

    // Now RSP points to our SyscallFrame
    // Pass frame pointer as first argument (RDI)
    mov rdi, rsp

    // Store user RIP and RFLAGS for potential use
    mov rsi, rcx    // User RIP (second argument)
    mov rdx, r11    // User RFLAGS (third argument)

    // Align stack to 16-byte boundary (required by System V ABI)
    and rsp, -16

    // Call the Rust handler
    call syscall_entry_rust

    // Restore original stack pointer (frame is still there)
    mov rsp, rdi

    // Store return value in frame's RAX slot
    mov [rsp], rax

    // Restore user registers in exact reverse order
    pop rax         // Return value now in RAX
    pop rbx
    pop rcx         // This restores user RIP for SYSRET
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11         // This restores user RFLAGS for SYSRET
    pop r12
    pop r13
    pop r14
    pop r15

    // SYSRET expects:
    // - RCX = user RIP (already restored)
    // - R11 = user RFLAGS (already restored)
    // - RAX = return value (already set)
    sysretq
"#);

// Rust-side syscall entry called from assembly thunk.
#[no_mangle]
extern "C" fn syscall_entry_rust(frame_ptr: *mut SyscallFrame, _user_rip: u64, _user_rflags: u64) -> u64 {
    // SAFETY: assembly stub guarantees frame_ptr is valid
    let frame = unsafe { &*frame_ptr };

    // Extract syscall arguments according to userspace ABI:
    // syscall_num in RAX, arguments in RBX, RCX, RDX, RSI, RDI, R8
    // BUT: RCX was overwritten by SYSCALL with user RIP, so we need to handle this
    let syscall_num = frame.rax;
    let arg0 = frame.rbx;
    // arg1 would be in RCX, but SYSCALL overwrote it with user RIP
    // So userspace needs to use a different register - let's use R9
    let arg1 = frame.r9;   // Changed from RCX to R9 due to SYSCALL behavior
    let arg2 = frame.rdx;
    let arg3 = frame.rsi;
    let arg4 = frame.rdi;
    let arg5 = frame.r8;

    // Get caller PID - create dummy if not available
    let caller_pid = get_current_process_id().unwrap_or_else(|| crate::api::ProcessId::new());

    let args = SyscallArgs {
        syscall_num,
        arg0,
        arg1,
        arg2,
        arg3,
        arg4,
        arg5,
    };

    // Dispatch syscall
    dispatch_syscall(args, caller_pid)
}

/// Set up SYSCALL/SYSRET MSRs for fast system calls
/// 
/// This configures the x86_64 SYSCALL mechanism which allows userspace
/// to make system calls with minimal overhead.
pub fn setup_syscall_msrs() {
    unsafe {
        // Enable SYSCALL/SYSRET in EFER
        let mut efer = Efer::read();
        efer |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        Efer::write(efer);

        // Set up STAR register using proper GDT selectors
        let selectors = crate::gdt::get_selectors();
        
        kprintln!("    GDT Layout for SYSCALL:");
        kprintln!("       Kernel CS: {:#x}", selectors.kernel_code().0);
        kprintln!("       Kernel DS: {:#x}", selectors.kernel_data().0);
        kprintln!("       User CS:   {:#x}", selectors.user_code().0);
        kprintln!("       User DS:   {:#x}", selectors.user_data().0);
        
        let result = Star::write(
            selectors.user_code(),
            selectors.user_data(), 
            selectors.kernel_code(),
            selectors.kernel_data(),
        );
        
        if let Err(e) = result {
            kprintln!("    STAR MSR write failed: {:?}, continuing without SYSCALL support", e);
            kprintln!("    This means userspace will need to use interrupts instead of SYSCALL");
            return;
        }

        // Set up LSTAR register (syscall entry point) to the assembly thunk symbol
        LStar::write(VirtAddr::new(syscall_entry_asm as u64));

        // Set up SFMASK register (flags to clear on syscall)
        // Clear interrupt flag to disable interrupts during syscall
        SFMask::write(x86_64::registers::rflags::RFlags::INTERRUPT_FLAG);
    }

    kprintln!("    [INFO] SYSCALL MSRs configured");
}

/// Assembly syscall entry point
/// 
/// This is the entry point that gets called when userspace executes
/// the SYSCALL instruction. For now, we'll use a stub implementation.
pub extern "C" fn syscall_entry() -> ! {
    // For now, just halt - in a real implementation this would be a naked function
    // with proper register saving and syscall dispatch
    kprintln!("[ERR] Syscall entry reached - not implemented yet");
    loop {
        x86_64::instructions::hlt();
    }
}

/// C-compatible syscall handler wrapper
/// 
/// This function extracts the syscall arguments from registers and calls
/// the main syscall dispatcher.
#[no_mangle]
extern "C" fn syscall_handler_wrapper(
    syscall_num: u64,
    arg0: u64, 
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> u64 {
    // Get the current process ID
    let caller_pid = get_current_process_id().unwrap_or_else(|| {
        kprintln!("⚠️  Syscall from unknown process");
        crate::api::ProcessId::new()
    });

    // Package arguments
    let args = SyscallArgs {
        syscall_num,
        arg0,
        arg1, 
        arg2,
        arg3,
        arg4,
        arg5,
    };

    // Dispatch the syscall
    dispatch_syscall(args, caller_pid)
}

/// Alternative syscall entry for testing/debugging
/// 
/// This can be used when SYSCALL/SYSRET is not available or for debugging.
pub fn test_syscall(
    syscall_num: u64,
    arg0: u64,
    arg1: u64, 
    arg2: u64,
    arg3: u64,
    arg4: u64,
) -> u64 {
    let caller_pid = get_current_process_id().unwrap_or_else(|| {
        crate::api::ProcessId::new()
    });

    let args = SyscallArgs {
        syscall_num,
        arg0,
        arg1,
        arg2,
        arg3,
        arg4,
        arg5: 0,
    };

    dispatch_syscall(args, caller_pid)
}