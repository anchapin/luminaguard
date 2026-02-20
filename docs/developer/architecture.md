# Architecture Overview

This document provides a comprehensive overview of LuminaGuard's system architecture, components, and data flow.

## Table of Contents

- [Design Philosophy](#design-philosophy)
- [High-Level Architecture](#high-level-architecture)
- [Component Details](#component-details)
- [Data Flow](#data-flow)
- [Security Model](#security-model)
- [Performance Characteristics](#performance-characteristics)
- [Technology Stack](#technology-stack)

## Design Philosophy

LuminaGuard follows a **"Rust Wrapper, Python Brain"** architecture that splits responsibilities between:

1. **Rust Orchestrator** - Lightweight binary for system-level operations
2. **Python Agent Loop** - Reasoning and decision-making logic

### Core Principles

- **Invisible Security:** Isolation happens automatically via JIT Micro-VMs
- **Standardization:** Native MCP client, no proprietary plugin systems
- **Agentic Engineering:** Small, auditable codebase (loop.py < 4,000 lines)
- **Performance:** <500ms startup, <200ms VM spawn, <200MB memory footprint

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         User CLI                            │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Rust Orchestrator (Binary)                     │
│  ┌──────────────┐  ┌────────────┐  ┌──────────────────┐   │
│  │   CLI Layer  │  │  VM Module │  │   MCP Client     │   │
│  │   (clap)     │  │ (Firecracker)│  │  (JSON-RPC 2.0) │   │
│  └──────────────┘  └────────────┘  └──────────────────┘   │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ JSON-RPC 2.0 over stdio
                         ▼
┌─────────────────────────────────────────────────────────────┐
│           Python Agent Logic (loop.py)                      │
│  ┌──────────────┐  ┌────────────┐  ┌──────────────────┐   │
│  │ Reasoning    │  │ MCP Client │  │  Tool Execution  │   │
│  │   Loop       │  │  (Python)  │  │   (spawned)      │   │
│  └──────────────┘  └────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                         │
                         │ subprocess
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              JIT Micro-VM (Firecracker)                     │
│         Stripped-down Linux, ephemeral execution             │
└─────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. Rust Orchestrator (`orchestrator/`)

**Purpose:** Lightweight binary handling system-level operations

**Responsibilities:**
- CLI interface via `clap` (derive API)
- JIT Micro-VM spawning (<200ms target via Firecracker)
- MCP client connections (stdio transport, JSON-RPC 2.0)
- Approval cliff logic (Green/Red action filtering)
- Memory management and resource isolation

**Key Modules:**

| Module | Path | Responsibility |
|--------|------|----------------|
| `main.rs` | `src/main.rs` | Entry point, CLI argument parsing |
| `vm/` | `src/vm/` | Micro-VM spawning and lifecycle |
| `mcp/` | `src/mcp/` | MCP protocol client |
| `approval/` | `src/approval/` | User approval UI |

**VM Module Structure:**

```
src/vm/
├── mod.rs              # Public API
├── firecracker.rs      # Firecracker process lifecycle
├── jailer/             # Enhanced security sandboxing
│   └── mod.rs
├── pool.rs             # Snapshot pooling for fast spawn
├── snapshot.rs         # VM snapshot creation/loading
├── seccomp.rs          # Syscall filtering
├── firewall.rs         # Network isolation (iptables)
├── vsock.rs            # Virtio-vsock communication
├── config.rs           # VM configuration
└── rootfs/             # Root filesystem management
    └── mod.rs
```

**MCP Module Structure:**

```
src/mcp/
├── mod.rs              # Public API
├── client.rs           # MCP client implementation
├── protocol.rs         # JSON-RPC 2.0 protocol
├── transport.rs       # stdio transport
├── http_transport.rs   # HTTP transport (Phase 2)
├── retry.rs            # Exponential backoff logic
└── integration.rs      # Integration tests
```

**Technology Stack:**
- **tokio** 1.40 - Async runtime
- **clap** 4.4 - CLI parsing
- **serde** + **serde_json** - JSON-RPC serialization
- **tracing** - Structured logging
- **hyper** + **reqwest** - HTTP client (Phase 2)

**Performance:**
- Binary size: ~5MB (stripped)
- Startup time: <500ms target
- Memory footprint: ~50MB baseline

### 2. Python Agent Loop (`agent/`)

**Purpose:** Agent reasoning loop and decision-making logic

**Responsibilities:**
- Agent thinking/reasoning cycle (forked from Nanobot)
- Tool use planning and execution
- MCP client operations (Python wrapper)
- Context management and state tracking

**Key Files:**

| File | Purpose |
|------|---------|
| `loop.py` | Main reasoning loop (<4,000 lines enforced by CI) |
| `mcp_client.py` | Python MCP client wrapper |
| `bot_factory.py` | Bot creation and configuration |
| `create_bot.py` | CLI for creating bots |

**Technology Stack:**
- Python 3.11+
- Zero production dependencies (pure Python)
- Hypothesis for property-based testing

**Constraints:**
- Maximum 4,000 lines (auditability requirement)
- Cyclomatic complexity ≤ 10
- Documentation coverage ≥ 60%
- Test coverage target: 75%

### 3. MCP Integration (Model Context Protocol)

**Architecture:** Native MCP client, not a plugin system

**Transport Layers:**
- **Phase 1:** stdio transport (implemented)
- **Phase 2:** HTTP transport (planned)

**Protocol:**
- JSON-RPC 2.0 over stdio
- Request/response pattern with batch support
- Exponential backoff retry logic
- Connection lifecycle (initialize → list_tools → call_tool → shutdown)

**Security:**
- Command validation (shell injection prevention)
- Sandboxed tool execution in Micro-VMs
- Approval cliff for destructive actions

**Available MCP Servers:**
- `@modelcontextprotocol/server-filesystem` - File system operations
- `@modelcontextprotocol/server-github` - GitHub API integration
- `@modelcontextprotocol/server-slack` - Slack messaging
- `@modelcontextprotocol/server-postgres` - PostgreSQL database queries
- And more: https://github.com/modelcontextprotocol/servers

### 4. JIT Micro-VMs (Phase 2)

**Technology:** Firecracker-like Micro-VMs

**Lifecycle:**
1. **Spawn:** <200ms from request to execution
2. **Execute:** Browser/tools run inside VM
3. **Dispose:** VM destroyed after task completion

**Security Model:**
- Kernel-based isolation (KVM)
- Minimal attack surface (stripped Linux)
- No persistence (ephemeral by design)
- Malware cannot persist ("infected computer no longer exists")

**Snapshot Pooling:**
- Pre-created snapshots for fast spawn (10-50ms target)
- Pool size configurable (default: 5 snapshots)
- Automatic refresh based on configured interval

**Performance Targets:**
- VM spawn: <200ms
- Memory per VM: ~100MB
- Startup overhead: <50ms after first spawn (snapshot pooling)

## Data Flow

### 1. Agent Request Flow

```
User Input
    ↓
Rust Orchestrator (parse CLI)
    ↓
Python Agent Loop (reasoning)
    ↓
MCP Client (connect to server)
    ↓
Tool Execution (in Micro-VM)
    ↓
Result → Python → Rust → User
```

### 2. Approval Cliff Flow

```
Agent requests action
    ↓
Rust Orchestrator classifies (Green/Red)
    ↓
Green → Auto-execute
Red → Pause, show Diff Card
    ↓
User approves → Execute
User rejects → Cancel, log reason
```

### 3. VM Spawn Flow (Phase 2)

```
Agent requests tool execution
    ↓
Check snapshot pool
    ↓
Snapshot available? → Load from pool (10-50ms)
No snapshot? → Create new VM (~200ms)
    ↓
Execute tool in VM
    ↓
Return result to agent
    ↓
Destroy VM (cleanup)
```

## Security Model

LuminaGuard implements **Defense in Depth** with multiple security layers:

### Layer 1: Rust Memory Safety
- No buffer overflows, use-after-free
- Compile-time guarantees via ownership
- Type-safe operations

### Layer 2: Micro-VM Isolation
- KVM-based virtualization
- Separate kernel context per agent
- No shared filesystem with host

### Layer 3: Jailer Sandbox
- chroot for filesystem isolation
- cgroups for resource limits
- Namespaces for process isolation
- UID/GID separation (non-root execution)

### Layer 4: Seccomp Filters
- Syscall whitelisting (Basic/Advanced/Strict levels)
- 99% of syscalls blocked at Basic level
- Fine-grained syscall control

### Layer 5: Firewall Rules
- Network isolation via iptables
- Whitelist-based network access
- Default-deny policy

### Layer 6: Approval Cliff
- Human-in-the-loop for destructive actions
- Diff cards show exact changes
- Audit log of all decisions

### Layer 7: Command Validation
- Shell metacharacter detection
- Known-safe command allowlist
- Subprocess list invocation (no shell)

## Performance Characteristics

### Startup Performance

| Component | Target | Status |
|-----------|--------|--------|
| Orchestrator spawn | <100ms | ✅ Achieved (~80ms) |
| Python interpreter | <200ms | ✅ Achieved (~150ms) |
| Agent loop init | <100ms | ✅ Achieved (~60ms) |
| VM spawn | <200ms | ⏳ Phase 2 |
| **Total** | <500ms | ⏳ ~330ms without VM |

### Memory Footprint

| Component | Target | Status |
|-----------|--------|--------|
| Rust Orchestrator | <50MB | ✅ Achieved (~45MB) |
| Python Agent | <100MB | ✅ Achieved (~75MB) |
| Per-VM Overhead | <100MB | ⏳ Phase 2 |
| **Total** | <200MB | ⏳ ~120MB without VM |

### Throughput

| Metric | Target | Status |
|--------|--------|--------|
| Tool call latency | <100ms | ✅ Achieved (~50ms) |
| MCP request/response | <50ms | ✅ Achieved (~30ms) |
| Concurrent agents | 10+ | ⏳ Phase 3 (Mesh) |

## Technology Stack

### Rust Orchestrator

| Category | Technology | Version |
|----------|------------|---------|
| Language | Rust | 1.70+ |
| Async Runtime | tokio | 1.40+ |
| CLI Parsing | clap | 4.4+ |
| Serialization | serde | 1.0+ |
| Serialization | serde_json | 1.0+ |
| Logging | tracing | 0.1+ |
| HTTP Client | hyper | 0.14+ |
| HTTP Client | reqwest | 0.11+ |
| Testing | proptest | 1.0+ |

### Python Agent

| Category | Technology | Version |
|----------|------------|---------|
| Language | Python | 3.10+ |
| Testing | pytest | 7.0+ |
| Property Testing | hypothesis | 6.0+ |
| Formatting | black | 22.0+ |
| Type Checking | mypy | 0.990+ |
| Linting | pylint | 2.15+ |
| Coverage | pytest-cov | 3.0+ |

### Infrastructure

| Category | Technology | Purpose |
|----------|------------|---------|
| Micro-VM | Firecracker | JIT VM isolation |
| Sandbox | Jailer | Enhanced security |
| Protocol | MCP | Tool integration |
| CI/CD | GitHub Actions | Quality gates |

## Directory Structure

```
luminaguard/
├── orchestrator/           # Rust orchestrator
│   ├── src/
│   │   ├── main.rs        # Entry point
│   │   ├── vm/            # VM modules
│   │   ├── mcp/           # MCP client
│   │   └── approval/      # Approval UI
│   └── Cargo.toml
├── agent/                 # Python agent
│   ├── loop.py           # Main reasoning loop
│   ├── mcp_client.py     # MCP client wrapper
│   ├── bot_factory.py    # Bot creation
│   ├── tests/            # Test suite
│   └── .venv/            # Python virtual env
├── docs/                 # Documentation
│   ├── developer/        # Developer docs
│   ├── architecture/     # Architecture docs
│   ├── testing/          # Testing docs
│   └── security/         # Security docs
├── scripts/              # Development tools
├── .github/              # CI/CD workflows
└── .quint/              # FPF decision records
```

## Related Documentation

- [Setup Guide](setup.md) - Development environment setup
- [Testing Guide](testing.md) - Testing strategy and requirements
- [Contribution Guidelines](contributing.md) - Coding standards and PR process
- [Architecture Details](../../architecture/architecture.md) - Detailed architecture documentation
- [Security Features](../../security/) - Security implementation details
- [Snapshot Pool Guide](../../snapshot-pool-guide.md) - VM snapshot management

## References

- **PRD:** `../../luminaguard_prd.md` - Complete product specification
- **CLAUDE.md:** `../../CLAUDE.md` - Developer instructions
- **MCP Protocol:** https://modelcontextprotocol.io/
- **Nanobot Core:** Reference for Python reasoning loop
- **Firecracker:** https://github.com/firecracker-microvm/firecracker
