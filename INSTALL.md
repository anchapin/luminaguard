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

## Installation Modes

LuminaGuard supports multiple installation modes:

### Standard Installation
Recommended for most users who want a stable, working installation.

### **Hackable Installation Mode** (Developers)
For developers who want full control, direct source code access, and custom modifications.

**Quick start:**
```bash
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard
./scripts/install-dev-mode.sh
```

See [DEV_MODE_GUIDE.md](DEV_MODE_GUIDE.md) for full developer setup.

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

### 🤖 Create a 24/7 Bot (Fastest Path — No VM Required)

The quickest way to get started. Works immediately after installation, no Firecracker or KVM needed:

```bash
cd agent

# Check your LLM setup status
python create_bot.py --status

# Send a one-shot message
python create_bot.py --message "Hello"
# Output: Please setup environment variables for your LLM

# Start an interactive REPL
python create_bot.py
```

**Enable AI responses** by configuring an LLM provider:

```bash
# Recommended: copy the example env file and fill in your key(s)
cp .env.example .env
# Edit .env and set at least one of:
#   OPENAI_API_KEY, ANTHROPIC_API_KEY, or OLLAMA_HOST
source .env   # or use direnv / python-dotenv

# Alternatively, export directly in your shell:
export OPENAI_API_KEY=sk-…          # OpenAI / GPT
export ANTHROPIC_API_KEY=sk-ant-…   # Anthropic / Claude
export OLLAMA_HOST=http://localhost:11434  # Local Ollama (free, no API key)
```

See [`.env.example`](.env.example) for the full list of supported variables.

**From Python:**

```python
from bot_factory import create_bot

# Zero-config — auto-detects LLM from environment
bot = create_bot()
print(bot.chat("Hello"))

# Custom bot
bot = create_bot(bot_name="MyBot", username="alice", use_case="monitoring")
bot.run_repl()  # interactive REPL
```

`BotFactory` handles all setup steps automatically:
1. Daemon configuration (sensible defaults)
2. Persona & onboarding profile (persisted to `~/.luminaguard/bot/`)
3. LLM client (auto-detected from env vars, falls back to mock)
4. Message router wired to the LLM

See [`agent/bot_factory.py`](agent/bot_factory.py) for the full API.

---

### Rust Orchestrator Usage

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

Copy [`.env.example`](.env.example) to `.env` and edit it to configure LuminaGuard:

```bash
cp .env.example .env
```

Key variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_API_KEY` | _(unset)_ | OpenAI API key (enables GPT models) |
| `ANTHROPIC_API_KEY` | _(unset)_ | Anthropic API key (enables Claude models) |
| `OLLAMA_HOST` | _(unset)_ | Ollama server URL (e.g. `http://localhost:11434`) |
| `LUMINAGUARD_APPROVAL_TIMEOUT` | `300` | Approval timeout in seconds |
| `LUMINAGUARD_LOG_LEVEL` | `INFO` | Logging level |
| `LUMINAGUARD_MODE` | `host` | Execution mode: `host` or `vm` |
| `RUST_LOG` | `info` | Rust log verbosity |
| `RUST_BACKTRACE` | `1` | Enable backtrace on panic |

See [`.env.example`](.env.example) for the complete list with descriptions.

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
