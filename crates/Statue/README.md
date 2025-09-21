# Statue 🗿

[![Crates.io](https://img.shields.io/crates/v/statue)](https://crates.io/crates/statue)
[![Documentation](https://docs.rs/statue/badge.svg)](https://docs.rs/statue)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

**Statue** is a production-ready, no-std Rust library for parsing and executing ELF (Executable and Linkable Format) binaries in operating systems and embedded environments.

## ✨ Features

- **🚫 No-std compatible**: Works in kernel space and embedded environments
- **🔒 Memory-safe**: Zero unsafe code with comprehensive validation
- **🏗️ Zero dependencies**: Self-contained implementation
- **🎯 Multi-architecture**: Support for x86_64, AArch64, and RISC-V
- **📄 Complete ELF support**: Headers, sections, symbols, relocations
- **⚡ Production-ready**: Extensive error handling and edge case coverage
- **🔗 Dynamic linking**: Support for shared libraries and PLT/GOT
- **💾 Memory management**: Flexible memory allocation with custom allocators

## 🚀 Quick Start

Add Statue to your `Cargo.toml`:

```toml
[dependencies]
statue = "0.1"
```

### Basic ELF Parsing

```rust
#![no_std]
extern crate alloc;

use statue::{ElfFile, Result};

fn parse_elf_binary(data: &[u8]) -> Result<()> {
    // Parse the ELF file
    let elf = ElfFile::parse(data)?;

    println!("ELF type: {:?}", elf.header().file_type);
    println!("Architecture: {:?}", elf.header().machine);
    println!("Entry point: 0x{:x}", elf.header().entry);

    Ok(())
}
```

### Loading and Executing ELF Binaries

```rust
#![no_std]
extern crate alloc;

use statue::{ElfFile, ElfLoader, LoaderConfig, SimpleAllocator};
use statue::{ExecutionContext, ExecutionEnvironment};

fn load_and_execute(elf_data: &[u8]) -> statue::Result<u64> {
    // Parse the ELF file
    let elf = ElfFile::parse(elf_data)?;

    // Set up memory allocator
    let mut memory = vec![0u8; 1024 * 1024]; // 1MB memory pool
    let allocator = SimpleAllocator::new(&mut memory);

    // Create loader configuration
    let config = LoaderConfig::new(allocator)
        .with_relocations(true)
        .with_symbol_resolution(true);

    // Load the binary
    let mut loader = ElfLoader::new(config);
    let loaded_binary = loader.load(&elf)?;

    // Set up execution environment
    let environment = ExecutionEnvironment::new()
        .with_arg("program")
        .with_arg("--help")
        .with_env("PATH", "/usr/bin");

    // Create execution context
    let mut context = ExecutionContext::new(loaded_binary, environment)?;

    // Execute the binary
    context.execute()
}
```

### Custom Memory Allocator

```rust
use statue::{MemoryAllocator, Result, ElfError};

struct MyCustomAllocator {
    // Your allocator implementation
}

impl MemoryAllocator for MyCustomAllocator {
    fn allocate(&mut self, size: usize, alignment: usize) -> Result<*mut u8> {
        // Your allocation logic
        todo!()
    }

    fn deallocate(&mut self, ptr: *mut u8, size: usize) {
        // Your deallocation logic
    }

    fn map_at(&mut self, vaddr: u64, size: usize, writable: bool, executable: bool) -> Result<*mut u8> {
        // Your memory mapping logic
        todo!()
    }

    fn unmap(&mut self, vaddr: u64, size: usize) -> Result<()> {
        // Your unmapping logic
        Ok(())
    }

    fn protect(&mut self, vaddr: u64, size: usize, writable: bool, executable: bool) -> Result<()> {
        // Your protection logic
        Ok(())
    }
}
```

## 📚 Architecture Support

Statue supports multiple target architectures with comprehensive relocation and calling convention handling:

### x86_64
- Complete register state management
- System V ABI calling convention
- Full relocation type support (R_X86_64_*)
- SIMD and FPU state (planned)

### AArch64
- ARMv8-A instruction set
- AAPCS calling convention
- Exception level management
- SVE support (planned)

### RISC-V
- RV64I base instruction set
- Standard calling convention
- Supervisor and user modes
- Vector extensions (planned)

## 🏗️ Core Components

### ELF Parsing
- **Header validation**: Magic numbers, architecture, endianness
- **Program headers**: Loadable segments, dynamic linking info
- **Section headers**: Code, data, symbol tables, relocations
- **String tables**: Section and symbol name resolution

### Memory Management
- **Segment loading**: Virtual memory layout, permissions
- **Relocation processing**: Static and dynamic relocations
- **Symbol resolution**: Local and global symbol tables
- **Dynamic linking**: PLT/GOT, shared library support

### Execution Environment
- **Process creation**: Stack, heap, argument setup
- **Context switching**: Register state management
- **System calls**: Kernel interface (planned)
- **Signal handling**: POSIX signals (planned)

## 🔧 Advanced Usage

### Symbol Resolution

```rust
use statue::{SymbolResolver, SymbolTable};

// Set up symbol resolver
let mut resolver = SymbolResolver::new();

// Add symbol tables
if let Some(symtab_data) = get_symbol_table_data() {
    let symbol_table = SymbolTable::new(
        symtab_data,
        Some(string_table_data),
        elf.header().is_64bit(),
        elf.header().is_little_endian(),
    )?;
    resolver.set_symbol_table(symbol_table);
}

// Resolve symbols
if let Some(symbol) = resolver.resolve("main")? {
    println!("Found main at: 0x{:x}", symbol.value);
}
```

### Relocation Processing

```rust
use statue::{RelocationProcessor, RelocationIter};

let processor = RelocationProcessor::new(base_address);

// Process relocations
for relocation_result in RelocationIter::new(reloc_data, is_64bit, is_little_endian, machine)? {
    let relocation = relocation_result?;
    processor.apply_relocation(&relocation, &symbol_resolver, memory)?;
}
```

### Architecture-Specific Features

```rust
use statue::arch::{ArchitectureType, MemoryLayout, X86_64};

// Create architecture-specific configuration
let arch = ArchitectureType::from_machine(elf.header().machine)?;
let memory_layout = MemoryLayout::default_for_architecture(arch);

// Architecture-specific operations
match arch {
    ArchitectureType::X86_64(x86_arch) => {
        x86_arch.check_features()?;
        let exec_state = x86_arch.setup_execution_state()?;
    }
    _ => {}
}
```

## 🛠️ Error Handling

Statue provides comprehensive error handling with detailed error types:

```rust
use statue::{ElfError, Result};

match parse_result {
    Err(ElfError::InvalidMagic) => {
        eprintln!("Not a valid ELF file");
    }
    Err(ElfError::UnsupportedArchitecture) => {
        eprintln!("Architecture not supported");
    }
    Err(ElfError::AllocationFailed) => {
        eprintln!("Out of memory");
    }
    Err(e) => {
        eprintln!("Error: {}", e.description());
    }
    Ok(result) => {
        // Handle success
    }
}
```

## 🔬 Testing

Run the test suite:

```bash
cargo test --all-features
```

Run tests in no-std environment:

```bash
cargo test --no-default-features
```

## 📖 Documentation

Full API documentation is available at [docs.rs/statue](https://docs.rs/statue).

Build documentation locally:

```bash
cargo doc --open --all-features
```

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/statue.git
   cd statue
   ```

2. Install Rust toolchain:
   ```bash
   rustup install stable
   rustup component add clippy rustfmt
   ```

3. Run checks:
   ```bash
   cargo check
   cargo clippy
   cargo fmt --check
   ```

## 📜 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🙏 Acknowledgments

- The [ELF specification](https://refspecs.linuxfoundation.org/elf/elf.pdf)
- The Rust embedded working group for no-std patterns
- The [goblin](https://github.com/m4b/goblin) crate for ELF parsing inspiration

## 🗺️ Roadmap

- [ ] Complete dynamic linking implementation
- [ ] DWARF debug information parsing
- [ ] ELF core dump generation
- [ ] SIMD and vector extension support
- [ ] WebAssembly target support
- [ ] Formal verification with Kani

---

**Statue** - Building robust systems, one ELF at a time. 🗿