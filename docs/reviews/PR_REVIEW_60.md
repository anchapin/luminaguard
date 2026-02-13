# Review of PR #60: "fix: Resolve firewall collision risk and rootfs corruption"

## Summary of Changes
- Implemented FNV-1a hashing for firewall chain names to prevent collisions.
- Configured rootfs as read-only to prevent corruption.
- Added vsock configuration for VMs.
- Updated tests and guarded Unix-specific code.

## Review Feedback

### 🔴 Critical Issues

1.  **Coverage Drop**: The PR lowers the `rust_ratchet` in `.coverage-baseline.json` from ~74% to **51.3%**.
    *   This violates the project constraint: *"Overall coverage must not decrease"*.
    *   The new `vsock.rs` module has very low coverage (17.7%).
    *   **Action Required**: Add unit tests for `vsock.rs` and other new modules to bring coverage back up to baseline.

### 🟡 Warnings

1.  **Blocking I/O in Async Context**:
    *   **File**: `orchestrator/src/vm/firewall.rs`
    *   **Issue**: `FirewallManager` uses `std::process::Command` (blocking) inside `spawn_vm` (async). This can block the `tokio` executor.
    *   **Suggestion**: Refactor to use `tokio::process::Command`.

2.  **Input Validation**:
    *   **File**: `orchestrator/src/vm/config.rs` / `orchestrator/src/vm/firecracker.rs`
    *   **Issue**: `VmConfig::new` and `start_firecracker` use `vm_id` directly in file paths without sanitization.
    *   **Risk**: Potential path traversal if `vm_id` is user-controlled.
    *   **Suggestion**: Implement strict validation for `vm_id` in `VmConfig::new`.

### 💡 Suggestions

1.  **Property Testing**:
    *   **File**: `orchestrator/src/vm/firewall.rs`
    *   **Issue**: `test_chain_name_always_valid` uses manual iteration.
    *   **Suggestion**: Use `proptest` crate for robust property-based testing.

### ✅ Positives

*   **FNV-1a Hashing**: Effectively prevents collisions.
*   **Rootfs Read-Only**: Crucial fix for stability.
*   **Cross-Platform Support**: Correctly guarded `#[cfg(unix)]`.
*   **Code Style**: Adheres to `cargo fmt` and `clippy`.

**Recommendation**: 🔴 **Request Changes**. Please address the coverage drop and blocking I/O issues.
