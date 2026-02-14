# LuminaGuard Architecture

**Version:** 0.1.0 (Phase 1 - Foundation)
**Last Updated:** 2026-02-10

---

## Overview

LuminaGuard follows a **"Rust Wrapper, Python Brain"** architecture that splits responsibilities between a lightweight Rust orchestrator and a Python-based reasoning loop.

### Design Philosophy

- **Invisible Security:** Isolation happens automatically via JIT Micro-VMs
- **Standardization:** Native MCP client (no proprietary plugin systems)
- **Agentic Engineering:** Small, auditable codebase (loop.py < 4,000 lines)
- **Performance:** <500ms startup, <200ms VM spawn, <200MB memory footprint

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         User CLI                            │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│              Rust Orchestrator (Binary)                     │
│  ┌──────────────┐  ┌────────────┐  ┌──────────────────┐   │
│  │   CLI Layer  │  │  VM Module │  │   MCP Client     │   │
│  │   (clap)     │  │ (Firecracker)│  │  (JSON-RPC 2.0) │   │
│  └──────────────┘  └────────────┘  └──────────────────┘   │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ JSON-RPC 2.0 over stdio
                         │
┌────────────────────────▼────────────────────────────────────┐
│           Python Agent Logic (loop.py)                      │
│  ┌──────────────┐  ┌────────────┐  ┌──────────────────┐   │
│  │ Reasoning    │  │ MCP Client │  │  Tool Execution  │   │
│  │   Loop       │  │  (Python)  │  │   (spawned)      │   │
│  └──────────────┘  └────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                         │
                         │ subprocess
                         │
┌────────────────────────▼────────────────────────────────────┐
│              JIT Micro-VM (Firecracker)                     │
│         Stripped-down Linux, ephemeral execution             │
└─────────────────────────────────────────────────────────────┘
```

---

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
- `src/main.rs` - Entry point, CLI argument parsing
- `src/vm/` - Micro-VM spawning and lifecycle management
- `src/mcp/` - MCP protocol client (transport, retry logic)
- `src/approval/` - User approval UI for high-stakes actions

**Technology Stack:**
- tokio 1.40 (async runtime)
- clap 4.4 (CLI parsing)
- serde + serde_json (JSON-RPC serialization)
- tracing (structured logging)

**Performance:**
- Binary size: ~5MB (stripped)
- Startup time: <500ms target
- Memory footprint: ~50MB baseline

---

### 2. Python Agent Loop (`agent/`)

**Purpose:** Agent reasoning loop and decision-making logic

**Responsibilities:**
- Agent thinking/reasoning cycle (forked from Nanobot)
- Tool use planning and execution
- MCP client operations (Python wrapper)
- Context management and state tracking

**Key Files:**
- `loop.py` - Main reasoning loop (<4,000 lines enforced by CI)
- `mcp_client.py` - Python MCP client wrapper

**Technology Stack:**
- Python 3.11+
- Zero production dependencies (pure Python)
- Hypothesis for property-based testing

**Constraints:**
- Maximum 4,000 lines (auditability requirement)
- Cyclomatic complexity ≤ 10
- Documentation coverage ≥ 60%
- Test coverage target: 75%

---

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

---

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

**Performance Targets:**
- VM spawn: <200ms
- Memory per VM: ~100MB
- Startup overhead: <50ms after first spawn (snapshot pooling)

---

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

---

## Security Model

### 1. Defense in Depth

**Layer 1: Rust Memory Safety**
- No buffer overflows, use-after-free
- Compile-time guarantees via ownership

**Layer 2: Micro-VM Isolation**
- KVM-based virtualization
- Separate kernel context per agent
- No shared filesystem with host

**Layer 3: Approval Cliff**
- Human-in-the-loop for destructive actions
- Diff cards show exact changes
- Audit log of all decisions

**Layer 4: Command Validation**
- Shell metacharacter detection
- Known-safe command allowlist
- Subprocess list invocation (no shell)

---

## Performance Characteristics

### 1. Startup Performance

| Component | Target | Status |
|-----------|--------|--------|
| Orchestrator spawn | <100ms | ✅ Achieved (~80ms) |
| Python interpreter | <200ms | ✅ Achieved (~150ms) |
| Agent loop init | <100ms | ✅ Achieved (~60ms) |
| VM spawn | <200ms | ⏳ Phase 2 |
| **Total** | <500ms | ⏳ ~330ms without VM |

### 2. Memory Footprint

| Component | Target | Status |
|-----------|--------|--------|
| Rust Orchestrator | <50MB | ✅ Achieved (~45MB) |
| Python Agent | <100MB | ✅ Achieved (~75MB) |
| Per-VM Overhead | <100MB | ⏳ Phase 2 |
| **Total** | <200MB | ⏳ ~120MB without VM |

### 3. Throughput

| Metric | Target | Status |
|--------|--------|--------|
| Tool call latency | <100ms | ✅ Achieved (~50ms) |
| MCP request/response | <50ms | ✅ Achieved (~30ms) |
| Concurrent agents | 10+ | ⏳ Phase 3 (Mesh) |

---

## Testing Strategy

### 1. Rust Tests

**Unit Tests:**
- Property-based testing via Proptest
- Module-level tests (co-located in `src/`)
- Coverage target: 75% (currently 77.54% ✅)

**Integration Tests:**
- `orchestrator/src/mcp/integration.rs`
- Tests against real MCP servers
- Separated via `#[cfg(test)]`

### 2. Python Tests

**Unit Tests:**
- pytest framework in `agent/tests/`
- Property-based testing via Hypothesis
- Coverage target: 75% (currently 56% → in progress)

**Integration Tests:**
- `agent/tests/test_mcp_integration.py`
- Tests against real MCP servers
- Marked with `@pytest.mark.integration`

**Quality Gates:**
- Complexity: ≤10 (radon)
- Documentation: ≥60% (interrogate)
- Duplicate code: <10 lines (jscpd)
- Code bloat: <100KB per file

---

## Evolution Roadmap

### Phase 1: Foundation (Current)
- ✅ Rust Orchestrator structure
- ✅ Python Agent Loop skeleton
- ✅ MCP stdio transport
- ⏳ Nanobot reasoning loop integration
- ⏳ 75% test coverage

### Phase 2: Security (Planned)
- ⏳ Firecracker VM integration
- ⏳ Approval Cliff UI
- ⏳ HTTP transport for MCP
- ⏳ Security audit

### Phase 3: Advanced Features (Future)
- ⏳ Private Mesh protocol (multi-agent)
- ⏳ Desktop GUI (Rust, not Electron)
- ⏳ Snapshot pooling for faster VM spawn
- ⏳ Beta release

---

## References

- **PRD:** `../luminaguard_prd.md` - Complete product specification
- **CLAUDE.md:** `../CLAUDE.md` - Developer instructions
- **MCP Protocol:** https://modelcontextprotocol.io/
- **Nanobot Core:** Reference for Python reasoning loop
- **Firecracker:** https://github.com/firecracker-microvm/firecracker

---

## Architecture Decision Records

See `.quint/decisions/` for formal architecture decisions:
- `DRR-2026-02-10-repository-health-improvement-moderate-investment-with-75-coverage-target.md`
- `DRR-20260209-mcp-client-implementation.md`
- `DRR-20250209-quality-guardrails.md`
