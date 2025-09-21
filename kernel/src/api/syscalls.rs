use alloc::{sync::Arc, vec::Vec};
use core::arch::asm;

use super::{
    objects::*, get_registry, IpcMessage, IpcResponse, ObjectHandle, 
    ProcessId, Rights, PixelFormat
};
use crate::{elf::ElfLoader, executor::task::Task, memory::BootInfoFrameAllocator};
use x86_64::VirtAddr;

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
    CreateProcess = 5,
    LoadElf = 6,
    StartProcess = 7,
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
            5 => Ok(SyscallNumber::CreateProcess),
            6 => Ok(SyscallNumber::LoadElf),
            7 => Ok(SyscallNumber::StartProcess),
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
        SyscallNumber::CreateProcess => sys_create_process(frame),
        SyscallNumber::LoadElf => sys_load_elf(frame),
        SyscallNumber::StartProcess => sys_start_process(frame),
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
            registry.write().register_object(surface, process_id, Rights::ALL)
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
            registry.write().register_object(buffer, process_id, Rights::ALL)
                .map_err(|_| "Failed to register buffer")
        }
        2 => {
            // Create EventStream
            let stream = Arc::new(EventStream::new());
            registry.write().register_object(stream, process_id, Rights::ALL)
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
    
    match registry.read().get_object(handle, process_id, required_rights) {
        Ok(_) => 0, // Success
        Err(_) => 1, // Error
    }
}

fn sys_call_object(frame: &SyscallFrame) -> u64 {
    let handle = ObjectHandle(frame.rbx);
    let method = frame.rcx;
    let arg1 = frame.rdx;
    let _arg2 = frame.rsi;
    
    // For demo purposes, assume process ID 1
    let process_id = ProcessId::new();

    let registry = get_registry();
    
    let object = match registry.read().get_object(handle, process_id, Rights::WRITE) {
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
    
    match registry.write().transfer_capability(handle, from, to, rights) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

fn sys_revoke_capability(frame: &SyscallFrame) -> u64 {
    let handle = ObjectHandle(frame.rbx);
    let process = ProcessId(frame.rcx);

    let registry = get_registry();
    
    match registry.write().revoke_capability(handle, process) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

fn sys_exit(frame: &SyscallFrame) -> u64 {
    let _exit_code = frame.rbx;
    
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
    // Star::write(Star::from_bits_unchecked(star_value)); // Disabled due to API change

    // LSTAR: syscall entry point
    // LStar::write(VirtAddr::new(syscall_entry as u64)); // Disabled for now

    // FMASK: flags to clear on syscall
    // SFMask::write(RFlags::INTERRUPT_FLAG); // Clear IF (interrupt flag) - disabled

    // Enable syscalls in EFER
    use x86_64::registers::model_specific::Efer;
    let mut efer = Efer::read();
    // efer |= Efer::SYSTEM_CALL_EXTENSIONS; // Commented out due to API change
    Efer::write(efer);
}

// Syscall entry point disabled for now due to naked_asm! requirement
// TODO: Re-enable once naked_asm! is available or use alternative approach

fn sys_create_process(_frame: &SyscallFrame) -> u64 {
    // Create a new process with empty memory space
    let process_id = ProcessId::new();
    
    // TODO: Create task and store in scheduler
    // let task = Task::new(VirtAddr::new(0));
    // Store task in scheduler (placeholder - would need proper process management)
    
    process_id.as_u64()
}

fn sys_load_elf(frame: &SyscallFrame) -> u64 {
    let _process_id = ProcessId(frame.rbx);
    let elf_data_ptr = frame.rcx as *const u8;
    let elf_data_len = frame.rdx as usize;
    
    if elf_data_ptr.is_null() || elf_data_len == 0 {
        return u64::MAX;
    }
    
    // Copy ELF data from userspace (unsafe but necessary)
    let elf_data = unsafe {
        let slice = core::slice::from_raw_parts(elf_data_ptr, elf_data_len);
        Vec::from(slice)
    };
    
    // Parse and load ELF
    let elf_loader = match ElfLoader::new(elf_data) {
        Ok(loader) => loader,
        Err(_) => return u64::MAX,
    };
    
    // TODO: Get proper mapper and frame allocator for the process
    // For now this is a placeholder that would need proper process memory management
    // let mut mapper = get_process_mapper(process_id);
    // let mut frame_allocator = get_frame_allocator();
    // 
    // match elf_loader.load_segments(&mut mapper, &mut frame_allocator) {
    //     Ok(_) => elf_loader.entry_point().as_u64(),
    //     Err(_) => u64::MAX,
    // }
    
    // Return entry point for now
    elf_loader.entry_point().as_u64()
}

fn sys_start_process(frame: &SyscallFrame) -> u64 {
    let _process_id = ProcessId(frame.rbx);
    let _entry_point = VirtAddr::new(frame.rcx);
    
    // TODO: Switch to the process and start execution
    // This would involve:
    // 1. Setting up process memory space
    // 2. Creating initial stack
    // 3. Setting up registers for userspace execution
    // 4. Switching to the process via scheduler
    
    // For now, just return success
    0
}

#[no_mangle]
unsafe extern "C" fn handle_syscall_wrapper(frame: *mut SyscallFrame) {
    handle_syscall(&mut *frame);
}