# Developer Setup Guide

This guide covers setting up a complete LuminaGuard development environment.

## Prerequisites

Before you begin, ensure you have the following installed on your system:

| Dependency | Minimum Version | Purpose |
|------------|----------------|---------|
| **Rust** | 1.70+ | Building the orchestrator |
| **Python** | 3.10+ | Running the agent |
| **Node.js** | 18+ | Running MCP servers |
| **Git** | 2.30+ | Version control |
| **Firecracker** | 1.3+ | JIT Micro-VM isolation (optional for development) |
| **KVM** | - | Hardware virtualization (Linux, optional) |

### Checking Prerequisites

```bash
# Rust
rustc --version
cargo --version

# Python
python3 --version

# Node.js
node --version
npm --version

# Git
git --version

# Firecracker (optional)
firecracker --version

# KVM (Linux only, optional)
ls /dev/kvm
```

## Installation Methods

LuminaGuard supports two installation modes:

### 1. Standard Installation (Recommended)

For most developers who want a stable, working environment:

```bash
# Clone the repository
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard

# Install all dependencies
make install

# Run tests to verify setup
make test
```

This will:
- Install Rust dependencies via Cargo
- Create Python virtual environment at `agent/.venv/`
- Install Python testing tools (pytest, hypothesis, black, mypy)
- Set up pre-commit hooks
- Install development dependencies

### 2. Hackable Installation (For Advanced Development)

For developers who want hot-reload, full source control, and custom modifications:

```bash
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard
./scripts/install-dev-mode.sh
```

See [DEV_MODE_GUIDE.md](../../DEV_MODE_GUIDE.md) for full details on hackable mode setup.

## Manual Setup Steps

If you prefer to set up each component manually:

### Step 1: Clone the Repository

```bash
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard
```

### Step 2: Install Rust Dependencies

```bash
# Install Rust toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version

# Install additional Rust tools
cargo install cargo-tarpaulin  # For coverage reporting
```

### Step 3: Set Up Python Environment

```bash
cd agent

# Create virtual environment
python3 -m venv .venv

# Activate virtual environment
source .venv/bin/activate  # On Linux/macOS
# .venv\Scripts\activate   # On Windows

# Upgrade pip
pip install --upgrade pip

# Install development dependencies
pip install pytest hypothesis black mypy pylint radon interrogate

# Return to project root
cd ..
```

### Step 4: Install Pre-commit Hooks

```bash
# Install pre-commit framework
pip install pre-commit

# Install hooks from configuration
pre-commit install

# Run hooks on all files (optional)
pre-commit run --all-files
```

### Step 5: Install Node.js (for MCP Servers)

```bash
# Install Node.js 18+ via nvm
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
source ~/.bashrc  # or restart your shell
nvm install 18
nvm use 18

# Verify installation
node --version
npm --version
```

### Step 6: Install Firecracker (Optional)

For VM-based testing and development:

```bash
# Download and install Firecracker
curl -L https://github.com/firecracker-microvm/firecracker/releases/download/v1.3.0/firecracker-v1.3.0-x86_64.tgz | tar -xz
sudo mv firecracker-v1.3.0-x86_64/release/firecracker /usr/local/bin/
sudo chmod +x /usr/local/bin/firecracker

# Verify installation
firecracker --version
```

### Step 7: Download VM Resources (Optional)

For running real Micro-VM tests:

```bash
# Download VM resources to default location
./scripts/download-firecracker-resources.sh

# Or to a custom location
./scripts/download-firecracker-resources.sh /custom/path/to/resources
```

This downloads:
- **vmlinux.bin** - Linux kernel image
- **rootfs.ext4** - Ubuntu root filesystem

## Verification

After installation, verify everything is working:

```bash
# Run all tests
make test

# Run specific test suites
make test-rust      # Rust orchestrator tests only
make test-python    # Python agent tests only

# Build the project
cargo build --release

# Run the orchestrator
cargo run --release
```

## Development Environment

### IDE/Editor Setup

#### VS Code

Recommended extensions:
- **rust-analyzer** - Rust language support
- **Python** - Python language support
- **Pylance** - Python type checking
- **CodeLLDB** - Debugging support
- **Better TOML** - TOML file highlighting

#### Neovim/Vim

Install using your preferred plugin manager:

```lua
-- Using lazy.nvim
{
    'simrat39/rust-tools.nvim',
    'neovim/nvim-lspconfig',
    'jose-elias-alvarez/null-ls.nvim',
    'mfussenegger/nvim-dap',
}
```

#### JetBrains IDEs

- **CLion** or **IntelliJ IDEA** with Rust and Python plugins

### Environment Configuration

Copy the example environment file and customize:

```bash
cp .env.example .env
```

Key variables for development:

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_API_KEY` | _(unset)_ | OpenAI API key for testing |
| `ANTHROPIC_API_KEY` | _(unset)_ | Anthropic API key for testing |
| `OLLAMA_HOST` | _(unset)_ | Ollama server URL |
| `RUST_LOG` | `info` | Rust log verbosity (debug, info, warn, error) |
| `RUST_BACKTRACE` | `1` | Enable backtrace on panic |
| `LUMINAGUARD_LOG_LEVEL` | `INFO` | Python log level |
| `LUMINAGUARD_MODE` | `host` | Execution mode: `host` or `vm` |

## Common Development Tasks

### Running the Agent

```bash
# Interactive mode
cargo run --release

# Single task
cargo run --release -- run "Read the README.md file"

# Test MCP connection
cargo run --release -- test-mcp --command npx --args "-y" "@modelcontextprotocol/server-filesystem" "."
```

### Spawning VMs

```bash
# Spawn a test VM
cargo run --release -- spawn-vm

# Spawn with custom configuration
cargo run --release -- spawn-vm --vcpus 2 --memory 512
```

### Testing

```bash
# Run all tests
make test

# Run specific Rust tests
cd orchestrator && cargo test --lib vm::
cd orchestrator && cargo test --lib mcp::

# Run specific Python tests
cd agent && .venv/bin/python -m pytest tests/test_loop.py -v

# Run tests with coverage
cd agent && .venv/bin/python -m pytest tests/ --cov=. --cov-report=html
cd orchestrator && cargo tarpaulin --out Html

# Run integration tests
cd agent && .venv/bin/python -m pytest tests/ -m integration
```

### Code Quality

```bash
# Format all code
make fmt

# Run linters
make lint

# Fix auto-fixable issues
cargo clippy --fix
cd agent && black .
```

## Troubleshooting

### "cargo: command not found"

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### "Python venv not found"

```bash
cd agent
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

### Pre-commit hooks failing

```bash
# Update pre-commit
pre-commit autoupdate

# Run hooks manually to see issues
pre-commit run --all-files

# Skip hooks temporarily (not recommended)
git commit --no-verify
```

### Firecracker not found

```bash
# Install Firecracker
curl -L https://github.com/firecracker-microvm/firecracker/releases/download/v1.3.0/firecracker-v1.3.0-x86_64.tgz | tar -xz
sudo mv firecracker-v1.3.0-x86_64/release/firecracker /usr/local/bin/
sudo chmod +x /usr/local/bin/firecracker
```

### KVM not available

```bash
# Enable virtualization in BIOS, then:
sudo modprobe kvm
sudo modprobe kvm_intel  # or kvm_amd
```

### Tests failing on CI but passing locally

Check environment differences:
```bash
# Check versions
rustc --version
python3 --version
node --version

# Run tests with same settings as CI
cargo test --workspace --locked
python -m pytest tests/ -v --tb=short
```

## Next Steps

- Read the [Architecture Overview](architecture.md) to understand the system design
- Review the [Testing Guide](testing.md) for testing best practices
- Check the [Contribution Guidelines](contributing.md) for coding standards
- Explore the [API Documentation](../../api-guide.md) for detailed API reference

## Additional Resources

- [Rust Book](https://doc.rust-lang.org/book/) - Learn Rust
- [Python Documentation](https://docs.python.org/3/) - Learn Python
- [MCP Protocol](https://modelcontextprotocol.io/) - Model Context Protocol
- [Firecracker Documentation](https://github.com/firecracker-microvm/firecracker) - Micro-VM runtime
