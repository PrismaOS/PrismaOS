/// PrismaOS System Call Interface
/// 
/// This module provides the complete system call interface for userspace programs.
/// It includes syscall dispatch, validation, and implementation of all system services.

use crate::{
    kprintln,
    api::ProcessId,
    // scheduler::{scheduler},
    // elf::ElfLoader,
    // memory::BootInfoFrameAllocator,
};

pub mod entry;
pub mod handlers;

/// System call numbers
/// 
/// These correspond to the syscall numbers used by userspace programs.
/// Each syscall has a specific purpose and argument format.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallNumber {
    /// Create a new kernel object (Surface, Buffer, EventStream, etc.)
    CreateObject = 0,
    /// Get a handle to an existing object
    GetObject = 1,
    /// Call a method on an object
    CallObject = 2,
    /// Transfer capability to another process
    TransferCapability = 3,
    /// Revoke a capability
    RevokeCapability = 4,
    /// Create a new process
    CreateProcess = 5,
    /// Load an ELF binary into a process
    LoadElf = 6,
    /// Start execution of a process
    StartProcess = 7,
    /// Memory management operations
    Mmap = 8,
    Munmap = 9,
    Mprotect = 10,
    /// File system operations
    Open = 20,
    Close = 21,
    Read = 22,
    Write = 23,
    /// Process control
    Fork = 30,
    Exec = 31,
    Wait = 32,
    /// Exit the current process
    Exit = 99,
}

impl TryFrom<u64> for SyscallNumber {
    type Error = SyscallError;

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
            8 => Ok(SyscallNumber::Mmap),
            9 => Ok(SyscallNumber::Munmap),
            10 => Ok(SyscallNumber::Mprotect),
            20 => Ok(SyscallNumber::Open),
            21 => Ok(SyscallNumber::Close),
            22 => Ok(SyscallNumber::Read),
            23 => Ok(SyscallNumber::Write),
            30 => Ok(SyscallNumber::Fork),
            31 => Ok(SyscallNumber::Exec),
            32 => Ok(SyscallNumber::Wait),
            99 => Ok(SyscallNumber::Exit),
            _ => Err(SyscallError::InvalidSyscall),
        }
    }
}

/// System call errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallError {
    /// Invalid syscall number
    InvalidSyscall,
    /// Invalid argument
    InvalidArgument,
    /// Permission denied
    PermissionDenied,
    /// Resource not found
    NotFound,
    /// Resource already exists
    AlreadyExists,
    /// Out of memory
    OutOfMemory,
    /// Operation not supported
    NotSupported,
    /// Invalid state
    InvalidState,
}

impl From<SyscallError> for u64 {
    fn from(error: SyscallError) -> u64 {
        match error {
            SyscallError::InvalidSyscall => u64::MAX - 0,
            SyscallError::InvalidArgument => u64::MAX - 1,
            SyscallError::PermissionDenied => u64::MAX - 2,
            SyscallError::NotFound => u64::MAX - 3,
            SyscallError::AlreadyExists => u64::MAX - 4,
            SyscallError::OutOfMemory => u64::MAX - 5,
            SyscallError::NotSupported => u64::MAX - 6,
            SyscallError::InvalidState => u64::MAX - 7,
        }
    }
}

/// System call arguments
/// 
/// Represents the register state when a syscall is made.
/// Arguments are passed in registers according to the System V ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallArgs {
    /// Syscall number (from RAX)
    pub syscall_num: u64,
    /// First argument (from RBX)
    pub arg0: u64,
    /// Second argument (from RCX)
    pub arg1: u64,
    /// Third argument (from RDX)
    pub arg2: u64,
    /// Fourth argument (from RSI)
    pub arg3: u64,
    /// Fifth argument (from RDI)
    pub arg4: u64,
    /// Sixth argument (from R8)
    pub arg5: u64,
}

/// System call result
pub type SyscallResult = Result<u64, SyscallError>;

/// Main syscall dispatch function
/// 
/// This function is called from the syscall entry point and routes the
/// syscall to the appropriate handler based on the syscall number.
/// 
/// # Arguments
/// * `args` - Syscall arguments from registers
/// * `caller_pid` - Process ID of the calling process
/// 
/// # Returns
/// * Result value to return to userspace (in RAX)
pub fn dispatch_syscall(args: SyscallArgs, caller_pid: ProcessId) -> u64 {
    let syscall_num = match SyscallNumber::try_from(args.syscall_num) {
        Ok(num) => num,
        Err(err) => {
            kprintln!("[ERR] Invalid syscall number: {}", args.syscall_num);
            return err.into();
        }
    };

    kprintln!("[INFO] Syscall: {:?} from process {}", syscall_num, caller_pid.as_u64());

    let result = match syscall_num {
        SyscallNumber::CreateObject => handlers::create_object(caller_pid, args.arg0, args.arg1, args.arg2, args.arg3),
        SyscallNumber::GetObject => handlers::get_object(caller_pid, args.arg0, args.arg1),
        SyscallNumber::CallObject => handlers::call_object(caller_pid, args.arg0, args.arg1, args.arg2, args.arg3),
        SyscallNumber::TransferCapability => handlers::transfer_capability(caller_pid, args.arg0, args.arg1, args.arg2),
        SyscallNumber::RevokeCapability => handlers::revoke_capability(caller_pid, args.arg0),
        SyscallNumber::CreateProcess => handlers::create_process(caller_pid),
        SyscallNumber::LoadElf => handlers::load_elf(caller_pid, args.arg0, args.arg1, args.arg2),
        SyscallNumber::StartProcess => handlers::start_process(caller_pid, args.arg0, args.arg1),
        SyscallNumber::Mmap => handlers::mmap(caller_pid, args.arg0, args.arg1, args.arg2, args.arg3),
        SyscallNumber::Munmap => handlers::munmap(caller_pid, args.arg0, args.arg1),
        SyscallNumber::Mprotect => handlers::mprotect(caller_pid, args.arg0, args.arg1, args.arg2),
        SyscallNumber::Open => handlers::open(caller_pid, args.arg0, args.arg1, args.arg2),
        SyscallNumber::Close => handlers::close(caller_pid, args.arg0),
        SyscallNumber::Read => handlers::read(caller_pid, args.arg0, args.arg1, args.arg2),
        SyscallNumber::Write => handlers::write(caller_pid, args.arg0, args.arg1, args.arg2),
        SyscallNumber::Fork => handlers::fork(caller_pid),
        SyscallNumber::Exec => handlers::exec(caller_pid, args.arg0, args.arg1, args.arg2, args.arg3),
        SyscallNumber::Wait => handlers::wait(caller_pid, args.arg0),
        SyscallNumber::Exit => {
            // Exit doesn't return a value, it terminates the process
            Ok(0)
        },
        _ => {
            kprintln!("[ERR] Syscall {:?} not implemented yet", syscall_num);
            Err(SyscallError::NotSupported)
        }
    };

    match result {
        Ok(value) => {
            kprintln!("[INFO] Syscall {:?} returned: {:#x}", syscall_num, value);
            value
        }
        Err(err) => {
            kprintln!("[ERR] Syscall {:?} failed: {:?}", syscall_num, err);
            err.into()
        }
    }
}

/// Initialize the syscall system
/// 
/// Sets up the syscall entry point and configures the MSRs for fast syscalls.
pub fn init_syscalls() {
    kprintln!("Initializing syscall interface...");
    
    // Set up SYSCALL/SYSRET MSRs
    entry::setup_syscall_msrs();
}

/// Test syscall interface (for debugging)
pub fn test_syscall(
    syscall_num: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
) -> u64 {
    entry::test_syscall(syscall_num, arg0, arg1, arg2, arg3, arg4)
}