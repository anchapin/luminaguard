# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**IronClaw** is a local-first Agentic AI runtime designed to provide secure agent execution through Just-in-Time (JIT) Micro-VMs. The project is in early planning/prototype phase.

**Core Vision:** Replace the insecure "vibe coding" paradigm with rigorous "Agentic Engineering" - combining OpenClaw's usability with Nanoclaw's security.

## Architecture Principles

### The "Rust Wrapper, Python Brain" Design

The codebase follows a split architecture:

1. **Orchestrator (Rust)** (`orchestrator/`): Lightweight binary handling:
   - Entry point: `src/main.rs`
   - CLI interface via `clap`
   - Memory management
   - Micro-VM spawning (target: <200ms startup) - module: `src/vm/`
   - MCP client connections - module: `src/mcp/`
   - Approval cliff logic - module: `src/approval/`

2. **Agent Logic (Python)** (`agent/`): The reasoning loop:
   - Core: `loop.py` - The main agent reasoning loop (forked from Nanobot)
   - Kept under 4,000 lines for auditability (enforced by CI)
   - Handles agent decision-making and tool use
   - Tests: `tests/` directory with pytest + Hypothesis

### Key Directories

- `orchestrator/src/` - Rust source code
  - `main.rs` - Entry point, CLI interface
  - `vm/` - Micro-VM modules (firecracker, jailer, pool, snapshot, seccomp, firewall, vsock, config, rootfs)
  - `mcp/` - MCP protocol client (transport, protocol, client, retry, http_transport)
  - `approval/` - User approval UI for high-stakes actions
- `agent/` - Python agent code (loop.py, tests/, .venv/)
- `docs/` - Project documentation (architecture/, testing/, snapshot-pool-guide.md)
- `scripts/` - Development tooling (coverage-ratchet check, git-workflow, branch-protection)
- `.github/workflows/` - CI/CD pipelines (quality gates, coverage ratchet, Jules AI integration)
- `.quint/` - FPF (Formal Proof Framework) reasoning context and decisions

### Just-in-Time (JIT) Micro-VMs

Instead of persistent containers or host execution, agents run in ephemeral Micro-VMs:

- Spawn: Stripped-down Linux VM in <200ms
- Execute: Browser/tools run inside the VM
- Dispose: VM is destroyed after task completion
- Security: Malware cannot persist (the "infected" computer no longer exists)

### Native MCP Support

IronClaw is a native Model Context Protocol (MCP) client:
- Connects to any standard MCP Server (Google Drive, Slack, GitHub, Postgres, etc.)
- No proprietary "AgentSkills" or custom plugin systems
- Leverages the growing enterprise MCP ecosystem
- **Transport Layers**: stdio (implemented), HTTP (Phase 2)
- **Protocol**: JSON-RPC 2.0 with exponential backoff retry logic

## Key Security Feature: The "Approval Cliff"

High-stakes actions require explicit human approval:

**Green Actions (Autonomous):**
- Reading files
- Searching the web
- Checking logs
- Read-only operations

**Red Actions (Require Approval):**
- Editing code
- Deleting files
- Sending emails
- Transferring crypto/assets
- Any destructive or external communication

The UI presents a "Diff Card" showing exactly what will change before execution.

## Development Commands

### Initial Setup
```bash
make install           # Install all dependencies (Rust, Python venv, pre-commit hooks)
```

### Testing
```bash
make test              # Run all tests (Rust + Python) + check invariants
make test-rust         # Run Rust tests only (cargo test)
make test-python       # Run Python tests only (pytest)

# Run specific Rust tests
cd orchestrator && cargo test --lib vm::      # Test VM module only
cd orchestrator && cargo test --lib mcp::     # Test MCP module only

# Run specific Python tests
cd agent && .venv/bin/python -m pytest tests/test_specific.py -v

# Property-based tests
cd orchestrator && cargo test -- --nocapture  # Rust with output
cd agent && .venv/bin/python -m pytest tests/ -v -k "test_"  # Python with filter
```

**Python testing**: Tests are in `agent/tests/`, configured with Hypothesis for property-based testing.

**Rust testing**: Tests are co-located in `orchestrator/src/` with `#[cfg(test)]` modules. Uses Proptest for property-based testing.

**VM Module Testing**: The VM module (`orchestrator/src/vm/`) includes integration tests that require Firecracker to be installed. These tests are automatically skipped if Firecracker is not available.

### Current Test Coverage

**Overall Coverage:** 76% (exceeds 75% target) ✅

| Component | Coverage | Target | Status |
|-----------|----------|--------|--------|
| Rust (Orchestrator) | 74.2% | 75.0% | ⚠️ Near target |
| Python (Agent) | 78.0% | 75.0% | ✅ Exceeds |
| `loop.py` | 73% | 75.0% | ⚠️ Near target |
| `mcp_client.py` | 80% | 75.0% | ✅ Exceeds |

**Coverage Ratchet:** See `.coverage-baseline.json` for current requirements. CI fails if coverage decreases.

**Test Strategy:**
- Unit tests with mocking (fast, deterministic)
- Property-based testing (Hypothesis for Python, Proptest for Rust)
- Integration tests with real MCP servers (marked with `@pytest.mark.integration`)

### MCP Client Usage

The Python agent can connect to MCP servers using the `McpClient` class:

```python
from mcp_client import McpClient

# Basic usage with context manager (recommended)
with McpClient("filesystem", ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]) as client:
    # Client is automatically spawned and initialized
    tools = client.list_tools()
    print(f"Available tools: {[t.name for t in tools]}")

    # Call a tool
    result = client.call_tool("read_file", {"path": "test.txt"})
    print(f"Content: {result}")

# Client is automatically shut down when exiting context

# Manual lifecycle management
client = McpClient("github", ["npx", "-y", "@modelcontextprotocol/server-github"])
client.spawn()
client.initialize()

tools = client.list_tools()
result = client.call_tool("create_issue", {
    "owner": "repo-owner",
    "repo": "repo-name",
    "title": "Test issue"
})

client.shutdown()
```

**Available MCP Servers:**
- `@modelcontextprotocol/server-filesystem` - File system operations
- `@modelcontextprotocol/server-github` - GitHub API integration
- `@modelcontextprotocol/server-slack` - Slack messaging
- `@modelcontextprotocol/server-postgres` - PostgreSQL database queries
- And many more: https://github.com/modelcontextprotocol/servers

**Security Note:** All commands are validated for shell injection prevention. Only known-safe commands (npx, python, node, cargo) are allowed by default.

**Rust MCP Client (Orchestrator):**
```rust
use ironclaw_orchestrator::mcp::McpClient;

// Create client (stdio transport)
let mut client = McpClient::connect_stdio(
    "filesystem",
    &["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
).await?;

// Initialize
client.initialize().await?;

// List tools
let tools = client.list_tools().await?;

// Call tool
let result = client.call_tool("read_file", json!({"path": "test.txt"})).await?;

// Shutdown
client.shutdown().await?;
```

### VM Module Architecture

The Rust VM module (`orchestrator/src/vm/`) provides comprehensive Micro-VM management:

**Core Modules:**
- `firecracker.rs` - Firecracker process lifecycle (start/stop via HTTP API)
- `jailer/` - Enhanced security sandboxing (chroot, cgroups, namespaces, privilege drop)
- `pool.rs` - Snapshot pooling for fast VM spawn (10-50ms target)
- `snapshot.rs` - VM snapshot creation/loading (Phase 2: Firecracker API integration)
- `seccomp.rs` - Syscall filtering (99% of syscalls blocked at Basic level)
- `firewall.rs` - Network isolation via iptables rules
- `vsock.rs` - Virtio-vsock communication between host and guest
- `config.rs` - VM configuration (kernel, rootfs, memory, CPU)
- `rootfs/` - Root filesystem management and hardening

**Security Layers (Defense in Depth):**
1. **Rust Memory Safety** - No buffer overflows, use-after-free
2. **Micro-VM Isolation** - KVM-based virtualization, separate kernel context
3. **Jailer Sandbox** - chroot, cgroups, namespaces, UID/GID separation
4. **Seccomp Filters** - Syscall whitelisting (Basic/Advanced/Strict levels)
5. **Firewall Rules** - Network isolation (iptables)
6. **Approval Cliff** - Human-in-the-loop for destructive actions

**VM Spawn API:**
```rust
use ironclaw_orchestrator::vm;

// Basic spawn (auto-enables seccomp Basic filter)
let handle = vm::spawn_vm("my-task").await?;
println!("VM {} spawned in {:.2}ms", handle.id, handle.spawn_time_ms);

// Custom config with seccomp
use ironclaw_orchestrator::vm::{config::VmConfig, seccomp::{SeccompFilter, SeccompLevel}};

let config = VmConfig::new("my-task".to_string());
let config_with_seccomp = VmConfig {
    seccomp_filter: Some(SeccompFilter::new(SeccompLevel::Basic)),
    ..config
};
let handle = vm::spawn_vm_with_config("my-task", &config_with_seccomp).await?;

// Jailed spawn (enhanced security)
use ironclaw_orchestrator::vm::jailer::JailerConfig;

let jailer_config = JailerConfig::new("my-task".to_string())
    .with_user(1000, 1000); // Run as non-root user
let handle = vm::spawn_vm_jailed("my-task", &config, &jailer_config).await?;

// Destroy VM (required for security)
vm::destroy_vm(handle).await?;
```

**Snapshot Pool API:**
```rust
use ironclaw_orchestrator::vm;

// Warm up pool on startup (pre-creates 5 snapshots)
vm::warmup_pool().await?;

// Get pool statistics
let stats = vm::pool_stats().await?;
println!("Pool size: {}/{}", stats.current_size, stats.max_size);

// Spawn VM automatically uses pool (10-50ms target)
let handle = vm::spawn_vm("task").await?; // Uses pool if available
```

**Environment Variables (VM Configuration):**
| Variable | Default | Description |
|----------|---------|-------------|
| `IRONCLAW_POOL_SIZE` | `5` | Number of snapshots to maintain (1-20) |
| `IRONCLAW_SNAPSHOT_REFRESH_SECS` | `3600` | Refresh interval in seconds (min: 60) |
| `IRONCLAW_SNAPSHOT_PATH` | `/var/lib/ironclaw/snapshots` | Snapshot storage location |

**VM Module Status:**
- ✅ Basic Firecracker integration (~110ms spawn time)
- ✅ Jailer sandboxing (chroot, cgroups, namespaces)
- ✅ Seccomp filters (Basic level auto-enabled)
- ✅ Firewall isolation (iptables rules)
- ✅ Snapshot pooling (prototype, Phase 2: Firecracker API integration)
- ⏳ HTTP transport for MCP (Phase 2)

**Documentation:**
- `docs/snapshot-pool-guide.md` - Complete snapshot pool documentation
- `docs/architecture/architecture.md` - System architecture details
- `docs/testing/testing.md` - Testing strategy and coverage

### Code Quality
```bash
make fmt               # Format all code (rustfmt + black)
make lint              # Run linters (clippy + mypy + pylint)
make clean             # Remove build artifacts
```

### Development Workflow
```bash
make dev               # Show commands for running orchestrator + agent in separate terminals
# Terminal 1: cd orchestrator && cargo run
# Terminal 2: cd agent && source .venv/bin/activate && python loop.py
```

## Git Workflow (AI-Agent Enforced)

### Philosophy

IronClaw requires disciplined git workflow to ensure code quality and traceability. **AI coding agents (Claude Code, GitHub Copilot, etc.) must follow the same workflow as human developers.**

### Core Principles

1. **Issue Tracking**: All work must link to a GitHub issue
2. **Code Review**: All changes must go through pull requests
3. **Automated Enforcement**: Pre-commit hooks block direct commits to protected branches
4. **Branch Protection**: GitHub rules prevent bypassing review

### Workflow

#### Step 1: Create GitHub Issue

All work starts with an issue for tracking:

```bash
gh issue create \
  --title "Implement MCP client connection" \
  --body "Add ability to connect to MCP servers from orchestrator"
# Returns: Issue #42
```

#### Step 2: Start Feature Branch

Use the workflow helper script (validates issue exists):

```bash
./scripts/git-workflow.sh start 42 "mcp-client-connection"
# Creates: feature/42-mcp-client-connection
# Switches to new branch
```

The script automatically:
- Validates the issue exists
- Shows issue title and state
- Creates properly formatted branch name
- Warns if issue is not open

#### Step 3: Work and Commit

Make changes and commit normally:

```bash
git add .
git commit -m "Add MCP client module"
```

**Pre-commit hook will block commits to `main`, `master`, or `develop` branches** with clear error message.

#### Step 4: Submit Pull Request

When work is ready, create a PR:

```bash
./scripts/git-workflow.sh submit
```

The script automatically:
- Extracts issue number from branch name
- Creates PR with descriptive title
- Links PR to issue (using "Closes #42")
- Opens PR in browser for review

#### Step 5: Monitor Status

Check workflow status anytime:

```bash
./scripts/git-workflow.sh status
```

Shows:
- Current branch
- Linked issue (title, state, labels)
- PR status (if created)

### Branch Protection Rules

**Protected Branches**: `main`, `master`, `develop`

**Rules Enforced**:
- ❌ Direct pushes are BLOCKED
- ✅ Pull requests required (1 approval)
- ✅ Pre-commit checks must pass
- ✅ PRs must link to existing issue

**Setup** (one-time, requires admin access):

```bash
make branch-protection
# Or run directly:
./scripts/setup-branch-protection.sh
```

### Error Messages

#### If you try to commit to main:

```
❌ BLOCKED: Cannot commit directly to main

IronClaw requires all changes to go through pull requests.

Required workflow:
  1. Create GitHub issue:
     gh issue create --title 'Description' --body 'Details'

  2. Create feature branch:
     git checkout -b feature/ISSUE-NUM-description
     Or use the workflow script:
     ./scripts/git-workflow.sh start ISSUE-NUM 'description'

  3. Make changes and commit normally

  4. Create pull request:
     gh pr create --body 'Closes #ISSUE-NUM'
     Or use the workflow script:
     ./scripts/git-workflow.sh submit

Documentation: See CLAUDE.md section 'Git Workflow (AI-Agent Enforced)'
```

#### If issue doesn't exist:

```
❌ Issue #999 does not exist

Create it first:
  gh issue create --title 'Description' --body 'Implementation details...'
```

### AI Agent Integration

When working with AI coding assistants:

1. **The agent will naturally follow the workflow** (blocked otherwise)
2. **Pre-commit hooks guide the agent** to correct process
3. **No manual enforcement needed** - it's automatic

**Example Conversation**:
```
User: "Add MCP client to main branch"

AI: "I'll help you add the MCP client. First, let me create a GitHub issue..."

[Agent creates issue #123]
[Agent runs: ./scripts/git-workflow.sh start 123 'mcp-client']
[Agent makes changes on feature branch]
[Agent runs: ./scripts/git-workflow.sh submit]
```

### Advanced Commands

#### Sync with Main

Update feature branch with latest changes:

```bash
./scripts/git-workflow.sh sync
# Fetches origin/main and rebases current branch
```

#### Manual Git Commands

If you prefer git commands:

```bash
# Start new feature
git checkout main
git pull
git checkout -b feature/42-mcp-client

# Submit PR
gh pr create --title "Work on #42: MCP client" --body "Closes #42"
```

### Troubleshooting

**Problem**: Script not found
```bash
# Make sure you're in repo root
ls scripts/git-workflow.sh

# Make executable
chmod +x scripts/git-workflow.sh
```

**Problem**: gh CLI not authenticated
```bash
gh auth login
```

**Problem**: Branch protection setup fails
- Ensure you have admin access to repository
- Check that `gh` CLI is authenticated
- Configure manually at: `https://github.com/OWNER/REPO/settings/branches`

### Why This Workflow?

1. **Traceability**: Every change links to an issue with context
2. **Review**: All code goes through PR review process
3. **Safety**: Automated enforcement prevents mistakes
4. **AI Compatibility**: Agents work within same rules as humans
5. **Quality**: Pre-commit hooks catch issues before commit

## Quality Guardrails

### Pre-commit Hooks (Automatic)
The `.pre-commit-config.yaml` enforces quality before commits:

**Rust:**
- `cargo-fmt`: Auto-formatting
- `cargo-clippy`: Linting with `-D warnings`

**Python:**
- `black`: Auto-formatting (line-length=88)
- `flake8`: Linting
- `radon`: Complexity analysis (max complexity: 10)
- `interrogate`: Documentation coverage (min: 60%)
- `pycln`: Dead code detection
- `jscpd`: Duplicate code detection (min: 10 lines)

**General:**
- Large file detection (max 100KB)
- Private key detection
- TOML/YAML/JSON validation
- Coverage ratchet checking

### CI Enforced Invariants
GitHub Actions (`.github/workflows/quality-gates.yml`) enforces:
- **loop.py line limit**: Must remain under 4,000 lines (critical auditability requirement)
- Code bloat prevention: Python files <100KB
- Complexity thresholds via radon
- Documentation coverage requirements

### Virtual Environments
- Python: `agent/.venv/` (created by `make install`)
- Dependencies defined in `agent/pyproject.toml`
- Rust: Managed via Cargo in `orchestrator/Cargo.toml`

## Development Guidelines

### Code Philosophy

1. **Agentic Engineering over Vibe Coding**: Every line must be intentional, reviewed, and necessary
2. **Invisible Security**: Isolation happens automatically - users should never need to write Dockerfiles or manage containers manually
3. **Standardization**: Use existing protocols (MCP) rather than building custom systems
4. **Auditability**: Keep the codebase small and deterministic

### Performance Targets

- Startup time: <500ms for new agent sessions
- Memory footprint: Significantly less than OpenClaw (~200MB baseline)
- VM spawn time: <200ms using Firecracker-like technology

### Safety Requirements

- Zero reported RCEs or container escapes
- All file system writes must go through the Approval Cliff
- Micro-VMs must be truly ephemeral
- No persistence of agent state between VM sessions

### TDD Workflow

IronClaw follows strict Test-Driven Development:

1. **Red**: Write failing test first
2. **Green**: Implement minimal code to pass
3. **Refactor**: Improve code while keeping tests green
4. **Verify**: Run `make test` to ensure all tests pass
5. **Ship**: Commit only when all tests and quality gates pass

**Important**: Always run tests before committing. Pre-commit hooks will format code automatically, but tests must be run manually.

## Roadmap Context

### Phase 1 - Foundation (Current/Planned)
- Fork Nanobot for Python reasoning loop
- Build Rust Orchestrator for MCP connections
- Implement basic autonomous ("Green Action") capabilities

### Phase 2 - Security
- Integrate Firecracker for JIT Micro-VM spawning
- Implement Approval Cliff UI for file operations
- Beta release to security-conscious developers

### Phase 3 - Advanced Features
- Private Mesh protocol for multi-agent collaboration
- Desktop GUI (Rust-based, NOT Electron)

## Competitive Context

IronClaw aims to position between:
- **OpenClaw**: Usable but insecure (host execution, CVE-2026-25253)
- **Nanoclaw**: Secure but high friction (manual Docker management)
- **Nanobot**: Minimalist codebase reference for the Python loop

## References

- PRD: `ironclaw_prd.md` - Complete product specification
- Nanobot core: Reference for the Python reasoning loop architecture
- MCP Protocol: Standard for tool/server connections

## Configuration Files

- `Makefile` - Unified development automation (test, fmt, lint, install)
- `agent/pyproject.toml` - Python dependencies and tool config (black, mypy, pytest)
- `orchestrator/Cargo.toml` - Rust dependencies and build config
- `.pre-commit-config.yaml` - Pre-commit hooks (formatting, linting, quality gates)
- `.coverage-baseline.json` - Coverage ratchet baseline (enforced via CI)

## FPF (Formal Proof Framework) Integration

IronClaw uses the Quint FPF system for rigorous architecture decision-making. Available skills:

- `q0-init` - Initialize FPF context (bounded context, vocabulary, invariants)
- `q1-hypothesize` - Generate hypotheses via abduction
- `q2-verify` - Verify logic via deduction (L0 → L1)
- `q3-validate` - Validate via induction (L1 → L2)
- `q4-audit` - Audit evidence (trust calculus)
- `q5-decide` - Finalize decision (DRR)
- `q-status` - Show current FPF phase and context
- `q-actualize` - Reconcile FPF state with repository changes

**Decision Records:** Architecture decisions are stored in `.quint/decisions/` with formal reasoning trace.

**Documentation:** See `docs/jules-github-integration-guide.md` for Jules AI agent integration.
