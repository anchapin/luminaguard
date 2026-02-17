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
