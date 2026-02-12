# Firecracker Feasibility Prototype

## Overview

This is a **Quick 1-week prototype** to determine if Firecracker is viable for JIT Micro-VMs in IronClaw.

### GOAL
Test if Firecracker can actually boot VMs on the target system.

### OUTCOME
- **If fails** → Abandon JIT VM entirely (re-architect)
- **If passes** → Proceed to full Phase 3 validation (12-week program)

## Background

IronClaw's JIT Micro-VM feature requires:
- Spawn time: **<200ms** (target)
- Ephemeral: VM destroyed after task completion
- Security: Full isolation from host

Previous FPF analysis identified **H3: Snapshot Pool** as the predicted winner, achieving 10-50ms spawn time (4-20x better than target). However, this requires empirical validation.

## Running the Prototype

### 1. Build with Prototype Feature

```bash
cd orchestrator
cargo build --features vm-prototype
```

### 2. Run Feasibility Test

```bash
./target/debug/ironclaw test-vm-prototype
```

### 3. Read the Report

The test will output a detailed report with:
- Prerequisites check (Firecracker binary, KVM availability)
- Spawn time measurement
- Recommendation (Proceed/Abandon/Investigate)

## Understanding the Results

### ✅ Recommendation: PROCEED

Firecracker is viable for JIT Micro-VMs.

**Next steps:**
1. Implement Snapshot Pool (H3)
2. Validate 10-50ms spawn time with snapshots
3. Full 12-week validation program

### ❌ Recommendation: ABANDON

Firecracker is not viable on this system.

**Alternative approaches:**
1. Use container-based isolation (user namespaces)
2. Use WebAssembly (Wasmtime/Wasmer)
3. Accept host execution with approval cliff only

### ⚠️ Recommendation: INVESTIGATE

Partial success - needs more work.

**Issues to investigate:**
1. Spawn time optimization
2. Kernel/rootfs configuration
3. System compatibility issues

## Test Assets

The prototype expects kernel and rootfs files at:
- Kernel: `/tmp/ironclaw-fc-test/vmlinux.bin`
- Rootfs: `/tmp/ironclaw-fc-test/rootfs.ext4`

These assets are **not** automatically downloaded (prototype limitation).

### To create test assets:

1. **Download kernel:**
   ```bash
   mkdir -p /tmp/ironclaw-fc-test
   cd /tmp/ironclaw-fc-test

   # Download official Firecracker kernel
   wget https://s3.amazonaws.com/spec.ccfc.min/images/terraform/aws-k8s-1.23/test-1.23-x86_64/kernel.bin
   mv kernel.bin vmlinux.bin
   ```

2. **Create minimal rootfs:**
   ```bash
   # Create 64MB empty file
   dd if=/dev/zero of=rootfs.ext4 bs=1M count=64

   # Format as ext4
   mkfs.ext4 rootfs.ext4
   ```

## Technical Details

### What the Test Does

1. **Check prerequisites:**
   - Firecracker binary installed?
   - KVM module available?

2. **Prepare test assets:**
   - Create temp directory
   - Check for kernel and rootfs files

3. **Run spawn test:**
   - Create Unix socket for Firecracker API
   - Start Firecracker process
   - Send minimal VM configuration (1 vCPU, 256MB RAM)
   - Start VM instance
   - Measure spawn time
   - Shutdown VM

4. **Generate recommendation:**
   - Spawn time < 500ms: ✅ Proceed (excellent)
   - Spawn time < 2000ms: ✅ Proceed (acceptable)
   - Spawn time > 2000ms: ⚠️ Investigate (needs optimization)

### Architecture

```
orchestrator/src/vm/prototype/
├── mod.rs          # Main entry point, run_feasibility_test()
├── resources.rs    # Asset management (kernel, rootfs)
└── spawn_test.rs   # Actual Firecracker spawn test
```

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Spawn time (cold boot) | <200ms | Baseline target |
| Spawn time (snapshot) | 10-50ms | With H3: Snapshot Pool |
| Memory overhead | <100MB | Excluding agent payload |
| VM startup API calls | <5 | Initialize → Configure → Start |

## References

- Firecracker: https://github.com/firecracker-microvm/firecracker
- FPF Decision: `.quint/knowledge/L1/firecracker-snapshot-pool-*.md`
- AWS Lambda: Uses snapshot pool for <20ms cold starts (production proven)

## Troubleshooting

### "Firecracker binary not found"

Install Firecracker:
```bash
# Download release
wget https://github.com/firecracker-microvm/firecracker/releases/download/v1.7.0/firecracker-v1.7.0-x86_64
chmod +x firecracker-v1.7.0-x86_64
sudo mv firecracker-v1.7.0-x86_64 /usr/local/bin/firecracker
```

### "KVM not available"

Check if hardware virtualization is supported:
```bash
# Check CPU flags
grep -E 'vmx|svm' /proc/cpuinfo

# Check KVM module
lsmod | grep kvm

# Load KVM module (Intel)
sudo modprobe kvm_intel

# Load KVM module (AMD)
sudo modprobe kvm_amd
```

### "Test assets not ready"

Download/create assets as described in "Test Assets" section above.

## License

MIT
