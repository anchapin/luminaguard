# Review of PR 57: Identify Critical Collision Vulnerability

**Summary of Changes:**
The PR introduces network isolation using `iptables` via `FirewallManager`, `vsock` support, and `seccomp` filtering. It also updates GitHub workflows.

**Potential Issues:**

🔴 **Critical**: **Collision Vulnerability in `FirewallManager`**
- **File**: `orchestrator/src/vm/firewall.rs` (lines 31-35)
- **Logic**: `vm_id` sanitization maps all non-alphanumeric characters to `_`.
- **Impact**: Multiple VMs with distinct IDs (e.g., `task-1`, `task_1`) map to the same iptables chain `IRONCLAW_task_1`. This allows VMs to share and potentially manipulate each other's firewall rules.
- **Suggestion**: Use a cryptographic hash (e.g., SHA256 or FNV-1a) of the full `vm_id` as part of the chain name to ensure uniqueness.

🔴 **Critical**: **Ineffective Firewall Isolation**
- **File**: `orchestrator/src/vm/firewall.rs` (lines 58-75)
- **Issue**: The `FirewallManager` creates a custom chain and adds DROP rules to it, but it never links this chain to the main `INPUT`, `OUTPUT`, or `FORWARD` chains. The rules are effectively dead code, and the VM is NOT isolated.
- **Suggestion**: Add a rule to jump from `INPUT`/`FORWARD` to the custom chain.

🔴 **Critical**: **Path Traversal Vulnerability**
- **File**: `orchestrator/src/vm/config.rs` (line 61) and `orchestrator/src/vm/firecracker.rs` (line 73), `orchestrator/src/vm/vsock.rs` (line 141)
- **Issue**: `vm_id` is used directly in file paths (`/tmp/ironclaw/vsock/{}.sock`, `/tmp/firecracker-{}.socket`) without sufficient validation or sanitization.
- **Impact**: A malicious `vm_id` (e.g., `../../etc/passwd`) could allow overwriting critical system files.
- **Suggestion**: Validate `vm_id` format (e.g., alphanumeric + hyphens only) or strictly sanitize it for file paths.

🔴 **Critical**: **Data Loss in Vsock Client**
- **File**: `orchestrator/src/vm/vsock.rs` (lines 350-352)
- **Issue**: `VsockClientConnection::send_request` recreates `BufReader` on every call. Any data buffered in the reader but not consumed (e.g., partial messages or subsequent notifications) is discarded when the reader is dropped.
- **Suggestion**: Keep `BufReader` in the `VsockClientConnection` struct.

🟡 **Warning**: **Blocking IO in Async Context**
- **File**: `orchestrator/src/vm/firewall.rs`
- **Issue**: Uses blocking `std::process::Command` inside async methods. This blocks the async executor.
- **Suggestion**: Use `tokio::process::Command`.

💡 **Suggestion**: **Python Type Hints**
- **File**: `agent/loop.py` (line 89)
- **Issue**: `mcp_client` parameter in `execute_tool` is missing type hint.
- **Suggestion**: `mcp_client: McpClient`.

**Approval Decision**: Request Changes.
