# PrismaOS Development Guide

This guide covers how to build, test, and develop PrismaOS.

## Quick Start

```bash
# Setup development environment (only needed once)
make setup

# Build everything and run in QEMU
make all-prisma
make run-prisma
```

## Build System

PrismaOS uses the existing GNUmakefile with integrated PrismaOS-specific targets:

### Primary Build Targets

```bash
# Build kernel and create bootable ISO
make all

# Build kernel + userspace + ISO (recommended)
make all-prisma

# Build only the kernel
make kernel

# Build only userspace components
make userspace

# Build with debug symbols
make kernel-debug
```

### Running PrismaOS

```bash
# Run with enhanced QEMU settings (recommended)
make run-prisma

# Standard run with UEFI firmware
make run

# Run with legacy BIOS
make run-bios

# Debug with GDB
make debug-gdb
```

### Development Workflow

```bash
# Format all code
make fmt

# Lint all code
make lint

# Run all tests
make test

# Generate documentation
make docs

# Performance benchmarks
make bench

# Security audit
make audit
```

### Architecture Support

PrismaOS supports multiple architectures through the `KARCH` variable:

```bash
# x86_64 (default)
make all-prisma

# ARM64
make all-prisma KARCH=aarch64

# RISC-V 64-bit
make all-prisma KARCH=riscv64

# LoongArch 64-bit  
make all-prisma KARCH=loongarch64
```

## Project Structure

```
PrismaOS/
├── kernel/                 # Kernel implementation
│   ├── src/
│   │   ├── main.rs        # Kernel entry point
│   │   ├── memory/        # Memory management
│   │   ├── executor/      # Async task executor
│   │   ├── api/           # Object-based IPC system
│   │   ├── gdt.rs         # Global descriptor table
│   │   └── interrupts.rs  # Interrupt handling
│   └── Cargo.toml
├── userspace/             # Userspace components
│   ├── compositor/        # Multithreaded compositor
│   │   ├── src/
│   │   │   ├── lib.rs     # Compositor core
│   │   │   ├── surface.rs # Surface management
│   │   │   ├── renderer.rs# Software renderer
│   │   │   ├── input.rs   # Input management
│   │   │   └── exclusive.rs# Exclusive display access
│   │   └── Cargo.toml
│   └── apps/
│       └── demo/          # Demo application
│           ├── src/lib.rs # Demo showcase app
│           └── Cargo.toml
├── limine/                # Limine bootloader (git submodule)
├── limine.conf           # Boot configuration
├── GNUmakefile           # Build system
├── README.md             # Project overview
└── DEVELOPMENT.md        # This file
```

## Development Principles

### 1. Safety First
- Minimize `unsafe` blocks and document invariants
- Use Rust's type system to enforce correctness
- Comprehensive error handling with proper Result types

### 2. Performance-Oriented
- Zero-copy data paths where possible
- Lock-free data structures in hot paths
- NUMA-aware memory allocation strategies
- Sub-millisecond latency targets for critical paths

### 3. Modern Architecture
- No central event loop (avoid Unix anti-pattern)
- Multithreaded from the ground up
- Capability-based security model
- Object-oriented IPC (not string-based)

## Key Subsystems

### Memory Management (`kernel/src/memory/`)
- Page frame allocator using memory map from bootloader
- Heap allocator for kernel objects
- Virtual memory management with identity mapping
- Zero-copy buffer sharing for graphics

### Task Executor (`kernel/src/executor/`)
- Async/await-based cooperative multitasking
- Wake-based scheduling without busy loops
- Keyboard input handling via async streams
- Future-based I/O operations

### Object-Based IPC (`kernel/src/api/`)
- Typed objects: Surface, Buffer, EventStream, Display
- Capability handles with fine-grained permissions
- Secure object method dispatch
- Zero-copy parameter passing

### Compositor (`userspace/compositor/`)
- Multithreaded rendering pipeline
- Software renderer with alpha blending
- Input event routing and focus management
- Exclusive display access for low-latency apps

## Testing Strategy

### Unit Tests
```bash
# Test individual kernel components
make test-kernel

# Test userspace components
cd userspace/compositor && cargo test
```

### Integration Tests
```bash
# Full system test in QEMU
make test

# Manual testing
make run-prisma
```

### Performance Testing
```bash
# Benchmark critical paths
make bench

# Latency measurements (requires hardware)
# Frame latency: should be < 16ms windowed, < 3ms exclusive
# Context switch: should be < 5μs
# IPC round-trip: should be < 10μs
```

## Debugging

### QEMU + GDB
```bash
# Start debug session
make debug-gdb

# In GDB:
(gdb) break kmain
(gdb) continue
(gdb) print framebuffer_response
```

### Serial Output
All kernel output goes to both framebuffer and serial console:
```bash
# View serial output in separate terminal
make run-prisma 2>&1 | tee kernel.log
```

### Logging
Use the `println!` macro for kernel debugging:
```rust
println!("Framebuffer: {}x{} at {:p}", width, height, addr);
```

## Contributing Guidelines

### Code Style
- Follow standard Rust formatting (`make fmt`)
- Use descriptive variable names
- Keep functions focused and small
- Document public APIs with rustdoc

### Performance Requirements
- Frame latency: < 16ms windowed mode, < 3-4ms exclusive mode
- Memory usage: Kernel < 64MB, efficient userspace allocation
- Boot time: < 5 seconds to compositor ready
- Input latency: < 10ms keyboard/mouse to application

### Security Model
- All resource access via capability handles
- Fine-grained permissions (READ, WRITE, EXECUTE, DELETE, SHARE)
- Capability revocation for security events
- Process isolation with separate virtual address spaces

## Advanced Features

### Exclusive Display Access
PrismaOS supports ultra-low latency graphics for demanding applications:

```rust
// Request exclusive access
let result = exclusive_manager.request_exclusive(
    surface_id,
    ExclusiveMode::DirectPlane,
    200 // High priority
)?;

// Get direct framebuffer access
let direct_fb = exclusive_manager.get_direct_framebuffer(surface_id)?;

// Render directly to scanout buffer (< 3ms latency)
unsafe { render_directly_to_hardware(direct_fb, frame_data); }
```

### Object-Based IPC Examples
```rust
// Create typed surface object  
let surface = syscall_create_object(ObjectType::Surface, 800, 600, PixelFormat::Rgba8888)?;

// Call methods on the object
surface.attach_buffer(buffer_handle)?;
surface.set_position(100, 100)?;
surface.commit()?;

// Transfer capability to another process
transfer_capability(surface, from_pid, to_pid, Rights::READ | Rights::WRITE)?;
```

## Roadmap

### Phase 1: Core System ✅
- [x] Memory management and virtual memory
- [x] Async task executor
- [x] Object-based IPC system
- [x] Basic device drivers (keyboard, framebuffer)
- [x] Multithreaded compositor
- [x] Exclusive display access

### Phase 2: Advanced Features 🚧
- [ ] SMP scheduler with work-stealing
- [ ] Network stack and storage drivers
- [ ] GPU acceleration hooks
- [ ] POSIX compatibility layer (optional)

### Phase 3: Production Hardening 📋
- [ ] Formal verification of critical paths
- [ ] Hardware certification testing
- [ ] Performance optimization
- [ ] Security audit and hardening

## Troubleshooting

### Build Issues
```bash
# Clean everything and rebuild
make distclean
make setup
make all-prisma
```

### QEMU Issues
```bash
# If graphics don't appear, try different VGA options:
make run-prisma QEMUFLAGS="-vga virtio"

# For debugging boot issues:
make run-prisma QEMUFLAGS="-d int,cpu_reset"
```

### Performance Issues
- Check frame counters in compositor output
- Monitor memory usage with kernel statistics
- Profile critical paths with `cargo bench`
- Use hardware performance counters when available

## Resources

- [Limine Boot Protocol](https://github.com/limine-bootloader/limine/blob/trunk/PROTOCOL.md)
- [Rust Embedded Book](https://docs.rust-embedded.org/book/)
- [OSDev Wiki](https://wiki.osdev.org/)
- [Intel SDM](https://software.intel.com/content/www/us/en/develop/articles/intel-sdm.html)
- [AMD Manual](https://www.amd.com/system/files/TechDocs/40332.pdf)