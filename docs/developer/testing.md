# Testing Guide

This guide covers LuminaGuard's testing strategy, requirements, and best practices.

## Table of Contents

- [Overview](#overview)
- [Coverage Targets](#coverage-targets)
- [Testing Philosophy](#testing-philosophy)
- [Rust Testing](#rust-testing)
- [Python Testing](#python-testing)
- [Quality Gates](#quality-gates)
- [Test Writing Guidelines](#test-writing-guidelines)
- [Continuous Integration](#continuous-integration)
- [Troubleshooting](#troubleshooting)

## Overview

LuminaGuard follows **Test-Driven Development (TDD)** with comprehensive coverage targets and property-based testing. Quality gates are enforced via CI/CD to prevent technical debt accumulation.

### Key Principles

1. **Red-Green-Refactor:** Write tests first, implement to pass, then refactor
2. **Property-Based Testing:** Test invariants and edge cases automatically
3. **Fast Feedback:** Unit tests should run in <100ms each
4. **Isolation:** Unit tests use mocks; integration tests use real dependencies
5. **Coverage as a Floor:** Meet minimum coverage, aim for comprehensive testing

## Coverage Targets

### Current Coverage Status

| Component | Target | Current | Status |
|-----------|--------|---------|--------|
| **Rust (Orchestrator)** | 75.0% | 74.2% | ⚠️ Near target |
| **Python (Agent)** | 75.0% | 78.0% | ✅ Exceeds target |
| **Overall** | 75.0% | ~76% | ✅ Exceeds target |

### Coverage Enforcement

- Coverage ratchet in `.coverage-baseline.json`
- CI fails if coverage decreases
- PRs blocked if below target
- Manual updates to baseline only after improving coverage

### Per-File Coverage Targets

| File | Target | Current |
|------|--------|---------|
| `loop.py` | 75% | 73% |
| `mcp_client.py` | 75% | 80% |

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
        let mut client = McpClient::new();
        for _ in 0..10 {
            let request = client.create_request();
            assert!(request.id >= id);
        }
    }
}
```

**Python:** Hypothesis for properties

```python
from hypothesis import given, strategies as st

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

## Rust Testing (`orchestrator/`)

### Test Structure

```
orchestrator/src/
├── mcp/
│   ├── client.rs          # Main code
│   ├── client_tests.rs    # Unit tests (#[cfg(test)])
│   └── integration.rs     # Integration tests
├── vm/
│   ├── mod.rs             # Tests inline
│   └── pool.rs            # Tests inline
└── main.rs                # CLI tests
```

### Running Tests

```bash
# All Rust tests
cargo test --workspace

# Specific module
cargo test -p orchestrator --lib mcp

# Specific function
cargo test test_mcp_request_creation

# With output
cargo test -- --nocapture

# Run in parallel (default)
cargo test --workspace --jobs 4

# Run ignored tests (integration tests)
cargo test -- --ignored
```

### Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --workspace --out Html --output-dir coverage/

# View results
firefox coverage/index.html

# Generate coverage report
cargo tarpaulin --workspace --out Stdout --output-dir coverage/
```

### Current Rust Coverage

| Module | Coverage | Lines |
|--------|----------|-------|
| `protocol.rs` | 100% | 59/59 |
| `transport.rs` | 92% | 58/63 |
| `client.rs` | 78% | 111/142 |
| **Overall** | **74.2%** | 321/414 |

**Good:** Protocol layer (JSON-RPC parsing) thoroughly tested
**To Improve:** Error handling paths in client.rs (5 uncovered lines)

### Writing Rust Tests

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

    #[test]
    fn test_request_id_positive() {
        let request = McpRequest::new(42, McpRequestMethod::Ping, json!({}));
        assert_eq!(request.id, 42);
    }
}
```

**Property-Based Test Example:**

```rust
proptest! {
    #[test]
    fn test_request_id_positive(id in 1u64..u64::MAX) {
        let request = McpRequest::new(id, McpRequestMethod::Ping, json!({}));
        assert_eq!(request.id, id);
    }

    #[test]
    fn test_json_rpc_request_serializable(method in ".*", id in 0u64..10000u64) {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "id": id
        });
        let serialized = serde_json::to_string(&request).unwrap();
        assert!(!serialized.is_empty());
    }
}
```

**Integration Test Example:**

```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_real_mcp_server() {
    let mut client = McpClient::connect_stdio(
        "filesystem",
        &["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    ).await.unwrap();

    client.initialize().await.unwrap();

    let tools = client.list_tools().await.unwrap();
    assert!(!tools.is_empty());

    client.shutdown().await.unwrap();
}
```

## Python Testing (`agent/`)

### Test Structure

```
agent/
├── tests/
│   ├── __init__.py
│   ├── test_loop.py                # Unit tests for loop.py
│   ├── test_mcp_client.py          # Unit tests for mcp_client.py
│   ├── test_mcp_integration.py     # Integration tests with real MCP servers
│   ├── test_security_code_execution.py # Security validation
│   ├── test_approval_cliff.py      # Approval Cliff TUI testing
│   └── test_llm_integration.py     # LLM integration testing
├── loop.py
├── mcp_client.py
└── bot_factory.py
```

### Running Tests

```bash
# All tests
cd agent
python -m pytest tests/ -v

# Specific file
python -m pytest tests/test_loop.py -v

# Specific test
python -m pytest tests/test_mcp_client.py::test_mcp_client_lifecycle -v

# With coverage
python -m pytest tests/ --cov=. --cov-report=term-missing

# With HTML coverage
python -m pytest tests/ --cov=. --cov-report=html

# Integration tests only
python -m pytest tests/ -m integration

# Unit tests only (skip integration)
python -m pytest tests/ -m "not integration"

# Run with output
python -m pytest tests/ -v -s

# Stop on first failure
python -m pytest tests/ -x
```

### Coverage

```bash
# Generate HTML report
python -m pytest tests/ --cov=. --cov-report=html

# View results
firefox htmlcov/index.html

# Generate terminal report
python -m pytest tests/ --cov=. --cov-report=term-missing

# Update baseline
python -m pytest tests/ --cov=. --cov-report=term > .coverage.new
# (manual update to .coverage-baseline.json)
```

### Current Python Coverage

| File | Coverage | Status |
|------|----------|--------|
| `loop.py` | 73% | Missing fallback cases and execute_tool() path |
| `mcp_client.py` | 80% | ✅ Comprehensive (540 lines of tests) |
| `test_approval_cliff.py` | 100% | ✅ Complete TUI testing |
| `test_loop.py` | 100% | ✅ Complete loop tests |
| `test_mcp_client.py` | 100% | ✅ 100+ comprehensive tests |
| **Overall** | **75%** | 1020/1363 lines covered ✅ |

### Writing Python Tests

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

## Test Writing Guidelines

### Test Organization

1. **One test, one assertion** - Keep tests focused
2. **Descriptive names** - `test_command_validation_blocks_shell_injection` vs `test_command_validation`
3. **Arrange-Act-Assert** - Clear test structure
4. **Use fixtures** - Reduce duplication with pytest fixtures
5. **Mock appropriately** - Mock external dependencies, not internal logic

### Test Data

```python
# Use fixtures for common test data
@pytest.fixture
def sample_config():
    return {
        "name": "test-bot",
        "llm_provider": "openai",
        "model": "gpt-4"
    }

def test_bot_creation(sample_config):
    bot = Bot(**sample_config)
    assert bot.name == "test-bot"
```

### Error Testing

```python
# Always test error paths
def test_mcp_client_handles_connection_failure():
    with patch('subprocess.Popen') as mock_popen:
        mock_popen.side_effect = FileNotFoundError("npx not found")
        with pytest.raises(McpError, match="Failed to spawn MCP server"):
            client = McpClient("test", ["npx", "bad-server"])
            client.spawn()
```

### Property-Based Testing Best Practices

1. **Start with simple properties** - Basic invariants first
2. **Use appropriate strategies** - Don't generate arbitrary data
3. **Keep properties simple** - One property per test
4. **Hypothesis configuration** - Set max_examples for faster runs

```python
from hypothesis import given, settings, strategies as st

@settings(max_examples=100)  # Default is 100
@given(st.text(min_size=1, max_size=100))
def test_handles_any_text_input(text):
    """Property: System handles any text input without crashing"""
    result = process_text(text)
    assert isinstance(result, str)
```

## Continuous Integration

### GitHub Actions Workflow

```yaml
name: Quality Gates

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy

      - name: Install Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.11'

      - name: Install dependencies
        run: make install

      - name: Run Rust tests
        run: cargo test --workspace

      - name: Run Python tests
        run: |
          cd agent
          python -m pytest tests/ --cov=. --cov-report=xml

      - name: Check coverage
        run: python scripts/check-coverage-ratchet.py

      - name: Quality gates
        run: pre-commit run --all-files
```

### Coverage Badge (README.md)

```markdown
![Rust Coverage](https://img.shields.io/badge/Rust-74.2%25-yellow)
![Python Coverage](https://img.shields.io/badge/Python-78.0%25-brightgreen)
```

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

### Slow Tests

**Strategies:**
1. Mock external dependencies
2. Use pytest marks to skip slow tests
3. Parallelize test execution
4. Use fixtures to reduce setup overhead

```python
# Mark slow tests
@pytest.mark.slow
def test_slow_operation():
    # This test is slow and can be skipped with:
    # pytest -m "not slow"
    pass
```

## Related Documentation

- [Setup Guide](setup.md) - Development environment setup
- [Architecture Overview](architecture.md) - System design and components
- [Contribution Guidelines](contributing.md) - Coding standards and PR process
- [Testing Strategy](../../testing/testing.md) - Detailed testing strategy

## References

- **TDD Guide:** https://testdriven.io/blog/tdd-python/
- **Hypothesis:** https://hypothesis.readthedocs.io/
- **Proptest:** https://altsysrq.github.io/proptest-book/
- **pytest:** https://docs.pytest.org/
- **Rust Testing:** https://doc.rust-lang.org/book/ch11-00-testing.html
