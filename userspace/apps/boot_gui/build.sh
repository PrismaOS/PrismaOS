#!/bin/bash
# Build script for boot GUI binary

set -e

echo "Building boot-gui for x86_64-unknown-linux-gnu..."

# Build for Linux target to get ELF binary
cargo build --release --target x86_64-unknown-linux-gnu

# Copy the binary to a known location
cp target/x86_64-unknown-linux-gnu/release/libboot_gui.so target/boot_gui.elf

echo "ELF binary created at target/boot_gui.elf"
ls -la target/boot_gui.elf