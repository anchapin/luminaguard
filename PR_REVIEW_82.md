# Review of PR #82

**Summary of Changes:**
This PR merges extensive changes related to:
- JIT Micro-VM spawning logic (`orchestrator/src/vm/firecracker.rs`).
- Network isolation via Firewall (`orchestrator/src/vm/firewall.rs`).
- System Call filtering via Seccomp (`orchestrator/src/vm/seccomp.rs`).
- MCP Client integration testing (`agent/mcp_client.py`).
- Documentation and Workflow updates.

While the feature set is impressive and aligns with the "Agentic Engineering" philosophy, there are **multiple critical security vulnerabilities and functional issues** that block approval.

**Potential Issues:**

🔴 **Critical (Must Fix):**

1.  **Security: Firewall Rules Not Linked (High Severity)**
    -   **File:** `orchestrator/src/vm/firewall.rs`
    -   **Issue:** The `configure_isolation` function creates a new `iptables` chain (`IRONCLAW_...`) and adds rules to it, but **never links this chain to the built-in INPUT, OUTPUT, or FORWARD chains**.
    -   **Impact:** The firewall rules are completely ineffective. The VM is not isolated.
    -   **Fix:** Add rules to jump from `INPUT`/`OUTPUT`/`FORWARD` to the `IRONCLAW_...` chain.

2.  **Security: Seccomp Filter Ignored (High Severity)**
    -   **File:** `orchestrator/src/vm/firecracker.rs`, function `start_firecracker`
    -   **Issue:** The function calls `configure_vm`, which sets up boot source, rootfs, and machine config, but **completely ignores `config.seccomp_filter`**.
    -   **Impact:** VMs run without any syscall filtering, negating the "Defense-in-Depth" implementation in `seccomp.rs`.
    -   **Fix:** Implement seccomp configuration in `configure_vm` or a new helper function, serializing the filter to JSON and sending it to Firecracker.

3.  **Security: Unbounded Audit Log (DoS Vulnerability)**
    -   **File:** `orchestrator/src/vm/seccomp.rs`
    -   **Issue:** `SeccompAuditLog` uses an `Arc<RwLock<Vec<SeccompAuditEntry>>>` that grows indefinitely with every blocked syscall.
    -   **Impact:** An attacker (or malfunctioning agent) can exhaust orchestrator memory by triggering rapid syscall violations.
    -   **Fix:** Implement a ring buffer (fixed size) or drop old entries when a limit is reached.

4.  **Security: Path Traversal Vulnerability**
    -   **File:** `orchestrator/src/vm/config.rs`, function `VmConfig::new`
    -   **Issue:** `vm_id` is used directly in `vsock_path` generation (`format!("/tmp/ironclaw/vsock/{}.sock", config.vm_id)`) without sanitization.
    -   **Impact:** A malicious `vm_id` (e.g., `../../etc/passwd`) could allow writing/reading outside the intended directory.
    -   **Fix:** Sanitize `vm_id` in `VmConfig::new` (allow only alphanumeric/dashes) similar to `FirewallManager`.

5.  **Integration: Broken MCP Command**
    -   **File:** `agent/mcp_client.py`, line 155
    -   **Issue:** The client spawns the orchestrator with `["cargo", "run", "--", "mcp", "stdio"]`.
    -   **Context:** `orchestrator/src/main.rs` **does not have an `mcp` subcommand**. It only has `run`, `spawn-vm`, and `test-mcp`.
    -   **Impact:** The Python agent will fail to connect to the orchestrator.
    -   **Fix:** Implement the `mcp` subcommand in `orchestrator/src/main.rs`.

6.  **Compilation Errors**
    -   **File:** `orchestrator/src/vm/mod.rs`: Duplicate `mod tests` block (lines 164-200) conflicts with `mod tests;` (line 20).
    -   **File:** `orchestrator/src/vm/tests.rs`: Calls `config.validate_anyhow()`, but the method is named `validate()` in `VmConfig`.
    -   **Impact:** The code does not compile (`cargo check --tests` fails).

🟡 **Warnings (Should Fix):**

1.  **Incomplete Validation:** `VmConfig::validate` checks CPU/RAM but fails to enforce `enable_networking == false`.
2.  **Swallowed Logs:** `start_firecracker` redirects `stdout`/`stderr` to `/dev/null`, making debugging startup failures impossible. Suggest logging to a file or capturing output.
3.  **Missing Type Hints:** `agent/mcp_client.py` is missing type hints in `__exit__` and `__init__`, and fails to check if `self._process` is None before access in some paths.

**Decision:**
**Request Changes.**
This PR introduces critical security flaws and broken functionality. Please address the compilation errors and security findings before merging.

**Testing:**
- Please ensure `cargo check --tests` passes.
- Please verify network isolation by attempting to ping out from a VM.
