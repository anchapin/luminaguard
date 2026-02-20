# Contribution Guidelines

This guide covers the standards, processes, and best practices for contributing to LuminaGuard.

## Table of Contents

- [Getting Started](#getting-started)
- [Coding Standards](#coding-standards)
- [Git Workflow](#git-workflow)
- [Pull Request Process](#pull-request-process)
- [Code Review Guidelines](#code-review-guidelines)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Testing Requirements](#testing-requirements)
- [Documentation Requirements](#documentation-requirements)

## Getting Started

### Prerequisites

Before contributing, ensure you have:

1. Set up your development environment following the [Setup Guide](setup.md)
2. Read the [Architecture Overview](architecture.md) to understand the system
3. Reviewed the [Testing Guide](testing.md) for testing requirements
4. Familiarized yourself with the existing codebase

### First Contribution

1. Find a good first issue in the GitHub issue tracker (labeled "good first issue")
2. Create a feature branch following the [Git Workflow](#git-workflow)
3. Implement your changes following the [Coding Standards](#coding-standards)
4. Write tests for your changes
5. Submit a pull request following the [Pull Request Process](#pull-request-process)

## Coding Standards

### Rust Code Standards

#### Style

- Use `cargo fmt` for automatic formatting
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo clippy` for linting (fails on warnings)
- Run `make fmt` and `make lint` before committing

#### Naming Conventions

```rust
// Structs: PascalCase
struct McpClient { }

// Enums: PascalCase
enum McpRequestMethod { }

// Functions: snake_case
fn create_client() -> McpClient { }

// Constants: SCREAMING_SNAKE_CASE
const MAX_RETRIES: u32 = 3;

// Modules: snake_case
mod mcp_client { }
```

#### Error Handling

```rust
// Use Result types for operations that can fail
pub async fn connect_stdio(
    name: &str,
    command: &[&str]
) -> Result<McpClient, McpError> {
    // Implementation
}

// Use thiserror for custom error types
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Failed to spawn MCP server: {0}")]
    SpawnError(#[from] io::Error),

    #[error("JSON-RPC error: {0}")]
    JsonRpcError(String),
}
```

#### Documentation

```rust
/// Connects to an MCP server using stdio transport.
///
/// # Arguments
///
/// * `name` - A unique identifier for this MCP server
/// * `command` - The command to spawn the MCP server (e.g., `["npx", "-y", "@server/fs"]`)
///
/// # Returns
///
/// Returns a `McpClient` instance connected to the server.
///
/// # Errors
///
/// Returns `McpError::SpawnError` if the subprocess fails to start.
///
/// # Examples
///
/// ```
/// use luminaguard_orchestrator::mcp::McpClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = McpClient::connect_stdio(
///     "filesystem",
///     &["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn connect_stdio(
    name: &str,
    command: &[&str]
) -> Result<McpClient, McpError> {
    // Implementation
}
```

### Python Code Standards

#### Style

- Use `black` for automatic formatting (line-length 88)
- Follow [PEP 8](https://pep8.org/) style guide
- Use `pylint` for linting
- Run `make fmt` and `make lint` before committing

#### Naming Conventions

```python
# Classes: PascalCase
class McpClient:
    pass

# Functions and variables: snake_case
def create_client():
    client = McpClient()
    return client

# Constants: UPPER_SNAKE_CASE
MAX_RETRIES = 3

# Private members: leading underscore
class MyClass:
    def __init__(self):
        self._private_var = None
```

#### Type Hints

```python
from typing import Optional, List, Dict, Any
from mcp_client import McpClient

def process_request(
    client: McpClient,
    tool_name: str,
    arguments: Dict[str, Any],
    timeout: Optional[int] = None
) -> Dict[str, Any]:
    """Process a tool request."""
    # Implementation
    return result
```

#### Error Handling

```python
# Use exceptions for error handling
class McpError(Exception):
    """Base exception for MCP client errors."""
    pass

def connect_to_server(server_name: str) -> McpClient:
    """Connect to an MCP server.

    Raises:
        McpError: If connection fails.
    """
    try:
        # Connection logic
        return client
    except Exception as e:
        raise McpError(f"Failed to connect to {server_name}: {e}")
```

#### Documentation

```python
def call_tool(
    self,
    tool_name: str,
    arguments: Optional[Dict[str, Any]] = None
) -> Dict[str, Any]:
    """Call a tool on the MCP server.

    Args:
        tool_name: Name of the tool to call.
        arguments: Optional arguments to pass to the tool.

    Returns:
        The tool's response as a dictionary.

    Raises:
        McpError: If the tool call fails.
        ValueError: If tool_name is not found.

    Examples:
        >>> client = McpClient("filesystem", ["npx", "-y", "@server/fs"])
        >>> client.initialize()
        >>> result = client.call_tool("read_file", {"path": "/tmp/test.txt"})
    """
    # Implementation
```

### Cross-Language Standards

1. **Consistent terminology** - Use the same terms across Rust and Python
2. **Parallel structure** - Keep APIs similar across languages where possible
3. **Shared documentation** - Reference each other's documentation
4. **Unified error handling** - Map error types consistently

## Git Workflow

LuminaGuard requires disciplined git workflow to ensure code quality and traceability. **AI coding agents (Claude Code, GitHub Copilot, etc.) must follow the same workflow as human developers.**

### Core Principles

1. **Issue Tracking:** All work must link to a GitHub issue
2. **Code Review:** All changes must go through pull requests
3. **Automated Enforcement:** Pre-commit hooks block direct commits to protected branches
4. **Branch Protection:** GitHub rules prevent bypassing review

### Branch Naming Convention

Feature branches must follow the format:

```
feature/ISSUE-NUMBER-short-description
```

Examples:
- `feature/42-mcp-client-connection`
- `feature/123-add-snapshot-pooling`
- `feature/256-improve-error-handling`

### Workflow Steps

#### Step 1: Create GitHub Issue

All work starts with an issue for tracking:

```bash
gh issue create \
  --title "Implement MCP client connection" \
  --body "Add ability to connect to MCP servers from orchestrator"
# Returns: Issue #42
```

#### Step 2: Start Feature Branch

Use the workflow helper script (validates issue exists):

```bash
./scripts/git-workflow.sh start 42 "mcp-client-connection"
# Creates: feature/42-mcp-client-connection
# Switches to new branch
```

The script automatically:
- Validates the issue exists
- Shows issue title and state
- Creates properly formatted branch name
- Warns if issue is not open

#### Step 3: Work and Commit

Make changes and commit normally:

```bash
git add .
git commit -m "Add MCP client module"
```

**Pre-commit hook will block commits to `main`, `master`, or `develop` branches** with clear error message.

#### Step 4: Submit Pull Request

When work is ready, create a PR:

```bash
./scripts/git-workflow.sh submit
```

The script automatically:
- Extracts issue number from branch name
- Creates PR with descriptive title
- Links PR to issue (using "Closes #42")
- Opens PR in browser for review

#### Step 5: Monitor Status

Check workflow status anytime:

```bash
./scripts/git-workflow.sh status
```

Shows:
- Current branch
- Linked issue (title, state, labels)
- PR status (if created)

### Branch Protection Rules

**Protected Branches:** `main`, `master`, `develop`

**Rules Enforced:**
- ❌ Direct pushes are BLOCKED
- ✅ Pull requests required (1 approval)
- ✅ Pre-commit checks must pass
- ✅ PRs must link to existing issue

### Error Messages

#### If you try to commit to main:

```
❌ BLOCKED: Cannot commit directly to main

LuminaGuard requires all changes to go through pull requests.

Required workflow:
  1. Create GitHub issue:
     gh issue create --title 'Description' --body 'Details'

  2. Create feature branch:
     git checkout -b feature/ISSUE-NUM-description
     Or use the workflow script:
     ./scripts/git-workflow.sh start ISSUE-NUM 'description'

  3. Make changes and commit normally

  4. Create pull request:
     gh pr create --body 'Closes #ISSUE-NUM'
     Or use the workflow script:
     ./scripts/git-workflow.sh submit

Documentation: See CONTRIBUTING.md section 'Git Workflow'
```

#### If issue doesn't exist:

```
❌ Issue #999 does not exist

Create it first:
  gh issue create --title 'Description' --body 'Implementation details...'
```

## Pull Request Process

### Before Submitting

1. **Run tests locally:**
   ```bash
   make test
   ```

2. **Run quality checks:**
   ```bash
   make fmt
   make lint
   ```

3. **Check coverage:**
   ```bash
   cargo tarpaulin --workspace --out Html --output-dir coverage/
   cd agent && python -m pytest tests/ --cov=. --cov-report=html
   ```

4. **Update documentation** if your changes affect behavior

### PR Title Format

Follow the format:

```
[Component] Brief description
```

Examples:
- `[mcp] Add HTTP transport support`
- `[vm] Implement snapshot pooling`
- `[docs] Update architecture documentation`

### PR Description Template

```markdown
## Overview
Brief description of what this PR does and why.

## Changes
- Change 1
- Change 2
- Change 3

## Testing
- Added unit tests for X
- Added integration tests for Y
- Verified manual testing for Z

## Related Issue
Closes #42

## Checklist
- [ ] Tests pass locally
- [ ] Coverage meets requirements (75%)
- [ ] Documentation updated
- [ ] Code follows style guidelines
```

### PR Review Process

1. **Automated Checks:** CI/CD runs tests and quality gates
2. **Peer Review:** At least one approval required
3. **Address Feedback:** Make requested changes
4. **Approval & Merge:** Maintainer merges after approval

## Code Review Guidelines

### For Reviewers

1. **Be constructive:** Focus on improving code, not criticizing
2. **Be timely:** Review within 48 hours if possible
3. **Check test coverage:** Ensure new code has tests
4. **Verify documentation:** Check docs are updated
5. **Test locally:** Run the code if possible

### What to Review

1. **Correctness:** Does the code do what it claims?
2. **Performance:** Are there performance implications?
3. **Security:** Are there security concerns?
4. **Style:** Does it follow the coding standards?
5. **Documentation:** Is it well-documented?
6. **Tests:** Are tests comprehensive?

### Review Comments

Use these prefixes for review comments:

- `[nit]` - Minor stylistic issue (optional to fix)
- `[suggestion]` - Suggested improvement (optional)
- `[question]` - Question about implementation
- `[required]` - Must be fixed before merge (no prefix)

## Commit Message Guidelines

Follow [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Test changes
- `chore`: Build process or auxiliary tool changes

### Examples

```
feat(mcp): Add HTTP transport support

Implement HTTP transport for MCP clients to support
remote MCP servers. Includes retry logic and timeout
configuration.

Closes #123
```

```
fix(vm): Handle Firecracker process termination gracefully

Fix issue where VM cleanup would fail if Firecracker
process terminated unexpectedly. Now uses SIGTERM
before SIGKILL.

Fixes #456
```

```
docs(architecture): Update VM module documentation

Add details about snapshot pooling and performance
targets. Include new diagrams.
```

## Testing Requirements

### Before Committing

1. **All tests must pass:**
   ```bash
   make test
   ```

2. **Coverage must not decrease:**
   - Check `.coverage-baseline.json`
   - Run coverage report to verify

3. **New code must have tests:**
   - Unit tests for functions/methods
   - Integration tests for cross-component logic
   - Property-based tests for invariants

### Test Quality

- **Fast:** Unit tests should run in <100ms
- **Isolated:** No external dependencies in unit tests
- **Deterministic:** Same result every time
- **Comprehensive:** Cover happy path and error cases
- **Clear:** Test names describe what is being tested

### Coverage Requirements

| Component | Minimum Target | Recommended |
|-----------|----------------|-------------|
| Rust (Orchestrator) | 75% | 80%+ |
| Python (Agent) | 75% | 80%+ |
| Critical paths | 90% | 95%+ |

## Documentation Requirements

### When to Document

1. **Public APIs** - Always document
2. **Complex logic** - Add comments explaining why
3. **Non-obvious code** - Document the approach
4. **Configuration changes** - Update setup guides
5. **Breaking changes** - Update migration guides

### Documentation Standards

#### Rust

- Use `///` for public API documentation
- Include examples in documentation
- Document all parameters and return values
- Use `#[doc(hidden)]` for internal APIs

#### Python

- Use docstrings for all public functions/classes
- Follow Google style or NumPy style
- Include examples in docstrings
- Document all parameters and return values

### Updating Documentation

When making changes:

1. **Update inline documentation** in the code
2. **Update guides** if behavior changes
3. **Update examples** if API changes
4. **Update README** if user-facing changes

## Release Process

### Versioning

Follow [Semantic Versioning](https://semver.org/):
- `MAJOR.MINOR.PATCH`
- Increment MAJOR for breaking changes
- Increment MINOR for new features
- Increment PATCH for bug fixes

### Release Checklist

- [ ] All tests pass
- [ ] Coverage meets requirements
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in Cargo.toml and pyproject.toml
- [ ] Tagged release in git
- [ ] GitHub release created with notes

## Getting Help

### Questions

- Use GitHub Discussions for general questions
- Use GitHub Issues for bug reports and feature requests

### Communication Channels

- **Discussions:** https://github.com/anchapin/luminaguard/discussions
- **Issues:** https://github.com/anchapin/luminaguard/issues
- **Code Review:** Through PR comments

### Resources

- [Developer Setup Guide](setup.md)
- [Architecture Overview](architecture.md)
- [Testing Guide](testing.md)
- [CLAUDE.md](../../CLAUDE.md) - Developer instructions

## License

By contributing to LuminaGuard, you agree that your contributions will be licensed under the same license as the project (see [LICENSE](../../LICENSE)).
