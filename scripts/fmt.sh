#!/bin/bash
# LuminaGuard Code Formatter
# Formats all code (Rust + Python)

set -e

echo "🎨 Formatting LuminaGuard code..."
echo ""

# Format Rust code
echo "[Rust] Formatting with rustfmt..."
cd orchestrator
cargo fmt --all
echo "  ✅ Rust code formatted"
cd ..

# Format Python code
echo "[Python] Formatting with black..."
cd agent
.venv/bin/black loop.py tests/
echo "  ✅ Python code formatted"
cd ..

echo ""
echo "✅ All code formatted!"
echo ""
echo "Tip: Run 'make lint' to check for additional issues"
