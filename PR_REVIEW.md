# PR Review: fix: Resolve firewall chain name length and memory bounds issues

**Summary of Changes**
- Implements `FirewallManager` truncation to 19 characters to satisfy `iptables` 28-char limit.
- Adds `SeccompAuditLog` capacity limit (10,000 entries) to prevent memory leaks.
- Guards `vsock.rs` and `firecracker.rs` with `#[cfg(unix)]` to fix Windows builds.
- Updates `.coverage-baseline.json` (lowering Rust coverage to 50%).

**Feedback**

🔴 **Critical: Firewall Chain Name Collision Risk**
In `orchestrator/src/vm/firewall.rs`:
```rust
        let sanitized_id: String = vm_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .take(19) // <--- Risk here
            .collect();
```
Truncating to the first 19 characters creates a high risk of collision for VMs with identical prefixes (e.g., `agent-task-optimization-1` vs `agent-task-optimization-2`).
If a collision occurs:
- The second VM will fail to spawn because `iptables -N` will fail (chain already exists).
- Or worse, if the logic handled existing chains, it might share rules inappropriately.

**Fix:** Append a hash of the full `vm_id` to the truncated name to ensure uniqueness.
Example: `format!("IC_{}_{}", &sanitized_id[..10], hash(&vm_id)[..8])`.

🟡 **Warning: Significant Coverage Drop (68% -> 50%)**
Rust coverage dropped significantly. This appears to be due to excluding large modules (`firecracker.rs`, `vsock.rs`) on non-Unix platforms or integration tests being skipped due to missing binaries. While necessary to pass CI if the environment lacks Firecracker, ensure this drop is justified and consider adding mocks for Windows testing.

💡 **Suggestion: Explicit Collision Handling**
Even with hashing, consider explicitly handling `iptables` chain creation failures to provide clearer error messages.

**Code Quality & Testing**
- **Code Quality**: Adheres to `CLAUDE.md`. Changes are targeted and readable.
- **Testing**: Added tests verify truncation and memory bounds correctly. Integration tests are skipped in the current environment due to missing Firecracker binary, which is expected but limits verification of the full flow.
- **Security**: Memory bound fix prevents potential DoS via memory exhaustion. Collision risk is the primary security concern.

**Decision**
**Request Changes** (due to collision risk).
