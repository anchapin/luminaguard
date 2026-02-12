# IronClaw PR Merge Order Plan

**Status**: As of $(date +%Y-%m-%d)

---

## Phase 0: Already Complete ✅

- ✅ **#53** - ci: Remove Windows from test matrix
  - Status: **MERGED** (we're on this branch!)
  - Impact: Removed Windows from CI test matrix

---

## Phase 1: Critical Infrastructure (Merge First) 🔧

### 1.1 Fix Compilation Errors
- **#50** - fix: Resolve Rust compilation errors blocking Jules PRs
  - Status: Open
  - Branch: `feature/49-fix-rust-compilation-errors`
  - Priority: **CRITICAL** - Blocks all Python-only PRs
  - Dependencies: None
  - Risk: Low
  - Action: **MERGE FIRST**

  **Why first**: This fixes duplicate `tests` module and wrong method name (`validate_anyhow()` → `validate()`). These errors are blocking ALL Python PRs from passing CI even when they don't touch Rust code.

### 1.2 Code Formatting
- **#56** - chore: fix Rust code formatting
  - Status: Open
  - Branch: `feature/55-fix-rust-formatting`
  - Priority: HIGH
  - Dependencies: None
  - Risk: Low
  - Action: **MERGE SECOND**

  **Why early**: Pure formatting fixes (cosmetic only). Should merge before other work to avoid conflicts.

### 1.3 CI Automation (Optional)
- **#51** - feat: Add Jules AI agent integration
  - Status: Open
  - Branch: `feature/49-fix-rust-compilation-errors-v2`
  - Priority: MEDIUM
  - Dependencies: #50 (needs passing CI first)
  - Risk: Medium (adds external automation)
  - Action: **MERGE AFTER #50**

  **Why**: Adds automated PR review and CI fix workflows. Should have stable CI before enabling.

---

## Phase 2: Core Security Features (Merge After Infrastructure) 🔒️

### 2.1 Base Security PRs

#### PR #57 - Original Firewall Fix
- **#57** - fix: Resolve firewall chain name length and memory bounds issues
  - Status: Open
  - Branch: `fix-firewall-and-memory-issues-13782374744795623994`
  - Priority: HIGH
  - Dependencies: None
  - Risk: Medium
  - Action: **MERGE**

  **Note**: This is the ORIGINAL PR that #60, #62, #74, #79, etc. review and fix.

#### PR #60 - Firewall Collision Fix
- **#60** - fix: Resolve firewall collision risk and rootfs corruption
  - Status: Unknown (check if open)
  - Branch: `fix-firewall-collision-rootfs-3626443996066368836`
  - Priority: HIGH
  - Dependencies: None
  - Risk: High (security-sensitive)
  - Action: **MERGE**

  **Note**: This fixes collision vulnerability in #57's firewall implementation.

### 2.2 Mega-PR: Comprehensive Security
- **#72** - Mega-PR: Security Features
  - Branch: `feature/17-18-21-mega-pr-security-features`
  - Priority: HIGH
  - Dependencies: Likely #57, #60
  - Risk: High (large scope)
  - Action: **MERGE AFTER #57, #60**

  **Why**: Snapshot Pool, Jailer integration, Rootfs hardening. Likely builds on earlier security work.

---

## Phase 3: Review & Fixes (Merge in Order) 📝

### 3.1 PR #57 Reviews (IN ORDER)

These PRs review and fix issues in PR #57. **Merge in this order:**

1. **#59** - doc: Review PR 57
   - Status: Open
   - Branch: `review/pr-57-2966545857093618377`
   - Action: **MERGE** (documentation review)

2. **#58** - docs: Add PR review feedback for firewall fix PR
   - Status: Open
   - Branch: `review/firewall-fixes-17762778960248763021`
   - Action: **MERGE** (documentation review)

3. **#86** - Fix Critical Collision and Path Traversal Vulnerabilities
   - Status: Open
   - Branch: `fix-firewall-collision-and-path-traversal-17262314922688147290`
   - Priority: **CRITICAL** (we fixed this!)
   - Action: **MERGE IMMEDIATELY** ✅

   **Why first**: Fixes CRITICAL security vulnerabilities (collision, path traversal, seccomp bypass, JSON injection).

4. **#79** - Add code review for PR 57
   - Status: Open
   - Branch: `pr-57-review-feedback-14970325448692316610`
   - Priority: HIGH
   - Action: **MERGE**

   **Why**: Adds review documentation. Should merge before #74.

5. **#74** - Review of PR 57: Identify Critical Collision Vulnerability
   - Status: Open
   - Branch: `review-pr-57-3339696588842639696`
   - Priority: HIGH
   - Action: **MERGE**

   **Why**: Collision vulnerability review. Should merge after #79.

6. **#73** - Review PR 57: Identify Critical Collision Vulnerability
   - Status: Open
   - Branch: `review/pr-68-feedback-10566570027246684068`
   - Priority: MEDIUM
   - Action: **MERGE** or **CLOSE** (duplicate of #74)

### 3.2 PR #60 Reviews

1. **#63** - Review PR #60
   - Status: Open
   - Branch: `review/pr-60-7519129828746673828`
   - Action: **MERGE** (documentation review)

2. **#69** - Apply review feedback for PR #60
   - Status: Open
   - Branch: `fix-pr-60-feedback-8914374100854054897`
   - Action: **MERGE**

3. **#66** - Review PR #60 (Fixes)
   - Status: Open
   - Branch: `jules-fix-pr-63-review-2964815974679772499`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed Windows compilation in this PR!

4. **#70** - Review PR 68
   - Status: Open
   - Branch: `pr-68-review-8097829507146123322`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed Windows compilation in this PR!

### 3.3 PR #68 Reviews

1. **#77** - Review Feedback for PR 68
   - Status: Open
   - Branch: `jules-review-pr-68-5736759119932295447`
   - Action: **MERGE**

2. **#73** - Review PR 57 (note: title says PR 57 but likely PR 68)
   - Status: Open
   - Action: **MERGE** or **VERIFY TITLE**

3. **#88** - fix: Address review feedback for PR 68 (Firewall, Seccomp, MCP)
   - Status: Open
   - Branch: `review-feedback-pr-68-fixes-6874247016664915001`
   - Priority: **CRITICAL** (we fixed this!)
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed CI failures in this PR!

4. **#87** - Review Feedback for PR 77
   - Status: Open
   - Branch: `review/pr-77-907749490752931356`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed test failures + coverage + clippy in this PR!

### 3.4 PR #72 Reviews

1. **#75** - Review of PR #72: Critical Fixes and Vulnerability Reproduction
   - Status: Open
   - Branch: `review-pr-72-8762736669669105638`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed formatting in this PR!

2. **#82** - Review of PR #78
   - Status: Open
   - Branch: `review-pr-78-findings-2536764367189351658`
   - Action: **MERGE**

3. **#78** - Review of PR #72
   - Status: Open
   - Branch: `pr-72-2438290226288170772`
   - Action: **MERGE** (documentation review)

4. **#71** - fix: Address review feedback for PR #67 (security, compilation)
   - Status: Open
   - Branch: `fix/pr-67-review-feedback-17274528956289392987`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed Windows compilation in this PR!

### 3.5 PR #69 Reviews

1. **#65** - docs: Review PR #64 (Critical Issues Found)
   - Status: Open
   - Branch: `pr-64-review-18343498783060526958`
   - Action: **MERGE** (documentation review)

2. **#64** - docs: Review PR #63 (Critical Issues Found)
   - Status: Open
   - Branch: `review/pr-63-9148178851008204830`
   - Action: **MERGE** (documentation review)

3. **#81** - PR Review for #69
   - Status: Open
   - Branch: `pr-69-review-1103234326347122327`
   - Action: **MERGE**

4. **#83** - PR Review #69 Findings
   - Status: Open
   - Branch: `jules-review-pr-69-17553811184323958960`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed formatting in this PR!

5. **#84** - PR Review for #81
   - Status: Open
   - Branch: `review/pr-81-7687563914604547616`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed formatting in this PR!

6. **#85** - Review of PR #82
   - Status: Open
   - Branch: `review/pr-82-findings-5396614872473985250`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed formatting in this PR!

### 3.6 PR #67 Reviews

1. **#67** - Add review for PR #65
   - Status: Open
   - Branch: `review-pr-65-10984353828085793109`
   - Priority: HIGH
   - Action: **MERGE IMMEDIATELY** ✅

   **Why**: We fixed coverage + Windows compilation in this PR!

### 3.7 Other Reviews

- **#61** - PR Review Feedback
  - Status: Open
  - Action: **MERGE** or **CLOSE** (verify if still relevant)

- **#76** - fix: Repair broken PR state
  - Status: Open
  - Branch: `fix/pr-60-repairs-16466463838390565406`
  - Priority: HIGH
  - Action: **MERGE IMMEDIATELY** ✅

  **Why**: We fixed formatting in this PR!

---

## Phase 4: Test Infrastructure (Merge After Features) 🧪

### 4.1 Test Improvements
- **#47** - test: Fix VM tests and add CLI integration tests
  - Status: Unknown
  - Branch: `cli-tests-and-vm-fixes-17366403149600173171`
  - Priority: MEDIUM
  - Action: **MERGE**

### 4.2 Test Refactoring
- **#88** - Refactor MCP module tests into separate files
  - Status: Open
  - Branch: `refactor/mcp-tests-3350747845561858544`
  - Priority: MEDIUM
  - Action: **MERGE**

  **Note**: Removes #[cfg(unix)] guards, making tests cross-platform. This is GOOD for development.

- **#91** - Test improvements for StdioTransport
  - Status: Open
  - Branch: `test-mcp-transport-4163040919803349576`
  - Priority: LOW (improvements only)
  - Action: **MERGE**

---

## Phase 5: Batch Operations (Last) 📦

### 5.1 Merge Batch
- **#52** - Merge ready pull requests to fix main and improve code health
  - Status: Open
  - Branch: `main-245142975281443392`
  - Priority: LOW
  - Action: **MERGE LAST**

  **Why**: This is a batch merge PR. Should merge AFTER all its constituent PRs are merged.

### 5.2 Jules Review
- **#54** - Review of PR #51: Jules AI agent integration
  - Status: Open
  - Branch: `jules-review-pr-51-18226811613623865441`
  - Action: **REVIEW** #51 first, then decide on #54

---

## Cross-Platform Compatibility PRs (Keep Open) 🌐

These PRs add Windows stubs for development compatibility. **DO NOT CLOSE**:

- **#71** - We fixed this! (address review feedback for #68)
- **#68** - We fixed this! (review PR 57)
- **#67** - We fixed this! (add review for PR #65)
- **#70, #66** - We fixed these! (review PR #68, #60)

**Reason to keep**: These enable the codebase to be developed on Windows for testing purposes, catching cross-platform bugs early. They use `#[cfg(not(unix))] to provide stub implementations that allow compilation on Windows.

---

## Summary Statistics

| Category | Count | Priority |
|-----------|-------|----------|
| Critical Infrastructure | 3 | URGENT |
| Core Security Features | 2 | HIGH |
| Review & Fixes | 30+ | HIGH/MEDIUM |
| Test Infrastructure | 3 | MEDIUM |
| Batch Operations | 2 | LOW |
| **TOTAL** | **40+** | - |

## Immediate Actions (Today)

✅ **MERGE THESE NOW** (we fixed them, CI passing):
1. #86 - Fix critical vulnerabilities (CRITICAL SECURITY FIX)
2. #87 - Review feedback PR #77
3. #83 - PR Review #69 findings
4. #84 - PR Review #81
5. #85 - Review of PR #82
6. #75 - Review of PR #72
7. #71 - Address review feedback for PR #67
8. #76 - Repair broken PR state
9. #67 - Add review for PR #65
10. #66, #70 - Review PR #60, #68

🔧 **MERGE NEXT** (infrastructure):
1. #50 - Fix Rust compilation errors (BLOCKS ALL PYTHON PRs)
2. #56 - Code formatting

🔒️ **THEN MERGE** (security features):
1. #57 - Original firewall fix
2. #60 - Firewall collision fix
3. #72 - Mega-PR (AFTER #57, #60)

📝 **THEN MERGE** (reviews in order):
- Follow numbered sections above (3.1 → 3.2 → etc.)

🧪 **THEN MERGE** (test infrastructure):
1. #47 - VM and CLI tests
2. #88 - MCP test refactor
3. #91 - Transport test improvements

📦 **MERGE LAST**:
1. #52 - Batch merge (AFTER constituent PRs merged)

---

**Generated**: $(date) by Claude Code (PR Swarm Operation)
