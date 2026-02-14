# GEMINI.md - LuminaGuard Project Context

## Project Overview
**LuminaGuard** is a local-first Agentic AI runtime designed for secure agent execution through Just-in-Time (JIT) Micro-VMs. It combines a high-performance Rust orchestrator with a Python-based reasoning loop, emphasizing security, auditability, and minimal code.

### Core Vision
Replace "vibe coding" with "Agentic Engineering" by enforcing rigorous security boundaries (Firecracker Micro-VMs) and human-in-the-loop approvals for high-stakes actions.

---

## Architecture

LuminaGuard follows a **"Rust Wrapper, Python Brain"** design:

1.  **Orchestrator (Rust)** (`/orchestrator`):
    *   **Purpose**: Lightweight binary for CLI, memory management, VM spawning, and MCP client connections.
    *   **Technologies**: Rust 1.75+, Tokio, Clap, Serde, Hyper, Firecracker/Jailer.
    *   **Key Modules**:
        *   `src/vm/`: Micro-VM lifecycle management (Firecracker, Jailer, Seccomp, Firewall).
        *   `src/mcp/`: Native Model Context Protocol (MCP) client.
        *   `src/approval/`: Approval Cliff logic for high-stakes actions.

2.  **Agent Logic (Python)** (`/agent`):
    *   **Purpose**: The reasoning loop forked from Nanobot, kept small for auditability.
    *   **Technologies**: Python 3.11+, Pytest, Hypothesis, MyPy.
    *   **Core File**: `loop.py` (Strictly enforced limit of < 4,000 lines).

---

## Building and Running

### Setup
```bash
make install           # Install Rust & Python dependencies, setup venv, and git hooks
```

### Development
```bash
make dev               # Display instructions for running orchestrator and agent
# Terminal 1: cd orchestrator && cargo run
# Terminal 2: cd agent && source .venv/bin/activate && python loop.py
```

### Testing
```bash
make test              # Run all tests (Rust + Python) and check invariants
make test-rust         # Run Rust tests (cargo test)
make test-python       # Run Python tests (pytest)
```

### Code Quality
```bash
make fmt               # Format code (rustfmt + black)
make lint              # Run linters (clippy + mypy + pylint)
```

---

## Development Conventions

### 1. Test-Driven Development (TDD)
*   **Red**: Write a failing test first.
*   **Green**: Implement minimal code to pass the test.
*   **Refactor**: Clean up code while maintaining green tests.
*   **Commit**: Only when all tests and quality gates pass.

### 2. Security-First Principles
*   **Zero Host Execution**: Agents always run inside ephemeral Micro-VMs.
*   **Approval Cliff**: Destructive "Red Actions" (writing files, sending data) require explicit human approval via a diff UI.
*   **Ephemeral VMs**: VMs are destroyed immediately after task completion.

### 3. Git Workflow (Enforced)
Direct commits to `main` are blocked. All work must follow:
1.  Create a GitHub Issue.
2.  Start a feature branch: `./scripts/git-workflow.sh start <ISSUE_NUM> "<description>"`
3.  Commit changes (pre-commit hooks will run formatting and linting).
4.  Submit PR: `./scripts/git-workflow.sh submit`

### 4. Quality Gates
*   **Coverage**: Minimum 75% coverage (enforced by a coverage ratchet).
*   **Auditability**: `agent/loop.py` must remain under 4,000 lines.
*   **Complexity**: Max Cyclomatic Complexity of 10 for Python code.

---

## Key Files & Directories
*   `README.md`: High-level project summary.
*   `CLAUDE.md`: Comprehensive architecture and development guide (Highly recommended reading).
*   `Makefile`: Centralized automation for build, test, and linting.
*   `luminaguard_prd.md`: Product Requirements Document.
*   `docs/`: Detailed architectural and testing documentation.
*   `.quint/`: Formal Proof Framework (FPF) reasoning trace.
