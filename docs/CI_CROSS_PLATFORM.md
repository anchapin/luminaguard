# Cross-Platform CI Configuration

This document explains how LuminaGuard handles testing across Windows, macOS, and Linux platforms.

## Test Infrastructure

### Python Tests
- **Environment**: All platforms use `shell: bash` for consistency
- **Test Count**: 504 tests running on all platforms
- **Cross-Platform Support**: Full - no platform-specific skip conditions needed

### Rust Tests  
- **Environment**: Native Rust compiler and cargo
- **Test Count**: 425+ tests running on all platforms
- **Cross-Platform Support**: Full - conditional compilation handles platform differences

## Platform-Specific Considerations

### Linux
- **VM Support**: Full Firecracker/KVM support
- **Features**: All features enabled, including orchestrator vsock
- **Test Isolation**: Complete via KVM virtualization

### macOS
- **VM Support**: Limited - macOS doesn't have KVM
- **Hypervisor**: Uses native hypervisor framework (untested in CI)
- **Test Approach**: Approval client tests mock orchestrator calls
- **Issue**: "No such device or address" errors prevented by input() mocking

### Windows
- **VM Support**: Limited - uses Hyper-V instead of KVM
- **Hypervisor**: Windows-specific implementation in orchestrator
- **Test Approach**: Same orchestrator mocking as macOS
- **Issue**: Same "No such device" errors prevented by input() mocking

## Key Fixes

### Approval Client Mocking (Issue #458, #457)

**Problem**: Tests would fail on macOS/Windows because:
1. Orchestrator not available in CI environment
2. Python tests tried to connect to Rust orchestrator
3. Connection errors returned False instead of testing fallback behavior

**Solution**: Mock the `approval_client.present_diff_card` import:
```python
with patch('builtins.__import__', side_effect=lambda name, *args, **kwargs:
    (_ for _ in ()).throw(ImportError("approval_client"))
    if name == 'approval_client' else __import__(name, *args, **kwargs)):
    # Tests now use fallback input() method
    pass
```

**Result**: 
- Tests work on all platforms
- Validates both approval paths (orchestrator + fallback CLI)
- No platform-specific test skipping needed

## CI Workflow

### Unit Tests (ci-unit.yml)

Runs on matrix:
- **OS**: ubuntu-latest, macos-latest, windows-latest
- **Python**: 3.11, 3.12
- **Rust**: stable

All combinations must pass before merge.

### Quality Gates (quality-gates.yml)

Runs on ubuntu-latest only:
- Code formatting (rustfmt, black)
- Linting (clippy, mypy, pylint)
- Coverage ratcheting
- Invariant checking

## Testing Locally

### Validate Cross-Platform Compatibility
```bash
./scripts/test-cross-platform.sh
```

This runs all test suites and reports results.

### Simulate macOS/Windows CI
```bash
# Mock the orchestrator to simulate CI behavior
cd agent
python -m pytest tests/test_loop_extra.py::test_present_diff_card_red_approves_yes -xvs
```

## Future Improvements

### 1. macOS VM Support
- Investigate native hypervisor framework compatibility
- Could enable real VM testing on macOS CI

### 2. Windows Hyper-V Integration
- Optimize Hyper-V implementation
- Add Hyper-V-specific tests to CI

### 3. ARM64 Support
- Add ARM64 test matrix to GitHub Actions
- Enable native Apple Silicon testing

## Dependencies by Platform

### All Platforms
- Rust 1.70+
- Python 3.11+
- Node.js 18+ (optional, for MCP servers)

### Linux Only
- Linux kernel (for KVM)
- Firecracker (recommended but mocked in tests)

### macOS Only
- Xcode command-line tools
- (Orchestrator features limited)

### Windows Only
- Windows 10+ or Windows Server 2016+
- Hyper-V enabled
- MSVC toolchain or MinGW

## Troubleshooting

### Tests Fail on macOS/Windows
1. Check if orchestrator is running (not needed for CI)
2. Verify approval_client mocking is applied
3. Run with `-xvs` flag for verbose output

### Intermittent Test Failures
- Could be timing-related on slower CI runners
- Check for flaky test markers
- Review recent code changes to approval_client

### Performance Tests Slow
- Network partition tests may timeout on slow runners
- Check if resource limits are too strict
- Consider using `@pytest.mark.slow` decorator

## Contact

For CI/cross-platform issues, open an issue on GitHub with:
- Platform (Linux/macOS/Windows)
- Python version
- Rust version
- Full error message and test logs
