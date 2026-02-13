# PR Review #119: fix: Resolve VM module compilation and restore Firecracker/Seccomp functionality

**Review Status: 🔴 Request Changes**

## Summary
The PR attempts to restore VM functionality but fails to deliver a working implementation in the current state. There are significant discrepancies between the commit log and the actual file content, which prevent the module from being compiled or functioning correctly.

## Critical Issues (🔴)

### 1. Module Not Enabled in `lib.rs`
The PR description claims to "Enable `mod vm` in `orchestrator/src/lib.rs`", and the git log shows a modification (`1 +`). However, the actual file content of `orchestrator/src/lib.rs` in the working directory is missing `pub mod vm;`.
- **Impact**: The entire `vm` module is dead code and is not compiled. This is likely why `cargo check` passes despite the broken code inside `vm/`.
- **File**: `orchestrator/src/lib.rs`

### 2. `start_firecracker` Implementation Missing & Signature Mismatch
The `start_firecracker` function in `orchestrator/src/vm/firecracker.rs` is a placeholder that returns a dummy process.
- It accepts `&str` (`_config: &str`) but is called with `&VmConfig` in `orchestrator/src/vm/mod.rs`.
- This would cause a compilation error if the module were actually enabled.
- The new helper functions (`configure_vm`, `start_instance`, etc.) are present but unused.
- **File**: `orchestrator/src/vm/firecracker.rs` (Line 66)

### 3. Missing `Vsock` Implementation
The commit message claims to "Add Vsock struct and configure_vsock to restore VSOCK support", but:
- `orchestrator/src/vm/vsock.rs` is missing from the file system.
- `Vsock` struct is not defined in `firecracker.rs`.
- **Impact**: VSOCK functionality is completely missing.

### 4. Missing `prototype.rs`
The commit message mentions adding `orchestrator/src/vm/prototype.rs` (and the log shows it), but the file is not present in the `orchestrator/src/vm/` directory.

## Verification
I created a reproduction script `orchestrator/src/bin/repro_check.rs` that attempts to use the `vm` module. Running `cargo check --bin repro_check` failed with:
```
error[E0433]: failed to resolve: could not find `vm` in `ironclaw_orchestrator`
```
This confirms the module is not exposed in the current build.

## Recommendations
1. Ensure `pub mod vm;` is correctly applied to `orchestrator/src/lib.rs`.
2. Update `start_firecracker` signature to accept `&VmConfig` and implement it using the added helper functions.
3. Restore the missing `vsock.rs` file and `Vsock` struct.
4. Ensure `prototype.rs` is correctly committed and present.
5. Verify `cargo check` passes *after* enabling the module.

## Code Quality
- **Seccomp**: The `seccomp.rs` module looks well-structured and follows security best practices (default deny, whitelisting).
- **Config**: `VmConfig` validation correctly enforces `enable_networking: false`.

Please investigate why the file changes (especially `lib.rs` and `prototype.rs`) are missing from the codebase despite the commit log.
