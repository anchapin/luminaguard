# LuminaGuard Testing Strategy

**Version:** 0.1.0
**Last Updated:** 2026-02-10

---

## Overview

LuminaGuard follows **Test-Driven Development (TDD)** with comprehensive coverage targets and property-based testing. Quality gates are enforced via CI/CD to prevent technical debt accumulation.

---

## Coverage Targets

| Component | Target | Current | Status |
|-----------|--------|---------|--------|
| **Rust (Orchestrator)** | 75.0% | 74.2% | ⚠️ Near target |
| **Python (Agent)** | 75.0% | 78.0% | ✅ Exceeds target |
| **Overall** | 75.0% | ~76% | ✅ Exceeds target |

**Enforcement:**
- Coverage ratchet in `.coverage-baseline.json`
- CI fails if coverage decreases
- PRs blocked if below target

---

## Testing Philosophy

### 1. Red-Green-Refactor (TDD)

**Workflow:**
1. **Red:** Write failing test first
2. **Green:** Implement minimal code to pass
3. **Refactor:** Improve while keeping tests green
4. **Verify:** Run `make test` before committing

**Mandatory:** All new code must follow TDD

### 2. Property-Based Testing

**Rust:** Proptest for invariants
```rust
proptest! {
    #[test]
    fn test_mcp_request_id_monotonic(id in 0u64..10000) {
        // Test that request IDs increase monotonically
    }
}
```

**Python:** Hypothesis for properties
```python
@given(st.lists(st.text()))
def test_state_handles_any_message_list(messages):
    """Property: Agent state can handle any message list"""
    state = AgentState()
    for msg in messages:
        state.handle(msg)
    assert state.is_valid()
```

### 3. Test Isolation

**Unit Tests:** No external dependencies, fast execution (<100ms each)
- Mock subprocess calls
- Mock network I/O
- Deterministic results

**Integration Tests:** Real dependencies, slower execution
- Test against real MCP servers
- Marked with `@pytest.mark.integration` or `#[tokio::test]`
- Run separately in CI

---

## Rust Testing (`orchestrator/`)

### Test Structure

```
orchestrator/src/
├── mcp/
│   ├── client.rs          # Main code
│   ├── client_tests.rs    # Unit tests (#[cfg(test)])
│   └── integration.rs     # Integration tests
├── vm/
│   └── mod.rs             # Tests inline
└── main.rs                # CLI tests
```

### Running Tests

```bash
# All Rust tests
cargo test --workspace

# Specific module
cargo test -p orchestrator --lib mcp

# With output
cargo test -- --nocapture

# Run in parallel (default)
cargo test --workspace --jobs 4
```

### Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --workspace --out Html --output-dir coverage/

# View results
firefox coverage/index.html
```

### Current Coverage

| Module | Coverage | Lines |
|--------|----------|-------|
| `protocol.rs` | 100% | 59/59 |
| `transport.rs` | 92% | 58/63 |
| `client.rs` | 78% | 111/142 |
| **Overall** | **74.2%** | 321/414 |

**Good:** Protocol layer (JSON-RPC parsing) thoroughly tested
**To Improve:** Error handling paths in client.rs (5 uncovered lines)

---

## Python Testing (`agent/`)

### Test Structure

```
agent/
├── tests/
│   ├── __init__.py
│   ├── test_loop.py           # Unit tests for loop.py
│   ├── test_mcp_client.py     # Unit tests for mcp_client.py (TODO)
│   └── test_mcp_integration.py # Integration tests (TODO)
├── loop.py
└── mcp_client.py
```

### Running Tests

```bash
# All tests
cd agent
python -m pytest tests/ -v

# Specific file
python -m pytest tests/test_loop.py -v

# With coverage
python -m pytest tests/ --cov=. --cov-report=term-missing

# Integration tests only
python -m pytest tests/ -m integration

# Unit tests only (skip integration)
python -m pytest tests/ -m "not integration"
```

### Coverage

```bash
# Generate HTML report
python -m pytest tests/ --cov=. --cov-report=html

# View results
firefox htmlcov/index.html

# Update baseline
python -m pytest tests/ --cov=. --cov-report=term > .coverage.new
# (manual update to .coverage-baseline.json)
```

### Current Coverage

| File | Coverage | Missing Lines |
|------|----------|---------------|
| `loop.py` | 73% | 15 lines (think() placeholder) |
| `mcp_client.py` | 80% | 28 lines |
| **Overall** | **78.0%** | 70/322 lines |

**Gap Analysis:**
- Current: 161/322 lines covered (56%)
- Target: 241/322 lines covered (75%)
- **Gap: 80 lines**

**Priority:** `mcp_client.py` needs 15-20 new tests

---

## Quality Gates

### Pre-commit Hooks (`.pre-commit-config.yaml`)

**Rust:**
- `cargo-fmt` - Auto-formatting
- `cargo-clippy` - Linting (fails on warnings)

**Python:**
- `black` - Auto-formatting (line-length 88)
- `radon` - Complexity analysis (max CC=10)
- `interrogate` - Documentation coverage (min 60%)
- `pycln` - Dead code detection
- `jscpd` - Duplicate code detection (min 10 lines)

**General:**
- Large file detection (max 100KB)
- Private key detection
- TOML/YAML/JSON validation
- Coverage ratchet check

### CI/CD (`.github/workflows/quality-gates.yml`)

**Enforced Invariants:**
- `loop.py` < 4,000 lines (CRITICAL - auditability)
- Python files < 100KB (code bloat prevention)
- Cyclomatic complexity ≤ 10
- Documentation coverage ≥ 60%
- Coverage ratchet (no regression)

**Blocking:** All gates must pass before merge

---

## Test Writing Guidelines

### 1. Rust Tests

**Unit Test Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_request_creation() {
        let request = McpRequest::new(
            1,
            McpRequestMethod::Initialize,
            json!({"protocolVersion": "2024-11-05"})
        );
        assert_eq!(request.id, 1);
        assert!(matches!(request.method, McpRequestMethod::Initialize));
    }

    #[proptest]
    fn test_request_id_positive(id in 1u64..u64::MAX) {
        let request = McpRequest::new(id, McpRequestMethod::Ping, json!({}));
        assert_eq!(request.id, id);
    }
}
```

**Integration Test Example:**
```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_real_mcp_server() {
    let mut client = McpClient::connect(stdio_transport(
        "npx", "-y", "@modelcontextprotocol/server-filesystem"
    )).await.unwrap();

    let tools = client.list_tools().await.unwrap();
    assert!(!tools.is_empty());
}
```

### 2. Python Tests

**Unit Test Example:**
```python
import pytest
from unittest.mock import Mock, patch
from mcp_client import McpClient, McpError

class TestMcpClientCommandValidation:
    """Test command validation logic"""

    def test_accepts_safe_commands(self):
        """Test that known-safe commands are accepted"""
        client = McpClient("test", ["echo", "test"])
        result = client._validate_command(["npx", "-y", "@server/fs"])
        assert result == ["npx", "-y", "@server/fs"]

    def test_rejects_shell_operators(self):
        """Test that shell injection attempts are blocked"""
        client = McpClient("test", ["echo", "test"])
        with pytest.raises(McpError):
            client._validate_command(["rm", "-rf", "/", ";", "ls"])

    @given(st.lists(st.text()))
    def test_handles_any_command_list(self, commands):
        """Property: Validation handles any command list"""
        client = McpClient("test", ["echo", "test"])
        # Should not crash, may raise McpError
        try:
            result = client._validate_command(commands)
            assert isinstance(result, list)
        except McpError:
            pass  # Expected for invalid commands
```

**Integration Test Example:**
```python
@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run"
)
def test_real_mcp_server_lifecycle():
    """Test against real MCP server"""
    with McpClient("filesystem", ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]) as client:
        # Initialize
        client.initialize()
        assert client.server_capabilities is not None

        # List tools
        tools = client.list_tools()
        assert len(tools) > 0

        # Call tool
        result = client.call_tool("read_file", {"path": "/tmp/test.txt"})
        assert result is not None
```

---

## Test Priority Matrix

### High Priority (Blockers)

1. **Security Tests** - Command validation, shell injection prevention
2. **Core Logic** - MCP protocol, request/response parsing
3. **Lifecycle** - Spawn, initialize, shutdown sequences

### Medium Priority (Important)

1. **Error Handling** - Connection failures, timeouts
2. **Edge Cases** - Empty responses, malformed JSON
3. **Performance** - Request latency, memory usage

### Low Priority (Nice to Have)

1. **UI/UX** - Progress bars, error messages
2. **Logging** - Log format, verbosity levels
3. **Metrics** - Instrumentation, telemetry

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: Quality Gates

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run Rust tests
        run: cargo test --workspace

      - name: Run Python tests
        run: |
          cd agent
          python -m pytest tests/ --cov=. --cov-report=xml

      - name: Coverage ratchet
        run: |
          python scripts/check-coverage-ratchet.py

      - name: Quality gates
        run: |
          pre-commit run --all-files
```

### Coverage Badge (README.md)

```markdown
![Rust Coverage](https://img.shields.io/badge/Rust-74.2%25-yellow)
![Python Coverage](https://img.shields.io/badge/Python-78.0%25-brightgreen)
```

---

## Troubleshooting

### Tests Failing in CI but Pass Locally

**Common Causes:**
1. **Environment differences** - Check Python version, Rust toolchain
2. **Missing dependencies** - Ensure `make install` runs in CI
3. **Race conditions** - Tests may be running in parallel
4. **Integration tests** - May require external services

**Debug:**
```bash
# Run tests exactly as CI does
cargo test --workspace --locked
python -m pytest tests/ -v --tb=short

# Check environment
rustc --version
python --version
```

### Coverage Regression

**If coverage decreases:**
1. Identify new code without tests
2. Write tests before fixing (TDD)
3. Update baseline only after improving

```bash
# Check what changed
python -m pytest tests/ --cov=. --cov-report=term-missing --compare-coverage

# View detailed report
python -m pytest tests/ --cov=. --cov-report=html
```

---

## References

- **TDD Guide:** https://testdriven.io/blog/tdd-python/
- **Hypothesis:** https://hypothesis.readthedocs.io/
- **Proptest:** https://altsysrq.github.io/proptest-book/
- **pytest:** https://docs.pytest.org/
