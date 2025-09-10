#![no_std]

extern crate alloc;

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Userspace heap configuration
const HEAP_SIZE: usize = 1024 * 1024; // 1MB userspace heap

#[repr(align(16))]
struct UserHeap([u8; HEAP_SIZE]);

static mut USER_HEAP: UserHeap = UserHeap([0; HEAP_SIZE]);

pub unsafe fn init_userspace_heap() {
    ALLOCATOR.lock().init(USER_HEAP.0.as_mut_ptr(), HEAP_SIZE);
}

//#[panic_handler]
//fn panic(_info: &core::panic::PanicInfo) -> ! {
//    syscall_exit(1);
//}

// Note: alloc_error_handler requires nightly, commenting out for stable build
// #[alloc_error_handler]
// fn alloc_error_handler(_layout: alloc::alloc::Layout) -> ! {
//     syscall_exit(2);
// }

// Syscall interface
pub fn syscall_create_object(object_type: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 0",        // SyscallNumber::CreateObject
            "mov rbx, {obj_type}", // object_type
            "mov rcx, {a1}",     // arg1
            "mov rdx, {a2}",     // arg2
            "mov rsi, {a3}",     // arg3
            "syscall",
            out("rax") result,
            obj_type = in(reg) object_type,
            a1 = in(reg) arg1,
            a2 = in(reg) arg2,
            a3 = in(reg) arg3,
            out("rcx") _,
            out("rdx") _,
            out("rsi") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_get_object(handle: u64, rights: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 1",      // SyscallNumber::GetObject
            "mov rbx, {h}",    // handle
            "mov rcx, {r}",    // rights
            "syscall",
            out("rax") result,
            h = in(reg) handle,
            r = in(reg) rights,
            out("rcx") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_call_object(handle: u64, method: u64, arg1: u64, arg2: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 2",    // SyscallNumber::CallObject
            "mov rbx, {h}",  // handle
            "mov rcx, {m}",  // method
            "mov rdx, {a1}", // arg1
            "mov rsi, {a2}", // arg2
            "syscall",
            out("rax") result,
            h = in(reg) handle,
            m = in(reg) method,
            a1 = in(reg) arg1,
            a2 = in(reg) arg2,
            out("rcx") _,
            out("rdx") _,
            out("rsi") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_create_process() -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 5",    // SyscallNumber::CreateProcess
            "syscall",
            out("rax") result,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_load_elf(process_id: u64, elf_data: &[u8]) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 6",     // SyscallNumber::LoadElf
            "mov rbx, {pid}", // process_id
            "mov rcx, {ptr}", // elf_data.as_ptr()
            "mov rdx, {len}", // elf_data.len()
            "syscall",
            out("rax") result,
            pid = in(reg) process_id,
            ptr = in(reg) elf_data.as_ptr(),
            len = in(reg) elf_data.len(),
            out("rcx") _,
            out("rdx") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_start_process(process_id: u64, entry_point: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 7",    // SyscallNumber::StartProcess
            "mov rbx, {pid}", // process_id
            "mov rcx, {ep}",  // entry_point
            "syscall",
            out("rax") result,
            pid = in(reg) process_id,
            ep = in(reg) entry_point,
            out("rcx") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_exit(exit_code: u64) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rax, 99",     // SyscallNumber::Exit
            "mov rbx, {code}", // exit_code
            "syscall",
            code = in(reg) exit_code,
            options(noreturn)
        );
    }
}

// Simple graphics API wrapper
pub struct Surface {
    handle: u64,
}

impl Surface {
    pub fn new(width: u32, height: u32, format: u32) -> Option<Self> {
        let handle = syscall_create_object(0, width as u64, height as u64, format as u64);
        if handle == u64::MAX {
            None
        } else {
            Some(Surface { handle })
        }
    }

    pub fn attach_buffer(&self, buffer_handle: u64) -> bool {
        syscall_call_object(self.handle, 0, buffer_handle, 0) == 0
    }

    pub fn commit(&self) -> bool {
        syscall_call_object(self.handle, 1, 0, 0) == 0
    }
    
    pub fn handle(&self) -> u64 {
        self.handle
    }
}

pub struct Buffer {
    handle: u64,
}

impl Buffer {
    pub fn new(width: u32, height: u32, format: u32) -> Option<Self> {
        let handle = syscall_create_object(1, width as u64, height as u64, format as u64);
        if handle == u64::MAX {
            None
        } else {
            Some(Buffer { handle })
        }
    }
    
    pub fn handle(&self) -> u64 {
        self.handle
    }
}

pub struct EventStream {
    handle: u64,
}

impl EventStream {
    pub fn new() -> Option<Self> {
        let handle = syscall_create_object(2, 0, 0, 0);
        if handle == u64::MAX {
            None
        } else {
            Some(EventStream { handle })
        }
    }

    pub fn has_events(&self) -> bool {
        syscall_call_object(self.handle, 0, 0, 0) == 0
    }
}