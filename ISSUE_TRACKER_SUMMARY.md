# GitHub Issues Summary - Phase 2/3 Roadmap

**Date Created:** 2026-02-14  
**Total Issues Created:** 13  
**Status:** All issues created and ready for work

---

## Overview

This document tracks all GitHub issues created to capture remaining work from the LuminaGuard roadmap. Issues span Phase 2 (Security/Approval Cliff/Advanced Features) and Phase 3 (Production Validation).

## Issues by Phase and Category

### Phase 2: Security & Advanced Features (11 issues)

#### 🔧 Core Infrastructure (3 issues)

| # | Title | Effort | Labels | Status |
|---|-------|--------|--------|--------|
| #194 | Implement Firecracker API integration for VM snapshots | Large (60h) | enhancement, design | OPEN |
| #197 | Implement active VM and task queue tracking | Small (8h) | enhancement | OPEN |
| #202 | Implement rootfs security hardening | Large (40h) | enhancement, security | OPEN |

**Dependencies:** #194 must complete before #197 can be fully utilized.

#### 🖥️ User Interface (2 issues)

| # | Title | Effort | Labels | Status |
|---|-------|--------|--------|--------|
| #192 | Implement Approval Cliff UI (High-Stakes Authorizations) | Large | enhancement | OPEN |
| #200 | Implement Approval Cliff TUI for Red action authorization | Large (50h) | enhancement, design | OPEN |

**Note:** #192 and #200 are related - #192 is the orchestrator side, #200 is the TUI implementation.

#### 🧠 Agent Logic (2 issues)

| # | Title | Effort | Labels | Status |
|---|-------|--------|--------|--------|
| #193 | Replace placeholder keyword-based reasoning with LLM integration | Medium (40h) | enhancement, research, design | OPEN |
| #198 | Implement comprehensive MCP client tests (Python) | Medium (20h) | enhancement, test-failure | OPEN |

**Blocker:** #192 (Approval Cliff) may block advanced reasoning workflows.

#### 🌐 Cross-Platform & Transport (2 issues)

| # | Title | Effort | Labels | Status |
|---|-------|--------|--------|--------|
| #199 | Implement real Apple Virtualization.framework VM integration | Large (50h) | enhancement, platform, macos | OPEN |
| #203 | Add load balancing support to HTTP MCP transport | Medium (16h) | enhancement | OPEN |

**Status:** #199 relates to completed PR #189 (Wave 1) - implementation stub needs real code.

#### 📋 Quality & Operations (2 issues)

| # | Title | Effort | Labels | Status |
|---|-------|--------|--------|--------|
| #195 | Clean up compiler warnings in orchestrator | Small (2h) | tech-debt | OPEN |
| #196 | Investigate and fix HTTP transport integration test timeouts | Medium (8h) | test-failure, ci-cd | OPEN |

---

### Phase 3: Production Validation (2 issues)

#### 🔬 Research & Planning (2 issues)

| # | Title | Effort | Labels | Status |
|---|-------|--------|--------|--------|
| #201 | Plan Phase 3 validation program (12-week) | Large (200h) | research, design | OPEN |
| #204 | Audit and update documentation for Phase 2 completion | Small (10h) | documentation | OPEN |

**Timeline:** Phase 3 validation is a 12-week program spanning weeks 1-12 of production readiness.

---

## Issue Dependencies & Critical Path

```
Phase 2 Foundation
├── #194 (Firecracker API) ─┬─→ #197 (VM Tracking)
│                            └─→ #201 (Phase 3 Prep)
├── #192 (Approval Cliff Core)
│   └─→ #200 (TUI Implementation)
│       └─→ #193 (LLM Reasoning)
├── #199 (Apple HV - Phase 2)
└── #203 (HTTP Load Balancing)

Quality & Tests (Parallel)
├── #195 (Compiler Warnings)
├── #196 (Integration Tests)
├── #198 (MCP Tests)
└── #202 (Rootfs Hardening)

Phase 3 Planning
└── #201 (Validation Program)
    └── #204 (Documentation)
```

### Critical Path Items

1. **#194** (Firecracker API) - Enables snapshot pooling, <50ms spawn target
2. **#192 + #200** (Approval Cliff) - Required for Red action authorization
3. **#193** (LLM Reasoning) - Completes autonomous agent capabilities

---

## Effort Breakdown by Category

### By Effort Level

| Level | Issues | Total Hours | Examples |
|-------|--------|-------------|----------|
| Small | 3 | ~20h | #195 (warnings), #196 (tests), #204 (docs) |
| Medium | 4 | ~84h | #193 (LLM), #197 (tracking), #198 (tests), #203 (LB) |
| Large | 6 | ~340h | #194 (Firecracker), #199 (Apple HV), #200 (TUI), #201 (validation), #202 (hardening), #192 |
| **Total** | **13** | **~444h** | ~11 weeks @ 40h/week |

### By Category

| Category | # Issues | Total Effort |
|----------|----------|--------------|
| Core Infrastructure | 3 | Large (108h) |
| User Interface | 2 | Large (100h+) |
| Agent Logic | 2 | Medium-Large (60h) |
| Cross-Platform | 2 | Large + Medium (66h) |
| Quality | 2 | Small-Medium (10h) |
| Planning & Docs | 2 | Large + Small (210h) |

---

## Label Inventory

Labels used in created issues:

| Label | Count | Description |
|-------|-------|-------------|
| enhancement | 11 | New features or improvements |
| design | 5 | Architecture/design decisions |
| research | 2 | Research and investigation |
| platform | 1 | Cross-platform work |
| macos | 1 | macOS-specific |
| security | 1 | Security hardening |
| test-failure | 2 | Testing issues |
| ci-cd | 1 | CI/CD infrastructure |
| tech-debt | 1 | Technical debt |
| documentation | 1 | Documentation updates |

---

## Next Steps by Role

### For Product Manager
- [ ] Prioritize issues based on business value
- [ ] Schedule Phase 2 completion target (estimate: 4-6 weeks)
- [ ] Plan Phase 3 validation timeline (12 weeks parallel)
- [ ] Review dependencies for critical path

### For Engineering Lead
- [ ] Assign issues to team members
- [ ] Create milestones for Phase 2 and Phase 3
- [ ] Establish sprint planning with dependency tracking
- [ ] Monitor #196 (integration test timeouts) for potential regressions

### For Individual Contributors
- [ ] Review assigned issue details
- [ ] Follow git workflow: create GitHub issue → feature branch → PR
- [ ] Each issue should link to a PR with `Closes #NNN` in description
- [ ] Maintain ≥75% test coverage
- [ ] Run `make fmt && make lint` before committing

---

## Roadmap Timeline (Estimate)

```
Week 1-2:    Phase 2 Parallel Tracks
  ├─ #194 (Firecracker API)     ▓▓▓▓
  ├─ #192 (Approval Cliff)       ▓▓
  ├─ #199 (Apple HV)              ▓▓▓
  ├─ #203 (HTTP LB)               ▓
  └─ #195 (Warnings)              ▓

Week 3-4:    Phase 2 Integration
  ├─ #200 (TUI)                  ▓▓▓▓
  ├─ #193 (LLM Reasoning)        ▓▓
  ├─ #198 (MCP Tests)            ▓▓
  ├─ #202 (Rootfs Hardening)     ▓▓
  └─ #196 (Int Tests)            ▓

Week 5-6:    Phase 2 Polish + Phase 3 Planning
  ├─ Bug fixes & optimization    ▓
  ├─ #204 (Documentation)        ▓
  └─ #201 (Phase 3 Planning)     ▓▓

Week 7-18:   Phase 3 Validation (12 weeks)
  └─ #201 Validation Program     ▓▓▓▓▓▓▓▓▓▓▓▓

Legend: ▓ = 1 week sprint
```

---

## Success Criteria

All issues must meet these criteria before closure:

- [ ] Acceptance criteria from issue description are met
- [ ] Test coverage maintained ≥75% (ratchet baseline)
- [ ] All tests pass: `cargo test --lib` + `pytest`
- [ ] Code quality gates pass: `make fmt && make lint`
- [ ] PR created with `Closes #NNN` reference
- [ ] PR reviewed and merged to main
- [ ] Related documentation updated

---

## References

- **Project Guide:** CLAUDE.md
- **Architecture:** docs/architecture/architecture.md
- **Feasibility Report:** FIRECRACKER_FEASIBILITY_REPORT.md
- **Platform Status:** PLATFORM_SUPPORT_STATUS.md
- **Phase 2 Dispatch:** PHASE2_DISPATCH.md

---

## Review & Updates

**Last Updated:** 2026-02-14  
**Created By:** Amp Agent  
**Review Cycle:** Weekly (Mondays)

**To Update This Document:**
```bash
# After resolving an issue
git add ISSUE_TRACKER_SUMMARY.md
git commit -m "docs: Update issue tracker summary for resolved issues"
```
