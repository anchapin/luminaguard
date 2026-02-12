# PR Review: fix: Resolve firewall chain name length and memory bounds issues

## Summary of Changes
This PR (originally PR #57) addressed:
1.  **Firewall Chain Name Length:** Truncates the sanitized VM ID to 19 characters in `FirewallManager` to ensure the resulting `iptables` chain name (`IRONCLAW_<id>`) stays within the 28-character kernel limit.
2.  **Seccomp Audit Log Memory Cap:** Switches `SeccompAuditLog` to use a `VecDeque` with a fixed capacity of 10,000 entries (`MAX_SECCOMP_LOG_ENTRIES`) to prevent unbounded memory growth.
3.  **Testing:** Adds comprehensive tests for truncation, sanitization, and memory limits, including property-based tests.

## Review Feedback

**Code Quality:** ✅
- Adheres to project guidelines and CLAUDE.md.
- Changes are minimal, targeted, and self-documenting.
- Rust code uses `anyhow::Result` and proper error handling.

**Testing:** ✅
- New tests cover the edge cases (long IDs, special characters, log capacity).
- Tests pass locally (`cargo test` verified).

**Security:** ✅
- Prevents `iptables` failures which could lead to unisolated VMs or orchestration crashes.
- Prevents DoS via memory exhaustion in the audit log.

## Critical Issue Identified: VM ID Collision Risk 🔴

**Original Code (PR #57):**
```rust
        // Sanitize vm_id to only contain alphanumeric characters
        // and truncate to ensure chain name <= 28 chars (kernel limit)
        // IRONCLAW_ is 9 chars, so we have 19 chars for the ID
        let sanitized_id: String = vm_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .take(19)
            .collect();
```

**The Vulnerability:**
Truncating the VM ID to 19 characters introduces a **critical collision risk**. If multiple VMs have IDs that share the same first 19 alphanumeric characters (e.g., `agent-task-optimization-1` and `agent-task-optimization-2`), they would both produce `IRONCLAW_agent_task_opt`.

**Impact:**
- The second VM will fail to spawn because `iptables -N` will return "Chain already exists"
- This is a **denial-of-service vulnerability** - an attacker could craft VM IDs to prevent legitimate VMs from spawning
- Or worse, if the logic reused existing chains, VMs could inappropriately share firewall rules

**Fix Applied in Main Branch:**
The main branch has addressed this with deterministic FNV-1a hashing:
```rust
fn fnv1a_hash(text: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// In FirewallManager::new():
let hash = fnv1a_hash(&vm_id);
let chain_name = format!("IRONCLAW_{:016x}", hash);
```

This ensures:
- Uniqueness: Full VM ID is hashed, no truncation
- Determinism: Same VM ID always produces same chain name
- Length compliance: Chain name is always 25 characters (9 + 16 hex)
- No collisions: FNV-1a provides excellent distribution

## Additional Concerns

🟡 **Coverage Drop:** Rust coverage dropped from 68% to 50%, largely due to `firecracker.rs` and `vsock.rs` being excluded on non-Unix platforms. This is acceptable since Windows is not a target platform, but should be documented.

## Decision

**Historical Review: Approve with Documentation** ✅

This review documents a critical vulnerability discovery during PR #57 review. The vulnerability (chain name collision via truncation) was identified during code review and **has been fixed in the main branch** using FNV-1a hashing.

The original PR correctly addressed the immediate crashes (iptables failures, memory exhaustion), but the collision vulnerability was discovered during this review process. The improved solution in main provides robust protection against this DoS vector.

**This review PR (#68) serves as documentation of the vulnerability discovery and fix.**
