# PR Review for #81

This PR introduces critical infrastructure for Firecracker VM spawning, network isolation, and Seccomp filtering. While the architecture is sound, there are several **critical security and functional issues** that must be addressed before merging.

## Summary

The PR implements:
- Firecracker VM spawning via HTTP API over Unix sockets.
- Network isolation using `iptables` chains per VM.
- Seccomp BPF filter generation and auditing.
- Vsock-based host-guest communication.

## Review Decision

🔴 **Request Changes**

## Specific Feedback

### 1. 🔴 Critical: Seccomp Filter Not Applied

In `orchestrator/src/vm/firecracker.rs`, the `configure_vm` function sends configuration for boot source, rootfs, and machine config, but it **completely ignores** the Seccomp filter.

Although `orchestrator/src/vm/seccomp.rs` can generate the filter JSON, and `VmConfig` holds it, it is never passed to the Firecracker process. Firecracker requires the seccomp filter to be passed via the `--seccomp-filter` command-line argument or loaded via API (though usually it's a launch-time config).

**File:** `orchestrator/src/vm/config.rs`
```rust
    /// Convert to Firecracker JSON config
    pub fn to_firecracker_json(&self) -> String {
        // TODO: Implement actual Firecracker JSON format
        format!(
            r#"{{
  "boot-source": {{ ...
```
The `to_firecracker_json` method is a TODO and doesn't include seccomp.

**Remediation:**
- Serialize the seccomp filter to a file.
- Pass the file path to Firecracker using `--seccomp-filter`.
- Ensure `firecracker.rs` uses this file.

### 2. 🔴 Critical: Firewall "Fail-Open" Behavior

In `orchestrator/src/vm/mod.rs`, if `firewall_manager.configure_isolation()` fails (e.g., due to missing root privileges or iptables errors), the code logs a warning and **proceeds to spawn the VM**.

**File:** `orchestrator/src/vm/mod.rs`
```rust
    // Apply firewall rules (may fail if not root)
    match firewall_manager.configure_isolation() {
        Ok(_) => { ... }
        Err(e) => {
            tracing::warn!(
                "Failed to configure firewall (running without root?): {}. \
                VM will still have networking disabled in config, but firewall rules are not applied.",
                e
            );
            // Continue anyway - networking is still disabled in config
        }
    }
```
This violates the "Secure by Default" principle. If isolation cannot be guaranteed, the VM should not start.

**Remediation:**
- Change this to return an error if firewall configuration fails.
- If running without root is a supported use case (e.g., dev), make it explicit via a feature flag or configuration, but default to fail-closed.

### 3. 🔴 Critical: Vsock Data Loss

In `orchestrator/src/vm/vsock.rs`, the `handle_messages` loop recreates the `BufReader` on every iteration.

**File:** `orchestrator/src/vm/vsock.rs`
```rust
    async fn handle_messages<H>(mut self, handler: H) -> Result<()>
    where
        H: VsockMessageHandler + 'static,
    {
        let mut reader = BufReader::new(&mut self.socket);

        loop {
            match Self::read_message(&mut reader).await {
                Ok(Some(msg)) => {
                    // ... (handling logic) ...

                    // Drop the reader borrow before writing
                    drop(reader);
                    Self::write_message(&mut self.socket, &response).await?;
                    reader = BufReader::new(&mut self.socket); // <--- CRITICAL BUG
                }
                // ...
            }
        }
```
If `BufReader` reads more than one message from the socket (which is likely for small messages), dropping `reader` **discards the buffered data**. The next `read_message` will start reading from the socket again, losing the buffered bytes of the next message.

**Remediation:**
- Split the socket into read and write halves using `tokio::io::split` or `into_split`.
- Keep the `BufReader` alive across the loop.

### 4. 🔴 Critical: Blocking I/O in Async Context

The `FirewallManager` uses `std::process::Command`, which is a **blocking** call.

**File:** `orchestrator/src/vm/firewall.rs`
```rust
use std::process::Command; // <--- Blocking

// ...

    pub fn configure_isolation(&self) -> Result<()> {
        // ...
        let output = Command::new("iptables") // <--- Blocks the thread
            .args(["-N", &self.chain_name])
            .output()
            .context("Failed to create iptables chain")?;
```
In an async orchestrator, this will block the Tokio worker thread, potentially causing latency spikes or deadlocks if many VMs are spawned.

**Remediation:**
- Use `tokio::process::Command`.

### 5. 🔴 Critical: Firewall Chain Name Collisions & Overflow

The `FirewallManager` sanitizes VM IDs by replacing non-alphanumeric characters with `_`, leading to collisions. Also, it does not enforce the 28-character limit for iptables chain names.

**File:** `orchestrator/src/vm/firewall.rs`
```rust
        let sanitized_id: String = vm_id
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();

        let chain_name = format!("IRONCLAW_{}", sanitized_id);
```
- `vm-1` -> `IRONCLAW_vm_1`
- `vm_1` -> `IRONCLAW_vm_1` (Collision!)
- `very-long-vm-id-that-exceeds-limit` -> `IRONCLAW_very_long_vm_id_that_exceeds_limit` (Overflow!)

**Remediation:**
- Use a hash (e.g., SHA256 truncated) or a unique ID mechanism if the VM ID is user-controlled.
- Enforce the 28-character limit.

### 6. 🟡 Warning: Unbounded Memory in Audit Log

**File:** `orchestrator/src/vm/seccomp.rs`
`SeccompAuditLog` appends entries to a `Vec` without limit. A compromised or buggy VM could flood the log, causing OOM.

**Remediation:**
- Use a ring buffer (e.g., `VecDeque` with max size) or truncate old logs.

### 7. 🟡 Warning: Missing Python Type Hints

**File:** `agent/loop.py`
The `execute_tool` function lacks a type hint for `mcp_client`.
```python
def execute_tool(call: ToolCall, mcp_client) -> Dict[str, Any]:
```

**Remediation:**
- Add `mcp_client: 'McpClient'` (using string forward reference if needed) or `Any`.

### 8. 💡 Suggestion: Test Assertion Weakness

**File:** `orchestrator/src/vm/tests.rs`
The test `test_vm_with_long_id` asserts that the chain name contains valid characters, but it doesn't assert that it was truncated or handled correctly regarding length.
```rust
        // With 20 chars, total is 9 + 3 + 20 = 32 chars, which exceeds 28
        // So we just verify it contains valid characters
        assert!(chain.chars().all(|c| c.is_alphanumeric() || c == '_'));
```
This test effectively ignores the bug.

---

## Action Plan

1.  **Fix Vsock Data Loss**: Rewrite `handle_messages` to use `tokio::io::split`.
2.  **Fix Firewall Blocking**: Switch to `tokio::process::Command`.
3.  **Fix Firewall Naming**: Implement safe chain naming (e.g., `IRONCLAW_{hash}`).
4.  **Fix Seccomp**: Ensure the filter is actually passed to Firecracker.
5.  **Fix Fail-Open**: Make firewall failure a hard error.
