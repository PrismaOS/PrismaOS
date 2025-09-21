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
// NOTE: Due to SYSCALL instruction behavior, RCX gets overwritten with return RIP
// So we use R9 for arg1 instead of RCX. Calling convention:
// RAX = syscall_num, RBX = arg0, R9 = arg1, RDX = arg2, RSI = arg3, RDI = arg4, R8 = arg5
pub fn syscall_create_object(object_type: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 0",          // SyscallNumber::CreateObject
            "mov rbx, {obj_type}", // arg0 = object_type
            "mov r9, {a1}",        // arg1 (using R9 instead of RCX)
            "mov rdx, {a2}",       // arg2
            "mov rsi, {a3}",       // arg3
            "syscall",
            out("rax") result,
            obj_type = in(reg) object_type,
            a1 = in(reg) arg1,
            a2 = in(reg) arg2,
            a3 = in(reg) arg3,
            out("rcx") _,   // RCX will be overwritten by SYSCALL
            out("rdx") _,
            out("rsi") _,
            out("r9") _,
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
            "mov rbx, {h}",    // arg0 = handle
            "mov r9, {r}",     // arg1 = rights (using R9)
            "syscall",
            out("rax") result,
            h = in(reg) handle,
            r = in(reg) rights,
            out("rcx") _,
            out("r9") _,
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
            "mov rbx, {h}",  // arg0 = handle
            "mov r9, {m}",   // arg1 = method (using R9)
            "mov rdx, {a1}", // arg2 = arg1
            "mov rsi, {a2}", // arg3 = arg2
            "syscall",
            out("rax") result,
            h = in(reg) handle,
            m = in(reg) method,
            a1 = in(reg) arg1,
            a2 = in(reg) arg2,
            out("rcx") _,
            out("rdx") _,
            out("rsi") _,
            out("r9") _,
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
            out("rcx") _,    // RCX will be overwritten by SYSCALL
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
            "mov rbx, {pid}", // arg0 = process_id
            "mov r9, {ptr}",  // arg1 = elf_data.as_ptr() (using R9)
            "mov rdx, {len}", // arg2 = elf_data.len()
            "syscall",
            out("rax") result,
            pid = in(reg) process_id,
            ptr = in(reg) elf_data.as_ptr(),
            len = in(reg) elf_data.len(),
            out("rcx") _,
            out("rdx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_start_process(process_id: u64, entry_point: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 7",     // SyscallNumber::StartProcess
            "mov rbx, {pid}", // arg0 = process_id
            "mov r9, {ep}",   // arg1 = entry_point (using R9)
            "syscall",
            out("rax") result,
            pid = in(reg) process_id,
            ep = in(reg) entry_point,
            out("rcx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_exit(exit_code: u64) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rax, 99",     // SyscallNumber::Exit
            "mov rbx, {code}", // arg0 = exit_code
            "syscall",
            code = in(reg) exit_code,
            options(noreturn)
        );
    }
}

// Additional syscall wrappers
pub fn syscall_mmap(addr: u64, length: u64, prot: u64, flags: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 8",      // SyscallNumber::Mmap
            "mov rbx, {addr}", // arg0 = addr
            "mov r9, {len}",   // arg1 = length
            "mov rdx, {prot}", // arg2 = prot
            "mov rsi, {flags}", // arg3 = flags
            "syscall",
            out("rax") result,
            addr = in(reg) addr,
            len = in(reg) length,
            prot = in(reg) prot,
            flags = in(reg) flags,
            out("rcx") _,
            out("rdx") _,
            out("rsi") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_munmap(addr: u64, length: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 9",      // SyscallNumber::Munmap
            "mov rbx, {addr}", // arg0 = addr
            "mov r9, {len}",   // arg1 = length
            "syscall",
            out("rax") result,
            addr = in(reg) addr,
            len = in(reg) length,
            out("rcx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_mprotect(addr: u64, length: u64, prot: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 10",     // SyscallNumber::Mprotect
            "mov rbx, {addr}", // arg0 = addr
            "mov r9, {len}",   // arg1 = length
            "mov rdx, {prot}", // arg2 = prot
            "syscall",
            out("rax") result,
            addr = in(reg) addr,
            len = in(reg) length,
            prot = in(reg) prot,
            out("rcx") _,
            out("rdx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

// File system syscalls
pub fn syscall_open(path_ptr: *const u8, path_len: u64, flags: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 20",      // SyscallNumber::Open
            "mov rbx, {ptr}",   // arg0 = path_ptr
            "mov r9, {len}",    // arg1 = path_len
            "mov rdx, {flags}", // arg2 = flags
            "syscall",
            out("rax") result,
            ptr = in(reg) path_ptr,
            len = in(reg) path_len,
            flags = in(reg) flags,
            out("rcx") _,
            out("rdx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_close(fd: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 21",   // SyscallNumber::Close
            "mov rbx, {fd}", // arg0 = fd
            "syscall",
            out("rax") result,
            fd = in(reg) fd,
            out("rcx") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_read(fd: u64, buf_ptr: *mut u8, count: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 22",    // SyscallNumber::Read
            "mov rbx, {fd}",  // arg0 = fd
            "mov r9, {ptr}",  // arg1 = buf_ptr
            "mov rdx, {cnt}", // arg2 = count
            "syscall",
            out("rax") result,
            fd = in(reg) fd,
            ptr = in(reg) buf_ptr,
            cnt = in(reg) count,
            out("rcx") _,
            out("rdx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_write(fd: u64, buf_ptr: *const u8, count: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 23",    // SyscallNumber::Write
            "mov rbx, {fd}",  // arg0 = fd
            "mov r9, {ptr}",  // arg1 = buf_ptr
            "mov rdx, {cnt}", // arg2 = count
            "syscall",
            out("rax") result,
            fd = in(reg) fd,
            ptr = in(reg) buf_ptr,
            cnt = in(reg) count,
            out("rcx") _,
            out("rdx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

// Process control syscalls
pub fn syscall_fork() -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 30",  // SyscallNumber::Fork
            "syscall",
            out("rax") result,
            out("rcx") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_exec(path_ptr: *const u8, path_len: u64, args_ptr: *const u64, args_count: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 31",      // SyscallNumber::Exec
            "mov rbx, {path}",  // arg0 = path_ptr
            "mov r9, {plen}",   // arg1 = path_len
            "mov rdx, {args}",  // arg2 = args_ptr
            "mov rsi, {acnt}",  // arg3 = args_count
            "syscall",
            out("rax") result,
            path = in(reg) path_ptr,
            plen = in(reg) path_len,
            args = in(reg) args_ptr,
            acnt = in(reg) args_count,
            out("rcx") _,
            out("rdx") _,
            out("rsi") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_wait(pid: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 32",   // SyscallNumber::Wait
            "mov rbx, {pid}", // arg0 = pid
            "syscall",
            out("rax") result,
            pid = in(reg) pid,
            out("rcx") _,
            clobber_abi("system")
        );
        result
    }
}

// Capability syscalls
pub fn syscall_transfer_capability(handle: u64, target_pid: u64, rights: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 3",       // SyscallNumber::TransferCapability
            "mov rbx, {handle}", // arg0 = handle
            "mov r9, {target}", // arg1 = target_pid
            "mov rdx, {rights}", // arg2 = rights
            "syscall",
            out("rax") result,
            handle = in(reg) handle,
            target = in(reg) target_pid,
            rights = in(reg) rights,
            out("rcx") _,
            out("rdx") _,
            out("r9") _,
            clobber_abi("system")
        );
        result
    }
}

pub fn syscall_revoke_capability(handle: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            "mov rax, 4",       // SyscallNumber::RevokeCapability
            "mov rbx, {handle}", // arg0 = handle
            "syscall",
            out("rax") result,
            handle = in(reg) handle,
            out("rcx") _,
            clobber_abi("system")
        );
        result
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