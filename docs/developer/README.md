# Developer Documentation

Welcome to the LuminaGuard developer documentation. This guide provides comprehensive information for developers working on the LuminaGuard project.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Documentation Index](#documentation-index)
3. [Quick Links](#quick-links)
4. [Development Resources](#development-resources)

## Getting Started

### New Developers

If you're new to LuminaGuard, start here:

1. **[Setup Guide](setup.md)** - Set up your development environment
2. **[Architecture Overview](architecture.md)** - Understand the system design
3. **[Testing Guide](testing.md)** - Learn about testing requirements
4. **[Contribution Guidelines](contributing.md)** - Follow coding standards

### Prerequisites

Before you begin, ensure you have:

| Dependency | Minimum Version | Check Command |
|------------|----------------|---------------|
| **Rust** | 1.70+ | `rustc --version` |
| **Python** | 3.10+ | `python3 --version` |
| **Node.js** | 18+ | `node --version` |
| **Git** | 2.30+ | `git --version` |

### Quick Start

```bash
# Clone the repository
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard

# Install all dependencies
make install

# Run tests to verify setup
make test
```

## Documentation Index

### Core Documentation

| Document | Description | Audience |
|----------|-------------|----------|
| [Setup Guide](setup.md) | Development environment setup | All developers |
| [Architecture Overview](architecture.md) | System design and components | All developers |
| [Testing Guide](testing.md) | Testing strategy and requirements | All developers |
| [Contribution Guidelines](contributing.md) | Coding standards and PR process | Contributors |

### Related Documentation

| Document | Path | Description |
|----------|------|-------------|
| Main README | `../../README.md` | Project overview and quick start |
| CLAUDE.md | `../../CLAUDE.md` | Developer instructions and commands |
| Architecture Docs | `../architecture/architecture.md` | Detailed architecture documentation |
| Testing Strategy | `../testing/testing.md` | Testing strategy and coverage |
| Installation Guide | `../../INSTALL.md` | Installation instructions for users |
| Quickstart Guide | `../../QUICKSTART.md` | 5-minute quick start |

## Quick Links

### Common Tasks

- [Run Tests](testing.md#running-tests) - How to run tests
- [Code Style](contributing.md#coding-standards) - Style guidelines
- [Git Workflow](contributing.md#git-workflow) - Branching and PR process
- [Coverage Requirements](testing.md#coverage-targets) - Minimum coverage targets

### Code Components

- [Rust Orchestrator](architecture.md#rust-orchestrator) - Rust binary for system operations
- [Python Agent Loop](architecture.md#python-agent-loop) - Python reasoning loop
- [MCP Integration](architecture.md#mcp-integration) - Model Context Protocol client
- [VM Module](architecture.md#jit-micro-vms-phase-2) - Micro-VM management

### Development Tools

- [Pre-commit Hooks](testing.md#quality-gates) - Automated quality checks
- [CI/CD](testing.md#continuous-integration) - GitHub Actions workflows
- [Coverage Reports](testing.md#coverage) - Generating coverage reports

## Development Resources

### Official Documentation

- [Rust Book](https://doc.rust-lang.org/book/) - Learn Rust
- [Python Documentation](https://docs.python.org/3/) - Learn Python
- [MCP Protocol](https://modelcontextprotocol.io/) - Model Context Protocol
- [Firecracker](https://github.com/firecracker-microvm/firecracker) - Micro-VM runtime

### Testing Tools

- [pytest](https://docs.pytest.org/) - Python testing framework
- [Hypothesis](https://hypothesis.readthedocs.io/) - Property-based testing for Python
- [Proptest](https://altsysrq.github.io/proptest-book/) - Property-based testing for Rust

### Code Quality Tools

- [rustfmt](https://github.com/rust-lang/rustfmt) - Rust code formatter
- [clippy](https://github.com/rust-lang/rust-clippy) - Rust linter
- [black](https://github.com/psf/black) - Python code formatter
- [pylint](https://pylint.org/) - Python linter

### Community

- [GitHub Discussions](https://github.com/anchapin/luminaguard/discussions) - General questions
- [GitHub Issues](https://github.com/anchapin/luminaguard/issues) - Bug reports and feature requests
- [Pull Requests](https://github.com/anchapin/luminaguard/pulls) - Code review

## Project Structure

```
luminaguard/
├── docs/
│   └── developer/          # Developer documentation
│       ├── README.md       # This file
│       ├── setup.md        # Setup guide
│       ├── architecture.md # Architecture overview
│       ├── testing.md      # Testing guide
│       └── contributing.md # Contribution guidelines
├── orchestrator/          # Rust orchestrator
│   └── src/
│       ├── main.rs        # Entry point
│       ├── vm/            # VM modules
│       ├── mcp/           # MCP client
│       └── approval/      # Approval UI
├── agent/                # Python agent
│   ├── loop.py           # Main reasoning loop
│   ├── mcp_client.py     # MCP client wrapper
│   ├── tests/            # Test suite
│   └── .venv/            # Python virtual env
└── scripts/              # Development tools
```

## Key Concepts

### Rust Wrapper, Python Brain

LuminaGuard uses a split architecture:

- **Rust Orchestrator** - Lightweight binary handling system-level operations
- **Python Agent Loop** - Reasoning and decision-making logic

### JIT Micro-VMs

Just-in-Time Micro-VMs provide secure, ephemeral execution:

- Spawn: <200ms
- Execute: Tools run inside VM
- Dispose: VM destroyed after task

### Native MCP Support

LuminaGuard is a native Model Context Protocol client:

- Connects to any standard MCP Server
- No proprietary plugin systems
- Supports stdio and HTTP transports

### Approval Cliff

High-stakes actions require explicit human approval:

- **Green Actions:** Reading files, searching (autonomous)
- **Red Actions:** Editing code, deleting files (requires approval)

## Getting Help

### Questions?

- Check the [FAQ](#frequently-asked-questions) below
- Search [GitHub Discussions](https://github.com/anchapin/luminaguard/discussions)
- Open a new discussion if needed

### Issues?

- Search existing [GitHub Issues](https://github.com/anchapin/luminaguard/issues)
- Create a new issue with detailed information

### Code Review?

- Follow the [PR Process](contributing.md#pull-request-process)
- Use review comment prefixes for clarity

## Frequently Asked Questions

### How do I set up my development environment?

See the [Setup Guide](setup.md) for complete instructions.

### How do I run tests?

```bash
# Run all tests
make test

# Run specific test suites
make test-rust    # Rust tests only
make test-python  # Python tests only
```

See the [Testing Guide](testing.md) for more details.

### What are the coding standards?

See the [Coding Standards](contributing.md#coding-standards) section in the contribution guidelines.

### How do I submit a pull request?

Follow the [Git Workflow](contributing.md#git-workflow) and [Pull Request Process](contributing.md#pull-request-process).

### What is the test coverage requirement?

Minimum 75% coverage for both Rust and Python code. See [Coverage Targets](testing.md#coverage-targets).

### How do I run the project?

```bash
# Run the orchestrator
cargo run --release

# Run the agent
cd agent
python loop.py
```

See the [Quickstart Guide](../../QUICKSTART.md) for more examples.

## Contributing

We welcome contributions! Please read the [Contribution Guidelines](contributing.md) before submitting pull requests.

### Ways to Contribute

- Report bugs
- Suggest new features
- Submit pull requests
- Improve documentation
- Help answer questions in discussions

### Contribution Guidelines

1. Follow the [Git Workflow](contributing.md#git-workflow)
2. Write tests for your changes
3. Update documentation
4. Ensure all tests pass
5. Submit a pull request with a clear description

## License

By contributing to LuminaGuard, you agree that your contributions will be licensed under the same license as the project (see [LICENSE](../../LICENSE)).

---

**Last Updated:** 2026-02-19
