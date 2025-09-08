use alloc::sync::Arc;
use core::arch::asm;

use super::{
    objects::*, get_registry, IpcMessage, IpcResponse, ObjectHandle, 
    ProcessId, Rights, PixelFormat
};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub rsp: u64,
    pub rbp: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum SyscallNumber {
    CreateObject = 0,
    GetObject = 1,
    CallObject = 2,
    TransferCapability = 3,
    RevokeCapability = 4,
    Exit = 99,
}

impl TryFrom<u64> for SyscallNumber {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SyscallNumber::CreateObject),
            1 => Ok(SyscallNumber::GetObject),
            2 => Ok(SyscallNumber::CallObject),
            3 => Ok(SyscallNumber::TransferCapability),
            4 => Ok(SyscallNumber::RevokeCapability),
            99 => Ok(SyscallNumber::Exit),
            _ => Err(()),
        }
    }
}

pub fn handle_syscall(frame: &mut SyscallFrame) {
    let syscall_num = match SyscallNumber::try_from(frame.rax) {
        Ok(num) => num,
        Err(_) => {
            frame.rax = u64::MAX; // Invalid syscall
            return;
        }
    };

    let result = match syscall_num {
        SyscallNumber::CreateObject => sys_create_object(frame),
        SyscallNumber::GetObject => sys_get_object(frame),
        SyscallNumber::CallObject => sys_call_object(frame),
        SyscallNumber::TransferCapability => sys_transfer_capability(frame),
        SyscallNumber::RevokeCapability => sys_revoke_capability(frame),
        SyscallNumber::Exit => sys_exit(frame),
    };

    frame.rax = result;
}

fn sys_create_object(frame: &SyscallFrame) -> u64 {
    let object_type = frame.rbx;
    let arg1 = frame.rcx;
    let arg2 = frame.rdx;
    let arg3 = frame.rsi;
    
    // For demo purposes, assume process ID 1
    let process_id = ProcessId::new();

    let registry = get_registry();

    let result: Result<ObjectHandle, &'static str> = match object_type {
        0 => {
            // Create Surface (width, height, format)
            let surface = Arc::new(Surface::new(
                arg1 as u32,
                arg2 as u32,
                match arg3 {
                    0 => PixelFormat::Rgba8888,
                    1 => PixelFormat::Rgb888,
                    2 => PixelFormat::Bgra8888,
                    3 => PixelFormat::Bgr888,
                    _ => return u64::MAX,
                }
            ));
            registry.register_object(surface, process_id, Rights::ALL)
                .map_err(|_| "Failed to register surface")
        }
        1 => {
            // Create Buffer (width, height, format)
            let buffer = Arc::new(Buffer::new(
                arg1 as u32,
                arg2 as u32,
                match arg3 {
                    0 => PixelFormat::Rgba8888,
                    1 => PixelFormat::Rgb888,
                    2 => PixelFormat::Bgra8888,
                    3 => PixelFormat::Bgr888,
                    _ => return u64::MAX,
                }
            ));
            registry.register_object(buffer, process_id, Rights::ALL)
                .map_err(|_| "Failed to register buffer")
        }
        2 => {
            // Create EventStream
            let stream = Arc::new(EventStream::new());
            registry.register_object(stream, process_id, Rights::ALL)
                .map_err(|_| "Failed to register event stream")
        }
        _ => Err("Unknown object type"),
    };

    match result {
        Ok(handle) => handle.as_u64(),
        Err(_) => u64::MAX,
    }
}

fn sys_get_object(frame: &SyscallFrame) -> u64 {
    let handle = ObjectHandle(frame.rbx);
    let required_rights = Rights(frame.rcx);
    
    // For demo purposes, assume process ID 1
    let process_id = ProcessId::new();

    let registry = get_registry();
    
    match registry.get_object(handle, process_id, required_rights) {
        Ok(_) => 0, // Success
        Err(_) => 1, // Error
    }
}

fn sys_call_object(frame: &SyscallFrame) -> u64 {
    let handle = ObjectHandle(frame.rbx);
    let method = frame.rcx;
    let arg1 = frame.rdx;
    let arg2 = frame.rsi;
    
    // For demo purposes, assume process ID 1
    let process_id = ProcessId::new();

    let registry = get_registry();
    
    let object = match registry.get_object(handle, process_id, Rights::WRITE) {
        Ok(obj) => obj,
        Err(_) => return u64::MAX,
    };

    // Method dispatch based on object type
    if let Some(surface) = object.as_any().downcast_ref::<Surface>() {
        match method {
            0 => {
                // attach_buffer
                surface.attach_buffer(ObjectHandle(arg1));
                0
            }
            1 => {
                // commit
                surface.commit();
                0
            }
            _ => u64::MAX,
        }
    } else if let Some(stream) = object.as_any().downcast_ref::<EventStream>() {
        match method {
            0 => {
                // poll_event
                if stream.has_events() {
                    0 // Has events
                } else {
                    1 // No events
                }
            }
            _ => u64::MAX,
        }
    } else {
        u64::MAX
    }
}

fn sys_transfer_capability(frame: &SyscallFrame) -> u64 {
    let handle = ObjectHandle(frame.rbx);
    let from = ProcessId(frame.rcx);
    let to = ProcessId(frame.rdx);
    let rights = Rights(frame.rsi);

    let registry = get_registry();
    
    match registry.transfer_capability(handle, from, to, rights) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

fn sys_revoke_capability(frame: &SyscallFrame) -> u64 {
    let handle = ObjectHandle(frame.rbx);
    let process = ProcessId(frame.rcx);

    let registry = get_registry();
    
    match registry.revoke_capability(handle, process) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

fn sys_exit(frame: &SyscallFrame) -> u64 {
    let exit_code = frame.rbx;
    
    // For now, just halt the system
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}

pub unsafe fn enable_syscalls() {
    // Set up syscall MSRs for x86_64
    use x86_64::registers::model_specific::{Msr, KernelGsBase, Star, LStar, SFMask};

    // STAR: Set up CS/SS for syscall/sysret
    let star_value = (0x20u64 << 48) | (0x08u64 << 32);
    Star::write(Star::from_bits_unchecked(star_value));

    // LSTAR: syscall entry point
    LStar::write(VirtAddr::new(syscall_entry as u64));

    // FMASK: flags to clear on syscall
    SFMask::write(0x200); // Clear IF (interrupt flag)

    // Enable syscalls in EFER
    use x86_64::registers::model_specific::Efer;
    let mut efer = Efer::read();
    efer |= Efer::SYSTEM_CALL_EXTENSIONS;
    Efer::write(efer);
}

use x86_64::VirtAddr;

#[naked]
unsafe extern "C" fn syscall_entry() {
    asm!(
        "push rbp",
        "push rsp",
        "push r11", // rflags
        "push r10", // user rip
        "push r9",
        "push r8",
        "push rdi",
        "push rsi",
        "push rdx",
        "push rcx",
        "push rbx",
        "push rax",
        
        "mov rdi, rsp", // pass frame pointer
        "call {handle_syscall}",
        
        "pop rax",
        "pop rbx",
        "pop rcx",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10", // user rip -> rcx for sysret
        "pop r11", // rflags -> r11 for sysret
        "pop rsp",
        "pop rbp",
        
        "sysretq",
        handle_syscall = sym handle_syscall_wrapper,
        options(noreturn),
    );
}

#[no_mangle]
unsafe extern "C" fn handle_syscall_wrapper(frame: *mut SyscallFrame) {
    handle_syscall(&mut *frame);
}