# PrismaOS

A modern, high-performance operating system kernel written in Rust, featuring object-based IPC, multithreaded compositor, and exclusive display ownership for ultra-low latency applications.

## Features

### Core Kernel
- **Memory Management**: Virtual memory with paging, heap allocation, zero-copy buffer sharing
- **Task Scheduling**: Preemptive multitasking with async/await executor
- **Object-Based IPC**: Typed, capability-based inter-process communication (not string-based Unix messages)
- **Device Drivers**: Framebuffer, keyboard, mouse, timer, and interrupt handling
- **Security**: Capability-based handles, fine-grained permissions, secure syscall interface

### Compositor & Graphics
- **Multithreaded Compositor**: High-performance window manager with double-buffering
- **Software Rendering**: Alpha blending, damage tracking, multiple pixel formats
- **Exclusive Display Access**: Ultra-low latency mode for games/VR (< 3-4ms extra latency)
- **Input Management**: Mouse, keyboard with proper focus handling and event routing
- **Multiple Display Modes**: Windowed, exclusive fullscreen, direct hardware plane access

### Object-Based IPC System
- **Surface Objects**: Window surfaces with attach_buffer(), commit(), set_scale() methods
- **Buffer Objects**: Shared memory buffers with zero-copy semantics  
- **EventStream Objects**: Input event delivery with poll_event(), async streaming
- **Display Objects**: Hardware display control with exclusive ownership
- **Capability Handles**: Secure, revocable object references with fine-grained rights

### Low-Latency Graphics Pipeline
- **Direct Framebuffer Access**: Bypass compositor for maximum performance
- **Hardware Plane Support**: Direct-to-scanout rendering for VR/gaming
- **Vsync Control**: Disable vsync for uncapped frame rates
- **Custom Refresh Rates**: Dynamic display timing for specialized applications
- **Zero-Copy Rendering**: Minimize memory bandwidth and CPU overhead

## Building

### Prerequisites
- Rust toolchain (stable)
- QEMU for testing
- xorriso for ISO creation
- GDB for debugging

### Setup Development Environment
```bash
make setup
```

### Build and Run
```bash
# Build everything
make all

# Create bootable ISO and run in QEMU
make run

# Debug with GDB
make debug

# Run tests
make test
```

## Performance Goals

- **Frame Latency**: < 16ms for windowed mode, < 3-4ms for exclusive mode
- **Context Switch**: < 5μs typical
- **IPC Round-trip**: < 10μs for typed object calls
- **Memory Allocation**: Buddy allocator for pages, slab for small objects
- **Interrupt Latency**: < 100μs worst-case
