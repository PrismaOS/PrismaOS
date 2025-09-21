/// System Call Entry Point
/// 
/// This module handles the low-level syscall entry and exit, including
/// setting up the SYSCALL/SYSRET mechanism and managing register state.

use core::arch::asm;
use x86_64::registers::model_specific::{Star, LStar, SFMask, Efer, EferFlags};
use x86_64::{VirtAddr, PrivilegeLevel};

use crate::{kprintln, scheduler::get_current_process_id};
use super::{dispatch_syscall, SyscallArgs};

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

        // Set up LSTAR register (syscall entry point)
        LStar::write(VirtAddr::new(syscall_entry as u64));

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

/// Re-export test function for external access
pub use test_syscall as test;