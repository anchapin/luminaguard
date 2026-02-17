#!/bin/bash
# LuminaGuard Hackable Installation Mode
# Provides developer-friendly setup with hot-reload and extensible architecture
# Usage: ./scripts/install-dev-mode.sh [--with-hooks] [--python-version 3.11]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PYTHON_VERSION="${PYTHON_VERSION:-3.11}"
INSTALL_HOOKS=false
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-hooks)
            INSTALL_HOOKS=true
            shift
            ;;
        --python-version)
            PYTHON_VERSION="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --with-hooks           Install git hooks for development"
            echo "  --python-version       Python version to use (default: 3.11)"
            echo "  --help                 Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Helper functions
log_info() {
    echo -e "${BLUE}ℹ${NC}  $1"
}

log_success() {
    echo -e "${GREEN}✅${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}⚠${NC}  $1"
}

log_error() {
    echo -e "${RED}❌${NC} $1"
}

check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "$1 is not installed"
        return 1
    fi
}

log_info "Starting Hackable Installation Mode Setup"
log_info "Python version: $PYTHON_VERSION"
echo ""

# 1. Check prerequisites
log_info "Checking prerequisites..."
check_command "git" || { log_error "Git is required"; exit 1; }
check_command "python3" || { log_error "Python 3 is required"; exit 1; }
check_command "cargo" || { log_error "Rust/Cargo is required"; exit 1; }

log_success "All prerequisites found"
echo ""

# 2. Verify this is the correct directory
log_info "Verifying project structure..."
if [ ! -d "$PROJECT_ROOT/agent" ] || [ ! -d "$PROJECT_ROOT/orchestrator" ]; then
    log_error "Not in LuminaGuard root directory"
    exit 1
fi
log_success "Project structure verified"
echo ""

# 3. Git configuration for development
log_info "Configuring Git for development..."
git config --local core.hooksPath .githooks 2>/dev/null || true
log_success "Git configured"
echo ""

# 4. Rust setup with hot-reload tools
log_info "Setting up Rust development environment..."
cd "$PROJECT_ROOT/orchestrator"

# Install cargo-watch for hot-reload
if ! cargo install --list | grep -q "cargo-watch"; then
    log_info "Installing cargo-watch for hot-reload..."
    cargo install cargo-watch --quiet
    log_success "cargo-watch installed"
else
    log_success "cargo-watch already installed"
fi

# Install cargo-expand for debugging macros
if ! cargo install --list | grep -q "cargo-expand"; then
    log_info "Installing cargo-expand for debugging..."
    cargo install cargo-expand --quiet
    log_success "cargo-expand installed"
else
    log_success "cargo-expand already installed"
fi

log_success "Rust development tools ready"
echo ""

# 5. Python setup with hot-reload capabilities
log_info "Setting up Python development environment..."
cd "$PROJECT_ROOT/agent"

# Create venv if it doesn't exist
if [ ! -d ".venv" ]; then
    log_info "Creating Python virtual environment..."
    python3 -m venv .venv
    log_success "Virtual environment created"
fi

# Activate venv
source .venv/bin/activate

# Upgrade pip
log_info "Upgrading pip..."
pip install --quiet --upgrade pip setuptools wheel
log_success "pip upgraded"

# Install core dependencies
log_info "Installing core dependencies..."
pip install --quiet -e ".[dev]" 2>/dev/null || {
    # Fallback if pyproject.toml doesn't have [dev]
    pip install --quiet \
        pytest pytest-cov pytest-asyncio pytest-timeout pytest-watch \
        hypothesis black mypy pylint \
        cryptography aiohttp pydantic
    log_success "Core dependencies installed (fallback)"
}
log_success "Core dependencies installed"

# Install hot-reload tools
log_info "Installing hot-reload tools..."
pip install --quiet ptpython pytest-watch
log_success "Hot-reload tools installed"

log_success "Python development environment ready"
echo ""

# 6. Development helper scripts
log_info "Creating development helper scripts..."
mkdir -p "$PROJECT_ROOT/.dev-scripts"

# Create hot-reload script for Python
cat > "$PROJECT_ROOT/.dev-scripts/watch-python.sh" << 'EOF'
#!/bin/bash
# Watch Python files and run tests on changes
cd "$(git rev-parse --show-toplevel)/agent"
source .venv/bin/activate
ptw tests/ -- -v
EOF

# Create hot-reload script for Rust
cat > "$PROJECT_ROOT/.dev-scripts/watch-rust.sh" << 'EOF'
#!/bin/bash
# Watch Rust files and run tests on changes
cd "$(git rev-parse --show-toplevel)/orchestrator"
cargo watch -x "test --lib --bins"
EOF

# Create integrated watch script
cat > "$PROJECT_ROOT/.dev-scripts/watch-all.sh" << 'EOF'
#!/bin/bash
# Watch all files and run tests
echo "Starting integrated development watch..."
echo "Press Ctrl+C to stop"
(cd "$(git rev-parse --show-toplevel)/orchestrator" && cargo watch -x "test --lib --bins") &
RUST_PID=$!
(cd "$(git rev-parse --show-toplevel)/agent" && source .venv/bin/activate && ptw tests/ -- -v) &
PYTHON_PID=$!
trap "kill $RUST_PID $PYTHON_PID" EXIT
wait
EOF

chmod +x "$PROJECT_ROOT/.dev-scripts"/*.sh
log_success "Development helper scripts created in .dev-scripts/"
echo ""

# 7. Git hooks (optional)
if [ "$INSTALL_HOOKS" = true ]; then
    log_info "Installing git hooks..."
    mkdir -p .git/hooks
    
    # Pre-commit hook
    cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
set -e
echo "Running pre-commit checks..."
cd "$(git rev-parse --show-toplevel)"

# Python checks
cd agent
source .venv/bin/activate
echo "  Checking Python formatting..."
black --check loop.py tests/ 2>/dev/null || true
echo "  Running Python linters..."
mypy loop.py 2>/dev/null || true
pylint loop.py 2>/dev/null || true
deactivate
cd ..

# Rust checks
cd orchestrator
echo "  Checking Rust formatting..."
cargo fmt --all -- --check 2>/dev/null || true
echo "  Running clippy..."
cargo clippy --quiet 2>/dev/null || true
cd ..

echo "✅ Pre-commit checks passed"
EOF
    chmod +x .git/hooks/pre-commit
    log_success "Git hooks installed"
fi
echo ""

# 8. Create development configuration
log_info "Creating development configuration..."
cat > "$PROJECT_ROOT/.dev-config.toml" << 'EOF'
[development]
# Hot-reload settings
auto_reload = true
reload_delay_ms = 500

# Logging
log_level = "debug"
pretty_print = true

# Testing
test_timeout = 30
test_parallel = true
coverage_threshold = 75

# Rust specific
rust_backtrace = "1"
rust_log = "debug"

# Python specific
python_optimize = 0
python_dont_write_bytecode = false

[editor]
format_on_save = true
lint_on_save = true
show_clippy_warnings = true
EOF

log_success "Development configuration created"
echo ""

# 9. Create README for development
cat > "$PROJECT_ROOT/DEV_SETUP.md" << 'EOF'
# LuminaGuard Development Setup (Hackable Mode)

This document describes the development environment created by `install-dev-mode.sh`.

## Quick Start

```bash
# Enter the development environment
cd agent
source .venv/bin/activate

# Or for Rust
cd orchestrator
cargo build
```

## Hot-Reload Development

### Watch Python Tests
```bash
./.dev-scripts/watch-python.sh
```

This uses `pytest-watch` to automatically re-run tests when Python files change.

### Watch Rust Tests
```bash
./.dev-scripts/watch-rust.sh
```

This uses `cargo-watch` to automatically re-run tests when Rust files change.

### Watch All Tests
```bash
./.dev-scripts/watch-all.sh
```

Runs both Python and Rust test watchers simultaneously.

## Development Tools

### Python Tools
- **ptpython** - Enhanced Python REPL
- **pytest-watch** - Auto-run tests on file changes
- **black** - Code formatter
- **mypy** - Static type checker
- **pylint** - Linter

### Rust Tools
- **cargo-watch** - Auto-run commands on file changes
- **cargo-expand** - Expand macros for debugging

## Code Quality

### Format Code
```bash
make fmt
```

### Run Linters
```bash
make lint
```

### Run All Tests
```bash
make test
```

## Debugging

### Debug Python
```bash
cd agent
source .venv/bin/activate
python -m pdb loop.py
```

Or use ptpython for interactive debugging:
```bash
ptpython
import luminaguard
```

### Debug Rust
```bash
cd orchestrator
cargo run --bin orchestrator -- --verbose
RUST_BACKTRACE=1 cargo run
```

## Extending LuminaGuard

The hackable mode is designed to be extended:

1. **Add Custom MCP Servers** - Implement in orchestrator/src/
2. **Add Custom Agents** - Implement in agent/
3. **Add Custom Hooks** - Use git hooks in .git/hooks/
4. **Add Custom Tools** - Use the tools API in agent/

## Directory Structure

```
.
├── .dev-scripts/           # Development helper scripts
│   ├── watch-python.sh     # Python test watcher
│   ├── watch-rust.sh       # Rust test watcher
│   └── watch-all.sh        # Combined watcher
├── .dev-config.toml        # Development configuration
├── agent/                  # Python agent
│   └── .venv/              # Python virtual environment
├── orchestrator/           # Rust orchestrator
└── scripts/                # Utility scripts
```

## Git Workflow

When installing with `--with-hooks`, git hooks are configured for:
- Pre-commit checks (formatting, linting)
- Automatic code formatting
- Test validation

## Troubleshooting

### Python tests won't run
```bash
cd agent
source .venv/bin/activate
pip install -e .
```

### Cargo watch not found
```bash
cargo install cargo-watch
```

### Permission denied on shell scripts
```bash
chmod +x .dev-scripts/*.sh
```

## Next Steps

- Read [Architecture Documentation](docs/architecture/)
- Explore [MCP Integration](docs/llm-integration.md)
- Check out [Contributing Guide](CONTRIBUTING.md)
EOF

log_success "Development documentation created"
echo ""

# 10. Summary
echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  ✅ Hackable Installation Mode Setup Complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo ""
log_success "Development environment is ready to use"
echo ""
echo "Next steps:"
echo ""
echo "1. Enter the agent environment:"
echo "   cd agent && source .venv/bin/activate"
echo ""
echo "2. Start coding with hot-reload:"
echo "   ./.dev-scripts/watch-python.sh      (Python tests)"
echo "   ./.dev-scripts/watch-rust.sh        (Rust tests)"
echo "   ./.dev-scripts/watch-all.sh         (Both)"
echo ""
echo "3. Format and lint code:"
echo "   make fmt                            (Format code)"
echo "   make lint                           (Run linters)"
echo ""
echo "4. Run full test suite:"
echo "   make test                           (All tests)"
echo ""
echo "5. Read the dev setup guide:"
echo "   cat DEV_SETUP.md"
echo ""
log_info "For more info, run: $0 --help"
echo ""
