# LuminaGuard

A local-first Agentic AI runtime designed to provide secure agent execution through Just-in-Time (JIT) Micro-VMs.

## What is LuminaGuard?

LuminaGuard replaces the insecure "vibe coding" paradigm with rigorous "Agentic Engineering" - combining the usability of OpenClaw with the security of Nanoclaw.

### Key Features

- **Just-in-Time Micro-VMs**: Agents run in ephemeral Micro-VMs that are spawned on-demand and destroyed after task completion
- **Native MCP Support**: Connects to any standard MCP Server (GitHub, Slack, Postgres, etc.)
- **The Approval Cliff**: High-stakes actions require explicit human approval before execution
- **Defense in Depth**: Multiple security layers including Rust memory safety, KVM virtualization, seccomp filters, and firewall isolation

## Quick Start

### Prerequisites

- Rust 1.70+ 
- Python 3.10+
- Firecracker (for VM features)
- Linux with KVM support

### Installation

#### Standard Installation

```bash
# Clone the repository
git clone https://github.com/anchapin/LuminaGuard.git
cd LuminaGuard

# Install all dependencies
make install

# Run tests to verify setup
make test
```

#### Hackable Installation (Developers)

For developers who want hot-reload development, extensible architecture, and full source code control:

```bash
git clone https://github.com/anchapin/luminaguard.git
cd luminaguard
./scripts/install-dev-mode.sh
```

See [DEV_MODE_GUIDE.md](DEV_MODE_GUIDE.md) for full developer setup with hot-reload and debugging tools.

### Basic Usage

#### Python Agent with MCP

```python
from mcp_client import McpClient

# Connect to a filesystem MCP server
with McpClient("filesystem", ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]) as client:
    tools = client.list_tools()
    result = client.call_tool("read_file", {"path": "test.txt"})
    print(f"Content: {result}")
```

#### Rust Orchestrator

```rust
use luminaguard_orchestrator::mcp::McpClient;

let mut client = McpClient::connect_stdio(
    "filesystem",
    &["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
).await?;

client.initialize().await?;
let tools = client.list_tools().await?;
```

#### VM Spawning (Security)

```rust
use luminaguard_orchestrator::vm;

let handle = vm::spawn_vm("my-task").await?;
println!("VM {} spawned in {:.2}ms", handle.id, handle.spawn_time_ms);

vm::destroy_vm(handle).await?;
```

## Architecture

LuminaGuard uses a "Rust Wrapper, Python Brain" design:

- **Orchestrator (Rust)**: Handles micro-VM spawning, MCP connections, and security
- **Agent (Python)**: The reasoning loop for agent decision-making

See [CLAUDE.md](CLAUDE.md) for detailed developer documentation.

## Security: The Approval Cliff

LuminaGuard implements a strict approval system:

| Action Type | Description | Approval Required |
|-------------|-------------|------------------|
| Green | Reading files, searching, checking logs | No |
| Red | Editing code, deleting files, sending emails | **Yes** |

Before any Red action executes, users see a "Diff Card" showing exactly what will change.

## Development

```bash
make test              # Run all tests
make test-rust        # Run Rust tests only
make test-python      # Run Python tests only
make fmt              # Format code
make lint             # Run linters
```

## Test Coverage

[![Coverage](https://img.shields.io/badge/coverage-76%25-green)](#)

| Component | Coverage | Target |
|-----------|----------|--------|
| Rust (Orchestrator) | 74.2% | 75.0% |
| Python (Agent) | 78.0% | 75.0% |

## License

See [LICENSE](LICENSE) file.

## Resources

- [Developer Documentation](CLAUDE.md)
- [Architecture Docs](docs/architecture/architecture.md)
- [Testing Strategy](docs/testing/testing.md)
- [Snapshot Pool Guide](docs/snapshot-pool-guide.md)
- [Security Validation](docs/validation/security-validation-plan.md)
- [MCP Protocol](https://modelcontextprotocol.io)
