#!/bin/bash
# LuminaGuard Test Runner
# Runs all tests (Rust + Python) with verbose output

set -e

echo "🧪 LuminaGuard Test Suite"
echo "======================"
echo ""

# Run Rust tests
echo "[Rust] Running orchestrator tests..."
cd orchestrator
cargo test --verbose
cd ..

echo ""

# Run Python tests
echo "[Python] Running agent tests..."
cd agent
.venv/bin/python -m pytest tests/ -v
cd ..

echo ""

# Check invariants
echo "[Quality] Checking invariants..."
cd agent
LINES=$(wc -l < loop.py)
echo "  loop.py has $LINES lines"
if [ $LINES -le 4000 ]; then
    echo "  ✅ Invariant #9: Under 4,000 lines"
else
    echo "  ❌ Invariant #9: Exceeds 4,000 lines"
    exit 1
fi
cd ..

echo ""
echo "✅ All tests passed!"
