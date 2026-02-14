# Agent Task: network-isolation

## Context
- **Issue:** 19
- **Feature:** network-isolation
- **Working Directory:** ../luminaguard-network-isolation
- **Branch:** feature/ISSUE_NUM-network-isolation

## Your Mission
You are implementing the **network-isolation** feature for LuminaGuard Phase 3.

## LuminaGuard Context

LuminaGuard is a local-first Agentic AI runtime with JIT Micro-VMs using Firecracker.
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
cd ../luminaguard-network-isolation/orchestrator

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

1. Work in: ../luminaguard-network-isolation
2. Review existing code in `orchestrator/src/vm/`
3. Implement feature incrementally
4. Test frequently (cargo test)
5. Commit often with clear messages
6. Ask for help only if blocked

## Git Workflow

```bash
cd ../luminaguard-network-isolation

# Make changes
# ... code ...

# Check status
git status

# Commit
git add .
git commit -m "feat: implement network-isolation component

- Add X module
- Implement Y functionality
- Add comprehensive tests
- Update documentation

Refs: 19"

# Push (when ready for PR)
git push -u origin feature/ISSUE_NUM-network-isolation
```

## PR Creation

When ready:
```bash
gh pr create \\
  --title "feat: network-isolation (Issue 19)" \\
  --body "Implements network-isolation for LuminaGuard.

- Summary of changes
- Tests added
- Documentation updated
- Performance benchmarks
- Ready for review

Closes 19"
```

---

**Agent Instructions:**
- Work autonomously
- Follow LuminaGuard coding standards
- Test frequently
- Ask for help only if blocked
- Focus on quality over speed

**Created by:** LuminaGuard Swarm Development
**Date:** 2026-02-10
