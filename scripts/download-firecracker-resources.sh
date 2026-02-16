#!/bin/bash
# Download Firecracker VM resources (kernel and rootfs)
# Required for running Firecracker integration tests
#
# Usage:
#   ./download-firecracker-resources.sh              # Download to ./resources/
#   ./download-firecracker-resources.sh /custom/path # Download to custom location
#
# The code expects resources at:
#   - ./resources/vmlinux     (kernel)
#   - ./resources/rootfs.ext4 (root filesystem)

set -e

# Default to ./resources/ in project root (where cargo is run from)
# This matches the paths expected by the code in orchestrator/src/vm/config.rs
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TARGET_DIR="${1:-$PROJECT_ROOT/resources}"

echo "Downloading Firecracker resources to: $TARGET_DIR"

# Create target directory
mkdir -p "$TARGET_DIR"

# Download kernel image (x86_64 vmlinux from Firecracker quickstart)
# Using the correct URL that provides vmlinux (not vmlinux.bin)
KERNEL_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/vmlinux"
KERNEL_FILE="$TARGET_DIR/vmlinux"

if [ -f "$KERNEL_FILE" ]; then
    echo "Kernel already exists: $KERNEL_FILE"
else
    echo "Downloading kernel image..."
    wget -q -O "$KERNEL_FILE" "$KERNEL_URL" || {
        echo "ERROR: Could not download kernel image from $KERNEL_URL"
        echo "Trying alternative URL with .bin extension..."
        # Fallback to the .bin version
        KERNEL_URL_BIN="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin"
        wget -q -O "$KERNEL_FILE" "$KERNEL_URL_BIN" || {
            echo "ERROR: Could not download kernel image from either URL"
            exit 1
        }
    }
    echo "Kernel downloaded: $KERNEL_FILE"
fi

# Download rootfs image (Ubuntu bionic from Firecracker quickstart)
ROOTFS_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/rootfs.ext4"
ROOTFS_FILE="$TARGET_DIR/rootfs.ext4"

if [ -f "$ROOTFS_FILE" ]; then
    echo "Rootfs already exists: $ROOTFS_FILE"
else
    echo "Downloading rootfs image..."
    wget -q -O "$ROOTFS_FILE" "$ROOTFS_URL" || {
        echo "ERROR: Could not download rootfs image from $ROOTFS_URL"
        exit 1
    }
    echo "Rootfs downloaded: $ROOTFS_FILE"
fi

# Verify files exist and have content
if [ ! -s "$KERNEL_FILE" ]; then
    echo "ERROR: Kernel file is empty"
    exit 1
fi

if [ ! -s "$ROOTFS_FILE" ]; then
    echo "ERROR: Rootfs file is empty"
    exit 1
fi

echo ""
echo "✅ Firecracker resources downloaded successfully!"
echo "   Kernel: $KERNEL_FILE ($(stat -c%s "$KERNEL_FILE") bytes)"
echo "   Rootfs: $ROOTFS_FILE ($(stat -c%s "$ROOTFS_FILE") bytes)"
echo ""
echo "To run tests from orchestrator directory:"
echo "  cd orchestrator"
echo "  cargo test --lib vm::integration_tests -- --ignored"
echo ""
echo "To spawn a VM:"
echo "  cargo run --bin luminaguard -- spawn-vm"
