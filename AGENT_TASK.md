# Agent Task: snapshot-pool

## Context
- **Issue:** 17
- **Feature:** snapshot-pool
- **Working Directory:** ../ironclaw-snapshot-pool
- **Branch:** feature/ISSUE_NUM-snapshot-pool

## Your Mission
You are implementing the **snapshot-pool** feature for IronClaw Phase 3.

## IronClaw Context

IronClaw is a local-first Agentic AI runtime with JIT Micro-VMs using Firecracker.
- Current spawn time: 110ms (45% better than 200ms target)
- Architecture: Rust orchestrator + Python agent
- Goal: Production-ready secure agent execution

## Code Context

**Key Files:**
- `orchestrator/src/vm/mod.rs` - Main VM module
- `orchestrator/src/vm/firecracker.rs` - Firecracker API client
- `orchestrator/src/vm/config.rs` - VM configuration
- `orchestrator/src/vm/prototype/` - Feasibility test (reference)

**Coding Standards:**
- Follow existing patterns in `orchestrator/src/vm/`
- Use `tokio` for async operations
- Return `anyhow::Result<T>` for error handling
- Use `tracing` for logs (debug, info, warn, error)
- Comprehensive tests (>90% coverage required)
- Run `cargo fmt` before committing
- Run `cargo clippy -- -D warnings` (no warnings allowed)

## Your Tasks

TASK_SPECIFIC

## Quality Gates

Before creating a PR, ensure:

```bash
cd ../ironclaw-snapshot-pool/orchestrator

# Run all tests
cargo test

# Check code coverage (target: >90%)
cargo tarpaulin --out Html

# Run linter (zero warnings)
cargo clippy -- -D warnings

# Format code
cargo fmt

# Verify formatting
cargo fmt --check
```

## Performance Requirements

If applicable, ensure performance meets targets:
- Snapshot load: <20ms
- VM spawn time: <50ms (p95)
- Memory overhead: <100MB
- No memory leaks

## Security Requirements

- No unsafe Rust without justification
- Comprehensive error handling
- Resource cleanup on all paths
- Audit log for security-relevant operations

## Documentation Requirements

Add/update:
- Rust doc comments for all public APIs
- Module-level documentation
- Examples in doc comments
- Integration guide (if new feature)

## Deliverables

- [ ] Implementation complete
- [ ] All tests passing (>90% coverage)
- [ ] Code formatted and linted
- [ ] Documentation updated
- [ ] Benchmarks (if performance-related)
- [ ] Ready for PR creation

## Working Instructions

1. Work in: ../ironclaw-snapshot-pool
2. Review existing code in `orchestrator/src/vm/`
3. Implement feature incrementally
4. Test frequently (cargo test)
5. Commit often with clear messages
6. Ask for help only if blocked

## Git Workflow

```bash
cd ../ironclaw-snapshot-pool

# Make changes
# ... code ...

# Check status
git status

# Commit
git add .
git commit -m "feat: implement snapshot-pool component

- Add X module
- Implement Y functionality
- Add comprehensive tests
- Update documentation

Refs: 17"

# Push (when ready for PR)
git push -u origin feature/ISSUE_NUM-snapshot-pool
```

## PR Creation

When ready:
```bash
gh pr create \\
  --title "feat: snapshot-pool (Issue 17)" \\
  --body "Implements snapshot-pool for IronClaw.

- Summary of changes
- Tests added
- Documentation updated
- Performance benchmarks
- Ready for review

Closes 17"
```

---

**Agent Instructions:**
- Work autonomously
- Follow IronClaw coding standards
- Test frequently
- Ask for help only if blocked
- Focus on quality over speed

**Created by:** IronClaw Swarm Development
**Date:** 2026-02-10
