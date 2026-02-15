#!/bin/bash
# Week 3-4: Concurrent Agent Performance Testing
#
# This script runs concurrent agent performance tests for LuminaGuard Week 3-4 validation.
#
# Usage:
#   ./scripts/run-concurrent-tests.sh [--quick]
#
# Options:
#   --quick    Run quick tests (1 iteration instead of 10)

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default iterations
ITERATIONS=10

# Parse arguments
if [ "$1" == "--quick" ]; then
    ITERATIONS=1
    echo -e "${YELLOW}Running quick concurrent tests (${ITERATIONS} iteration)${NC}"
else
    echo -e "${BLUE}Running full concurrent agent tests (${ITERATIONS} iterations)${NC}"
fi

echo ""
echo "================================================================"
echo "🚀 LuminaGuard Week 3-4: Concurrent Agent Performance Testing"
echo "================================================================"
echo ""
echo "Testing concurrency levels: 5, 10, 25, 50 agents"
echo "Measuring: Resource utilization, throughput, scaling behavior"
echo ""

# Ensure metrics directory exists
mkdir -p .beads/metrics/performance

echo ""
echo "================================================================"
echo "📊 Running Concurrent Agent Benchmarks"
echo "================================================================"
echo ""

# Build release version for accurate performance
echo -e "${BLUE}Building release version...${NC}"
cd orchestrator
cargo build --release --quiet

# Run concurrent agent benchmarks
echo -e "${BLUE}Running concurrent agent benchmarks...${NC}"
echo ""

# Run with criterion
if cargo bench --bench concurrent_agents -- --sample-size $ITERATIONS 2>&1; then
    echo ""
    echo -e "${GREEN}✓ Concurrent agent benchmarks completed${NC}"
else
    echo ""
    echo -e "${RED}✗ Concurrent agent benchmarks failed${NC}"
    exit 1
fi

cd ..
echo ""

echo "================================================================"
echo "📁 Metrics Location"
echo "================================================================"
echo ""

# List generated metrics files
if ls .beads/metrics/performance/concurrent_*.json 1> /dev/null 2>&1; then
    echo -e "${GREEN}Concurrent agent metrics saved:${NC}"
    ls -lh .beads/metrics/performance/concurrent_*.json | awk '{print "  " $9 " (" $5 ")"}'

    echo ""
    echo -e "${CYAN}Scaling metrics:${NC}"
    if ls .beads/metrics/performance/scaling_*.json 1> /dev/null 2>&1; then
        ls -lh .beads/metrics/performance/scaling_*.json | awk '{print "  " $9 " (" $5 ")"}'
    else
        echo -e "${YELLOW}  No scaling metrics found${NC}"
    fi
else
    echo -e "${YELLOW}⚠ No concurrent metrics files found${NC}"
fi

echo ""

echo "================================================================"
echo "📊 Test Results Summary"
echo "================================================================"
echo ""

# Summarize results from JSON files
if ls .beads/metrics/performance/concurrent_*.json 1> /dev/null 2>&1; then
    echo -e "${CYAN}Concurrent Agent Test Results:${NC}"
    echo ""

    for file in .beads/metrics/performance/concurrent_*.json; do
        if [ -f "$file" ]; then
            local agent_count=$(jq -r '.agent_count' "$file" 2>/dev/null || echo "?")
            local total_time=$(jq -r '.total_time_ms' "$file" 2>/dev/null || echo "?")
            local throughput=$(jq -r '.throughput_ops_per_min' "$file" 2>/dev/null || echo "?")
            local scaling=$(jq -r '.scaling_factor' "$file" 2>/dev/null || echo "?")
            local cpu=$(jq -r '.resources.cpu_percent' "$file" 2>/dev/null || echo "?")
            local memory=$(jq -r '.resources.memory_mb' "$file" 2>/dev/null || echo "?")

            echo -e "${BLUE}  ${agent_count} Agents:${NC}"
            echo -e "    Total Time:        ${total_time} ms"
            echo -e "    Throughput:        ${throughput} ops/min"
            echo -e "    Scaling Factor:    ${scaling} (target: ≈1.0)"
            echo -e "    CPU Usage:         ${cpu}%"
            echo -e "    Memory Usage:      ${memory} MB"
            echo ""
        fi
    done
else
    echo -e "${YELLOW}⚠ No concurrent agent results available${NC}"
    echo ""
fi

echo ""
echo "================================================================"
echo "✅ Week 3-4 Concurrent Agent Testing Complete"
echo "================================================================"
echo ""

# Print success criteria
echo -e "${CYAN}Success Criteria:${NC}"
echo "  - Linear scaling up to 50 agents (scaling factor ≈1.0)"
echo "  - No resource contention issues"
echo "  - Throughput increases with agent count"
echo "  - CPU usage remains reasonable (<80%)"
echo "  - Memory usage scales linearly"
echo ""

# Check if scaling behavior is acceptable
SCALING_ACCEPTABLE=true
if ls .beads/metrics/performance/concurrent_*.json 1> /dev/null 2>&1; then
    echo -e "${CYAN}Scaling Analysis:${NC}"
    for file in .beads/metrics/performance/concurrent_*.json; do
        if [ -f "$file" ]; then
            local scaling=$(jq -r '.scaling_factor' "$file" 2>/dev/null || echo "0")

            # Acceptable scaling: factor between 0.8 and 1.5
            if (( $(echo "$scaling > 1.5" | bc -l) )); then
                echo -e "  ${YELLOW}⚠ Scaling factor $scaling indicates degradation${NC}"
                SCALING_ACCEPTABLE=false
            elif (( $(echo "$scaling < 0.8" | bc -l) )); then
                echo -e "  ${YELLOW}⚠ Scaling factor $scaling indicates measurement error${NC}"
                SCALING_ACCEPTABLE=false
            else
                echo -e "  ${GREEN}✓ Scaling factor $scaling (acceptable)${NC}"
            fi
        fi
    done
    echo ""
fi

if [ "$SCALING_ACCEPTABLE" = true ]; then
    echo -e "${GREEN}✓ All scaling criteria met!${NC}"
else
    echo -e "${YELLOW}⚠ Some scaling criteria not met. Review results above.${NC}"
fi

echo ""
echo "Next steps:"
echo "  1. Review metrics files in .beads/metrics/performance/"
echo "  2. Compare scaling behavior across agent counts"
echo "  3. Identify bottlenecks (CPU, memory, locks, etc.)"
echo "  4. Document findings in project documentation"
echo "  5. View criterion report: cargo critcmp or cargo critcheck"
echo ""

echo "Criterion HTML report: orchestrator/target/criterion/concurrent_agents/report/index.html"
echo ""
