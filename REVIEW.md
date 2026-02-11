# Review PR 57: Identify Critical Collision Vulnerability

**PR Context:**
- Title: Review PR 57: Identify Critical Collision Vulnerability
- Branch: `origin/review/pr-57-2966545857093618377`
- Merge Commit: `4fcc3e3e7baf5989f28cb07de8bedf2bbf0a796f`

**Summary of Changes:**
This PR merges significant changes including network isolation (Firewall, vsock) and seccomp filters. It also includes a self-review document (`pr-57-review.md`) which correctly identifies a critical collision vulnerability. However, the PR *includes* the vulnerable code without fixing it.

**Potential Issues:**

### 🔴 Critical: Firewall Chain Name Collision
- **File:** `orchestrator/src/vm/firewall.rs:33`
- **Issue:** The chain name generation uses `.take(19)` to truncate the VM ID:
  ```rust
  let sanitized_id: String = vm_id
      .chars()
      .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
      .take(19) // <--- VULNERABILITY
      .collect();
  ```
  This causes collisions for IDs sharing the first 19 characters (e.g., `long-running-job-worker-1` vs `long-running-job-worker-2`). This allows one VM to overwrite another's firewall rules or fail to start.
- **Recommendation:** Replace truncation with a cryptographic hash (e.g., FNV-1a or truncated SHA-256) to ensure uniqueness within the 28-char limit (e.g., `IRONCLAW_<hash>`).

### 🔴 Critical: Vsock Path Collision
- **File:** `orchestrator/src/vm/config.rs:60`
- **Issue:** The vsock path generation uses the raw `vm_id`:
  ```rust
  config.vsock_path = Some(format!("/tmp/ironclaw/vsock/{}.sock", config.vm_id));
  ```
  If `vm_id` is derived from user input (as seen in `spawn_vm(task_id)`), this allows for socket path collision/predictability.
- **Recommendation:** Use a unique ID (UUID) for the socket path, independent of the user-provided `vm_id`.

### 🔴 Critical: Broken Agent-Orchestrator Integration
- **File:** `agent/mcp_client.py:112`
- **Issue:** The client attempts to spawn the orchestrator with `cargo run -- mcp stdio`:
  ```python
  orch_cmd = ["cargo", "run", "--", "mcp", "stdio"]
  ```
  However, `orchestrator/src/main.rs` does **not** implement the `mcp` subcommand (only `run`, `spawn-vm`, `test-mcp`). The client will fail to start.
- **Recommendation:** Implement the `mcp` subcommand in `orchestrator` (as an alias for `test-mcp` or a new command) or update the client to use the correct command.

### 🟡 Warning: Coverage Decrease
- **File:** `.coverage-baseline.json`
- **Issue:** Coverage baseline lowered significantly (Rust: 68% -> 50%). Ensure critical paths (especially security modules) are adequately tested.

### 🟡 Warning: Formatting
- **File:** `orchestrator/src/vm/vsock.rs`
- **Issue:** Formatting inconsistencies noted in review doc. Please run `cargo fmt`.

**Decision:**
🔴 **Request Changes**

The PR includes code with known critical vulnerabilities (Firewall & Vsock collisions) and broken integration (Agent `mcp` command). The self-review document (`pr-57-review.md`) correctly identifies the firewall issue, but the code must be fixed before merging. Please address the collision vulnerabilities and ensure the Agent can successfully spawn the Orchestrator.
