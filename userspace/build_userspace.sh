#!/bin/bash
# Build script for PrismaOS userspace programs

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
TARGET_SPEC="target-specs/x86_64-prismaos.json"
TARGET_NAME="x86_64-prismaos"
OUTPUT_DIR="target/userspace"

echo -e "${YELLOW}Building PrismaOS userspace programs...${NC}"

# Ensure we're in the userspace directory
if [ ! -f "$TARGET_SPEC" ]; then
    echo -e "${RED}Error: Must run from userspace directory${NC}"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Function to build a userspace app
build_app() {
    local app_name="$1"
    local app_dir="apps/$app_name"
    
    if [ ! -d "$app_dir" ]; then
        echo -e "${RED}Error: App directory $app_dir not found${NC}"
        return 1
    fi
    
    echo -e "${YELLOW}Building $app_name...${NC}"
    
    # Build the app using our custom target
    cd "$app_dir"
    
    # Use cargo to build with our custom target
    RUST_TARGET_PATH="../../target-specs" cargo build \
        --target="$TARGET_NAME" \
        --release \
        -Z build-std=core,alloc \
        -Z build-std-features=panic_immediate_abort
    
    # Copy the binary to our output directory
    if [ -f "target/$TARGET_NAME/release/$app_name" ]; then
        cp "target/$TARGET_NAME/release/$app_name" "../../$OUTPUT_DIR/"
        echo -e "${GREEN}Built $app_name successfully${NC}"
    else
        echo -e "${RED}Failed to build $app_name${NC}"
        return 1
    fi
    
    cd - > /dev/null
}

# Build runtime library first
echo -e "${YELLOW}Building runtime library...${NC}"
cd runtime
RUST_TARGET_PATH="../target-specs" cargo build \
    --target="$TARGET_NAME" \
    --release \
    -Z build-std=core,alloc \
    -Z build-std-features=panic_immediate_abort
cd ..

# Build all apps
for app_dir in apps/*/; do
    if [ -d "$app_dir" ]; then
        app_name=$(basename "$app_dir")
        build_app "$app_name"
    fi
done

echo -e "${GREEN}All userspace programs built successfully!${NC}"
echo -e "${YELLOW}Binaries available in: $OUTPUT_DIR${NC}"
ls -la "$OUTPUT_DIR"