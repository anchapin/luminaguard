# LuminaGuard Issue Wave Execution Plan

**Date**: 2025-02-14  
**Waves**: 3 (Research → Design → Implementation)  
**Target**: Complete all 3 open issues in parallel

## Overview

This document coordinates parallel sub-agent work on issues #152, #154, and #155 using git worktrees and isolated execution contexts.

## Issue Summary

| Issue | Title | Category | Status | Branch |
|-------|-------|----------|--------|--------|
| #152 | Cross-Platform VM Research | Research | In Progress | `feature/issue-152` |
| #154 | Platform-Agnostic VM Abstraction Layer | Design | In Progress | `feature/issue-154` |
| #155 | Implement macOS VM Backend | Implementation | In Progress | `feature/issue-155` |

## Wave Execution

### Wave 1 & 2 (Parallel): Research + Design
- **Issue #152**: Complete research documentation
  - Deliverable: `docs/architecture/cross-platform-research.md`
  - Status: Branch exists, needs completion
  - Sub-agent: Research + documentation
  
- **Issue #154**: Finalize platform-agnostic trait layer
  - Deliverable: Hypervisor trait implementation
  - Status: Branch exists, needs final integration
  - Sub-agent: Design + code review

### Wave 3 (Sequential): Implementation
- **Issue #155**: Complete macOS backend
  - Deliverable: `orchestrator/src/vm/apple_hv.rs` with tests
  - Dependencies: Issues #152 and #154 must be merged first
  - Sub-agent: Implementation + testing

## Worktree Status

```
.worktrees-pr/
├── pr-169 → feature/issue-152 (Research)
├── pr-170 → feature/issue-154 (Design)
└── pr-171 → feature/issue-155 (Implementation)
```

## Sub-Agent Tasks

### Sub-Agent 1: Issue #152 (Research)
**Goal**: Complete cross-platform VM research documentation

**Tasks**:
1. Navigate to `.worktrees-pr/pr-169`
2. Review current research document in `docs/architecture/cross-platform-research.md`
3. Complete missing sections:
   - macOS Hypervisor.framework analysis
   - Windows Hyper-V capabilities
   - Comparison matrix
4. Test builds and verify no syntax errors
5. Commit: `docs: Complete cross-platform VM research (#152)`
6. Exit status: 0 on success, 1 on failure

**Expected Output**:
- Complete research markdown document
- Comparison matrix table
- Risk assessment section

### Sub-Agent 2: Issue #154 (Design)
**Goal**: Finalize platform-agnostic VM abstraction layer

**Tasks**:
1. Navigate to `.worktrees-pr/pr-170`
2. Review trait definition in `orchestrator/src/vm/hypervisor.rs`
3. Ensure all methods are properly documented
4. Verify Firecracker implementation of the trait
5. Run: `cargo clippy && cargo test --lib vm::`
6. Fix any compilation or test failures
7. Commit: `feat: Complete platform-agnostic VM abstraction layer (#154)`
8. Exit status: 0 on success, 1 on failure

**Expected Output**:
- Clean trait definition with proper documentation
- Firecracker implementation passing all tests
- Zero compilation warnings

### Sub-Agent 3: Issue #155 (Implementation)
**Goal**: Complete macOS VM backend implementation

**Tasks** (Sequential, after #152 and #154):
1. Navigate to `.worktrees-pr/pr-171`
2. Sync with main to get latest changes from #154
3. Complete macOS backend in `orchestrator/src/vm/apple_hv.rs`
4. Implement all methods from Hypervisor trait
5. Add comprehensive tests
6. Run: `cargo test --lib vm::apple_hv`
7. Verify against macOS system (if on macOS)
8. Commit: `feat: Implement macOS VM backend (#155)`
9. Exit status: 0 on success, 1 on failure

**Expected Output**:
- macOS Hypervisor.framework bindings
- Full Hypervisor trait implementation for macOS
- Integration tests (with appropriate skip conditions)

## PR Submission Strategy

Once all waves complete, create PRs in sequence:

1. **PR for #152**: `Cross-Platform VM Research`
   - Body: `Closes #152`
   - Labels: research, platform, documentation

2. **PR for #154**: `Platform-Agnostic VM Abstraction Layer`
   - Body: `Closes #154` + `Depends on #152`
   - Labels: design, enhancement, platform

3. **PR for #155**: `macOS VM Backend Implementation`
   - Body: `Closes #155` + `Depends on #152, #154`
   - Labels: enhancement, feature, platform, macos

## Git Workflow

Each sub-agent follows LuminaGuard's standard workflow:

```bash
# In isolated worktree
cd /home/alexc/Projects/luminaguard/.worktrees-pr/pr-NNN

# Make changes
# ... implement feature ...

# Test before committing
make test

# Commit with issue reference
git commit -m "feat/fix: Description (Closes #NNN)"

# Signal completion (exit 0)
exit 0
```

## Parallel Execution Model

### Waves 1 & 2 (Can run in parallel)
- Issue #152 research can proceed independently
- Issue #154 design can proceed independently
- No merge conflicts expected (different files/directories)

### Wave 3 (Dependent)
- Waits for Wave 2 (#154) to complete
- Uses updated code from #154
- Then implements #155 on top

## Success Criteria

✅ **Wave 1 Success**: Research document is complete and merged
✅ **Wave 2 Success**: Hypervisor trait is implemented and tested
✅ **Wave 3 Success**: macOS backend is implemented and integrated

**Overall Success**: All 3 issues closed with merged PRs

## Rollback Plan

If any wave fails:
1. Sub-agent reports failure (exit 1)
2. Orchestrator captures error message
3. Branch remains open for manual fixes
4. Or: Delete worktree and restart from main

```bash
# Manual reset
git worktree remove --force .worktrees-pr/pr-NNN
git branch -D feature/issue-NNN
```

## Execution Commands

### Start Sub-Agents (Parallel)

```bash
# Terminal 1: Issue #152 Research
amp ask "Sub-Agent Research" "
Work on issue #152 in worktree .worktrees-pr/pr-169.
Complete the cross-platform VM research documentation...
"

# Terminal 2: Issue #154 Design
amp ask "Sub-Agent Design" "
Work on issue #154 in worktree .worktrees-pr/pr-170.
Implement and test the platform-agnostic VM abstraction layer...
"

# Terminal 3 (After Wave 2): Issue #155 Implementation
amp ask "Sub-Agent Implementation" "
Work on issue #155 in worktree .worktrees-pr/pr-171.
Complete the macOS VM backend implementation...
"
```

### Monitor Progress

```bash
# Check worktree status
git worktree list

# Check branch status
git branch -vv | grep "feature/issue-"

# View commits in each branch
git log --oneline feature/issue-152 -5
git log --oneline feature/issue-154 -5
git log --oneline feature/issue-155 -5
```

### Finalize PRs

```bash
# After all waves complete
for issue in 152 154 155; do
  gh pr create \
    --title "Issue #$issue" \
    --body "Closes #$issue" \
    --head feature/issue-$issue \
    --base main
done
```

## Next Steps

1. Review this plan
2. Spawn sub-agents for Waves 1 & 2 in parallel
3. Monitor progress and handle blockers
4. Spawn sub-agent for Wave 3 after Wave 2 completes
5. Create PRs when all waves finish
6. Coordinate human review and merge

---

**Status**: Ready for execution  
**Last Updated**: 2025-02-14
