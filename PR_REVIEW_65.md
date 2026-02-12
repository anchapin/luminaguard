# PR Review: #65 - docs: Review PR #64 (Critical Issues Found)

## Summary
The PR addresses several critical security issues but introduces new ones and leaves some unresolved.

## Findings

### 🔴 Critical Issues

1.  **Unbounded Audit Log (DoS Vulnerability)**
    -   **File:** `orchestrator/src/vm/seccomp.rs`
    -   **Issue:** The `SeccompAuditLog` uses an unbounded `Vec<SeccompAuditEntry>`. A compromised VM could trigger numerous audit events, exhausting host memory.
    -   **Fix:** Apply the fix from `seccomp_new.rs` (using `VecDeque` with `MAX_SECCOMP_LOG_ENTRIES`) to `orchestrator/src/vm/seccomp.rs`.

2.  **Fail-Open Firewall**
    -   **File:** `orchestrator/src/vm/firewall.rs` (and `firewall_new.rs`)
    -   **Issue:** The `configure_isolation` method creates a new iptables chain and adds rules to it, but **fails to link it to the `INPUT` or `FORWARD` chains**. Without a jump rule, the isolation rules are never enforced.
    -   **Fix:** Add a rule to jump from `INPUT`/`FORWARD` to the VM-specific chain.

3.  **`mcp_client.py` Ignores `root_dir`**
    -   **File:** `agent/mcp_client.py`
    -   **Issue:** The `spawn` method constructs the command using `self.command` but ignores `self.root_dir`. Tools relying on a root directory (e.g., filesystem server) will malfunction.
    -   **Fix:** Pass `root_dir` as an argument to the command or as an environment variable, depending on the tool's expectation.

### 🟡 Warnings

1.  **Unclean Repository State**
    -   **Issue:** Several temporary files (`seccomp_new.rs`, `firewall_new.rs`, `vsock_new.rs`, `mod_new.rs`, `firecracker_new.rs`) exist in the root directory. These should be integrated or removed before merging.

### 💡 Suggestions

1.  **Missing Type Hint in `loop.py`**
    -   **File:** `agent/loop.py`
    -   **Suggestion:** Add type hint for `mcp_client` in `execute_tool`: `mcp_client: McpClient`.

2.  **`think` Function Placeholder**
    -   **File:** `agent/loop.py`
    -   **Suggestion:** The `think` function is a placeholder returning `None`. Consider implementing basic logic.

## Conclusion
**Request Changes**: Please address the critical security vulnerabilities and clean up the repository before merging.
