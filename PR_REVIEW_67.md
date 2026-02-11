# Review of PR #67

**Summary**:
This PR primarily adds documentation (`PR_REVIEW_*.md` files) and applies fixes/refactoring to the `orchestrator/src/vm/` module, specifically in `seccomp.rs`, `firewall.rs`, and `mod.rs`.

## Critical Issues (🔴)

### 1. Unbounded Audit Log (DoS Risk)
-   **File**: `orchestrator/src/vm/seccomp.rs`
-   **Issue**: `SeccompAuditLog` uses an unbounded `Vec<SeccompAuditEntry>`.
    -   In a long-running system or under attack (e.g., a compromised VM spinning on a blocked syscall), this will lead to memory exhaustion (OOM), causing a Denial of Service.
    -   **Suggestion**: Implement a ring buffer (e.g., `VecDeque` with fixed capacity) or a strict size limit for the audit log.

## Warnings (🟡)

### 1. Mixed Testing Styles
-   **File**: `orchestrator/src/vm/mod.rs`
-   **Issue**: The module contains mixed inline tests (`mod tests { ... }`) and an external test file (`mod tests;`).
    -   This leads to confusion about where tests are located and potential duplication.
    -   **Suggestion**: Consolidate tests into `orchestrator/src/vm/tests.rs`.

### 2. Misleading "Property-based" Tests
-   **File**: `orchestrator/src/vm/tests.rs`
-   **Issue**: Tests are labeled "Property-based test" (e.g., `test_property_networking_always_disabled`) but implement simple table-driven tests with hardcoded inputs.
    -   True property-based testing (using `proptest`) would generate random inputs to cover edge cases.
    -   **Suggestion**: Integrate `proptest` for robust property verification as per project guidelines.

## Suggestions (💡)

### 1. Firewall Error Handling
-   **File**: `orchestrator/src/vm/firewall.rs` (implied from `mod.rs` usage)
-   **Suggestion**: Firewall configuration failures currently log a warning but proceed. In a security-critical context, failure to apply isolation should likely be a hard error, or at least configurable to be one.

### 2. Documentation Structure
-   **File**: `PR_REVIEW_*.md`
-   **Suggestion**: Adding multiple `PR_REVIEW_*.md` files to the root might clutter the repository. Consider organizing them under a `docs/reviews/` directory or consolidating them.

## Decision
**Request Changes** due to the critical Denial of Service risk in the audit logging implementation.
