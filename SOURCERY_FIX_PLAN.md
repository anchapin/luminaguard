# Sourcery Review Fix Plan

**Date**: 2026-02-10
**PR**: https://github.com/anchapin/ironclaw/pull/13
**Tool**: Sourcery AI Code Review

---

## Issues Identified

### Issue 1: Hardcoded External Command in Benchmark (Bug Risk)

**File**: `orchestrator/benches/mcp_startup.rs`
**Line**: 12, 22
**Severity**: Medium
**Type**: Bug Risk (Environment Dependency)

**Problem**:
```rust
StdioTransport::spawn("echo", &[]).await.unwrap()
```

The benchmark uses a hardcoded `"echo"` command, which:
- Is environment-dependent (requires POSIX-like environment)
- May fail on Windows or minimal CI environments
- Panics via `unwrap()` if `echo` is not found

**Impact**:
- Benchmark may fail on Windows CI
- Makes tests less portable
- Creates environment dependencies

---

### Issue 2: Command Injection Risk in Python MCP Client (Security)

**File**: `agent/mcp_client.py`
**Line**: 201-208
**Severity**: High
**Type**: Security (Command Injection)

**Problem**:
```python
orch_cmd = ["cargo", "run", "--", "mcp", "stdio"]
orch_cmd.extend(self.command)  # self.command comes from user input

self._process = subprocess.Popen(
    orch_cmd,  # User-controlled command and arguments
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    bufsize=1,
)
```

The code passes user-provided `self.command` directly to `subprocess.Popen`:
- `self.command` is a list of strings from the constructor
- While using a list (not shell=True) mitigates shell injection
- User could still pass malicious arguments to `cargo` or the subprocess

**Impact**:
- Potential for argument injection attacks
- User could manipulate cargo flags or MCP server arguments
- Violates principle of least privilege

---

## Fix Strategy

### Fix 1: Benchmark Command Configuration

**Approach**: Make benchmark command configurable via environment variables

**Implementation**:
1. Add `std::env` import
2. Read command from `MCP_STARTUP_BENCH_CMD` env var (default: `"echo"`)
3. Read args from `MCP_STARTUP_BENCH_ARGS` env var (default: empty)
4. Pass dynamic command to `StdioTransport::spawn()`

**Pros**:
- Maintains portability
- Allows CI to configure benign command if `echo` unavailable
- Documents environment dependency

**Cons**:
- Adds complexity to benchmark code

---

### Fix 2: Command Sanitization in MCP Client

**Approach**: Add validation and sanitization for user-provided commands

**Implementation**:
1. Validate command structure during initialization
2. Add allowlist for safe commands (e.g., `npx`, `python`, `node`)
3. Reject suspicious arguments (e.g., `--flags`, shell metacharacters)
4. Add documentation about security implications
5. Consider adding `Approval Cliff` warnings for dangerous commands

**Pros**:
- Reduces attack surface
- Provides explicit security boundaries
- Documents security assumptions

**Cons**:
- Adds validation overhead
- May break legitimate use cases
- Requires maintaining allowlist

---

## Implementation Plan

### Phase 1: Fix Benchmark (Low Risk)

**File**: `orchestrator/benches/mcp_startup.rs`

**Changes**:
1. Import `std::env`
2. Read `MCP_STARTUP_BENCH_CMD` environment variable
3. Parse `MCP_STARTUP_BENCH_ARGS` if provided
4. Use dynamic command in benchmarks
5. Add documentation comment

**Testing**:
- Run benchmark locally with default `echo` command
- Test with custom command: `MCP_STARTUP_BENCH_CMD=true`
- Verify CI passes

---

### Phase 2: Fix MCP Client Security (Medium Risk)

**File**: `agent/mcp_client.py`

**Changes**:
1. Add command validation in `__init__()`:
   - Check command is a non-empty list
   - Validate first element is a known-safe command
   - Reject suspicious arguments
2. Add `Approval Cliff` indicator for dangerous commands
3. Add security documentation
4. Add tests for validation

**Command Allowlist** (initial):
- `npx` - Node.js package runner
- `python`, `python3` - Python interpreters
- `node` - Node.js runtime
- `cargo` - Rust toolchain (for testing)

**Rejected Patterns**:
- Arguments with `;`, `&`, `|`, `$`, backticks
- Flags like `--exec`, `-e`, `--system`
- Absolute or relative paths with `..`

**Testing**:
- Test safe commands pass validation
- Test malicious commands are rejected
- Test edge cases (empty list, special characters)

---

## Implementation Order

1. ✅ Create this plan
2. ⏳ Fix benchmark (Phase 1) - Low risk, high value
3. ⏳ Fix MCP client (Phase 2) - Medium risk, requires careful testing
4. ⏳ Run all tests
5. ⏳ Commit and push fixes
6. ⏳ Verify CI passes

---

## Risk Assessment

| Fix | Risk | Complexity | Priority |
|-----|------|------------|----------|
| Benchmark | Low | Low | Medium |
| MCP Client | Medium | High | High |

**Note**: The MCP client fix is higher priority because it's a security issue, but also higher risk because it changes the API contract and could break existing code.

---

## Rollback Plan

If fixes cause issues:
1. Revert commits for both fixes
2. Address Sourcery comments as "acknowledged, deferred"
3. Create separate issue for security hardening
4. Document security assumptions in code comments

---

## Success Criteria

1. ✅ Benchmark runs on all CI platforms (ubuntu, macos, windows)
2. ✅ MCP client validates user input
3. ✅ All tests pass (21 Python, 104 Rust)
4. ✅ No Sourcery warnings remaining
5. ✅ Code is well-documented

---

## Notes

- The benchmark fix is straightforward and low-risk
- The MCP client fix requires careful consideration of the threat model
- The MCP client is designed for local-first use, which reduces but doesn't eliminate risk
- Future work: Add integration tests for security validation
