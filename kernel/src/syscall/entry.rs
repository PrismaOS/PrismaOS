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
// The assembly stub will push general-purpose registers in this order so
// the Rust handler can read syscall number (in rax) and arguments.
#[repr(C)]
pub struct SyscallFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    // The assembly stub will pass RIP and RFLAGS separately as arguments
}

extern "C" {
    // Symbol defined by the assembly thunk below; LSTAR will point here.
    fn syscall_entry_asm();
}

// Assembly thunk for SYSCALL entry.
// Behavior:
//  - On entry (user executes `syscall`), CPU sets RIP to LSTAR and switches to kernel CS.
//  - This thunk saves caller registers to the stack in a well-known layout and
//    passes a pointer to that frame, plus saved RIP and RFLAGS, to `syscall_entry_rust`.
//  - After Rust handler returns (return value in RAX), thunk restores registers
//    and performs `sysretq` to return to userspace.
// Note: Keep this asm minimal and careful about clobbers; we preserve call-clobbered regs.
global_asm!(r#"
    .global syscall_entry_asm
    .type syscall_entry_asm, @function
syscall_entry_asm:
    // Save general-purpose registers in a fixed order
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax

    // At this point RSP points to the saved frame. Pass pointer in RDI (first arg) to C handler
    mov rdi, rsp

    // The SYSCALL instruction places return RIP in RCX and RFLAGS in R11 per AMD/Intel conventions
    // but for our Rust handler we also pass them explicitly: pass RCX (user RIP) in RSI, R11 (rflags) in RDX
    mov rsi, rcx
    mov rdx, r11

    // Call the Rust handler: extern "C" fn syscall_entry_rust(frame: *mut SyscallFrame, rip: u64, rflags: u64) -> u64
    // The handler will return the result in RAX.
    call syscall_entry_rust

    // After return, we expect RAX contains return value to user. Put it in the saved RAX slot.
    mov [rsp + 8*0], rax  // top of saved pushes: rax is last pushed, which is at rsp (offset 0)

    // Restore registers in reverse order
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15



    // Return to userspace using SYSRETQ - RCX and R11 hold user RIP and RFLAGS respectively
    sysretq
"#);

// Rust-side syscall entry called from assembly thunk.
// Accepts a pointer to the saved frame, the return RIP, and RFLAGS. Returns a u64 syscall result.
#[no_mangle]
extern "C" fn syscall_entry_rust(frame_ptr: *mut SyscallFrame, _rip: u64, _rflags: u64) -> u64 {
    // SAFETY: assembly ensures frame_ptr is valid and points to pushed registers
    let frame = unsafe { &mut *frame_ptr };

    // Extract syscall number and args using the convention used by userspace runtime:
    // Userspace places syscall number in RAX, args in RBX, RCX, RDX, RSI, RDI, R8
    let syscall_num = frame.rax;
    let arg0 = frame.rbx;
    let arg1 = frame.rcx;
    let arg2 = frame.rdx;
    let arg3 = frame.rsi;
    let arg4 = frame.rdi;
    let arg5 = frame.r8;

    // Get caller PID if available; do not panic if unavailable
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

    // Dispatch syscall and convert result to u64
    let result = dispatch_syscall(args, caller_pid);

    // Return value will be placed in RAX by caller (assembly thunk)
    result
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