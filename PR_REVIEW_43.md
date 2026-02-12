# Review of PR #43

**PR Context:**
- Title: chore: remove obsolete comment about placeholder client
- URL: https://github.com/anchapin/ironclaw/pull/43

**Review Decision: 🔴 Request Changes**

The PR is in a critical state due to broken compilation of the VM module, which is currently dead code (not linked). While the comment removal in `mcp/mod.rs` is correct, the attempted fixes for VM compilation are incomplete and introduce severe errors.

## Summary of Changes
- Removed obsolete comment in `orchestrator/src/mcp/mod.rs`.
- Attempted to fix VM compilation issues but failed to link the module or verify it compiles.
- Updated `orchestrator/src/vm/config.rs` with validation logic.

## Critical Issues

### 1. 🔴 VM Module Not Linked (Dead Code)
The `orchestrator/src/vm` module is not linked in `orchestrator/src/lib.rs` (only `pub mod mcp;` exists) or `orchestrator/src/main.rs`. This means `cargo check` ignores all errors within the module, masking critical failures.

- **File:** `orchestrator/src/lib.rs`
- **Issue:** Missing `pub mod vm;`.
- **Fix:** Add `pub mod vm;` to `orchestrator/src/lib.rs` to enable compilation.

### 2. 🔴 Compilation Errors in `firecracker.rs`
Once the module is linked, `orchestrator/src/vm/firecracker.rs` fails to compile with numerous errors:

- **Missing Imports:** The file is missing imports for `VmConfig`, `Serialize`, `Path`, `Child`, `UnixStream`, and `hyper` types.
- **Signature Mismatch:** `start_firecracker` is defined as taking `_config: &str` (on Unix), but is called with `&VmConfig` in `orchestrator/src/vm/mod.rs`.
- **Struct Initialization:** `start_firecracker` returns a dummy `FirecrackerProcess` struct with missing fields (`child_process`, `seccomp_path`, `spawn_time_ms`).

- **File:** `orchestrator/src/vm/firecracker.rs`
- **Fix:** Add necessary imports and fix the implementation to match the expected signature and struct definition.

### 3. 🔴 Missing `vm-prototype` Feature
The feature `vm-prototype` is used in `orchestrator/src/vm/mod.rs` (`#[cfg(feature = "vm-prototype")]`) but is not defined in `orchestrator/Cargo.toml`.

- **File:** `orchestrator/Cargo.toml`
- **Fix:** Add `vm-prototype` to `[features]` in `Cargo.toml`.

### 4. 🟡 Missing `tests.rs`
Commit messages claim inline tests were moved to `tests.rs`, but `orchestrator/src/vm/tests.rs` does not exist on disk, and inline tests remain in `orchestrator/src/vm/mod.rs`.

- **File:** `orchestrator/src/vm/mod.rs` / `orchestrator/src/vm/tests.rs`
- **Fix:** Verify if `tests.rs` was accidentally deleted or if the commit message is misleading. If keeping inline tests, update the documentation.

## Verification

To verify the fixes, please run:
```bash
# Temporarily enable the module
echo "pub mod vm;" >> orchestrator/src/lib.rs
cd orchestrator
cargo check
cargo test
```
Currently, these commands fail or skip VM tests entirely.
