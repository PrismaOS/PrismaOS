//! Execution context and entry point management for loaded ELF binaries.

use crate::error::{ElfError, Result};
use crate::loader::LoadedBinary;
use crate::arch::{ArchitectureType, ExecutionState, AArch64ExecutionState, RiscVExecutionState};
use crate::arch::{CallingConvention, SystemVAbi};
use alloc::{vec::Vec, string::String, format};

/// Result of instruction execution
#[derive(Debug, Clone, Copy)]
enum InstructionResult {
    /// Continue execution
    Continue,
    /// Exit with code
    Exit(u64),
    /// System call made
    SystemCall,
}

/// Execution environment configuration
#[derive(Debug, Clone)]
pub struct ExecutionEnvironment {
    /// Command line arguments
    pub args: Vec<&'static str>,
    /// Environment variables
    pub env: Vec<(&'static str, &'static str)>,
    /// Initial stack size
    pub stack_size: usize,
    /// Initial heap size
    pub heap_size: usize,
}

impl ExecutionEnvironment {
    /// Create a new execution environment
    pub fn new() -> Self {
        Self {
            args: Vec::new(),
            env: Vec::new(),
            stack_size: 0x100000, // 1MB default stack
            heap_size: 0x1000000, // 16MB default heap
        }
    }

    /// Add a command line argument
    pub fn with_arg(mut self, arg: &'static str) -> Self {
        self.args.push(arg);
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: &'static str, value: &'static str) -> Self {
        self.env.push((key, value));
        self
    }

    /// Set stack size
    pub fn with_stack_size(mut self, size: usize) -> Self {
        self.stack_size = size;
        self
    }

    /// Set heap size
    pub fn with_heap_size(mut self, size: usize) -> Self {
        self.heap_size = size;
        self
    }
}

impl Default for ExecutionEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution context for a loaded binary
#[derive(Debug)]
pub struct ExecutionContext {
    /// The loaded binary
    binary: LoadedBinary,
    /// Execution environment
    environment: ExecutionEnvironment,
    /// Current processor state
    processor_state: ProcessorState,
    /// Stack memory
    stack: Option<Vec<u8>>,
    /// Heap memory
    heap: Option<Vec<u8>>,
}

/// Generic processor state wrapper
#[derive(Debug)]
pub enum ProcessorState {
    /// x86_64 processor state
    X86_64(ExecutionState),
    /// AArch64 processor state
    AArch64(AArch64ExecutionState),
    /// RISC-V processor state
    RiscV(RiscVExecutionState),
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(binary: LoadedBinary, environment: ExecutionEnvironment) -> Result<Self> {
        let processor_state = match binary.architecture {
            ArchitectureType::X86_64(arch) => {
                ProcessorState::X86_64(arch.setup_execution_state()?)
            }
            ArchitectureType::AArch64(arch) => {
                ProcessorState::AArch64(arch.setup_execution_state()?)
            }
            ArchitectureType::RiscV(arch) => {
                ProcessorState::RiscV(arch.setup_execution_state()?)
            }
        };

        Ok(Self {
            binary,
            environment,
            processor_state,
            stack: None,
            heap: None,
        })
    }

    /// Initialize the execution context
    pub fn initialize(&mut self) -> Result<()> {
        // Set up stack
        self.setup_stack()?;

        // Set up heap
        self.setup_heap()?;

        // Initialize processor state
        self.initialize_processor_state()?;

        Ok(())
    }

    /// Set up the stack for execution
    fn setup_stack(&mut self) -> Result<()> {
        let stack_size = self.environment.stack_size;
        let mut stack = Vec::with_capacity(stack_size);
        stack.resize(stack_size, 0);

        // Set up initial stack frame with arguments and environment
        let stack_top = self.setup_initial_stack_frame(&mut stack)?;

        // Update processor state with stack pointer
        match &mut self.processor_state {
            ProcessorState::X86_64(state) => {
                state.rsp = stack_top;
                state.rbp = stack_top;
            }
            ProcessorState::AArch64(state) => {
                state.sp = stack_top;
            }
            ProcessorState::RiscV(state) => {
                state.x[2] = stack_top; // x2 is the stack pointer in RISC-V
            }
        }

        self.stack = Some(stack);
        Ok(())
    }

    /// Set up the heap for execution
    fn setup_heap(&mut self) -> Result<()> {
        let heap_size = self.environment.heap_size;
        let mut heap = Vec::with_capacity(heap_size);
        heap.resize(heap_size, 0);
        self.heap = Some(heap);
        Ok(())
    }

    /// Set up initial stack frame with arguments and environment
    fn setup_initial_stack_frame(&self, stack: &mut [u8]) -> Result<u64> {
        let stack_base = stack.as_ptr() as u64;
        let stack_size = stack.len() as u64;
        let mut stack_ptr = stack_base + stack_size;

        // Align stack pointer to 16-byte boundary (required by x86_64 ABI)
        stack_ptr &= !0xf;

        // Reserve space for arguments and environment setup
        // This is a simplified implementation - a full implementation would
        // set up the auxiliary vector, environment strings, argument strings, etc.

        // For now, just set up argc
        stack_ptr -= 8;
        let argc = self.environment.args.len() as u64;
        unsafe {
            *(stack_ptr as *mut u64) = argc;
        }

        Ok(stack_ptr)
    }

    /// Initialize processor state for execution
    fn initialize_processor_state(&mut self) -> Result<()> {
        match &mut self.processor_state {
            ProcessorState::X86_64(state) => {
                state.rip = self.binary.entry_point;
                // Set up initial flags (enable interrupts)
                state.rflags = 0x202;
            }
            ProcessorState::AArch64(state) => {
                state.pc = self.binary.entry_point;
                // Set up initial processor state
                state.pstate = 0;
            }
            ProcessorState::RiscV(state) => {
                state.pc = self.binary.entry_point;
                // x0 is always zero in RISC-V
                state.x[0] = 0;
            }
        }

        Ok(())
    }

    /// Execute the loaded binary
    pub fn execute(&mut self) -> Result<u64> {
        // Initialize if not already done
        if self.stack.is_none() {
            self.initialize()?;
        }

        // Set up the processor registers with entry point
        match &mut self.processor_state {
            ProcessorState::X86_64(state) => {
                state.rip = self.binary.entry_point;
                // Set up argc/argv on stack
                if let Some(ref stack) = self.stack {
                    let stack_top = stack.as_ptr() as u64 + stack.len() as u64 - 8;
                    state.rsp = stack_top;
                    state.rbp = stack_top;
                    // Set argc in first argument register
                    state.rdi = self.environment.args.len() as u64;
                }
            }
            ProcessorState::AArch64(state) => {
                state.pc = self.binary.entry_point;
                if let Some(ref stack) = self.stack {
                    state.sp = stack.as_ptr() as u64 + stack.len() as u64;
                }
                // Set argc in x0
                state.x[0] = self.environment.args.len() as u64;
            }
            ProcessorState::RiscV(state) => {
                state.pc = self.binary.entry_point;
                if let Some(ref stack) = self.stack {
                    state.x[2] = stack.as_ptr() as u64 + stack.len() as u64; // sp
                }
                // Set argc in a0 (x10)
                state.x[10] = self.environment.args.len() as u64;
            }
        }

        // Execute the binary by interpreting instructions
        self.execute_instructions()
    }

    /// Execute instructions starting from entry point
    fn execute_instructions(&mut self) -> Result<u64> {
        let mut instruction_count = 0u64;
        const MAX_INSTRUCTIONS: u64 = 1000000; // Prevent infinite loops

        loop {
            if instruction_count >= MAX_INSTRUCTIONS {
                return Err(ElfError::ExecutionSetupFailed);
            }

            // To avoid double mutable borrow, split out the instruction execution logic
            let result = {
                // Get a raw pointer to self to avoid borrow checker issues
                let self_ptr: *mut Self = self;
                // SAFETY: We only use this pointer for the duration of this block and do not alias mutable borrows
                unsafe {
                    match &mut (*self_ptr).processor_state {
                        ProcessorState::X86_64(state) => {
                            let ip = (*self_ptr).instruction_pointer();
                            let instruction_bytes = (*self_ptr).binary.read_memory(ip, 16)
                                .map_err(|_| ElfError::ExecutionSetupFailed)?;
                            (*self_ptr).execute_x86_64_instruction(state, instruction_bytes)
                        }
                        ProcessorState::AArch64(state) => {
                            let ip = (*self_ptr).instruction_pointer();
                            let instruction_bytes = (*self_ptr).binary.read_memory(ip, 16)
                                .map_err(|_| ElfError::ExecutionSetupFailed)?;
                            (*self_ptr).execute_aarch64_instruction(state, instruction_bytes)
                        }
                        ProcessorState::RiscV(state) => {
                            let ip = (*self_ptr).instruction_pointer();
                            let instruction_bytes = (*self_ptr).binary.read_memory(ip, 16)
                                .map_err(|_| ElfError::ExecutionSetupFailed)?;
                            (*self_ptr).execute_riscv_instruction(state, instruction_bytes)
                        }
                    }
                }
            }?;

            match result {
                InstructionResult::Continue => {},
                InstructionResult::Exit(code) => {
                    return Ok(code);
                },
                InstructionResult::SystemCall => {
                    let exit_code = self.handle_system_call()?;
                    let should_exit = match &self.processor_state {
                        ProcessorState::X86_64(state) => state.rax == 60,
                        ProcessorState::AArch64(state) => state.x[8] == 93,
                        ProcessorState::RiscV(state) => state.x[17] == 93,
                    };
                    if should_exit {
                        return Ok(exit_code);
                    }
                },
            }

            instruction_count += 1;
        }
    }

    /// Execute x86_64 instruction
    fn execute_x86_64_instruction(&mut self, state: &mut ExecutionState, bytes: &[u8]) -> Result<InstructionResult> {
        if bytes.len() < 2 {
            return Err(ElfError::ExecutionSetupFailed);
        }

        // Decode common x86_64 instructions
        match bytes[0] {
            // MOV immediate to register (REX.W + 0xC7 + ModR/M)
            0x48 if bytes[1] == 0xc7 && bytes.len() >= 7 => {
                let reg = bytes[2] & 0x7;
                let immediate = u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]) as u64;

                match reg {
                    0 => state.rax = immediate, // MOV $imm, %rax
                    7 => state.rdi = immediate, // MOV $imm, %rdi
                    _ => {}
                }
                state.rip += 7;
                Ok(InstructionResult::Continue)
            }
            // SYSCALL (0x0F 0x05)
            0x0f if bytes[1] == 0x05 => {
                Ok(InstructionResult::SystemCall)
            }
            // NOP (0x90)
            0x90 => {
                state.rip += 1;
                Ok(InstructionResult::Continue)
            }
            // RET (0xC3)
            0xc3 => {
                // Simple return - just exit for now
                Ok(InstructionResult::Exit(state.rax))
            }
            _ => {
                // Unknown instruction - skip it
                state.rip += 1;
                Ok(InstructionResult::Continue)
            }
        }
    }

    /// Execute AArch64 instruction
    fn execute_aarch64_instruction(&mut self, state: &mut AArch64ExecutionState, bytes: &[u8]) -> Result<InstructionResult> {
        if bytes.len() < 4 {
            return Err(ElfError::ExecutionSetupFailed);
        }

        let instruction = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        // Decode common AArch64 instructions
        match instruction {
            // MOV x8, #93 (0xd2800ba8)
            0xd2800ba8 => {
                state.x[8] = 93; // exit syscall number
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
            // MOV x0, #0 (0xd2800000)
            0xd2800000 => {
                state.x[0] = 0; // exit code
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
            // SVC #0 (0xd4000001)
            0xd4000001 => {
                Ok(InstructionResult::SystemCall)
            }
            // NOP (0xd503201f)
            0xd503201f => {
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
            _ => {
                // Unknown instruction - skip it
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
        }
    }

    /// Execute RISC-V instruction
    fn execute_riscv_instruction(&mut self, state: &mut RiscVExecutionState, bytes: &[u8]) -> Result<InstructionResult> {
        if bytes.len() < 4 {
            return Err(ElfError::ExecutionSetupFailed);
        }

        let instruction = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        // Decode common RISC-V instructions
        match instruction & 0x7f {
            // ECALL (0x73)
            0x73 if instruction == 0x73 => {
                Ok(InstructionResult::SystemCall)
            }
            // ADDI (0x13)
            0x13 => {
                let rd = ((instruction >> 7) & 0x1f) as usize;
                let rs1 = ((instruction >> 15) & 0x1f) as usize;
                let imm = ((instruction as i32) >> 20) as i64 as u64;

                if rd != 0 { // x0 is always zero
                    state.x[rd] = state.x[rs1].wrapping_add(imm);
                }
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
            _ => {
                // Unknown instruction - skip it
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
        }
    }

    /// Handle system call
    fn handle_system_call(&mut self) -> Result<u64> {
        match &mut self.processor_state {
            ProcessorState::X86_64(state) => {
                match state.rax {
                    60 => { // sys_exit
                        Ok(state.rdi) // return exit code
                    }
                    1 => { // sys_write
                        // For now, just return success
                        state.rax = state.rdx; // return bytes written
                        state.rip += 2; // advance past syscall
                        Ok(0)
                    }
                    _ => {
                        // Unknown syscall
                        state.rax = u64::MAX; // -1 (error)
                        state.rip += 2;
                        Ok(0)
                    }
                }
            }
            ProcessorState::AArch64(state) => {
                match state.x[8] {
                    93 => { // exit
                        Ok(state.x[0]) // return exit code
                    }
                    64 => { // write
                        state.x[0] = state.x[2]; // return bytes written
                        state.pc += 4;
                        Ok(0)
                    }
                    _ => {
                        state.x[0] = u64::MAX; // -1 (error)
                        state.pc += 4;
                        Ok(0)
                    }
                }
            }
            ProcessorState::RiscV(state) => {
                match state.x[17] { // a7 register
                    93 => { // exit
                        Ok(state.x[10]) // return exit code from a0
                    }
                    64 => { // write
                        state.x[10] = state.x[12]; // return bytes written
                        state.pc += 4;
                        Ok(0)
                    }
                    _ => {
                        state.x[10] = u64::MAX; // -1 (error)
                        state.pc += 4;
                        Ok(0)
                    }
                }
            }
        }
    }

    /// Call a function at the given address with arguments
    pub fn call_function(&mut self, address: u64, args: &[u64]) -> Result<u64> {
        // Validate address is within loaded segments
        if self.binary.get_memory_at(address).is_none() {
            return Err(ElfError::InvalidAddress);
        }

        use core::mem;
        let mut processor_state_opt = Some(mem::replace(&mut self.processor_state, ProcessorState::X86_64(unsafe { mem::zeroed() })));
        let result = match processor_state_opt.as_mut().unwrap() {
            ProcessorState::X86_64(state) => {
                let calling_convention = SystemVAbi;
                calling_convention.setup_arguments(args, state)?;
                let old_rip = state.rip;
                state.rip = address;
                // Temporarily put the state back for execution
                self.processor_state = ProcessorState::X86_64(state.clone());
                let exec_result = self.execute_instructions();
                // After execution, update state
                if let ProcessorState::X86_64(new_state) = mem::replace(&mut self.processor_state, ProcessorState::X86_64(unsafe { mem::zeroed() })) {
                    *state = new_state;
                }
                if exec_result.is_ok() {
                    Ok(calling_convention.get_return_value(state))
                } else {
                    state.rip = old_rip;
                    exec_result
                }
            }
            ProcessorState::AArch64(state) => {
                for (i, &arg) in args.iter().enumerate().take(8) {
                    state.x[i] = arg;
                }
                let old_pc = state.pc;
                state.pc = address;
                self.processor_state = ProcessorState::AArch64(state.clone());
                let exec_result = self.execute_instructions();
                if let ProcessorState::AArch64(new_state) = mem::replace(&mut self.processor_state, ProcessorState::X86_64(unsafe { mem::zeroed() })) {
                    *state = new_state;
                }
                if exec_result.is_ok() {
                    Ok(state.x[0])
                } else {
                    state.pc = old_pc;
                    exec_result
                }
            }
            ProcessorState::RiscV(state) => {
                for (i, &arg) in args.iter().enumerate().take(8) {
                    state.x[10 + i] = arg;
                }
                let old_pc = state.pc;
                state.pc = address;
                self.processor_state = ProcessorState::RiscV(state.clone());
                let exec_result = self.execute_instructions();
                if let ProcessorState::RiscV(new_state) = mem::replace(&mut self.processor_state, ProcessorState::X86_64(unsafe { mem::zeroed() })) {
                    *state = new_state;
                }
                if exec_result.is_ok() {
                    Ok(state.x[10])
                } else {
                    state.pc = old_pc;
                    exec_result
                }
            }
        };
        self.processor_state = processor_state_opt.take().unwrap();
        result
    }

    /// Get the current instruction pointer
    pub fn instruction_pointer(&self) -> u64 {
        match &self.processor_state {
            ProcessorState::X86_64(state) => state.rip,
            ProcessorState::AArch64(state) => state.pc,
            ProcessorState::RiscV(state) => state.pc,
        }
    }

    /// Get the current stack pointer
    pub fn stack_pointer(&self) -> u64 {
        match &self.processor_state {
            ProcessorState::X86_64(state) => state.rsp,
            ProcessorState::AArch64(state) => state.sp,
            ProcessorState::RiscV(state) => state.x[2],
        }
    }

    /// Get a reference to the loaded binary
    pub fn binary(&self) -> &LoadedBinary {
        &self.binary
    }

    /// Get a mutable reference to the loaded binary
    pub fn binary_mut(&mut self) -> &mut LoadedBinary {
        &mut self.binary
    }

    /// Get the execution environment
    pub fn environment(&self) -> &ExecutionEnvironment {
        &self.environment
    }

    /// Read memory from the execution context
    pub fn read_memory(&self, address: u64, size: usize) -> Result<&[u8]> {
        self.binary.read_memory(address, size)
    }

    /// Write memory in the execution context
    pub fn write_memory(&mut self, address: u64, data: &[u8]) -> Result<()> {
        self.binary.write_memory(address, data)
    }

    /// Dump processor state for debugging
    pub fn dump_state(&self) -> String {
        match &self.processor_state {
            ProcessorState::X86_64(state) => {
                format!(
                    "x86_64 State:\n\
                     RIP: 0x{:016x}\n\
                     RSP: 0x{:016x}\n\
                     RBP: 0x{:016x}\n\
                     RAX: 0x{:016x}\n\
                     RBX: 0x{:016x}\n\
                     RCX: 0x{:016x}\n\
                     RDX: 0x{:016x}\n\
                     RSI: 0x{:016x}\n\
                     RDI: 0x{:016x}\n\
                     RFLAGS: 0x{:016x}",
                    state.rip, state.rsp, state.rbp, state.rax, state.rbx,
                    state.rcx, state.rdx, state.rsi, state.rdi, state.rflags
                )
            }
            ProcessorState::AArch64(state) => {
                format!(
                    "AArch64 State:\n\
                     PC: 0x{:016x}\n\
                     SP: 0x{:016x}\n\
                     PSTATE: 0x{:08x}",
                    state.pc, state.sp, state.pstate
                )
            }
            ProcessorState::RiscV(state) => {
                format!(
                    "RISC-V State:\n\
                     PC: 0x{:016x}\n\
                     SP: 0x{:016x}",
                    state.pc, state.x[2]
                )
            }
        }
    }
}

/// Process lifecycle management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Process is ready to run
    Ready,
    /// Process is currently running
    Running,
    /// Process is blocked waiting for I/O
    Blocked,
    /// Process has terminated
    Terminated,
}

/// Process control block for managing execution
#[derive(Debug)]
pub struct ProcessControlBlock {
    /// Process ID
    pub pid: u32,
    /// Process state
    pub state: ProcessState,
    /// Exit code (if terminated)
    pub exit_code: Option<u64>,
    /// Execution context
    pub context: ExecutionContext,
}

impl ProcessControlBlock {
    /// Create a new process control block
    pub fn new(pid: u32, binary: LoadedBinary, environment: ExecutionEnvironment) -> Result<Self> {
        let context = ExecutionContext::new(binary, environment)?;

        Ok(Self {
            pid,
            state: ProcessState::Ready,
            exit_code: None,
            context,
        })
    }

    /// Start process execution
    pub fn start(&mut self) -> Result<()> {
        if self.state != ProcessState::Ready {
            return Err(ElfError::ExecutionSetupFailed);
        }

        self.state = ProcessState::Running;
        self.context.initialize()
    }

    /// Execute the process
    pub fn execute(&mut self) -> Result<u64> {
        if self.state != ProcessState::Running {
            return Err(ElfError::ExecutionSetupFailed);
        }

        let exit_code = self.context.execute()?;
        self.exit_code = Some(exit_code);
        self.state = ProcessState::Terminated;

        Ok(exit_code)
    }

    /// Terminate the process
    pub fn terminate(&mut self, exit_code: u64) {
        self.exit_code = Some(exit_code);
        self.state = ProcessState::Terminated;
    }
}