//! System call handling for different architectures and operating systems.

use crate::error::{ElfError, Result};
use crate::arch::{ExecutionState, AArch64ExecutionState, RiscVExecutionState};
use alloc::{vec::Vec, string::String, vec, format};

/// System call number definitions for Linux x86_64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxSyscall {
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Stat = 4,
    Fstat = 5,
    Lstat = 6,
    Poll = 7,
    Lseek = 8,
    Mmap = 9,
    Mprotect = 10,
    Munmap = 11,
    Brk = 12,
    RtSigaction = 13,
    RtSigprocmask = 14,
    RtSigreturn = 15,
    Ioctl = 16,
    Pread64 = 17,
    Pwrite64 = 18,
    Readv = 19,
    Writev = 20,
    Access = 21,
    Pipe = 22,
    Select = 23,
    SchedYield = 24,
    Mremap = 25,
    Msync = 26,
    Mincore = 27,
    Madvise = 28,
    Shmget = 29,
    Shmat = 30,
    Shmctl = 31,
    Dup = 32,
    Dup2 = 33,
    Pause = 34,
    Nanosleep = 35,
    Getitimer = 36,
    Alarm = 37,
    Setitimer = 38,
    Getpid = 39,
    Sendfile = 40,
    Socket = 41,
    Connect = 42,
    Accept = 43,
    Sendto = 44,
    Recvfrom = 45,
    Sendmsg = 46,
    Recvmsg = 47,
    Shutdown = 48,
    Bind = 49,
    Listen = 50,
    Getsockname = 51,
    Getpeername = 52,
    Socketpair = 53,
    Setsockopt = 54,
    Getsockopt = 55,
    Clone = 56,
    Fork = 57,
    Vfork = 58,
    Execve = 59,
    Exit = 60,
    Wait4 = 61,
    Kill = 62,
    Uname = 63,
    Semget = 64,
    Semop = 65,
    Semctl = 66,
    Shmdt = 67,
    Msgget = 68,
    Msgsnd = 69,
    Msgrcv = 70,
    Msgctl = 71,
    Fcntl = 72,
    Flock = 73,
    Fsync = 74,
    Fdatasync = 75,
    Truncate = 76,
    Ftruncate = 77,
    Getdents = 78,
    Getcwd = 79,
    Chdir = 80,
    Fchdir = 81,
    Rename = 82,
    Mkdir = 83,
    Rmdir = 84,
    Creat = 85,
    Link = 86,
    Unlink = 87,
    Symlink = 88,
    Readlink = 89,
    Chmod = 90,
    Fchmod = 91,
    Chown = 92,
    Fchown = 93,
    Lchown = 94,
    Umask = 95,
    Gettimeofday = 96,
    Getrlimit = 97,
    Getrusage = 98,
    Sysinfo = 99,
    Times = 100,
}

impl LinuxSyscall {
    /// Convert from raw syscall number
    pub fn from_number(num: u64) -> Option<Self> {
        match num {
            0 => Some(Self::Read),
            1 => Some(Self::Write),
            2 => Some(Self::Open),
            3 => Some(Self::Close),
            9 => Some(Self::Mmap),
            10 => Some(Self::Mprotect),
            11 => Some(Self::Munmap),
            12 => Some(Self::Brk),
            39 => Some(Self::Getpid),
            60 => Some(Self::Exit),
            79 => Some(Self::Getcwd),
            96 => Some(Self::Gettimeofday),
            _ => None,
        }
    }
}

/// System call arguments
#[derive(Debug, Clone)]
pub struct SyscallArgs {
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
    pub arg5: u64,
}

/// System call result
#[derive(Debug, Clone)]
pub enum SyscallResult {
    /// Success with return value
    Success(u64),
    /// Error with errno
    Error(i32),
    /// Exit process
    Exit(u64),
}

/// File descriptor table for process
#[derive(Debug, Clone)]
pub struct FileDescriptorTable {
    fds: Vec<Option<FileDescriptor>>,
    next_fd: u32,
}

impl FileDescriptorTable {
    pub fn new() -> Self {
        let mut fds = Vec::with_capacity(64);

        // Standard file descriptors
        fds.push(Some(FileDescriptor::stdin()));   // 0
        fds.push(Some(FileDescriptor::stdout()));  // 1
        fds.push(Some(FileDescriptor::stderr()));  // 2

        // Fill remaining slots
        for _ in 3..64 {
            fds.push(None);
        }

        Self {
            fds,
            next_fd: 3,
        }
    }

    pub fn allocate(&mut self, fd: FileDescriptor) -> Result<u32> {
        // Find an empty slot
        for i in self.next_fd as usize..self.fds.len() {
            if self.fds[i].is_none() {
                self.fds[i] = Some(fd);
                return Ok(i as u32);
            }
        }

        // Extend table if needed
        if self.fds.len() < 1024 {
            let fd_num = self.fds.len() as u32;
            self.fds.push(Some(fd));
            Ok(fd_num)
        } else {
            Err(ElfError::AllocationFailed)
        }
    }

    pub fn get(&self, fd: u32) -> Option<&FileDescriptor> {
        self.fds.get(fd as usize)?.as_ref()
    }

    pub fn get_mut(&mut self, fd: u32) -> Option<&mut FileDescriptor> {
        self.fds.get_mut(fd as usize)?.as_mut()
    }

    pub fn deallocate(&mut self, fd: u32) -> Option<FileDescriptor> {
        if let Some(slot) = self.fds.get_mut(fd as usize) {
            slot.take()
        } else {
            None
        }
    }
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

/// File descriptor types
#[derive(Debug, Clone)]
pub enum FileDescriptor {
    /// Standard input
    Stdin,
    /// Standard output
    Stdout,
    /// Standard error
    Stderr,
    /// Regular file
    File {
        path: String,
        offset: u64,
        writable: bool,
    },
    /// Socket
    Socket {
        local_addr: Option<String>,
        remote_addr: Option<String>,
    },
    /// Pipe
    Pipe {
        read_end: bool,
    },
}

impl FileDescriptor {
    pub fn stdin() -> Self {
        Self::Stdin
    }

    pub fn stdout() -> Self {
        Self::Stdout
    }

    pub fn stderr() -> Self {
        Self::Stderr
    }

    pub fn file(path: String, writable: bool) -> Self {
        Self::File {
            path,
            offset: 0,
            writable,
        }
    }
}

/// Virtual file system interface
pub trait VirtualFileSystem {
    /// Open a file
    fn open(&mut self, path: &str, flags: u32) -> Result<FileDescriptor>;

    /// Read from file descriptor
    fn read(&mut self, fd: &mut FileDescriptor, buffer: &mut [u8]) -> Result<usize>;

    /// Write to file descriptor
    fn write(&mut self, fd: &mut FileDescriptor, data: &[u8]) -> Result<usize>;

    /// Close file descriptor
    fn close(&mut self, fd: FileDescriptor) -> Result<()>;

    /// Get file stats
    fn stat(&self, path: &str) -> Result<FileStat>;
}

/// File statistics
#[derive(Debug, Clone)]
pub struct FileStat {
    pub size: u64,
    pub is_dir: bool,
    pub is_file: bool,
    pub permissions: u32,
}

/// Simple in-memory VFS for testing
#[derive(Debug)]
pub struct SimpleVfs {
    files: Vec<(String, Vec<u8>)>,
}

impl SimpleVfs {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, path: String, content: Vec<u8>) {
        self.files.push((path, content));
    }
}

impl VirtualFileSystem for SimpleVfs {
    fn open(&mut self, path: &str, _flags: u32) -> Result<FileDescriptor> {
        for (file_path, _) in &self.files {
            if file_path == path {
                return Ok(FileDescriptor::file(String::from(path), true));
            }
        }
        Err(ElfError::InvalidAddress) // File not found
    }

    fn read(&mut self, fd: &mut FileDescriptor, buffer: &mut [u8]) -> Result<usize> {
        match fd {
            FileDescriptor::File { path, offset, .. } => {
                for (file_path, content) in &self.files {
                    if file_path == path {
                        let start = *offset as usize;
                        let end = (start + buffer.len()).min(content.len());
                        if start >= content.len() {
                            return Ok(0); // EOF
                        }
                        let bytes_read = end - start;
                        buffer[..bytes_read].copy_from_slice(&content[start..end]);
                        *offset += bytes_read as u64;
                        return Ok(bytes_read);
                    }
                }
                Err(ElfError::InvalidAddress)
            }
            _ => Err(ElfError::UnsupportedOperation),
        }
    }

    fn write(&mut self, fd: &mut FileDescriptor, data: &[u8]) -> Result<usize> {
        match fd {
            FileDescriptor::Stdout | FileDescriptor::Stderr => {
                // In a real implementation, this would write to console/log
                Ok(data.len())
            }
            FileDescriptor::File { path, writable, .. } => {
                if !*writable {
                    return Err(ElfError::PermissionDenied);
                }
                // Simple append-only write for now
                for (file_path, content) in &mut self.files {
                    if file_path == path {
                        content.extend_from_slice(data);
                        return Ok(data.len());
                    }
                }
                Err(ElfError::InvalidAddress)
            }
            _ => Err(ElfError::UnsupportedOperation),
        }
    }

    fn close(&mut self, _fd: FileDescriptor) -> Result<()> {
        Ok(())
    }

    fn stat(&self, path: &str) -> Result<FileStat> {
        for (file_path, content) in &self.files {
            if file_path == path {
                return Ok(FileStat {
                    size: content.len() as u64,
                    is_dir: false,
                    is_file: true,
                    permissions: 0o644,
                });
            }
        }
        Err(ElfError::InvalidAddress)
    }
}

impl Default for SimpleVfs {
    fn default() -> Self {
        Self::new()
    }
}

/// System call handler
pub struct SyscallHandler<VFS: VirtualFileSystem> {
    fd_table: FileDescriptorTable,
    vfs: VFS,
    pid: u32,
    exit_code: Option<u64>,
}

impl<VFS: VirtualFileSystem> SyscallHandler<VFS> {
    /// Create a new syscall handler
    pub fn new(vfs: VFS) -> Self {
        Self {
            fd_table: FileDescriptorTable::new(),
            vfs,
            pid: 1,
            exit_code: None,
        }
    }

    /// Handle x86_64 system call
    pub fn handle_x86_64_syscall(&mut self, state: &mut ExecutionState) -> Result<SyscallResult> {
        let syscall_num = state.rax;
        let args = SyscallArgs {
            arg0: state.rdi,
            arg1: state.rsi,
            arg2: state.rdx,
            arg3: state.r10,
            arg4: state.r8,
            arg5: state.r9,
        };

        let result = self.handle_syscall(syscall_num, args)?;

        // Update return value
        match &result {
            SyscallResult::Success(val) => state.rax = *val,
            SyscallResult::Error(errno) => state.rax = (*errno as i64) as u64,
            SyscallResult::Exit(_) => {} // Process will exit
        }

        Ok(result)
    }

    /// Handle AArch64 system call
    pub fn handle_aarch64_syscall(&mut self, state: &mut AArch64ExecutionState) -> Result<SyscallResult> {
        let syscall_num = state.x[8];
        let args = SyscallArgs {
            arg0: state.x[0],
            arg1: state.x[1],
            arg2: state.x[2],
            arg3: state.x[3],
            arg4: state.x[4],
            arg5: state.x[5],
        };

        let result = self.handle_syscall(syscall_num, args)?;

        // Update return value
        match &result {
            SyscallResult::Success(val) => state.x[0] = *val,
            SyscallResult::Error(errno) => state.x[0] = (*errno as i64) as u64,
            SyscallResult::Exit(_) => {} // Process will exit
        }

        Ok(result)
    }

    /// Handle RISC-V system call
    pub fn handle_riscv_syscall(&mut self, state: &mut RiscVExecutionState) -> Result<SyscallResult> {
        let syscall_num = state.x[17]; // a7
        let args = SyscallArgs {
            arg0: state.x[10], // a0
            arg1: state.x[11], // a1
            arg2: state.x[12], // a2
            arg3: state.x[13], // a3
            arg4: state.x[14], // a4
            arg5: state.x[15], // a5
        };

        let result = self.handle_syscall(syscall_num, args)?;

        // Update return value
        match &result {
            SyscallResult::Success(val) => state.x[10] = *val, // a0
            SyscallResult::Error(errno) => state.x[10] = (*errno as i64) as u64,
            SyscallResult::Exit(_) => {} // Process will exit
        }

        Ok(result)
    }

    /// Generic system call handler
    fn handle_syscall(&mut self, syscall_num: u64, args: SyscallArgs) -> Result<SyscallResult> {
        if let Some(syscall) = LinuxSyscall::from_number(syscall_num) {
            match syscall {
                LinuxSyscall::Read => self.sys_read(args),
                LinuxSyscall::Write => self.sys_write(args),
                LinuxSyscall::Open => self.sys_open(args),
                LinuxSyscall::Close => self.sys_close(args),
                LinuxSyscall::Exit => self.sys_exit(args),
                LinuxSyscall::Getpid => self.sys_getpid(args),
                LinuxSyscall::Brk => self.sys_brk(args),
                LinuxSyscall::Mmap => self.sys_mmap(args),
                LinuxSyscall::Munmap => self.sys_munmap(args),
                LinuxSyscall::Getcwd => self.sys_getcwd(args),
                LinuxSyscall::Gettimeofday => self.sys_gettimeofday(args),
                _ => {
                    // Unsupported syscall
                    Ok(SyscallResult::Error(-38)) // ENOSYS
                }
            }
        } else {
            // Unknown syscall number
            Ok(SyscallResult::Error(-38)) // ENOSYS
        }
    }

    /// sys_read implementation
    fn sys_read(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let fd = args.arg0 as u32;
        let buf_ptr = args.arg1;
        let count = args.arg2 as usize;

        if let Some(fd_entry) = self.fd_table.get_mut(fd) {
            // In a real implementation, we'd read into the buffer at buf_ptr
            // For now, simulate reading
            match fd_entry {
                FileDescriptor::Stdin => {
                    // Simulate reading from stdin
                    Ok(SyscallResult::Success(0)) // EOF
                }
                _ => {
                    let mut buffer = vec![0u8; count];
                    let bytes_read = self.vfs.read(fd_entry, &mut buffer)?;
                    Ok(SyscallResult::Success(bytes_read as u64))
                }
            }
        } else {
            Ok(SyscallResult::Error(-9)) // EBADF
        }
    }

    /// sys_write implementation
    fn sys_write(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let fd = args.arg0 as u32;
        let buf_ptr = args.arg1;
        let count = args.arg2 as usize;

        if let Some(fd_entry) = self.fd_table.get_mut(fd) {
            // In a real implementation, we'd read data from buf_ptr
            // For now, simulate writing
            let data = vec![0u8; count]; // Placeholder data
            let bytes_written = self.vfs.write(fd_entry, &data)?;
            Ok(SyscallResult::Success(bytes_written as u64))
        } else {
            Ok(SyscallResult::Error(-9)) // EBADF
        }
    }

    /// sys_open implementation
    fn sys_open(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let pathname_ptr = args.arg0;
        let flags = args.arg1 as u32;
        let _mode = args.arg2;

        // In a real implementation, we'd read the pathname from pathname_ptr
        let pathname = "/dev/null"; // Placeholder

        match self.vfs.open(pathname, flags) {
            Ok(fd_entry) => {
                match self.fd_table.allocate(fd_entry) {
                    Ok(fd_num) => Ok(SyscallResult::Success(fd_num as u64)),
                    Err(_) => Ok(SyscallResult::Error(-23)), // ENFILE
                }
            }
            Err(_) => Ok(SyscallResult::Error(-2)), // ENOENT
        }
    }

    /// sys_close implementation
    fn sys_close(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let fd = args.arg0 as u32;

        if let Some(fd_entry) = self.fd_table.deallocate(fd) {
            self.vfs.close(fd_entry)?;
            Ok(SyscallResult::Success(0))
        } else {
            Ok(SyscallResult::Error(-9)) // EBADF
        }
    }

    /// sys_exit implementation
    fn sys_exit(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let exit_code = args.arg0;
        self.exit_code = Some(exit_code);
        Ok(SyscallResult::Exit(exit_code))
    }

    /// sys_getpid implementation
    fn sys_getpid(&mut self, _args: SyscallArgs) -> Result<SyscallResult> {
        Ok(SyscallResult::Success(self.pid as u64))
    }

    /// sys_brk implementation (heap management)
    fn sys_brk(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let addr = args.arg0;

        // In a real implementation, this would manage heap memory
        // For now, just return the requested address
        Ok(SyscallResult::Success(addr))
    }

    /// sys_mmap implementation
    fn sys_mmap(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let addr = args.arg0;
        let length = args.arg1;
        let prot = args.arg2;
        let flags = args.arg3;
        let fd = args.arg4 as i32;
        let offset = args.arg5;

        // Simplified mmap implementation
        if fd == -1 {
            // Anonymous mapping
            let base_addr = if addr == 0 { 0x40000000 } else { addr };
            Ok(SyscallResult::Success(base_addr))
        } else {
            Ok(SyscallResult::Error(-22)) // EINVAL
        }
    }

    /// sys_munmap implementation
    fn sys_munmap(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let _addr = args.arg0;
        let _length = args.arg1;

        // In a real implementation, this would unmap memory
        Ok(SyscallResult::Success(0))
    }

    /// sys_getcwd implementation
    fn sys_getcwd(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let buf_ptr = args.arg0;
        let size = args.arg1;

        // In a real implementation, we'd write the current directory to buf_ptr
        // For now, just return success
        Ok(SyscallResult::Success(buf_ptr))
    }

    /// sys_gettimeofday implementation
    fn sys_gettimeofday(&mut self, args: SyscallArgs) -> Result<SyscallResult> {
        let tv_ptr = args.arg0;
        let tz_ptr = args.arg1;

        // In a real implementation, we'd fill in the timeval structure
        // For now, just return success
        Ok(SyscallResult::Success(0))
    }

    /// Get exit code if process has exited
    pub fn exit_code(&self) -> Option<u64> {
        self.exit_code
    }

    /// Check if process has exited
    pub fn has_exited(&self) -> bool {
        self.exit_code.is_some()
    }
}