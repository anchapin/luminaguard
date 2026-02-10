# IronClaw Agent - Examples

This directory contains example scripts demonstrating IronClaw's agent capabilities.

## Examples

### MCP Client Integration

**mcp_filesystem_demo.py** - Demonstrates MCP filesystem operations

Shows how to use IronClaw's MCP client to:
- Connect to MCP servers (e.g., @modelcontextprotocol/server-filesystem)
- List available tools
- Read/write files through MCP
- Manage file operations

**Usage**:
```bash
# Run the demo
python agent/examples/mcp_filesystem_demo.py

# Run with context manager mode
python agent/examples/mcp_filesystem_demo.py --mode context-manager
```

**Requirements**:
- Node.js and npm (for @modelcontextprotocol/server-filesystem)
- IronClaw Rust Orchestrator compiled (`cargo build --release`)
- Python 3.11+

## How It Works

```
Python Agent Loop (agent/loop.py)
        ↓
    Python MCP Client (agent/mcp_client.py)
        ↓ (JSON-RPC 2.0 over stdin/stdout)
    Rust Orchestrator (orchestrator/src/mcp/)
        ↓ (stdio transport)
    MCP Server (e.g., @modelcontextprotocol/server-filesystem)
        ↓ (native protocol)
    Filesystem / Network / External Service
```

## MCP Servers

### Official MCP Servers

- **@modelcontextprotocol/server-filesystem** - File system operations
- **@modelcontextprotocol/server-github** - GitHub integration
- **@modelcontextprotocol/server-slack** - Slack integration
- **@modelcontextprotocol/server-postgres** - PostgreSQL database

### Installation

```bash
# Install Node.js tools (for filesystem server)
npm install -g @modelcontextprotocol/server-filesystem

# Install GitHub server
npm install -g @modelcontextprotocol/server-github

# Install Slack server
npm install -g @modelcontextprotocol/server-slack
```

## Architecture

### Python Layer (agent/)

- **loop.py** - Main agent reasoning loop
- **mcp_client.py** - MCP client for Rust Orchestrator communication

### Rust Layer (orchestrator/src/mcp/)

- **client.rs** - MCP client implementation
- **transport.rs** - stdio transport
- **http_transport.rs** - HTTP transport
- **protocol.rs** - JSON-RPC 2.0 protocol types

## Example: Custom MCP Integration

```python
from agent.mcp_client import McpClient
from agent.loop import run_loop

# Create MCP client
client = McpClient(
    "filesystem",
    ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
    root_dir="/tmp"
)

try:
    # Connect to MCP server
    client.spawn()
    client.initialize()

    # Run agent loop
    state = run_loop(
        "Read /tmp/secret.txt and tell me the content",
        tools=["read_file", "write_file"],
        mcp_client=client
    )

    print(f"Final state: {len(state.messages)} messages")

finally:
    client.shutdown()
```

## Development

### Running Tests

```bash
# Test MCP client module
cd agent
python mcp_client.py

# Run agent loop (basic)
python loop.py "Hello, IronClaw!"

# Run filesystem demo
python examples/mcp_filesystem_demo.py
```

### Adding New Examples

To add a new example:

1. Create file in `agent/examples/`
2. Import from `agent.mcp_client` or `agent.loop`
3. Follow the existing example structure
4. Add usage instructions to this README

## See Also

- **MCP Protocol**: https://modelcontextprotocol.io/
- **IronClaw Documentation**: See `CLAUDE.md` and `README.md`
- **Rust MCP Implementation**: See `orchestrator/src/mcp/` modules
