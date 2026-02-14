# Network Isolation Security Verification Guide

## Overview

This guide provides step-by-step instructions for verifying that LuminaGuard are properly network-isolated. Follow these procedures to ensure your deployment meets security requirements.

## Pre-Verification Checklist

Before verifying network isolation, ensure:

- [ ] Orchestrator is running (with or without root privileges)
- [ ] iptables is installed (`iptables --version`)
- [ ] At least one VM has been spawned
- [ ] You have access to the VM handle or VM ID

## Verification Methods

### Method 1: Programmatic Verification (Recommended)

Use the built-in verification function:

```rust
use luminaguard_orchestrator::vm::{spawn_vm, verify_network_isolation};

#[tokio::main]
async fn main() -> Result<()> {
    let handle = spawn_vm("test-task").await?;

    // Verify network isolation
    let isolated = verify_network_isolation(&handle)?;

    if isolated {
        println!("✅ VM is properly network-isolated");
    } else {
        println!("⚠️  VM is NOT network-isolated (may not have root)");
    }

    Ok(())
}
```

**Expected Output**:
- With root: `✅ VM is properly network-isolated`
- Without root: `⚠️ VM is NOT network-isolated (may not have root)`

### Method 2: Firewall Rules Inspection

Inspect iptables rules directly:

```bash
# List all LuminaGuard firewall chains
sudo iptables -L | grep LUMINAGUARD

# Inspect specific VM chain
sudo iptables -L LUMINAGUARD_vm_test_task -n -v

# Verify DROP rule exists
sudo iptables -L LUMINAGUARD_vm_test_task | grep DROP
```

**Expected Output**:
```
Chain LUMINAGUARD_vm_test_task (0 references)
target     prot opt source               destination
DROP       all  --  0.0.0.0/0            0.0.0.0/0
```

### Method 3: VM Configuration Verification

Verify that VM configuration has networking disabled:

```rust
use luminaguard_orchestrator::vm::config::VmConfig;

let config = VmConfig::new("test-vm".to_string());

// Verify networking is disabled
assert!(!config.enable_networking);

// Verify validation passes
assert!(config.validate().is_ok());

// Try to enable networking (should fail)
config.enable_networking = true;
assert!(config.validate().is_err());
```

### Method 4: Runtime Network Testing (Advanced)

Test network connectivity from within the VM:

```bash
# From inside the VM (if you have shell access)

# Test DNS resolution (should fail)
ping -c 1 google.com
# Expected: ping: google.com: Name or service not known

# Test HTTP connection (should fail)
curl -I https://example.com
# Expected: curl: (6) Could not resolve host

# Test local network (should fail)
ping -c 1 192.168.1.1
# Expected: Network is unreachable

# Test vsock communication (should work)
echo "test" | nc -U /tmp/luminaguard/vsock/vm-test.sock
# Expected: Connection succeeds
```

## Security Tests

### Test 1: Configuration Security

Verify that VMs cannot be created with networking enabled:

```bash
# Run the test
cargo test test_vm_rejects_networking_enabled

# Expected: test should pass
```

### Test 2: Firewall Chain Creation

Verify that firewall chains are created for each VM:

```bash
# Spawn multiple VMs
cargo test test_multiple_vms_isolation

# Check that unique chains exist
sudo iptables -L | grep LUMINAGUARD | wc -l
# Expected: 2 or more chains
```

### Test 3: Firewall Persistence

Verify that firewall rules persist across VM lifecycle:

```bash
# Spawn VM
cargo test test_vm_spawn_and_destroy

# Verify firewall exists before destruction
sudo iptables -L | grep LUMINAGUARD_vm_test_task

# Destroy VM
# Verify firewall is cleaned up
sudo iptables -L | grep LUMINAGUARD_vm_test_task
# Expected: Chain no longer exists
```

### Test 4: vsock Communication

Verify that vsock communication works while network is blocked:

```bash
# Run vsock tests
cargo test test_vsock_message_serialization

# Expected: All tests pass
```

### Test 5: Message Size Limits

Verify that oversized messages are rejected:

```bash
# Run size limit test
cargo test test_vsock_message_size_limit

# Expected: Test passes (oversized messages rejected)
```

## Continuous Monitoring

### Log Monitoring

Monitor orchestrator logs for security events:

```bash
# View firewall configuration events
journalctl -u luminaguard-orchestrator | grep -i firewall

# View VM spawn events
journalctl -u luminaguard-orchestrator | grep -i "spawning vm"

# View network isolation verification
journalctl -u luminaguard-orchestrator | grep -i "network isolation"
```

### Audit Checklist

Regularly audit the following:

- [ ] All VMs have `enable_networking = false`
- [ ] All VMs have firewall chains configured
- [ ] No VMs have external network access
- [ ] vsock sockets are properly isolated
- [ ] Firewall rules are cleaned up after VM destruction

## Common Issues and Solutions

### Issue 1: "Permission denied" when configuring firewall

**Symptom**: Firewall configuration fails with permission error

**Diagnosis**:
```bash
# Check if running as root
id -u

# Check iptables availability
iptables --version
```

**Solution**:
```bash
# Option 1: Run with sudo
sudo luminaguard-orchestrator

# Option 2: Grant capabilities
sudo setcap cap_net_admin+ep luminaguard-orchestrator
```

### Issue 2: Firewall chain not found

**Symptom**: `verify_network_isolation()` returns `false`

**Diagnosis**:
```bash
# Check if chain exists
sudo iptables -L | grep LUMINAGUARD

# Check orchestrator logs
journalctl -u luminaguard-orchestrator | grep -i firewall
```

**Solution**:
- Verify VM was spawned successfully
- Check if orchestrator has root privileges
- Re-spawn VM with proper privileges

### Issue 3: VM has network access despite isolation

**Symptom**: Network traffic from VM is not blocked

**Diagnosis**:
```bash
# Check VM configuration
# (From orchestrator code or logs)

# Check firewall rules
sudo iptables -L LUMINAGUARD_vm_<id> -n -v

# Check packet counters
sudo iptables -L LUMINAGUARD_vm_<id> -n -v -Z
```

**Solution**:
- Verify firewall chain is referenced in the main chain
- Check if Firecracker networking is properly disabled
- Review VM configuration for misconfigurations

## Security Best Practices

### 1. Always Run with Root Privileges (Production)

In production, always run the orchestrator with `CAP_NET_ADMIN` capability:

```bash
# Grant capabilities
sudo setcap cap_net_admin,cap_net_raw+ep luminaguard-orchestrator
```

### 2. Monitor Firewall Rules

Regularly audit firewall rules:

```bash
# List all LuminaGuard rules
sudo iptables -L | grep -A 5 LUMINAGUARD
```

### 3. Validate Configuration

Always validate VM configuration before spawning:

```rust
let config = VmConfig::new(vm_id);
config.validate_anyhow()?; // Will fail if networking is enabled
```

### 4. Verify Isolation

After spawning VMs, verify isolation:

```rust
let handle = spawn_vm(task_id).await?;
assert!(verify_network_isolation(&handle)?);
```

### 5. Clean Up Properly

Always destroy VMs after use:

```rust
destroy_vm(handle).await?; // Cleanup happens automatically
```

## Reporting Security Issues

If you discover a security vulnerability:

1. **Do not** create a public issue
2. Email details to: security@luminaguard.dev
3. Include:
   - Steps to reproduce
   - Expected vs actual behavior
   - Environment details (OS, version, etc.)
4. We will respond within 48 hours

## Compliance

This network isolation implementation is designed to meet:

- **SOC 2**: Security and availability requirements
- **PCI DSS**: Network segmentation requirements
- **NIST 800-53**: System and communications protection (SC-7)

For specific compliance requirements, contact: compliance@luminaguard.dev
