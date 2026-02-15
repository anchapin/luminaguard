#!/bin/bash
# VM Crash Testing Script
#
# This script runs the Week 1-2 reliability VM crash testing suite.
#
# Usage:
#   ./scripts/run-reliability-tests.sh [kernel_path] [rootfs_path] [results_path]
#
# Example:
#   ./scripts/run-reliability-tests.sh \
#     /tmp/luminaguard-fc-test/vmlinux.bin \
#     /tmp/luminaguard-fc-test/rootfs.ext4 \
#     .beads/metrics/reliability

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default paths
KERNEL_PATH="${1:-/tmp/luminaguard-fc-test/vmlinux.bin}"
ROOTFS_PATH="${2:-/tmp/luminaguard-fc-test/rootfs.ext4}"
RESULTS_PATH="${3:-.beads/metrics/reliability}"

# Print banner
echo "========================================"
echo "  LuminaGuard VM Crash Testing"
echo "  Week 1-2: Reliability Tests"
echo "========================================"
echo ""

# Check if we're in the right directory
if [ ! -f "orchestrator/Cargo.toml" ]; then
    echo -e "${RED}Error: Must run from repository root${NC}"
    echo "Expected to find orchestrator/Cargo.toml"
    exit 1
fi

# Check for Firecracker assets
if [ ! -f "$KERNEL_PATH" ]; then
    echo -e "${YELLOW}Warning: Kernel not found at $KERNEL_PATH${NC}"
    echo "Tests will skip VM spawning and only run unit tests"
    echo ""
fi

if [ ! -f "$ROOTFS_PATH" ]; then
    echo -e "${YELLOW}Warning: Rootfs not found at $ROOTFS_PATH${NC}"
    echo "Tests will skip VM spawning and only run unit tests"
    echo ""
fi

# Create results directory
mkdir -p "$RESULTS_PATH"

# Run unit tests (always run)
echo "=== Running Reliability Unit Tests ==="
echo ""
cd orchestrator
cargo test --lib reliability_tests::tests -- --nocapture

# If assets exist, run full crash tests
if [ -f "$KERNEL_PATH" ] && [ -f "$ROOTFS_PATH" ]; then
    echo ""
    echo "=== Running Full Crash Tests ==="
    echo "Kernel: $KERNEL_PATH"
    echo "Rootfs: $ROOTFS_PATH"
    echo "Results: $RESULTS_PATH"
    echo ""

    # Run crash tests via binary
    cargo run --bin run_crash_tests -- \
        "$KERNEL_PATH" \
        "$ROOTFS_PATH" \
        "$RESULTS_PATH"
else
    echo ""
    echo -e "${YELLOW}Skipping full crash tests (assets not available)${NC}"
    echo "To run full tests, download Firecracker assets:"
    echo "  # Download from Firecracker releases"
    echo "  wget https://github.com/firecracker-microvm/firecracker/releases/download/v1.8.0/vmlinux-v1.8.0"
    echo "  wget https://github.com/firecracker-microvm/firecracker/releases/download/v1.8.0/ubuntu-22.04.ext4"
    echo ""
fi

cd ..

echo ""
echo "========================================"
echo -e "${GREEN}Reliability testing complete!${NC}"
echo "Results saved to: $RESULTS_PATH"
echo "========================================"
