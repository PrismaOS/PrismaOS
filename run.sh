#!/bin/bash

echo "Cleaning previous build..."
cargo clean

echo "Building project..."
cargo build

echo "Looking for disk image..."
# Find the disk image recursively in target directory
DISK_IMAGE=$(find target -name "*.img" -type f 2>/dev/null)

if [ -z "$DISK_IMAGE" ]; then
    echo "ERROR: No disk image found!"
    echo "Checking target directory structure:"
    ls -R target/
    exit 1
fi

echo "Found disk image at: $DISK_IMAGE"
echo "Starting QEMU..."
qemu-system-x86_64 \
    -drive format=raw,file="$DISK_IMAGE" \
    -serial stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04