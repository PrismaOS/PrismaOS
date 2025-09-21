//! Architecture-specific support for different target platforms.

use crate::error::{ElfError, Result};
use crate::header::ElfMachine;

/// CPU features that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFeature {
    /// x86_64 long mode support
    X86_64,
    /// SSE instruction set
    SSE,
    /// AVX instruction set
    AVX,
    /// SYSCALL/SYSRET instructions
    SYSCALL,
}

/// Architecture-specific information and helpers
pub trait Architecture {
    /// Get the pointer size in bytes
    fn pointer_size(&self) -> usize;

    /// Get the page size
    fn page_size(&self) -> usize;

    /// Get the required alignment for code sections
    fn code_alignment(&self) -> usize;

    /// Get the required alignment for data sections
    fn data_alignment(&self) -> usize;

    /// Check if an address is properly aligned for the architecture
    fn is_aligned(&self, address: u64, alignment: usize) -> bool {
        (address as usize) % alignment == 0
    }

    /// Align an address up to the required boundary
    fn align_up(&self, address: u64, alignment: usize) -> u64 {
        ((address as usize + alignment - 1) / alignment * alignment) as u64
    }

    /// Align an address down to the required boundary
    fn align_down(&self, address: u64, alignment: usize) -> u64 {
        ((address as usize) / alignment * alignment) as u64
    }
}

/// x86_64 architecture support
#[derive(Debug, Clone, Copy)]
pub struct X86_64;

impl Architecture for X86_64 {
    fn pointer_size(&self) -> usize {
        8
    }

    fn page_size(&self) -> usize {
        4096
    }

    fn code_alignment(&self) -> usize {
        16
    }

    fn data_alignment(&self) -> usize {
        8
    }
}

impl X86_64 {
    /// Create a new x86_64 architecture instance
    pub fn new() -> Self {
        Self
    }

    /// Check if the CPU supports required features
    pub fn check_features(&self) -> Result<()> {
        // Check for required x86_64 features
        if !self.has_feature(CpuFeature::X86_64) {
            return Err(ElfError::UnsupportedArchitecture);
        }

        // Check for SSE support (commonly required)
        if !self.has_feature(CpuFeature::SSE) {
            return Err(ElfError::UnsupportedArchitecture);
        }

        Ok(())
    }

    /// Check if a specific CPU feature is available
    pub fn has_feature(&self, feature: CpuFeature) -> bool {
        match feature {
            CpuFeature::X86_64 => {
                // Check for long mode support
                self.cpuid_extended_feature(0x80000001, 29) // LM bit
            }
            CpuFeature::SSE => {
                // Check for SSE support
                self.cpuid_feature(1, 25) // SSE bit in EDX
            }
            CpuFeature::AVX => {
                // Check for AVX support
                self.cpuid_feature(1, 28) // AVX bit in ECX
            }
            CpuFeature::SYSCALL => {
                // Check for SYSCALL/SYSRET support
                self.cpuid_extended_feature(0x80000001, 11) // SYSCALL bit
            }
        }
    }

    /// Execute CPUID instruction and check feature bit
    fn cpuid_feature(&self, leaf: u32, bit: u32) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            unsafe {
                let mut eax: u32;
                let mut ecx: u32;
                let mut edx: u32;
                core::arch::asm!(
                    "mov {leaf:e}, %eax",
                    "cpuid",
                    "mov %eax, {eax:e}",
                    "mov %ecx, {ecx:e}",
                    "mov %edx, {edx:e}",
                    leaf = in(reg) leaf,
                    eax = out(reg) eax,
                    ecx = out(reg) ecx,
                    edx = out(reg) edx,
                );
                // Check bit in ECX for leaf 1
                if leaf == 1 && bit < 32 {
                    (ecx & (1 << bit)) != 0
                } else {
                    // Check bit in EDX for other cases
                    (edx & (1 << bit)) != 0
                }
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            // For non-x86_64 platforms, assume features are available
            // This allows the library to compile on other architectures
            true
        }
    }

    /// Execute extended CPUID instruction and check feature bit
    fn cpuid_extended_feature(&self, leaf: u32, bit: u32) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            unsafe {
                let mut eax: u32;
                let mut ecx: u32;
                let mut edx: u32;
                core::arch::asm!(
                    "mov {leaf:e}, %eax",
                    "cpuid",
                    "mov %eax, {eax:e}",
                    "mov %ecx, {ecx:e}",
                    "mov %edx, {edx:e}",
                    leaf = in(reg) leaf,
                    eax = out(reg) eax,
                    ecx = out(reg) ecx,
                    edx = out(reg) edx,
                );
                (edx & (1 << bit)) != 0
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            true
        }
    }

    /// Set up initial processor state for execution
    pub fn setup_execution_state(&self) -> Result<ExecutionState> {
        Ok(ExecutionState {
            rip: 0,
            rsp: 0,
            rbp: 0,
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: 0x202, // Default flags with interrupts enabled
        })
    }
}

impl Default for X86_64 {
    fn default() -> Self {
        Self::new()
    }
}

/// x86_64 processor state
#[derive(Debug, Clone, Copy)]
pub struct ExecutionState {
    /// Instruction pointer
    pub rip: u64,
    /// Stack pointer
    pub rsp: u64,
    /// Base pointer
    pub rbp: u64,
    /// Accumulator register
    pub rax: u64,
    /// Base register
    pub rbx: u64,
    /// Counter register
    pub rcx: u64,
    /// Data register
    pub rdx: u64,
    /// Source index register
    pub rsi: u64,
    /// Destination index register
    pub rdi: u64,
    /// General purpose register 8
    pub r8: u64,
    /// General purpose register 9
    pub r9: u64,
    /// General purpose register 10
    pub r10: u64,
    /// General purpose register 11
    pub r11: u64,
    /// General purpose register 12
    pub r12: u64,
    /// General purpose register 13
    pub r13: u64,
    /// General purpose register 14
    pub r14: u64,
    /// General purpose register 15
    pub r15: u64,
    /// Flags register
    pub rflags: u64,
}

/// AArch64 architecture support
#[derive(Debug, Clone, Copy)]
pub struct AArch64;

impl Architecture for AArch64 {
    fn pointer_size(&self) -> usize {
        8
    }

    fn page_size(&self) -> usize {
        4096
    }

    fn code_alignment(&self) -> usize {
        4
    }

    fn data_alignment(&self) -> usize {
        8
    }
}

impl AArch64 {
    /// Create a new AArch64 architecture instance
    pub fn new() -> Self {
        Self
    }

    /// Set up initial processor state for execution
    pub fn setup_execution_state(&self) -> Result<AArch64ExecutionState> {
        Ok(AArch64ExecutionState {
            pc: 0,
            sp: 0,
            x: [0; 31],
            pstate: 0,
        })
    }
}

impl Default for AArch64 {
    fn default() -> Self {
        Self::new()
    }
}

/// AArch64 processor state
#[derive(Debug, Clone)]
pub struct AArch64ExecutionState {
    /// Program counter
    pub pc: u64,
    /// Stack pointer
    pub sp: u64,
    /// General purpose registers (X0-X30)
    pub x: [u64; 31],
    /// Processor state
    pub pstate: u32,
}

/// RISC-V architecture support
#[derive(Debug, Clone, Copy)]
pub struct RiscV;

impl Architecture for RiscV {
    fn pointer_size(&self) -> usize {
        8 // Assuming RV64
    }

    fn page_size(&self) -> usize {
        4096
    }

    fn code_alignment(&self) -> usize {
        4
    }

    fn data_alignment(&self) -> usize {
        8
    }
}

impl RiscV {
    /// Create a new RISC-V architecture instance
    pub fn new() -> Self {
        Self
    }

    /// Set up initial processor state for execution
    pub fn setup_execution_state(&self) -> Result<RiscVExecutionState> {
        Ok(RiscVExecutionState {
            pc: 0,
            x: [0; 32],
        })
    }
}

impl Default for RiscV {
    fn default() -> Self {
        Self::new()
    }
}

/// RISC-V processor state
#[derive(Debug, Clone)]
pub struct RiscVExecutionState {
    /// Program counter
    pub pc: u64,
    /// General purpose registers (x0-x31)
    pub x: [u64; 32],
}

/// Generic architecture abstraction
#[derive(Debug, Clone, Copy)]
pub enum ArchitectureType {
    /// x86_64 architecture
    X86_64(X86_64),
    /// AArch64 architecture
    AArch64(AArch64),
    /// RISC-V architecture
    RiscV(RiscV),
}

impl ArchitectureType {
    /// Create architecture type from ELF machine type
    pub fn from_machine(machine: ElfMachine) -> Result<Self> {
        match machine {
            ElfMachine::X86_64 => Ok(ArchitectureType::X86_64(X86_64::new())),
            ElfMachine::AArch64 => Ok(ArchitectureType::AArch64(AArch64::new())),
            ElfMachine::RiscV => Ok(ArchitectureType::RiscV(RiscV::new())),
            _ => Err(ElfError::UnsupportedArchitecture),
        }
    }

    /// Get the underlying architecture trait
    pub fn as_architecture(&self) -> &dyn Architecture {
        match self {
            ArchitectureType::X86_64(arch) => arch,
            ArchitectureType::AArch64(arch) => arch,
            ArchitectureType::RiscV(arch) => arch,
        }
    }

    /// Get the ELF machine type
    pub fn machine(&self) -> ElfMachine {
        match self {
            ArchitectureType::X86_64(_) => ElfMachine::X86_64,
            ArchitectureType::AArch64(_) => ElfMachine::AArch64,
            ArchitectureType::RiscV(_) => ElfMachine::RiscV,
        }
    }
}

/// Memory layout configuration for an architecture
#[derive(Debug, Clone)]
pub struct MemoryLayout {
    /// Base address for code sections
    pub code_base: u64,
    /// Base address for data sections
    pub data_base: u64,
    /// Stack base address
    pub stack_base: u64,
    /// Stack size
    pub stack_size: u64,
    /// Heap base address
    pub heap_base: u64,
    /// Heap size
    pub heap_size: u64,
}

impl MemoryLayout {
    /// Create a default memory layout for x86_64
    pub fn default_x86_64() -> Self {
        Self {
            code_base: 0x400000,
            data_base: 0x600000,
            stack_base: 0x7fffffff000,
            stack_size: 0x100000, // 1MB stack
            heap_base: 0x800000,
            heap_size: 0x100000000, // 4GB heap
        }
    }

    /// Create a default memory layout for AArch64
    pub fn default_aarch64() -> Self {
        Self {
            code_base: 0x400000,
            data_base: 0x600000,
            stack_base: 0x7fffffff000,
            stack_size: 0x100000, // 1MB stack
            heap_base: 0x800000,
            heap_size: 0x100000000, // 4GB heap
        }
    }

    /// Create a default memory layout for RISC-V
    pub fn default_riscv() -> Self {
        Self {
            code_base: 0x10000,
            data_base: 0x20000,
            stack_base: 0x7fffffff000,
            stack_size: 0x100000, // 1MB stack
            heap_base: 0x30000,
            heap_size: 0x100000000, // 4GB heap
        }
    }

    /// Create default memory layout for architecture
    pub fn default_for_architecture(arch: ArchitectureType) -> Self {
        match arch {
            ArchitectureType::X86_64(_) => Self::default_x86_64(),
            ArchitectureType::AArch64(_) => Self::default_aarch64(),
            ArchitectureType::RiscV(_) => Self::default_riscv(),
        }
    }

    /// Validate memory layout
    pub fn validate(&self) -> Result<()> {
        // Check for overlaps
        if self.code_base < self.data_base && self.code_base + 0x200000 > self.data_base {
            return Err(ElfError::InvalidAddress);
        }

        if self.data_base < self.heap_base && self.data_base + 0x200000 > self.heap_base {
            return Err(ElfError::InvalidAddress);
        }

        if self.heap_base < self.stack_base &&
           self.heap_base + self.heap_size > self.stack_base - self.stack_size {
            return Err(ElfError::InvalidAddress);
        }

        Ok(())
    }
}

/// Platform-specific calling convention helpers
pub trait CallingConvention {
    /// Set up function arguments in registers/stack
    fn setup_arguments(&self, args: &[u64], state: &mut dyn core::any::Any) -> Result<()>;

    /// Get return value from registers
    fn get_return_value(&self, state: &dyn core::any::Any) -> u64;
}

/// x86_64 System V ABI calling convention
#[derive(Debug, Clone, Copy)]
pub struct SystemVAbi;

impl CallingConvention for SystemVAbi {
    fn setup_arguments(&self, args: &[u64], state: &mut dyn core::any::Any) -> Result<()> {
        if let Some(exec_state) = state.downcast_mut::<ExecutionState>() {
            // First 6 arguments go in registers: RDI, RSI, RDX, RCX, R8, R9
            if args.len() > 0 { exec_state.rdi = args[0]; }
            if args.len() > 1 { exec_state.rsi = args[1]; }
            if args.len() > 2 { exec_state.rdx = args[2]; }
            if args.len() > 3 { exec_state.rcx = args[3]; }
            if args.len() > 4 { exec_state.r8 = args[4]; }
            if args.len() > 5 { exec_state.r9 = args[5]; }

            // Additional arguments would go on the stack
            // This is simplified - real implementation would need stack management

            Ok(())
        } else {
            Err(ElfError::ExecutionSetupFailed)
        }
    }

    fn get_return_value(&self, state: &dyn core::any::Any) -> u64 {
        if let Some(exec_state) = state.downcast_ref::<ExecutionState>() {
            exec_state.rax
        } else {
            0
        }
    }
}