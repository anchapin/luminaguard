# Firecracker Test Resources

This directory contains test resources for Firecracker and Jailer integration tests.

## Required Resources

For integration tests to run, you need the following resources:

1. **vmlinux** - Kernel image (e.g., v4.14 or newer)
2. **rootfs.ext4** - Root filesystem image

## Downloading Resources

### Option 1: Use Firecracker official resources

```bash
# Download kernel image
wget https://s3.amazonaws.com/spec.ccfc.min/img/kiwi-no-dash.img \
  -O vmlinux.bin

# Download rootfs (Ubuntu)
wget https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/0-rootfs.ext4 \
  -O rootfs.ext4
```

### Option 2: Build from source

See Firecracker documentation for building custom kernels:
https://github.com/firecracker-microvm/firecracker/blob/main/docs/rootfs-and-kernel-setup.md

### Option 3: Use pre-built snapshots

```bash
# Download pre-built snapshot
wget https://s3.amazonaws.com/spec.ccfc.min/snapshots/kernel.json \
  -O kernel.json
wget https://s3.amazonaws.com/spec.ccfc.min/snapshots/rootfs.json \
  -O rootfs.json
wget https://s3.amazonaws.com/spec.ccfc.min/snapshots/snapshot-file \
  -O snapshot-file
```

## Permissions

Make sure the resources are readable:

```bash
chmod 644 vmlinux rootfs.ext4
```

## Integration Tests

Integration tests require:
- Root privileges (for namespace/cgroup operations)
- `jailer` binary in `/usr/local/bin/jailer`
- `firecracker` binary in `/usr/local/bin/firecracker`
- Valid `vmlinux` and `rootfs.ext4` files in this directory

To run integration tests:

```bash
# As root
sudo cargo test --lib vm::jailer -- --ignored
```

## Skipping Integration Tests

If you don't have resources or root access, integration tests will automatically skip.
Unit tests (which don't require resources or root) will always run.

## Minimal Testing

For basic testing without full VM resources, you can test just the jailer binary execution:

```bash
cargo test --lib vm::jailer::tests::tests::test_real_jailer_execution
```

This test validates that:
- Jailer binary exists and is executable
- Jailer `--help` command works
- Jailer `--version` command works
- Jailer rejects invalid arguments

No root privileges or VM resources required for this test.
