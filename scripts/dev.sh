#!/bin/bash
# LuminaGuard Development Environment Setup
# This script sets up the development environment for LuminaGuard

set -e

echo "🦊 LuminaGuard Development Environment Setup"
echo "=========================================="
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] && [ ! -f "orchestrator/Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the LuminaGuard root directory"
    exit 1
fi

# Check Rust installation
echo "🔧 Checking Rust installation..."
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
else
    echo "✅ Rust already installed: $(cargo --version)"
fi

# Check Python installation
echo ""
echo "🐍 Checking Python installation..."
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 not found. Please install Python 3.11+"
    exit 1
else
    PYTHON_VERSION=$(python3 --version)
    echo "✅ Python installed: $PYTHON_VERSION"
fi

# Create Python virtual environment
echo ""
echo "📦 Creating Python virtual environment..."
cd agent
if [ ! -d ".venv" ]; then
    python3 -m venv .venv
    echo "✅ Virtual environment created"
else
    echo "✅ Virtual environment already exists"
fi

# Install Python dependencies
echo ""
echo "📥 Installing Python dependencies..."
.venv/bin/pip install --upgrade pip
.venv/bin/pip install pytest hypothesis black mypy pylint
echo "✅ Python dependencies installed"

cd ..

# Install pre-commit hooks
echo ""
echo "🔗 Installing pre-commit hooks..."
if command -v pre-commit &> /dev/null; then
    pre-commit install
    echo "✅ Pre-commit hooks installed"
else
    echo "⚠️  pre-commit not found. Install with: pip install pre-commit"
fi

# Run initial tests
echo ""
echo "🧪 Running initial tests..."
make test

echo ""
echo "✅ Development environment setup complete!"
echo ""
echo "Next steps:"
echo "  1. Run tests:        make test"
echo "  2. Format code:      make fmt"
echo "  3. Run linters:      make lint"
echo "  4. Start developing: make dev"
echo ""
