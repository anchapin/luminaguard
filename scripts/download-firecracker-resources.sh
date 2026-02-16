#!/bin/bash
# Download Firecracker VM resources (kernel and rootfs)
# Required for running Firecracker integration tests

set -e

TARGET_DIR="${1:-/tmp/luminaguard-fc-test}"

echo "Downloading Firecracker resources to: $TARGET_DIR"

# Create target directory
mkdir -p "$TARGET_DIR"

# Download kernel image (v4.14 from Firecracker releases)
KERNEL_URL="https://s3.amazonaws.com/spec.ccfc.min/img/kiwi-no-dash.img"
KERNEL_FILE="$TARGET_DIR/vmlinux.bin"

if [ -f "$KERNEL_FILE" ]; then
    echo "Kernel already exists: $KERNEL_FILE"
else
    echo "Downloading kernel image..."
    wget -q -O "$KERNEL_FILE" "$KERNEL_URL" || {
        echo "Failed to download kernel. Trying alternative..."
        # Fallback: try different URL
        wget -q -O "$KERNEL_FILE" "https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/vmlinux" || {
            echo "ERROR: Could not download kernel image"
            exit 1
        }
    }
    echo "Kernel downloaded: $KERNEL_FILE"
fi

# Download rootfs image (Ubuntu minimal)
ROOTFS_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/0-rootfs.ext4"
ROOTFS_FILE="$TARGET_DIR/rootfs.ext4"

if [ -f "$ROOTFS_FILE" ]; then
    echo "Rootfs already exists: $ROOTFS_FILE"
else
    echo "Downloading rootfs image..."
    wget -q -O "$ROOTFS_FILE" "$ROOTFS_URL" || {
        echo "Failed to download rootfs. Trying alternative..."
        wget -q -O "$ROOTFS_FILE" "https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/rootfs.ext4" || {
            echo "ERROR: Could not download rootfs image"
            exit 1
        }
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
echo "To run tests:"
echo "  cargo test --lib vm::tests"
