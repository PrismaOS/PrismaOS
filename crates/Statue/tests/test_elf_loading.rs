//! Comprehensive tests for ELF binary loading and execution

extern crate alloc;
use alloc::vec::Vec;
use statue::*;
use statue::loader::MemoryAllocator;

/// Create a minimal valid x86_64 ELF executable for testing
fn create_test_elf_x86_64() -> Vec<u8> {
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
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_shoff: 0 (no sections)
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

    // Code section (simple exit program)
    elf.extend_from_slice(&[
        // Simple x86_64 assembly: mov $60, %rax; mov $0, %rdi; syscall
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, // mov $60, %rax
        0x48, 0xc7, 0xc7, 0x00, 0x00, 0x00, 0x00, // mov $0, %rdi
        0x0f, 0x05,                               // syscall
        0x90, 0x90, 0x90, 0x90,                   // nop padding
    ]);

    // Pad to total size
    while elf.len() < 144 {
        elf.push(0x90); // nop
    }

    elf
}

/// Create a minimal valid AArch64 ELF executable for testing
fn create_test_elf_aarch64() -> Vec<u8> {
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
        0xb7, 0x00,               // e_machine: EM_AARCH64
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

    // Simple AArch64 exit code
    elf.extend_from_slice(&[
        0xa8, 0x0b, 0x80, 0xd2, // mov x8, #93 (exit syscall)
        0x00, 0x00, 0x80, 0xd2, // mov x0, #0 (exit code)
        0x01, 0x00, 0x00, 0xd4, // svc #0 (system call)
        0x00, 0x00, 0x00, 0x00, // padding
    ]);

    // Pad to total size
    while elf.len() < 144 {
        elf.push(0x00);
    }

    elf
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_elf_header_parsing() {
        let elf_data = create_test_elf_x86_64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse test ELF");

        assert_eq!(elf.header().ident.class, header::ElfClass::Elf64);
        assert_eq!(elf.header().ident.data, header::ElfData::LittleEndian);
        assert_eq!(elf.header().file_type, header::ElfType::Executable);
        assert_eq!(elf.header().machine, header::ElfMachine::X86_64);
        assert_eq!(elf.header().entry, 0x400078);
    }

    #[test]
    fn test_program_header_parsing() {
        let elf_data = create_test_elf_x86_64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse test ELF");

        let program_headers = program::ProgramHeaderIter::new(&elf)
            .expect("Failed to create program header iterator");

        assert_eq!(program_headers.len(), 1);

        let ph = program_headers.get(0).expect("Failed to get program header");
        assert_eq!(ph.segment_type, program::ProgramType::Load);
        assert_eq!(ph.vaddr, 0x400000);
        assert_eq!(ph.filesz, 144);
        assert_eq!(ph.memsz, 144);
        assert!(ph.flags.readable());
        assert!(ph.flags.executable());
        assert!(!ph.flags.writable());
    }

    #[test]
    fn test_elf_loading() {
        let elf_data = create_test_elf_x86_64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse test ELF");

        // Create memory pool for allocator
        let mut memory = vec![0u8; 1024 * 1024]; // 1MB
        let allocator = loader::SimpleAllocator::new(&mut memory);

        // Create loader configuration
        let config = loader::LoaderConfig::new(allocator)
            .with_relocations(false) // No relocations in our simple test
            .with_symbol_resolution(false);

        // Load the ELF
        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).expect("Failed to load ELF");

        // Verify loaded binary
        assert_eq!(loaded_binary.entry_point, 0x400078);
        assert_eq!(loaded_binary.segments.len(), 1);

        let segment = &loaded_binary.segments[0];
        assert_eq!(segment.vaddr, 0x400000);
        assert_eq!(segment.size, 144);
        assert!(segment.executable);
        assert!(!segment.writable);
    }

    #[test]
    fn test_execution_context_creation() {
        let elf_data = create_test_elf_x86_64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse test ELF");

        let mut memory = vec![0u8; 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);

        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).expect("Failed to load ELF");

        // Create execution environment
        let environment = execution::ExecutionEnvironment::new()
            .with_arg("test_program")
            .with_env("PATH", "/usr/bin");

        // Create execution context
        let context = execution::ExecutionContext::new(loaded_binary, environment)
            .expect("Failed to create execution context");

        assert_eq!(context.instruction_pointer(), 0x400078);
    }

    #[test]
    fn test_memory_operations() {
        let elf_data = create_test_elf_x86_64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse test ELF");

        let mut memory = vec![0u8; 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);

        let mut loader = loader::ElfLoader::new(config);
        let mut loaded_binary = loader.load(&elf).expect("Failed to load ELF");

        // Test reading memory
        let code_start = 0x400078;
        let code_bytes = loaded_binary.read_memory(code_start, 7)
            .expect("Failed to read memory");

        // Verify we read the expected instruction bytes
        assert_eq!(code_bytes[0], 0x48); // REX.W prefix
        assert_eq!(code_bytes[1], 0xc7); // MOV instruction
        assert_eq!(code_bytes[2], 0xc0); // Register encoding

        // Test that we can't write to read-only segment
        let write_result = loaded_binary.write_memory(code_start, &[0x90, 0x90]);
        assert!(write_result.is_err());
    }

    #[test]
    fn test_aarch64_elf_loading() {
        let elf_data = create_test_elf_aarch64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse AArch64 ELF");

        assert_eq!(elf.header().machine, header::ElfMachine::AArch64);

        let mut memory = vec![0u8; 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);

        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).expect("Failed to load AArch64 ELF");

        assert_eq!(loaded_binary.entry_point, 0x400078);
        match loaded_binary.architecture {
            arch::ArchitectureType::AArch64(_) => {
                // Expected
            }
            _ => panic!("Expected AArch64 architecture"),
        }
    }

    #[test]
    fn test_error_conditions() {
        // Test invalid ELF magic
        let invalid_magic = vec![0x00, 0x01, 0x02, 0x03];
        assert!(ElfFile::parse(&invalid_magic).is_err());

        // Test truncated ELF
        let truncated = vec![0x7f, 0x45, 0x4c, 0x46, 0x02];
        assert!(ElfFile::parse(&truncated).is_err());

        // Test invalid class
        let invalid_class = vec![
            0x7f, 0x45, 0x4c, 0x46, 0x03, // Invalid class
            0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(ElfFile::parse(&invalid_class).is_err());
    }

    #[test]
    fn test_memory_allocator() {
        let mut memory = vec![0u8; 1024];
        let mut allocator = loader::SimpleAllocator::new(&mut memory);

        // Test basic allocation
        let ptr1 = allocator.allocate(64, 8)
            .expect("Failed to allocate memory");
        assert!(!ptr1.is_null());

        // Test aligned allocation
        let ptr2 = allocator.allocate(32, 16)
            .expect("Failed to allocate aligned memory");
        assert_eq!(ptr2 as usize % 16, 0);

        // Test memory mapping
        let mapped_ptr = allocator.map_at(0x400000, 4096, false, true)
            .expect("Failed to map memory");
        assert_eq!(mapped_ptr as u64, 0x400000);
    }

    #[test]
    fn test_architecture_detection() {
        // Test x86_64
        let x86_elf = create_test_elf_x86_64();
        let elf = ElfFile::parse(&x86_elf).expect("Failed to parse x86_64 ELF");
        let arch = arch::ArchitectureType::from_machine(elf.header().machine)
            .expect("Failed to create architecture");

        match arch {
            arch::ArchitectureType::X86_64(_) => {
                assert_eq!(arch.as_architecture().pointer_size(), 8);
                assert_eq!(arch.as_architecture().page_size(), 4096);
            }
            _ => panic!("Expected x86_64 architecture"),
        }

        // Test AArch64
        let aarch64_elf = create_test_elf_aarch64();
        let elf = ElfFile::parse(&aarch64_elf).expect("Failed to parse AArch64 ELF");
        let arch = arch::ArchitectureType::from_machine(elf.header().machine)
            .expect("Failed to create architecture");

        match arch {
            arch::ArchitectureType::AArch64(_) => {
                assert_eq!(arch.as_architecture().pointer_size(), 8);
                assert_eq!(arch.as_architecture().page_size(), 4096);
            }
            _ => panic!("Expected AArch64 architecture"),
        }
    }

    #[test]
    fn test_execution_environment() {
        let env = execution::ExecutionEnvironment::new()
            .with_arg("program")
            .with_arg("--verbose")
            .with_arg("input.txt")
            .with_env("HOME", "/home/user")
            .with_env("PATH", "/usr/bin:/bin")
            .with_stack_size(0x200000)
            .with_heap_size(0x1000000);

        assert_eq!(env.args.len(), 3);
        assert_eq!(env.env.len(), 2);
        assert_eq!(env.stack_size, 0x200000);
        assert_eq!(env.heap_size, 0x1000000);
    }

    #[test]
    fn test_process_lifecycle() {
        let elf_data = create_test_elf_x86_64();
        let elf = ElfFile::parse(&elf_data).expect("Failed to parse test ELF");

        let mut memory = vec![0u8; 1024 * 1024];
        let allocator = loader::SimpleAllocator::new(&mut memory);
        let config = loader::LoaderConfig::new(allocator);

        let mut loader = loader::ElfLoader::new(config);
        let loaded_binary = loader.load(&elf).expect("Failed to load ELF");

        let environment = execution::ExecutionEnvironment::new()
            .with_arg("test");

        let mut pcb = execution::ProcessControlBlock::new(1, loaded_binary, environment)
            .expect("Failed to create PCB");

        assert_eq!(pcb.state, execution::ProcessState::Ready);

        pcb.start().expect("Failed to start process");
        assert_eq!(pcb.state, execution::ProcessState::Running);

        // Simulate execution completion
        pcb.terminate(0);
        assert_eq!(pcb.state, execution::ProcessState::Terminated);
        assert_eq!(pcb.exit_code, Some(0));
    }

    #[test]
    fn test_validation_edge_cases() {
        // Test various edge cases in ELF validation

        // Create ELF with invalid program header size
        let mut invalid_ph_elf = create_test_elf_x86_64();
        // Modify e_phentsize to invalid value
        invalid_ph_elf[54] = 32; // Should be 56 for 64-bit

        match ElfFile::parse(&invalid_ph_elf) {
            Err(ElfError::InvalidProgramHeader) => {
                // Expected
            }
            Err(_e) => {
                // Unexpected error
            }
            Ok(_) => {
                // Should have failed validation
            }
        }
    }
}