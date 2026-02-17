# LuminaGuard Hackable Installation Mode Guide

This guide covers the **Hackable Installation Mode** for developers who want full control over LuminaGuard's source code and custom modifications.

## What is Hackable Installation Mode?

Hackable Installation Mode provides:

- **Direct Git Clone** - Work directly with source code
- **Hot-Reload Development** - Automatic test re-runs on file changes
- **Developer-Friendly Tools** - Enhanced REPL, debugging, and profiling
- **Extensible Architecture** - Add custom MCP servers, agents, and tools
- **Full Control** - Modify any aspect of the system

## Installation

### Prerequisites

Before installing, ensure you have:

- **Git** - For version control
- **Rust 1.70+** - For orchestrator compilation
- **Python 3.11+** - For agent runtime
- **Node.js 18+** (optional) - For MCP servers

### Quick Install

```bash
# Clone the repository
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard

# Run the hackable installation
./scripts/install-dev-mode.sh

# Optional: Install git hooks for development
./scripts/install-dev-mode.sh --with-hooks
```

### Manual Installation

If you prefer full control, install manually:

```bash
# 1. Rust setup
cd orchestrator
cargo build --release

# 2. Python setup
cd ../agent
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
pip install pytest-watch ptpython

# 3. Install hot-reload tools
cargo install cargo-watch
pip install pytest-watch
```

## Development Workflow

### Starting Development

#### Option 1: Using Watch Scripts

```bash
# Watch Python tests
./.dev-scripts/watch-python.sh

# Watch Rust tests
./.dev-scripts/watch-rust.sh

# Watch both simultaneously
./.dev-scripts/watch-all.sh
```

#### Option 2: Manual Commands

```bash
# Terminal 1: Run Rust orchestrator
cd orchestrator
cargo watch -x "run --release"

# Terminal 2: Run Python agent
cd agent
source .venv/bin/activate
ptw tests/ -- -v
```

#### Option 3: Development Mode

```bash
make dev
```

This displays instructions for running the full development environment in three terminals.

### Hot-Reload Development

The development environment uses hot-reload for rapid iteration:

**Python Changes:**
```bash
./.dev-scripts/watch-python.sh
# Tests re-run automatically when you modify .py files
```

**Rust Changes:**
```bash
./.dev-scripts/watch-rust.sh
# Tests re-run automatically when you modify .rs files
```

### Code Quality

```bash
# Format code (Python + Rust)
make fmt

# Run linters
make lint

# Run full test suite
make test

# Run specific test suite
make test-python
make test-rust
```

## Extensibility

### Adding Custom MCP Servers

Create a new MCP server in `orchestrator/src/mcp/`:

```rust
use crate::mcp::MCP;

pub struct CustomServer {
    // Your implementation
}

impl CustomServer {
    pub fn new() -> Self {
        Self {}
    }
}
```

Register in `orchestrator/src/main.rs`:

```rust
let custom_server = CustomServer::new();
mcp.register("custom", custom_server);
```

### Adding Custom Tools

Create tools in `agent/tools/`:

```python
from agent.core import Tool

class CustomTool(Tool):
    name = "custom_tool"
    description = "My custom tool"
    
    async def execute(self, **kwargs):
        # Implementation
        return result
```

### Adding Custom Agents

Extend the agent in `agent/agents/`:

```python
from agent.core import Agent

class CustomAgent(Agent):
    def __init__(self):
        super().__init__()
        # Your implementation
    
    async def run(self, task: str):
        # Your logic
        pass
```

## Development Configuration

The `.dev-config.toml` file controls development behavior:

```toml
[development]
auto_reload = true
reload_delay_ms = 500
log_level = "debug"
pretty_print = true

[testing]
test_timeout = 30
test_parallel = true
coverage_threshold = 75

[rust]
backtrace = "1"
log = "debug"

[python]
optimize = 0
dont_write_bytecode = false

[editor]
format_on_save = true
lint_on_save = true
```

## Development Tools

### Python Development

**Enhanced REPL:**
```bash
cd agent && source .venv/bin/activate
ptpython
>>> import luminaguard
>>> luminaguard.
```

**Interactive Testing:**
```bash
pytest --pdb tests/test_file.py
```

**Coverage Analysis:**
```bash
pytest --cov=agent tests/
```

### Rust Development

**Macro Expansion:**
```bash
cd orchestrator
cargo expand
```

**Flamegraph Profiling:**
```bash
cargo install flamegraph
cargo flamegraph --bin orchestrator
```

**LLDB Debugging:**
```bash
rust-lldb ./target/debug/orchestrator
```

## Git Workflow

### Creating Feature Branches

```bash
git checkout -b feat/your-feature
```

### With Git Hooks

If you installed with `--with-hooks`, pre-commit checks will run automatically:

```bash
git commit -m "feat: your feature"
# Automatically formats, lints, and validates
```

### Manual Checks

```bash
git commit -m "feat: your feature"
```

## Directory Structure

```
luminaguard/
├── .dev-scripts/
│   ├── watch-python.sh       # Python test watcher
│   ├── watch-rust.sh         # Rust test watcher
│   └── watch-all.sh          # Combined watcher
├── .dev-config.toml          # Development configuration
├── DEV_SETUP.md              # Generated dev setup guide
├── agent/                    # Python agent
│   ├── .venv/                # Python virtual environment
│   ├── loop.py               # Main agent loop
│   ├── tools/                # Custom tools
│   ├── agents/               # Custom agents
│   └── tests/                # Test suite
├── orchestrator/             # Rust orchestrator
│   ├── src/
│   │   ├── main.rs
│   │   ├── mcp/              # MCP implementations
│   │   └── vm/               # VM management
│   └── tests/                # Test suite
├── scripts/
│   ├── install-dev-mode.sh   # Installation script
│   └── setup-branch-protection.sh
├── docs/                     # Documentation
└── Makefile                  # Development commands
```

## Troubleshooting

### Python Tests Won't Run

```bash
cd agent
source .venv/bin/activate
pip install -e ".[dev]"
pytest tests/
```

### Cargo Watch Not Found

```bash
cargo install cargo-watch
```

### Git Hooks Not Running

```bash
git config --local core.hooksPath .githooks
chmod +x .git/hooks/pre-commit
```

### Import Errors in Python

```bash
cd agent
pip install -e .
# or
pip install --force-reinstall -e .
```

### Module Not Found in Orchestrator

```bash
cd orchestrator
cargo clean
cargo build
```

## Performance Tips

### Faster Rust Compilation

```bash
# Use incremental compilation
export CARGO_INCREMENTAL=1

# Build in release mode for faster runtime
cargo build --release
```

### Faster Python Tests

```bash
# Run tests in parallel
pytest -n auto tests/

# Only run failed tests
pytest --lf tests/

# Stop on first failure
pytest -x tests/
```

## Advanced Usage

### Custom Watch Commands

Edit `.dev-scripts/watch-all.sh` to add your custom commands:

```bash
# Watch and run custom command
cargo watch -x "custom-command"
```

### Remote Development

For development on a remote machine:

```bash
# SSH with port forwarding
ssh -L 8000:localhost:8000 user@remote.host

# Then run watch scripts
./.dev-scripts/watch-python.sh
```

### Docker Development

Create a `Dockerfile.dev`:

```dockerfile
FROM rust:latest
WORKDIR /app
RUN apt-get update && apt-get install -y python3.11
COPY . .
RUN ./scripts/install-dev-mode.sh
CMD ["./.dev-scripts/watch-all.sh"]
```

Build and run:

```bash
docker build -f Dockerfile.dev -t luminaguard-dev .
docker run -it luminaguard-dev
```

## Next Steps

1. **Read the Architecture** - See `docs/architecture/`
2. **Explore MCP Integration** - See `docs/llm-integration.md`
3. **Check Contributing Guide** - See `CONTRIBUTING.md`
4. **Start Coding** - Use the watch scripts and enjoy hot-reload development

## Getting Help

- **Issues** - https://github.com/anchapin/luminaguard/issues
- **Discussions** - https://github.com/anchapin/luminaguard/discussions
- **Documentation** - https://github.com/anchapin/luminaguard/tree/main/docs

## Contributing

The hackable mode is designed for contributions. Please see `CONTRIBUTING.md` for guidelines.
