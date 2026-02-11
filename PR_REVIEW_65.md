# Review of PR #65: "docs: Review PR #64 (Critical Issues Found)"

This PR attempts to address the critical issues found in PR #64 (and #63) by implementing VSOCK configuration, FNV-1a hashing for firewall chains, and seccomp memory caps. However, it introduces significant compilation errors and fails to resolve several critical security and functional issues.

## Summary of Changes
- Adds `PR_REVIEW_64.md` documenting previous findings.
- Implements VSOCK configuration in `firecracker.rs`.
- Implements FNV-1a hashing for firewall chains.
- Updates `SeccompAuditLog` to use `VecDeque` with a cap.
- Adds comprehensive tests.

## Potential Issues

### 🔴 Critical Issues (Must Fix)

1.  **Compilation Errors in Orchestrator**
    *   **File:** `orchestrator/src/vm/mod.rs`
    *   **Issue:** The module `tests` is defined twice: once via `mod tests;` and once inline `mod tests { ... }`.
    *   **Impact:** Compilation fails (`cargo check --tests` errors with `E0428`).
    *   **Fix:** Remove the inline `mod tests { ... }` block and move its contents to `orchestrator/src/vm/tests.rs` (or delete if redundant).

2.  **Usage of Non-Existent Method `validate_anyhow`**
    *   **File:** `orchestrator/src/vm/tests.rs`
    *   **Issue:** Tests call `config.validate_anyhow()` which does not exist. It was replaced by `validate()`.
    *   **Impact:** Compilation fails (`E0599`).
    *   **Fix:** Update tests to use `config.validate()`.

3.  **Security: Firewall Rules are Ineffective (Detached Chain)**
    *   **File:** `orchestrator/src/vm/firewall.rs`
    *   **Issue:** The `configure_isolation` method creates a custom chain (e.g., `IRONCLAW_...`) and adds DROP rules to it, but **never links** this chain to the main `INPUT`, `OUTPUT`, or `FORWARD` chains.
    *   **Impact:** Traffic never traverses the custom chain. The firewall does absolutely nothing.
    *   **Fix:** Add jump rules from main chains to the custom chain (e.g., `iptables -I OUTPUT -j IRONCLAW_...`).

4.  **Security: Seccomp Filters Not Applied to Firecracker Process**
    *   **File:** `orchestrator/src/vm/firecracker.rs`
    *   **Issue:** The `start_firecracker` function uses the Firecracker HTTP API to configure the VM but **never sends the seccomp configuration**. The `VmConfig` struct has a `seccomp_filter` field, but it is ignored during startup.
    *   **Impact:** Seccomp filters are defined but never enforced. The VM runs with default (or no) seccomp restrictions.
    *   **Fix:** Firecracker's HTTP API does not support setting seccomp filters for the VMM process at runtime. You must serialize the filter to a JSON file and pass it via the `--seccomp-filter` command-line argument when spawning the `firecracker` process.

5.  **Functional: Seccomp Filter Blocks VSOCK**
    *   **File:** `orchestrator/src/vm/seccomp.rs`
    *   **Issue:** The `SeccompLevel::Basic` whitelist blocks `socket` and `connect` syscalls, which are required for VSOCK communication inside the guest (or VMM threads managing vsock).
    *   **Impact:** VSOCK communication will fail if seccomp is applied.
    *   **Fix:** Add `socket`, `connect`, `bind`, `listen`, `accept`, `shutdown` to the whitelist for VSOCK support.

6.  **Security Regression: `VmConfig::validate` Missing Network Check**
    *   **File:** `orchestrator/src/vm/config.rs`
    *   **Issue:** The `validate` method checks `vcpu_count` and `memory_mb` but **does not check** `enable_networking`. The memory/requirements state it must enforce `enable_networking == false`.
    *   **Impact:** A configuration with networking enabled is considered valid, bypassing security policy.
    *   **Fix:** Add `if self.enable_networking { anyhow::bail!("Networking must be disabled"); }` to `validate`.

### 🟡 Warnings (Should Fix)

1.  **Duplicate/Redundant Tests**
    *   **File:** `orchestrator/src/vm/mod.rs` vs `tests.rs`
    *   **Issue:** Inline tests in `mod.rs` seem to duplicate functionality covered in `tests.rs`.
    *   **Fix:** Consolidate tests into `tests.rs`.

2.  **Missing Feature Definition**
    *   **File:** `orchestrator/Cargo.toml`
    *   **Issue:** `vm-prototype` feature is used in `mod.rs` but not defined in `Cargo.toml`.
    *   **Fix:** Add `[features] vm-prototype = []` to `Cargo.toml`.

### ✅ Positives

- **VSOCK Configuration**: Correctly implemented in `firecracker.rs` (fixes issue from PR #64).
- **FNV-1a Hashing**: Correctly implemented in `firewall.rs` (fixes collision risk).
- **Seccomp Audit Log**: Correctly implemented with `VecDeque` cap (fixes memory exhaustion).
- **Agent Code**: `agent/loop.py` is clean and under 4000 lines. Python tests pass.

## Decision
**Request Changes** 🔴

The PR cannot be merged due to compilation errors and critical security/functional bugs. Please address the issues listed above.
