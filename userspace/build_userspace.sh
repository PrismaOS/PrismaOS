#!/bin/bash
# Build script for PrismaOS userspace programs

set -e

echo "🏗️  Building PrismaOS Userspace Programs"
echo "======================================="

# Build userspace runtime library
echo "📚 Building userspace runtime library..."
cd runtime
RUSTFLAGS="-C panic=abort" cargo build --release --quiet
cd ..

# Build hello world application
echo "👋 Building hello_world application..."
cd apps/hello_world
RUSTFLAGS="-C panic=abort" cargo build --release --quiet --target x86_64-unknown-linux-gnu
cd ../..

# Check if the binary was created
if [ -f "target/x86_64-unknown-linux-gnu/release/hello_world" ]; then
    echo "✅ hello_world binary created successfully"
    ls -la target/x86_64-unknown-linux-gnu/release/hello_world
    
    # Show binary info
    echo "📊 Binary information:"
    file target/x86_64-unknown-linux-gnu/release/hello_world
    size target/x86_64-unknown-linux-gnu/release/hello_world
else
    echo "❌ Failed to create hello_world binary"
    exit 1
fi

echo ""
echo "🎉 Userspace build complete!"
echo "Binary ready at: userspace/target/x86_64-unknown-linux-gnu/release/hello_world"