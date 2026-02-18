# LuminaGuard Quickstart

Get up and running with LuminaGuard in under 5 minutes!

## Prerequisites

Before you begin, ensure you have:

| Requirement | Check Command | Notes |
|-------------|---------------|-------|
| **Rust** | `rustc --version` | 1.70+ required |
| **Python** | `python3 --version` | 3.10+ required |
| **Node.js** | `node --version` | 18+ required |
| **Git** | `git --version` | For cloning |

### Optional (for real VM isolation)
| Requirement | Check Command |
|-------------|---------------|
| **Firecracker** | `firecracker --version` |
| **KVM** | `ls /dev/kvm` |

---

## 1️⃣ Clone & Install (One-Liner)

```bash
# Clone and install all dependencies
git clone https://github.com/anchapin/luminaguard.git && cd luminaguard && make install
```

This installs:
- Rust toolchain
- Python virtual environment with dependencies
- Pre-commit hooks

---

## 2️⃣ Create a 24/7 Bot (No VM Required)

The fastest way to get a LuminaGuard bot running — works immediately after cloning, no Firecracker or KVM needed:

```bash
cd agent

# Check your setup status
python create_bot.py --status

# Send a one-shot message
python create_bot.py --message "Hello"

# Start an interactive REPL
python create_bot.py
```

**Expected output (no LLM configured yet):**
```
Please setup environment variables for your LLM
```

**Enable AI responses** by configuring an LLM provider:

```bash
# Recommended: copy the example env file and fill in your key(s)
cp .env.example .env
# Edit .env and set at least one of:
#   OPENAI_API_KEY, ANTHROPIC_API_KEY, or OLLAMA_HOST
source .env   # or use a tool like direnv / python-dotenv

# Alternatively, export directly in your shell:
export OPENAI_API_KEY=sk-…          # OpenAI / GPT
export ANTHROPIC_API_KEY=sk-ant-…   # Anthropic / Claude
export OLLAMA_HOST=http://localhost:11434  # Local Ollama (free, no key needed)
```

See [`.env.example`](.env.example) for the full list of supported variables.

Then run again — the bot will use your LLM automatically.

**From Python:**
```python
from bot_factory import create_bot

bot = create_bot(bot_name="MyBot", username="alice")
print(bot.chat("Hello"))
```

See [`agent/bot_factory.py`](agent/bot_factory.py) for the full `BotFactory` / `ReadyBot` API.

---

## 3️⃣ Quick Test (No VM)

Test the Rust orchestrator without Firecracker:

```bash
# Run the agent with a simple task
cargo run --release -- run "Hello LuminaGuard"
```

You should see:
```
🚀 Starting task: Hello LuminaGuard
🧠 Thinking...
✅ Task complete!
```

---

## 4️⃣ Test with MCP Tools

```bash
# Start the agent with filesystem access
cargo run --release -- test-mcp --command npx --args "-y" "@modelcontextprotocol/server-filesystem" "."
```

This starts an MCP server and lists available tools.

---

## 5️⃣ Full Workflow (Requires Firecracker + KVM)

If you have Firecracker and KVM installed:

```bash
# Download VM resources (kernel + rootfs)
./scripts/download-firecracker-resources.sh

# Run a full agent task in an isolated VM
cargo run --release -- run "Read the README.md file"
```

---

## 🔧 Common Commands

```bash
# Build the project
cargo build --release

# Run tests
make test

# Format code
make fmt

# Lint code
make lint

# Run the orchestrator only
cargo run --release

# Spawn a test VM
cargo run --release -- spawn-vm
```

---

## 🆘 Troubleshooting

### "command not found: cargo"
```bash
source ~/.cargo/env
# Or reinstall: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### "npx not found"
Install Node.js: https://nodejs.org/

### "Firecracker not found"
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

---

## 📖 Next Steps

- Read the [Architecture Documentation](docs/architecture/architecture.md)
- Explore [MCP Integration](docs/llm-integration.md)
- Learn about [Security Features](docs/security/)
- Review [API Documentation](https://docs.rs/luminaguard_orchestrator/)

---

## 💬 Get Help

- Issues: https://github.com/anchapin/luminaguard/issues
- Discussions: https://github.com/anchapin/luminaguard/discussions
