# Review Feedback for PR 77

## Summary
This PR merges significant security features (seccomp, firewall) and GitHub integration workflows. While the architecture is sound, there are **Critical Security Vulnerabilities** in the implementation that must be addressed before merging.

## 🔴 Critical Issues

### 1. Firewall Isolation is Broken (Security Bypass)
**File:** `orchestrator/src/vm/firewall.rs`

The `FirewallManager` creates a new chain (e.g., `IRONCLAW_...`) and adds DROP rules to it, but **never links this chain to the main `INPUT` or `FORWARD` chains**.
Without a jump rule (e.g., `iptables -I INPUT -j IRONCLAW_...`), the traffic never traverses the custom chain, rendering the firewall completely ineffective. The VM has full network access despite the configuration.

**Recommendation:**
Add logic to `configure_isolation` to insert a jump rule:
```rust
Command::new("iptables")
    .args(["-I", "INPUT", "-j", &self.chain_name])
    .output()?;
```
And ensure it is removed in `cleanup`.

### 2. Seccomp Filters are Ignored (Security Bypass)
**File:** `orchestrator/src/vm/firecracker.rs`

The `SeccompFilter` is correctly configured in `VmConfig` (with a default `Basic` level), but it is **never applied** to the Firecracker process.
The `start_firecracker` function does not write the filter to a JSON file nor pass the `--seccomp-filter` argument to the `firecracker` binary.
The `configure_vm` function also fails to apply it via API.

**Recommendation:**
In `start_firecracker`, serialize the seccomp filter to a temporary file and pass it:
```rust
let seccomp_path = format!("/tmp/seccomp_{}.json", config.vm_id);
std::fs::write(&seccomp_path, config.seccomp_filter.as_ref().unwrap().to_firecracker_json()?)?;
command.arg("--seccomp-filter").arg(&seccomp_path);
```

### 3. Path Traversal Vulnerability
**File:** `orchestrator/src/vm/config.rs`

In `VmConfig::new`, the `vsock_path` is generated using `vm_id` directly:
```rust
config.vsock_path = Some(format!("/tmp/ironclaw/vsock/{}.sock", config.vm_id));
```
If `vm_id` contains `../` (e.g., `../../etc/passwd`), this allows writing the socket to arbitrary locations. While `FirewallManager` sanitizes the ID, `VmConfig` does not appear to share this sanitization logic or validate the input.

**Recommendation:**
Use the same sanitization logic as `FirewallManager` or `fnv1a_hash` to generate the filename segment.

### 4. JSON Injection Vulnerability
**File:** `orchestrator/src/vm/config.rs`

The `to_firecracker_json` method uses manual string formatting:
```rust
format!(r#"{{ "kernel_image_path": "{}" ... }}"#, self.kernel_path, ...)
```
This is vulnerable to JSON injection. If `kernel_path` contains `"` characters, it breaks the JSON structure or allows injecting arbitrary fields.

**Recommendation:**
Use `serde_json::to_string` to serialize the configuration safely.

## 🟡 Warnings

### 1. Blocking I/O in Async Context
**File:** `orchestrator/src/vm/firewall.rs`

The `Drop` implementation for `FirewallManager` calls `self.cleanup()`, which executes `Command::new("iptables").output()`. This is a blocking operation that will block the OS thread. In an async runtime like Tokio, this can cause performance issues or deadlocks.

**Recommendation:**
Use `tokio::process::Command` in `cleanup` (which is async), and for `Drop`, spawning a blocking task or using a separate cleanup mechanism is preferred. Since `Drop` must be synchronous, you cannot await there. Consider an explicit `async fn shutdown(self)` method and warn if dropped without calling it.

### 2. Vsock Implementation Mismatch
**File:** `orchestrator/src/vm/vsock.rs`

The `vsock.rs` module implements a protocol over `UnixListener` (host side). However, Firecracker's vsock implementation typically *binds* the UDS on the host, requiring the orchestrator to *connect* to it. The current implementation seems to expect to bind the socket itself.
Verify if Firecracker is configured with `uds_path` (client mode) or if it defaults to server mode.

## 💡 Suggestions

- **Hardcoded Users:** `.github/workflows/jules-bug-fixer.yml` hardcodes trusted users. Consider using a GitHub Team or a specialized Action for access control to make it easier to maintain.
- **Documentation:** Update `CLAUDE.md` to reflect the new architecture components (Firewall, Seccomp).

## Decision
**Request Changes** - The security vulnerabilities (Firewall, Seccomp, Injection) are critical and must be fixed before merging.
