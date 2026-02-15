# Parallel Workstream Status - February 14, 2026

**Session Status**: 6 parallel feature branches created and ready for development
**Base Commit**: `feature/202-tui-diffcard-rendering` with Phase 2.2 TUI color-coding complete
**Next Steps**: Execute workstreams in parallel, sync with beads regularly

---

## Summary of Work Completed This Session

### Phase 2.1 → 2.2 Transition
✅ Integrated complete approval module from wave4-approval-192 worktree
✅ Verified all 245 tests pass
✅ Implemented Phase 2.2 TUI color-coded diff card rendering
✅ Created comprehensive parallel implementation roadmap

### Parallel Workstream Setup
✅ Created 6 feature branches (all independent, no conflicts)
✅ Initialized beads task tracker with luminaquard issues
✅ Documented effort estimates and dependencies
✅ Prepared skeleton documentation for each workstream

---

## Active Branches & Status

### ✅ DONE: Phase 2.2 TUI DiffCard Rendering (10h complete)
**Branch**: `feature/202-tui-diffcard-rendering`
**Status**: READY FOR PR
**Work Completed**:
- Color-coded rendering by risk level (🟢🟡🟠🔴🔴)
- Risk-level emoji in header
- Syntax highlighting for change types (Create/Edit/Delete/Command)
- Scroll position indicator (e.g. [15/42])
- 12 unit tests for color mapping
- All 245 tests passing

**Next Phase 2.2 Work** (not yet started):
- Scrollbar visual indicator on right edge
- Text wrapping for terminal width
- Optional syntax highlighting for file paths/commands
- Additional polish and error recovery

**Effort Remaining**: 6-8 hours for full Phase 2.2

---

### 🚀 READY: LLM Agent Integration (#193)
**Branch**: `feature/193-llm-agent-reasoning`
**Status**: SKELETON READY
**Effort**: 40 hours
**Dependencies**: Phase 2.1 ✅
**Scope**:
- Replace keyword-based reasoning in `agent/loop.py`
- LLM integration framework (Claude/GPT-4/etc.)
- Action classification via `ActionType.requires_approval()`
- `ApprovalManager` as final safety gate
- Audit trail integration
- 80%+ test coverage required

**Starting Point**: Use `agent/loop.py` from attached files
- Current: Keyword-based `think()` function (lines 199-244)
- Target: LLM-based reasoning with approval cliff integration

**Key Integration Points**:
```rust
// In orchestrator approval module
pub async fn check_and_approve_tui(
    &mut self,
    action_type: ActionType,     // ← LLM will classify here
    description: String,
    changes: Vec<Change>,
) -> anyhow::Result<ApprovalDecision>  // ← Returns decision for execution
```

---

### 🚀 READY: VM Pool Tracking (#197)
**Branch**: `feature/197-vm-pool-tracking`
**Status**: SKELETON READY
**Effort**: 8 hours
**Dependencies**: Existing VM pool ✅
**Scope**:
- Track active VM instances
- Monitor task queue status
- Expose metrics API (REST or gRPC)
- Integration tests with synthetic load

**Key File**: `orchestrator/src/vm/pool.rs`
- Add `struct PoolMetrics` with active_vms, queued_tasks, avg_spawn_time_ms
- Implement `pub async fn get_pool_metrics() -> Result<PoolMetrics>`
- Export via HTTP endpoint (or MCP tool)

---

### 🚀 READY: HTTP Transport Integration Tests (#196)
**Branch**: `feature/196-http-transport-tests`
**Status**: SKELETON READY
**Effort**: 8 hours
**Dependencies**: HTTP transport ✅ (already implemented)
**Scope**:
- Fix test timeout flakiness
- Improve test isolation (distinct ports per test)
- Add race condition detection
- Ensure <5s total test runtime

**Key File**: `orchestrator/src/mcp/http_transport.rs`
- Review existing tests in `#[cfg(test)]` module
- Add proper server cleanup
- Implement timeout detection
- Use `tokio::test` with multi_thread flavor

---

### 🚀 READY: Rootfs Security Hardening (#202)
**Branch**: `feature/202-rootfs-security`
**Status**: SKELETON READY
**Effort**: 40 hours
**Dependencies**: Existing VM security ✅
**Scope**:
- Drop unnecessary Linux capabilities
- Enable SELinux in VMs
- Implement AppArmor profiles
- Enforce resource limits via cgroups

**Key Files**:
- `orchestrator/src/vm/jailer/` - Privilege dropping
- `orchestrator/src/vm/seccomp.rs` - Syscall filtering
- `orchestrator/src/vm/rootfs/` - Filesystem hardening
- `orchestrator/src/vm/config.rs` - Resource limits

**Security Improvements**:
- Drop: cap_kill, cap_sys_admin, cap_net_admin (keep only essentials)
- Enable: SELinux enforcing mode
- Add: AppArmor profiles for common operations
- Enforce: CPU/memory/I/O limits via cgroups

---

### 🚀 READY: Apple HV Implementation (#199)
**Branch**: `feature/199-apple-hv-impl`
**Status**: SKELETON READY
**Effort**: 50 hours (longest path - can parallelize)
**Dependencies**: Hypervisor trait ✅, VmConfig ✅
**Scope**:
- Implement `Hypervisor` trait for Apple Virtualization.framework
- VM lifecycle (spawn, stop, destroy)
- Integration with snapshot pool
- <500ms spawn time target

**Key Architecture**:
```rust
// In orchestrator/src/vm/apple_hv.rs (or hypervisor.rs)
pub struct AppleHvImpl;

#[async_trait]
impl Hypervisor for AppleHvImpl {
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>>;
    fn name(&self) -> &str { "apple-hv" }
}

pub struct AppleHvInstance {
    vm: vz::VirtualMachine,  // From vz crate (Virtualization.framework)
    id: String,
    socket: String,
}

#[async_trait]
impl VmInstance for AppleHvInstance {
    fn id(&self) -> &str;
    fn pid(&self) -> u32;
    fn socket_path(&self) -> &str;
    fn spawn_time_ms(&self) -> f64;
    async fn stop(&mut self) -> Result<()>;
}
```

**Dependencies**:
- Uncomment in `orchestrator/Cargo.toml`:
  ```toml
  [target.'cfg(target_os = "macos")'.dependencies]
  vz = "0.5"  # Or latest available
  ```

---

## Parallel Execution Strategy

### Workflow for Each Branch

1. **Start Work**:
   ```bash
   git checkout feature/XXX-name
   bd update luminaguard-ID --status in_progress
   ```

2. **During Work**:
   - Commit regularly: `git commit -m "feat: description"`
   - Add notes: `bd update luminaguard-ID --note "Progress: ..."`
   - Run tests: `cargo test --lib` (maintain 75%+ coverage)

3. **Complete Work**:
   ```bash
   git push origin feature/XXX-name
   gh pr create --title "Implement XYZ (Phase 2.2)" --body "Closes #NNN"
   bd update luminaguard-ID --status in_progress → completed
   bd sync  # Persist to git
   ```

### No Conflicts Expected
- **TUI** only touches `orchestrator/src/approval/tui.rs`
- **LLM** only touches `agent/loop.py`
- **VM Pool** only touches `orchestrator/src/vm/pool.rs`
- **HTTP Tests** only touches `orchestrator/src/mcp/http_transport.rs`
- **Rootfs** only touches `orchestrator/src/vm/` modules
- **Apple HV** only touches `orchestrator/src/vm/apple_hv.rs` (new file)

Merge order: TUI → LLM → VM Pool → HTTP Tests → Rootfs → Apple HV (safest to fastest)

---

## Critical Files & Locations

### Approval Module (Foundation for LLM)
```
orchestrator/src/approval/
├── action.rs       # ActionType::requires_approval() classification
├── diff.rs         # DiffCard rendering (now with colors)
├── history.rs      # ApprovalHistory audit trail
├── tui.rs          # TUI framework (Phase 2.2 complete ✅)
├── ui.rs           # CLI fallback
└── mod.rs          # ApprovalManager orchestration
```

### VM Module (Base for all VM work)
```
orchestrator/src/vm/
├── hypervisor.rs   # Hypervisor/VmInstance traits
├── firecracker.rs  # Firecracker implementation
├── apple_hv.rs     # [NEW] Apple HV implementation
├── pool.rs         # Snapshot pool + metrics (Phase 2.2)
├── config.rs       # VM configuration
├── jailer/         # Security sandboxing
├── seccomp.rs      # Syscall filtering
├── rootfs/         # Filesystem hardening (Phase 2.2)
└── tests.rs        # Integration tests
```

### Agent Module (LLM Integration)
```
agent/
├── loop.py         # Main reasoning loop (Phase 2.2 refactor)
├── mcp_client.py   # MCP client for tool execution
└── tests/          # Unit and integration tests
```

---

## Testing Strategy & Coverage Goals

### Phase 2.2 TUI (Already Complete)
- ✅ 245 tests passing
- ✅ 12 new color/rendering tests
- ✅ 75%+ coverage maintained
- ✅ Zero clippy warnings

### LLM Integration (Target 80%+)
- Unit tests with mocked LLM
- Integration tests with real approval workflow
- Safety tests ensuring approval cliff enforcement
- Cost estimation tests

### VM Pool Tracking (Target 80%+)
- Unit tests for metrics calculation
- Integration tests with spawned VMs
- Load testing with synthetic work

### HTTP Transport Tests (Target 85%+)
- Timeout and retry scenario tests
- Load balancing tests
- Connection state tests
- Race condition detection

### Rootfs Hardening (Target 80%+)
- Security tests for capability dropping
- SELinux enforcement tests
- AppArmor profile tests
- Resource limit enforcement tests

### Apple HV (Target 75%+)
- Unit tests with mocked vz
- Integration tests (macOS only)
- Performance benchmarks
- Snapshot pool integration tests

---

## Integration & Merge Plan

### Phase 2.2 Complete (All Workstreams)
After all PRs merge to main:
1. **Verification**: All tests pass, coverage maintained
2. **Performance**: Benchmark VM spawn, TUI render, LLM latency
3. **Documentation**: Update README with new capabilities
4. **Release**: Tag as v0.2.0-beta (Phase 2 complete)

### Rollout Order
1. **Week 1-2**: TUI + VM Pool + HTTP Tests (low risk, parallel)
2. **Week 3-4**: LLM Integration + Rootfs Security (high impact, staggered)
3. **Week 5-10**: Apple HV (longest, no dependency)

---

## For Next Session

### Quick Start Checklist
```bash
# 1. Verify current state
git status
bd ready

# 2. Pick a workstream
git checkout feature/197-vm-pool-tracking  # or any other

# 3. Start work
bd update luminaguard-sw3 --status in_progress
cargo test --lib  # Verify tests still pass

# 4. Make changes
# ... implement feature ...
git add .
git commit -m "feat: implement X"

# 5. Before pushing
cargo fmt
cargo clippy -- -D warnings
cargo test --lib

# 6. Push and PR
git push origin feature/197-vm-pool-tracking
gh pr create --title "Implement X" --body "Closes #197"
bd update luminaguard-sw3 --status completed
bd sync
```

### Sync with Beads
```bash
bd sync              # Before starting
bd update ID --status in_progress  # When starting
bd update ID --note "Completed X"  # Regularly
bd close ID --reason "Merged to main"  # When done
bd sync              # At end of session
```

---

## Metrics & Success Criteria

### Code Quality (All Branches)
- [ ] All tests pass (cargo test --lib)
- [ ] Coverage ≥75% (current: 76%)
- [ ] Zero clippy warnings (-D warnings)
- [ ] All commits follow convention

### Performance (End of Phase 2.2)
- [ ] TUI renders 60+ FPS
- [ ] LLM reasoning <5s average
- [ ] VM spawn <200ms
- [ ] HTTP transport <100ms latency

### Security (End of Phase 2.2)
- [ ] Rootfs hardening reduces attack surface
- [ ] Approval cliff blocks all Red actions
- [ ] Audit trails complete and queryable
- [ ] Apple HV secure and reliable

### Delivery (EOW Target)
- [ ] TUI + LLM Integration → v0.2.0-beta
- [ ] VM Pool & HTTP Tests → production ready
- [ ] Rootfs Security → security hardened
- [ ] Apple HV → macOS support

---

## Session Timeline

- **2026-02-14 09:00**: Phase 2.1 completion + planning (completed ✅)
- **2026-02-14 10:00**: Phase 2.2 TUI implementation (completed ✅)
- **2026-02-14 11:00**: Parallel branch setup (completed ✅)
- **2026-02-14 11:30**: Document & prepare for parallel work (this document)
- **Next Session**: Execute workstreams (recommend: pick 2-3 per session)

---

**Created**: 2026-02-14 11:30 UTC
**Status**: READY FOR PARALLEL IMPLEMENTATION
**Estimated Remaining**: 156 hours (6-8 weeks at ~20h/week)
**Critical Path**: Apple HV (50h) longest, can parallelize with others
**Fast Track**: HTTP Tests (8h), can complete in <1 week
