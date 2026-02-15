#!/bin/bash
# Week 1-2 Performance Baseline Runner
#
# This script runs all performance benchmarks for LuminaGuard Week 1-2 validation.
#
# Usage:
#   ./scripts/run-performance-benchmarks.sh [--quick]
#
# Options:
#   --quick    Run quick benchmarks (10 iterations instead of 100)

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default iterations
ITERATIONS=100

# Parse arguments
if [ "$1" == "--quick" ]; then
    ITERATIONS=10
    echo -e "${YELLOW}Running quick benchmarks (${ITERATIONS} iterations)${NC}"
else
    echo -e "${BLUE}Running full performance benchmarks (${ITERATIONS} iterations)${NC}"
fi

echo ""
echo "================================================================"
echo "🚀 LuminaGuard Week 1-2 Performance Baseline"
echo "================================================================"
echo ""

# Ensure metrics directory exists
mkdir -p .beads/metrics/performance

# Check for Firecracker test assets
if [ -f "/tmp/luminaguard-fc-test/vmlinux.bin" ] && [ -f "/tmp/luminaguard-fc-test/rootfs.ext4" ]; then
    echo -e "${GREEN}✓ Firecracker test assets found${NC}"
    HAVE_ASSETS=true
else
    echo -e "${YELLOW}⚠ Firecracker test assets not found${NC}"
    echo "  To download assets, run:"
    echo "    ./scripts/download-firecracker-assets.sh"
    echo ""
    echo "  Running with synthetic benchmarks only..."
    HAVE_ASSETS=false
fi

echo ""
echo "================================================================"
echo "📊 Running Rust Benchmarks"
echo "================================================================"
echo ""

# Build release version for accurate performance
echo -e "${BLUE}Building release version...${NC}"
cd orchestrator
cargo build --release --quiet

# Run Rust performance benchmarks
echo -e "${BLUE}Running Rust performance tests...${NC}"

# Try to run comprehensive benchmark
if cargo test --test baseline_benchmarks baseline_comprehensive --release -- --nocapture --test-threads=1 2>&1; then
    echo -e "${GREEN}✓ Rust benchmarks completed${NC}"
else
    echo -e "${RED}✗ Rust benchmarks failed${NC}"
    echo "  This may be expected if test assets are not available"
fi

cd ..
echo ""

echo "================================================================"
echo "📊 Running Python Benchmarks"
echo "================================================================"
echo ""

# Activate Python virtual environment
echo -e "${BLUE}Activating Python virtual environment...${NC}"
cd agent
source .venv/bin/activate

# Install required dependencies for benchmarks
echo -e "${BLUE}Installing benchmark dependencies...${NC}"
pip install --quiet psutil pytest 2>/dev/null || echo "  (Dependencies may already be installed)"

# Run Python performance benchmarks
echo -e "${BLUE}Running Python performance tests...${NC}"

if pytest tests/performance/agent_benchmarks.py -v -s 2>&1; then
    echo -e "${GREEN}✓ Python benchmarks completed${NC}"
else
    echo -e "${RED}✗ Python benchmarks failed${NC}"
fi

cd ..
echo ""

echo "================================================================"
echo "📁 Metrics Location"
echo "================================================================"
echo ""

# List generated metrics files
if ls .beads/metrics/performance/*.json 1> /dev/null 2>&1; then
    echo "Metrics saved to:"
    ls -lh .beads/metrics/performance/*.json | awk '{print "  " $9 " (" $5 ")"}'
else
    echo -e "${YELLOW}⚠ No metrics files found${NC}"
fi

echo ""

echo "================================================================"
echo "📊 Summary"
echo "================================================================"
echo ""

# Count metrics files
METRIC_COUNT=$(ls .beads/metrics/performance/*.json 2>/dev/null | wc -l)

echo "Total metrics files generated: ${METRIC_COUNT}"
echo ""
echo "Key files to review:"
echo "  - spawn_time_baseline_*.json    (VM spawn time metrics)"
echo "  - memory_baseline_*.json        (Memory usage metrics)"
echo "  - cpu_baseline_*.json           (CPU usage metrics)"
echo "  - network_baseline_*.json       (Network latency metrics)"
echo "  - comprehensive_baseline_*.json (All metrics combined)"
echo "  - agent_baseline_*.json        (Agent-side metrics)"
echo ""

echo "================================================================"
echo "✅ Week 1-2 Performance Baseline Complete"
echo "================================================================"
echo ""

# Print success criteria
echo "Success Criteria:"
echo "  - Spawn time:  <200ms (target)"
echo "  - Memory:      <200MB (target)"
echo "  - CPU:         <50% (target)"
echo "  - Network:     <50ms (target)"
echo ""

# Check if any benchmarks passed
if [ $METRIC_COUNT -gt 0 ]; then
    echo -e "${GREEN}✓ Baseline established! Review metrics files above.${NC}"
else
    echo -e "${YELLOW}⚠ No metrics generated. Check test output above.${NC}"
fi

echo ""
echo "Next steps:"
echo "  1. Review metrics files in .beads/metrics/performance/"
echo "  2. Compare against targets in docs/validation/performance-benchmarks.md"
echo "  3. Document findings in project documentation"
echo ""
