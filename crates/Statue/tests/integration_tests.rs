//! Integration tests for ELF loading and execution verification

use std::vec::Vec;
use statue::*;
use statue::arch::Architecture;

/// Create a minimal valid x86_64 ELF executable
fn create_minimal_x86_64_elf() -> Vec<u8> {
    let mut elf = Vec::new();

    // ELF Header (64 bytes)
    elf.extend_from_slice(&[
        // e_ident (16 bytes)
        0x7f, 0x45, 0x4c, 0x46,  // ELF magic
        0x02,                     // 64-bit
        0x01,                     // Little endian
        0x01,                     // ELF version
        0x00,                     // System V ABI
        0x00, 0x00, 0x00, 0x00,   // ABI version + padding
        0x00, 0x00, 0x00, 0x00,   // More padding

        // ELF header fields
        0x02, 0x00,               // e_type: ET_EXEC
        0x3e, 0x00,               // e_machine: EM_X86_64
        0x01, 0x00, 0x00, 0x00,   // e_version
        0x78, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, // e_entry: 0x400078
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_phoff: 0x40
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_shoff: 0
        0x00, 0x00, 0x00, 0x00,   // e_flags
        0x40, 0x00,               // e_ehsize: 64
        0x38, 0x00,               // e_phentsize: 56
        0x01, 0x00,               // e_phnum: 1
        0x00, 0x00,               // e_shentsize: 0
        0x00, 0x00,               // e_shnum: 0
        0x00, 0x00,               // e_shstrndx: 0
    ]);

    // Program Header (56 bytes)
    elf.extend_from_slice(&[
        0x01, 0x00, 0x00, 0x00,   // p_type: PT_LOAD
        0x05, 0x00, 0x00, 0x00,   // p_flags: PF_R | PF_X
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_offset: 0
        0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, // p_vaddr: 0x400000
        0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, // p_paddr: 0x400000
        0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_filesz: 144
        0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_memsz: 144
        0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // p_align: 4096
    ]);

    // Simple code that returns 42
    elf.extend_from_slice(&[
        0x48, 0xc7, 0xc0, 0x2a, 0x00, 0x00, 0x00, // mov $42, %rax (return value)
        0x48, 0xc7, 0xc7, 0x00, 0x00, 0x00, 0x00, // mov $0, %rdi
        0x0f, 0x05,                               // syscall (exit)
        0x90, 0x90, 0x90, 0x90,                   // nop padding
    ]);

    // Pad to 144 bytes
    while elf.len() < 144 {
        elf.push(0x90); // nop
    }

    elf
}

/// Create ELF with data segment
fn create_elf_with_data_segment() -> Vec<u8> {
    let mut elf = Vec::new();

    // ELF Header with 2 program headers
    elf.extend_from_slice(&[
        0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x3e, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x78, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x38, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);

    // Code segment program header
    elf.extend_from_slice(&[
        0x01, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);

    // Data segment program header
    elf.extend_from_slice(&[
        0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,
        0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);

    // Code section (32 bytes)
    elf.extend_from_slice(&[
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov $1, %rax
        0x48, 0x8b, 0x3d, 0x05, 0x10, 0x20, 0x00, // mov data(%rip), %rdi
        0x0f, 0x05,                               // syscall
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    ]);

    // Data section (16 bytes)
    elf.extend_from_slice(&[
        0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x77,
        0x6f, 0x72, 0x6c, 0x64, 0x21, 0x0a, 0x00, 0x00,
    ]);

    elf
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_elf_load_flow() {
        let elf_data = create_minimal_x86_64_elf();

        // Parse ELF
        let elf = ElfFile::parse(&elf_data)
            .expect("Failed to parse ELF file");

        // Verify basic properties
        assert_eq!(elf.header().machine, header::ElfMachine::X86_64);
        assert_eq!(elf.header().file_type, header::ElfType::Executable);
        assert_eq!(elf.header().entry, 0x400078);

        // Set up loader
        let mut memory = vec![0u8; 2 * 1024 * 1024]; // 2MB
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);

        // Load binary
        let loaded_binary = loader.load(&elf)
            .expect("Failed to load ELF binary");

        // Verify loading results
        assert_eq!(loaded_binary.entry_point, 0x400078);
        assert_eq!(loaded_binary.segments.len(), 1);

        let segment = &loaded_binary.segments[0];
        assert_eq!(segment.vaddr, 0x400000);
        assert_eq!(segment.size, 144);
        assert!(segment.executable);
        assert!(!segment.writable);

        // Verify code was loaded correctly
        let code_data = loaded_binary.read_memory(0x400078, 7)
            .expect("Failed to read loaded code");
        assert_eq!(code_data[0], 0x48); // REX.W prefix
        assert_eq!(code_data[3], 0x2a); // Immediate value 42
    }

    #[test]
    fn test_execution_context_setup() {
        let elf_data = create_minimal_x86_64_elf();
        let elf = ElfFile::parse(&elf_data).unwrap();

        let mut memory = vec![0u8; 2 * 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).unwrap();

        // Create execution environment
        let environment = execution::ExecutionEnvironment::new()
            .with_arg("test_program")
            .with_arg("--flag")
            .with_env("HOME", "/home/test")
            .with_stack_size(1024 * 1024)
            .with_heap_size(512 * 1024);

        // Create execution context
        let mut context = execution::ExecutionContext::new(loaded_binary, environment)
            .expect("Failed to create execution context");

        // Initialize context
        context.initialize()
            .expect("Failed to initialize context");

        // Verify initial state
        assert_eq!(context.instruction_pointer(), 0x400078);
        assert_ne!(context.stack_pointer(), 0);

        // Test state dumping
        let state_dump = context.dump_state();
        assert!(state_dump.contains("x86_64 State"));
        assert!(state_dump.contains("RIP: 0x0000000000400078"));
    }

    #[test]
    fn test_multi_segment_loading() {
        let elf_data = create_elf_with_data_segment();
        let elf = ElfFile::parse(&elf_data)
            .expect("Failed to parse multi-segment ELF");

        let mut memory = vec![0u8; 4 * 1024 * 1024]; // 4MB
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);

        let loaded_binary = loader.load(&elf)
            .expect("Failed to load multi-segment ELF");

        // Should have 2 segments: code and data
        assert_eq!(loaded_binary.segments.len(), 2);

        // Find code segment
        let code_segment = loaded_binary.segments.iter()
            .find(|s| s.executable)
            .expect("No executable segment found");
        assert_eq!(code_segment.vaddr, 0x400000);

        // Find data segment
        let data_segment = loaded_binary.segments.iter()
            .find(|s| s.writable)
            .expect("No writable segment found");
        assert_eq!(data_segment.vaddr, 0x601000);

        // Verify data content
        let data_content = loaded_binary.read_memory(0x601000, 13)
            .expect("Failed to read data segment");
        assert_eq!(&data_content[0..5], b"Hello");
    }

    #[test]
    fn test_memory_protection() {
        let elf_data = create_minimal_x86_64_elf();
        let elf = ElfFile::parse(&elf_data).unwrap();

        let mut memory = vec![0u8; 2 * 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);
        let mut loaded_binary = loader.load(&elf).unwrap();

        // Try to write to executable (read-only) segment
        let write_result = loaded_binary.write_memory(0x400078, &[0x90, 0x90]);
        assert!(write_result.is_err());

        if let Err(e) = write_result {
            assert!(matches!(e, ElfError::PermissionDenied));
        }

        // Reading should still work
        let read_result = loaded_binary.read_memory(0x400078, 4);
        assert!(read_result.is_ok());
    }

    #[test]
    fn test_function_calling() {
        let elf_data = create_minimal_x86_64_elf();
        let elf = ElfFile::parse(&elf_data).unwrap();

        let mut memory = vec![0u8; 2 * 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).unwrap();

        let environment = execution::ExecutionEnvironment::new();
        let mut context = execution::ExecutionContext::new(loaded_binary, environment).unwrap();
        context.initialize().unwrap();

        // Test calling function at entry point with arguments
        let args = [1, 2, 3, 4, 5, 6]; // Test register argument passing
        let result = context.call_function(0x400078, &args)
            .expect("Failed to call function");

        // In our test implementation, this should return 0 (from the calling convention)
        assert_eq!(result, 0);
    }

    #[test]
    fn test_process_control_block() {
        let elf_data = create_minimal_x86_64_elf();
        let elf = ElfFile::parse(&elf_data).unwrap();

        let mut memory = vec![0u8; 2 * 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).unwrap();

        let environment = execution::ExecutionEnvironment::new()
            .with_arg("test_process");

        // Create process control block
        let mut pcb = execution::ProcessControlBlock::new(42, loaded_binary, environment)
            .expect("Failed to create PCB");

        assert_eq!(pcb.pid, 42);
        assert_eq!(pcb.state, execution::ProcessState::Ready);
        assert_eq!(pcb.exit_code, None);

        // Start process
        pcb.start().expect("Failed to start process");
        assert_eq!(pcb.state, execution::ProcessState::Running);

        // Execute process (simulated)
        let exit_code = pcb.execute().expect("Failed to execute process");
        assert_eq!(pcb.state, execution::ProcessState::Terminated);
        assert_eq!(pcb.exit_code, Some(exit_code));
    }

    #[test]
    fn test_architecture_specific_features() {
        let elf_data = create_minimal_x86_64_elf();
        let elf = ElfFile::parse(&elf_data).unwrap();

        let arch = arch::ArchitectureType::from_machine(elf.header().machine)
            .expect("Failed to get architecture");

        match arch {
            arch::ArchitectureType::X86_64(x86_arch) => {
                // Test architecture features
                assert_eq!(x86_arch.pointer_size(), 8);
                assert_eq!(x86_arch.page_size(), 4096);
                assert_eq!(x86_arch.code_alignment(), 16);
                assert_eq!(x86_arch.data_alignment(), 8);

                // Test address alignment
                assert!(x86_arch.is_aligned(0x1000, 16));
                assert!(!x86_arch.is_aligned(0x1001, 16));

                // Test address alignment functions
                assert_eq!(x86_arch.align_up(0x1001, 16), 0x1010);
                assert_eq!(x86_arch.align_down(0x100f, 16), 0x1000);

                // Test execution state setup
                let exec_state = x86_arch.setup_execution_state()
                    .expect("Failed to setup execution state");
                assert_eq!(exec_state.rflags, 0x202); // Default flags
            }
            _ => panic!("Expected x86_64 architecture"),
        }
    }

    #[test]
    fn test_memory_layout() {
        let layout = arch::MemoryLayout::default_x86_64();

        assert_eq!(layout.code_base, 0x400000);
        assert_eq!(layout.data_base, 0x600000);
        assert_eq!(layout.stack_size, 0x100000);
        assert_eq!(layout.heap_size, 0x100000000);

        // Test validation
        layout.validate().expect("Memory layout should be valid");

        // Test layout for different architectures
        let aarch64_layout = arch::MemoryLayout::default_aarch64();
        aarch64_layout.validate().expect("AArch64 layout should be valid");

        let riscv_layout = arch::MemoryLayout::default_riscv();
        riscv_layout.validate().expect("RISC-V layout should be valid");
    }

    #[test]
    fn test_error_propagation() {
        // Test that errors propagate correctly through the entire loading pipeline

        // Invalid ELF should fail at parse stage
        let invalid_elf = vec![0x00; 64];
        assert!(ElfFile::parse(&invalid_elf).is_err());

        // Valid ELF with invalid segment should fail at load stage
        let mut invalid_segment_elf = create_minimal_x86_64_elf();
        // Corrupt the program header filesz to be larger than file
        invalid_segment_elf[88] = 0xff;
        invalid_segment_elf[89] = 0xff;

        let elf = ElfFile::parse(&invalid_segment_elf).unwrap();
        let mut memory = vec![0u8; 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);
        let mut loader = loader::ElfLoader::new(config);

        // This should fail during loading due to invalid segment size
        let load_result = loader.load(&elf);
        assert!(load_result.is_err());
    }

    #[test]
    fn test_comprehensive_validation() {
        let elf_data = create_minimal_x86_64_elf();
        let elf = ElfFile::parse(&elf_data).unwrap();

        // Test program header validation
        let ph_iter = program::ProgramHeaderIter::new(&elf).unwrap();
        for ph_result in ph_iter {
            let ph = ph_result.unwrap();
            ph.validate(elf_data.len() as u64)
                .expect("Program header should be valid");
        }

        // Test architecture detection and validation
        let arch = arch::ArchitectureType::from_machine(elf.header().machine).unwrap();
        match arch {
            arch::ArchitectureType::X86_64(x86_arch) => {
                x86_arch.check_features()
                    .expect("x86_64 features should be available");
            }
            _ => panic!("Expected x86_64 architecture"),
        }
    }
}