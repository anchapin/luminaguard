# Phase 2: Cross-Platform VM Support - Dispatch Summary

**Status**: Orchestration Complete - Awaiting Sub-Agent Execution
**Date**: 2026-02-14
**Scope**: Parallel implementation of macOS, Windows, and HTTP transport backends

## Overview

Phase 2 involves 3 independent workstreams executing in parallel:
1. **Wave 1**: macOS Virtualization.framework Integration (Issue #186)
2. **Wave 2**: Windows WHPX Backend Implementation (Issue #187)
3. **Wave 3**: HTTP Transport for MCP Servers (Issue #188)

## Orchestration Status

### ✓ Discovery Phase
- Created 3 GitHub issues with detailed acceptance criteria
- Issues linked to Phase 2 roadmap
- Research documents available for each

### ✓ Worktree Setup Phase
- Created 3 isolated git worktrees:
  - `.worktrees/wave1-macos-186` (feature/186-macos-virtualization)
  - `.worktrees/wave2-windows-187` (feature/187-windows-whpx)
  - `.worktrees/wave3-http-188` (feature/188-http-transport)
- Each worktree checked out from main
- Each has isolated branch for independent work

### ⏳ Sub-Agent Dispatch Phase
Instructions prepared for each wave:
- `/tmp/wave1-macos-instructions.md`
- `/tmp/wave2-windows-instructions.md`
- `/tmp/wave3-http-instructions.md`

## Wave Details

### Wave 1: macOS Virtualization.framework (Issue #186)

**Target**: `orchestrator/src/vm/apple_hv.rs`

**Deliverables**:
- Real VM lifecycle using `vz` crate
- All VmInstance trait methods implemented
- <200ms spawn time
- Comprehensive tests
- Updated PLATFORM_SUPPORT_STATUS.md

**Key Implementation Points**:
- Create vz::Partition from VmConfig
- Attach root filesystem via virtio-block
- Configure vCPU count and memory
- Boot loader integration
- Graceful shutdown

**Dependencies**:
- `vz = "0.5"` crate
- macOS 11+ (target_os = "macos")

**Success Metrics**:
- Compiles on all platforms (Linux/Windows with #[cfg] gates)
- All tests pass
- <200ms spawn time on macOS
- No unsafe code

---

### Wave 2: Windows WHPX Backend (Issue #187)

**Target**: `orchestrator/src/vm/hyperv.rs` (new file)

**Deliverables**:
- WHPX hypervisor backend using `libwhp`
- Partition management and lifecycle
- vCPU and memory configuration
- <200ms spawn time
- Feature parity with Firecracker
- Comprehensive tests

**Key Implementation Points**:
- Create WHP partition from VmConfig
- Attach VHD virtual disk
- Configure vCPU count and memory
- Virtual network interface support
- Partition stop/cleanup

**Dependencies**:
- `libwhp = "0.8"` crate
- Windows 10/11 Pro/Enterprise
- Hyper-V Platform enabled

**Success Metrics**:
- Compiles on Linux/macOS (Windows-specific with #[cfg] gates)
- All tests pass
- <200ms spawn time on Windows
- No unsafe code outside libwhp bindings

---

### Wave 3: HTTP Transport for MCP (Issue #188)

**Target**: `orchestrator/src/mcp/http_transport.rs` (new file)

**Deliverables**:
- HTTP/HTTPS transport for remote MCP servers
- Exponential backoff retry logic (1s → 32s)
- TLS certificate validation
- Load balancing support (round-robin)
- Mock server tests
- Updated CLAUDE.md

**Key Implementation Points**:
- HttpTransport struct with reqwest client
- Transport trait implementation
- 3-retry policy for transient errors
- TLS validation (strict in production)
- JSON-RPC request/response handling
- Health checks for load balancing

**Dependencies**:
- `reqwest = { version = "0.11", features = ["json"] }`
- `tokio` (already available)

**Success Metrics**:
- Compiles: `cargo test --lib mcp::http_transport`
- HTTP/HTTPS connectivity verified
- Retry logic tested
- All tests pass
- Zero unsafe code

---

## Execution Model

```
main (trunk)
├── Wave 1: feature/186-macos-virtualization
│   └── Sub-Agent 1 (isolated in .worktrees/wave1-macos-186)
│       └── PR #186 (when complete)
├── Wave 2: feature/187-windows-whpx
│   └── Sub-Agent 2 (isolated in .worktrees/wave2-windows-187)
│       └── PR #187 (when complete)
└── Wave 3: feature/188-http-transport
    └── Sub-Agent 3 (isolated in .worktrees/wave3-http-188)
        └── PR #188 (when complete)
```

### Parallel Coordination
- **Execution**: All 3 waves run simultaneously
- **Independence**: No dependencies between waves
- **Isolation**: Each worktree is separate
- **Communication**: Via git commits and PRs
- **Timeout**: 2 hours per wave (escalate if exceeded)

### Sub-Agent Responsibilities
Each sub-agent must:
1. Navigate to assigned worktree
2. Review instructions and acceptance criteria
3. Implement the feature/fix
4. Write tests (unit + integration)
5. Run: `cargo test --lib <module>::`
6. Run: `make fmt && make lint` to ensure quality gates pass
7. Commit with message: `Closes #NNN: <description>`
8. Push to origin branch
9. Create PR: `gh pr create --title "Closes #NNN" --body "<description>"`
10. Signal completion

## Quality Gates

All code must pass:

```bash
# Cross-platform compilation
cargo build
cargo build --target x86_64-pc-windows-gnu  # Via cross
cargo build --target x86_64-apple-darwin    # Via cross

# Tests
cargo test --lib vm::
cargo test --lib mcp::

# Linting and formatting
make fmt    # cargo fmt + black
make lint   # clippy + mypy + flake8
```

## Timeline

- **Wave Startup**: Immediate
- **Estimated Duration**: 1-2 hours per wave
- **Total Duration**: ~2 hours (parallel execution)
- **Integration**: Sequential PR review and merge

## Success Criteria (Phase 2 Complete)

- [ ] All 3 waves complete
- [ ] All tests pass
- [ ] All PRs created with issue links
- [ ] All PRs reviewed and merged
- [ ] PLATFORM_SUPPORT_STATUS.md updated
- [ ] Code coverage maintained >75%
- [ ] No regressions in existing functionality

## Next Steps After Phase 2

1. **Approval Cliff UI**: Desktop GUI for file operation approvals
2. **Advanced VM Features**: Snapshots, pooling improvements
3. **Enterprise MCP Servers**: HTTP transport integration testing
4. **Performance Tuning**: Sub-200ms spawn times on all platforms
5. **Production Hardening**: Security audit, fuzz testing

## References

- **Research**: `docs/architecture/cross-platform-research.md`
- **Architecture**: `PLATFORM_SUPPORT_STATUS.md`
- **Project Guide**: `CLAUDE.md`
- **Trait Definition**: `orchestrator/src/vm/hypervisor.rs`

## Troubleshooting

### Sub-agent stuck on compilation
- Check worktree is synced with main: `git pull origin main`
- Run: `cargo clean && cargo build` to rebuild
- Verify Rust toolchain: `rustc --version`

### Worktree conflicts
- List all worktrees: `git worktree list`
- Remove stale: `git worktree remove --force /path`
- Create new: `git worktree add /path --track -b feature/NNN-name`

### PR creation fails
- Verify branch is pushed: `git push origin feature/NNN-description`
- Check GitHub auth: `gh auth status`
- Try manual: `gh pr create --title "Closes #NNN" --body "Description"`

### Tests fail on cross-platform
- Verify conditional compilation: `#[cfg(target_os = "...")]`
- Test each platform: Use `cross` crate for cross-compilation
- Mark integration tests: `#[ignore]` if they need specific platform

---

**Status**: Ready for sub-agent dispatch
**Awaiting**: Confirmation to proceed with implementation
