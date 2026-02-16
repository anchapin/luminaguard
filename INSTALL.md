# LuminaGuard Installation Guide

This guide covers how to install and set up LuminaGuard on your system.

## Prerequisites

Before installing LuminaGuard, ensure you have the following:

### Required Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| **Rust** | 1.70+ | Building the orchestrator |
| **Python** | 3.10+ | Running the agent |
| **Node.js** | 18+ | Running MCP servers |
| **Firecracker** | 1.3+ | JIT Micro-VM isolation |
| **KVM** | - | Hardware virtualization (Linux) |

### Installing Prerequisites

#### Rust

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Python

```bash
# Most systems have Python pre-installed
python3 --version  # Should be 3.10+

# If needed, install via pyenv
curl https://pyenv.run | bash
```

#### Node.js

```bash
# Install Node.js 18+ via nvm
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18
node --version
```

#### Firecracker

```bash
# Download and install Firecracker
curl -L https://github.com/firecracker-microvm/firecracker/releases/download/v1.3.0/firecracker-v1.3.0-x86_64.tgz | tar -xz
sudo mv firecracker-v1.3.0-x86_64/release/firecracker /usr/local/bin/
sudo chmod +x /usr/local/bin/firecracker

# Verify
firecracker --version
```

#### KVM (Linux)

```bash
# Check if KVM is available
ls /dev/kvm

# If not available, enable in BIOS or install kvm
sudo modprobe kvm
sudo modprobe kvm_intel  # or kvm_amd
```

## Installation Steps

### 1. Clone the Repository

```bash
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard
```

### 2. Install Development Dependencies

```bash
# Using make (recommended)
make install

# Or manually
cd agent
python3 -m venv .venv
source .venv/bin/activate
pip install --upgrade pip
pip install pytest hypothesis black mypy pylint
```

### 3. Download Firecracker Resources

For running real VM tests, download the kernel and rootfs:

```bash
# Download VM resources (kernel + rootfs)
./scripts/download-firecracker-resources.sh

# Or to a custom location
./scripts/download-firecracker-resources.sh /custom/path
```

This downloads:
- **vmlinux.bin** - Linux kernel image
- **rootfs.ext4** - Ubuntu root filesystem

### 4. Verify Installation

```bash
# Run tests
make test

# Or run specific test suites
make test-rust    # Rust orchestrator tests
make test-python  # Python agent tests
```

## Quick Start

### Basic Usage

```bash
# Run the agent with a task
cargo run --release -- run "Read the README.md file"

# Spawn a VM (without running agent)
cargo run --release -- spawn-vm

# Test MCP connection
cargo run --release -- test-mcp
```

### Interactive Mode

```bash
# Start interactive session
cargo run --release
```

### Running MCP Servers

```bash
# Using filesystem MCP server
cargo run --release -- test-mcp --command npx --args "-y" "@modelcontextprotocol/server-filesystem" "."

# Using GitHub MCP server
cargo run --release -- test-mcp --command npx --args "-y" "@modelcontextprotocol/server-github" "/path/to/repo"
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LUMINAGUARD_APPROVAL_TIMEOUT` | 300 | Approval timeout in seconds |
| `RUST_LOG` | info | Logging level |
| `RUST_BACKTRACE` | 1 | Enable backtrace on panic |

### VM Configuration

VM settings can be customized in `orchestrator/src/vm/config.rs`:

```rust
let config = VmConfig {
    vcpu_count: 2,      // Number of virtual CPUs
    memory_mb: 512,      // Memory in MB
    ..Default::default()
};
```

## Troubleshooting

### "Firecracker binary not found"

```bash
# Install Firecracker
curl -L https://github.com/firecracker-microvm/firecracker/releases/download/v1.3.0/firecracker-v1.3.0-x86_64.tgz | tar -xz
sudo mv firecracker-v1.3.0-x86_64/release/firecracker /usr/local/bin/
```

### "KVM not available"

```bash
# Enable virtualization in BIOS, then:
sudo modprobe kvm
sudo modprobe kvm_intel  # or kvm_amd
```

### "Python venv not found"

```bash
cd agent
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

### "npx not found"

Install Node.js: https://nodejs.org/

## Next Steps

- Read the [Architecture Documentation](docs/architecture/architecture.md)
- Explore [MCP Integration](docs/llm-integration.md)
- Learn about [Security Features](docs/security/)

## Support

- Issues: https://github.com/anchapin/luminaguard/issues
- Discussions: https://github.com/anchapin/luminaguard/discussions
