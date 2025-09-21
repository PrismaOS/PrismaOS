use super::syscalls::{handle_syscall, SyscallFrame};
use crate::scheduler::{scheduler, switch::save_context_on_interrupt};
use x86_64::VirtAddr;

/// Syscall entry point called from userspace
/// This is the main entry point for all system calls
#[no_mangle]
pub unsafe extern "C" fn syscall_handler() {
    // Simple syscall handler that just creates a frame and calls the handler
    let mut frame = SyscallFrame {
        rax: 0, rbx: 0, rcx: 0, rdx: 0, rsi: 0, rdi: 0,
        r8: 0, r9: 0, r10: 0, r11: 0, rsp: 0, rbp: 0,
    };
    
    // Get syscall number and args from registers
    core::arch::asm!(
        "mov [rdi], rax",      // syscall number
        "mov [rdi + 8], rbx",  // arg1
        "mov [rdi + 16], rcx", // arg2  
        "mov [rdi + 24], rdx", // arg3
        "mov [rdi + 32], rsi", // arg4
        "mov [rdi + 40], rdi", // arg5
        in("rdi") &mut frame as *mut _,
        options(preserves_flags)
    );
    
    handle_syscall(&mut frame);
    
    // Return value is in frame.rax, put it back in rax register
    core::arch::asm!(
        "mov rax, [rdi]",
        in("rdi") &frame,
        lateout("rax") _,
        options(preserves_flags)
    );
}

/// C wrapper for syscall handler
#[no_mangle]
pub unsafe extern "C" fn handle_syscall_c(frame_ptr: *mut SyscallFrame) {
    let frame = &mut *frame_ptr;
    handle_syscall(frame);
}

/// Initialize syscall support
pub unsafe fn init_syscalls() {
    // For now, just log that syscalls are initialized
    // Full SYSCALL/SYSRET support requires more complex setup
    // that we'll implement when we have userspace processes
    crate::kprintln!("Syscall infrastructure ready (SYSCALL/SYSRET setup deferred)");
}

/// Fast syscall interface for common operations
pub fn syscall0(num: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") num => ret,
            lateout("rcx") _, // RCX clobbered by syscall
            lateout("r11") _, // R11 clobbered by syscall  
            options(nostack, preserves_flags)
        );
    }
    ret
}

pub fn syscall1(num: u64, arg1: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg1,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    ret
}

pub fn syscall2(num: u64, arg1: u64, arg2: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall", 
            inlateout("rax") num => ret,
            in("rdi") arg1,
            in("rsi") arg2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    ret
}

pub fn syscall3(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    ret
}

pub fn syscall4(num: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg1,
            in("rsi") arg2, 
            in("rdx") arg3,
            in("r10") arg4, // Note: r10 used instead of rcx for 4th arg
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    ret
}

/// Timer interrupt handler that calls scheduler
#[no_mangle]
pub extern "C" fn timer_interrupt_handler() {
    // Increment system tick counter
    crate::time::increment_tick();
    
    // Call scheduler tick
    crate::scheduler::scheduler_tick(0); // TODO: Get actual CPU ID
    
    // Check if we need to context switch  
    if let Some(next_process) = crate::scheduler::schedule_next() {
        // Perform context switch
        // This would normally save current context and switch to next process
        // For now, just update process runtime
        let current_tick = crate::time::current_tick();
        next_process.update_runtime(current_tick);
    }
}