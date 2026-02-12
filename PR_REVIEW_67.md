# Review of PR #67 (which includes changes from PR #65)

## Summary of Changes
This PR introduces significant security enhancements including `FirewallManager` for network isolation using `iptables` and `SeccompFilter` for syscall restriction. It also adds MCP protocol types and transport implementation.

## Potential Issues

### 🔴 Critical Issues

1. **Seccomp Filter Ignored in Firecracker Spawn**
   - **File:** `orchestrator/src/vm/firecracker.rs`
   - **Problem:** The `start_firecracker` function creates a `VmConfig` with a seccomp filter, but it fails to apply this filter when configuring the Firecracker VM. The `configure_vm` function calls `/boot-source`, `/drives/rootfs`, and `/machine-config`, but misses the seccomp configuration step (likely `/machine-config` payload or a separate endpoint).
   - **Impact:** VMs are running without the intended syscall restrictions, leaving them vulnerable to kernel exploitation.

2. **Iptables Chain Name Overflow**
   - **File:** `orchestrator/src/vm/firewall.rs` (lines 35-40)
   - **Problem:** The code generates iptables chain names using `format!("IRONCLAW_{}", sanitized_id)`. `iptables` has a strict 28-character limit for chain names. Since `IRONCLAW_` is 9 characters, this leaves only 19 characters for the VM ID.
   - **Impact:** If `vm_id` is longer than 19 characters (which is common for UUIDs or task IDs), `iptables` commands will fail.
   - **Root Cause:** `sanitized_id` does not truncate or hash the ID to ensure it fits.
   - **Evidence:** Verified with reproduction test `repro_collision.rs` (now deleted).

3. **Firewall Failure is Swallowed**
   - **File:** `orchestrator/src/vm/mod.rs` (lines 136-144)
   - **Problem:** In `spawn_vm_with_config`, if `firewall_manager.configure_isolation()` fails (e.g., due to the name overflow above), it logs a warning and *continues* spawning the VM.
   - **Impact:** "Fail-open" behavior. The VM starts without firewall rules. While `enable_networking` is false in config, the defense-in-depth provided by `iptables` is lost silently.

### 🟡 Warnings

1. **Potential Crash in MCP Client (Python)**
   - **File:** `agent/mcp_client.py`
   - **Problem:** `mypy` reports "Item 'None' of 'Popen[Any] | None' has no attribute 'stdin'". If `_send_request` is called before `spawn` (or if spawn fails), `self._process` is None. The check `if self._state == McpState.SHUTDOWN` is insufficient because the initial state is `DISCONNECTED` (also not safe).
   - **Fix:** Ensure `_process` is not None check in `_send_request` or strictly enforce state transitions.

2. **Insecure String Interpolation in VmConfig**
   - **File:** `orchestrator/src/vm/config.rs`
   - **Problem:** `to_firecracker_json` uses `format!` to build JSON. If `kernel_path` contains quotes, it could lead to JSON injection.
   - **Fix:** Use `serde_json::to_string` to serialize the struct safely.

### 💡 Suggestions

1. **Use Hashing for Chain Names**
   - **File:** `orchestrator/src/vm/firewall.rs`
   - **Suggestion:** Instead of just sanitizing, use a hash (e.g., FNV-1a or SHA-256 truncated) of the `vm_id` to ensure unique, short, and valid chain names.
   - **Example:** `format!("IRONCLAW_{:x}", hash(vm_id))`

2. **Enhance Integration Tests**
   - **Suggestion:** Add a test that specifically verifies `iptables` rules are created (using a mock or running in a container with iptables capability). Current tests swallow failures.

## Decision
**Request Changes**

The critical security issues (missing seccomp, firewall overflow/fail-open) must be addressed before merging.
