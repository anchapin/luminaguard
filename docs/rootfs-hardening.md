# Root Filesystem Hardening

## Overview

LuminaGuard implements **read-only root filesystem hardening** to prevent agents from modifying system files while providing a writable workspace for agent operations. This is a critical security feature that ensures:

1. **System immutability**: Agents cannot modify system binaries, libraries, or configuration
2. **Privilege escalation prevention**: No possibility of persistent backdoors or rootkits
3. **Ephemeral execution**: All changes are lost on VM shutdown (the "infected computer no longer exists")

## Architecture

### Security Model

```
┌─────────────────────────────────────────┐
│   VM Boot Process                     │
│                                     │
│  1. Mount rootfs READ-ONLY           │
│     └─> SquashFS or ext4 (ro)      │
│                                     │
│  2. Mount overlay layer              │
│     ├─> tmpfs (ephemeral, default) │
│     └─> ext4 (persistent, optional) │
│                                     │
│  3. Activate OverlayFS              │
│     ├─> Base: Read-only rootfs     │
│     ├─> Upper: Writable overlay     │
│     └─> Work: Overlay workdir      │
│                                     │
│  4. Pivot root to OverlayFS        │
│     └─> /rom → Original rootfs (ro)│
│     └─> / → Merged filesystem      │
└─────────────────────────────────────────┘
```

### Directory Layout (Inside VM)

```
/                    # OverlayFS mount (read/write view)
├── bin/            # From base rootfs (read-only through OverlayFS)
├── lib/            # From base rootfs (read-only through OverlayFS)
├── usr/            # From base rootfs (read-only through OverlayFS)
├── etc/            # From base rootfs (read-only through OverlayFS)
├── home/agent/     # Writable workspace (in overlay layer)
├── tmp/            # Writable (in overlay layer)
└── rom/            # Original rootfs (read-only bind mount)
```

## Configuration

### Default: Ephemeral tmpfs Overlay

```rust
use ironclaw_orchestrator::vm::{config::VmConfig, spawn_vm};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Default config uses tmpfs overlay (ephemeral)
    let config = VmConfig::new("my-task".to_string());

    let handle = spawn_vm("my-task", &config).await?;

    // ... agent runs ...

    destroy_vm(handle).await?;
    Ok(())
}
```

**Characteristics:**
- Root filesystem: Read-only
- Overlay: tmpfs (in-memory)
- Data persistence: None (lost on shutdown)
- Performance: Fastest (no disk I/O)
- Use case: Standard agent tasks (recommended)

### Optional: Persistent ext4 Overlay

```rust
use ironclaw_orchestrator::vm::config::VmConfig;
use ironclaw_orchestrator::vm::rootfs::{RootfsConfig, OverlayType};

let config = VmConfig {
    rootfs_config: Some(RootfsConfig::with_persistent_overlay(
        "./resources/rootfs.squashfs".to_string(),
        "./overlays/my-task-overlay.ext4".to_string(),
        512, // 512 MB
    )),
    ..VmConfig::default()
};
```

**Characteristics:**
- Root filesystem: Read-only
- Overlay: ext4 image file (on disk)
- Data persistence: Across VM reboots
- Performance: Slower than tmpfs (disk I/O)
- Use case: Debugging, long-running workloads

## Implementation Details

### overlay-init Script

The `overlay-init` script is placed in the root filesystem at `/sbin/overlay-init` and is executed as the init process (PID 1) by the kernel.

**Key steps:**
1. Determine overlay type from kernel boot parameter (`overlay_root=ram` or `overlay_root=vdb`)
2. Mount overlay layer (tmpfs or ext4 device)
3. Create overlay directories (root, work)
4. Mount OverlayFS with lowerdir=/, upperdir=/overlay/root
5. Pivot root to make overlay the new filesystem root
6. Execute real init process to continue boot

Source: /home/alexc/Projects/luminaguard/orchestrator/resources/overlay-init

### Firecracker Drive Configuration

When rootfs hardening is enabled, Firecracker receives two drives:

```json
{
  "drives": [
    {
      "drive_id": "rootfs",
      "path_on_host": "./resources/rootfs.squashfs",
      "is_root_device": true,
      "is_read_only": true
    },
    {
      "drive_id": "overlayfs",
      "path_on_host": "./overlays/my-task-overlay.ext4",
      "is_root_device": false,
      "is_read_only": false
    }
  ]
}
```

### Kernel Boot Parameters

```
console=ttyS0 reboot=k panic=1 pci=off overlay_root=ram init=/sbin/overlay-init
                                  └──────────┬──────────┘ └───────┬────────┘
                                             │                      │
                                      Use tmpfs overlay      Use custom init
```

For persistent overlay:
```
overlay_root=vdb init=/sbin/overlay-init
     └────┬────┘
          │
  Use /dev/vdb (second drive)
```

## Security Guarantees

### What Agents CAN Do

- Write files to `/home/agent`, `/tmp`, and other writable locations
- Install packages to writable directories (if package manager supports it)
- Modify configuration files in overlay layer
- Execute code and create processes

### What Agents CANNOT Do

- Modify system binaries in `/bin`, `/usr/bin`, etc.
- Change system libraries in `/lib`, `/usr/lib`
- Alter system configuration in `/etc` (only copies in overlay)
- Install persistent backdoors or rootkits
- Modify kernel or bootloader
- Make changes that survive VM shutdown (with tmpfs overlay)

### Security Invariants

1. **Read-only base**: Root filesystem is mounted read-only (`is_read_only: true`)
2. **Immutable system**: System files cannot be changed through OverlayFS
3. **Ephemeral by default**: tmpfs overlay ensures no persistence
4. **No privilege escalation**: Even if agent gains root, system files remain immutable

## Rootfs Preparation

### Converting ext4 to SquashFS

To convert an existing ext4 rootfs to SquashFS (recommended for production):

```bash
# 1. Mount the ext4 rootfs
mkdir /tmp/rootfs-mount
sudo mount rootfs.ext4 /tmp/rootfs-mount

# 2. Add overlay-init script
sudo cp orchestrator/resources/overlay-init /tmp/rootfs-mount/sbin/
sudo chmod +x /tmp/rootfs-mount/sbin/overlay-init

# 3. Create overlay directories
sudo mkdir -p /tmp/rootfs-mount/overlay/{root,work}
sudo mkdir -p /tmp/rootfs-mount/rom

# 4. Create SquashFS image
sudo mksquashfs /tmp/rootfs-mount rootfs.squashfs -noappend

# 5. Unmount
sudo umount /tmp/rootfs-mount
```

**Benefits of SquashFS:**
- Compressed (smaller file size)
- Faster boot (no filesystem check)
- Natively read-only (enforced at filesystem level)
- Better performance (optimized for read access)

## Testing

### Unit Tests

```bash
cd orchestrator
cargo test vm::rootfs
```

Tests verify:
- Rootfs config validation
- Boot arguments generation
- Overlay type serialization
- Security invariants (always read-only)

### Integration Tests

```bash
cd orchestrator
cargo test --features vm-prototype
```

Tests verify:
- VM spawns with read-only rootfs
- Overlay filesystem is mounted
- Agents can write to workspace
- Agents cannot modify system files

### Manual Testing

To manually test rootfs hardening:

```bash
# Spawn VM with rootfs hardening
cd orchestrator
cargo run -- spawn --task-id test-rootfs

# In another terminal, connect to VM console
# Try to modify system file (should fail)
echo "malicious" > /bin/ls  # Should fail: read-only filesystem

# Write to workspace (should succeed)
echo "test" > /home/agent/test.txt  # Should succeed

# Shutdown VM
cargo run -- destroy --vm-id test-rootfs
```

## Troubleshooting

### "Failed to mount overlayfs"

**Symptom:** VM fails to boot, error in logs about overlay mount

**Solutions:**
1. Check overlay-init script exists in rootfs: `/sbin/overlay-init`
2. Verify kernel boot args include `init=/sbin/overlay-init`
3. For ext4 overlay, verify drive is attached and has ext4 filesystem

### "Permission denied writing to /home/agent"

**Symptom:** Agent cannot write to workspace

**Solutions:**
1. Verify overlay filesystem is mounted: `mount | grep overlay`
2. Check permissions: `ls -la /home/agent`
3. Ensure overlay directories exist: `/overlay/root` and `/overlay/work`

### "Changes persist after VM shutdown" (tmpfs)

**Symptom:** With tmpfs overlay, changes survive VM restart

**Solutions:**
1. This should NOT happen with tmpfs - check overlay type
2. Verify `overlay_root=ram` in boot args
3. Check if using ext4 overlay by mistake

## Performance

### Boot Time Impact

| Configuration | Boot Time | Notes |
|--------------|------------|-------|
| ext4 (rw) | ~110ms | Baseline |
| SquashFS (ro) + tmpfs | ~115ms | +5ms (acceptable) |
| SquashFS (ro) + ext4 overlay | ~130ms | +20ms (persistent storage) |

### Disk Space Usage

| Configuration | Disk Usage (per VM) | Notes |
|--------------|---------------------|-------|
| ext4 (rw) | ~500 MB | Full copy |
| SquashFS (ro) + tmpfs | ~200 MB | Shared base, no overlay file |
| SquashFS (ro) + ext4 overlay | ~200 MB + overlay size | Sparse overlay file |

## Future Enhancements

- [ ] Automatic SquashFS conversion in RootfsManager
- [ ] Overlay image pooling for faster VM spawn
- [ ] Support for multiple overlay layers
- [ ] Integration with snapshot system (Phase 3)
- [ ] Metrics on overlay usage (disk space, inodes)

## References

- [Firecracker OverlayFS Discussion](https://github.com/firecracker-microvm/firecracker/discussions/3061)
- [Scaling Firecracker with OverlayFS](https://e2b.dev/blog/scaling-firecracker-using-overlayfs-to-save-disk-space)
- [overlayfs-in-firecracker GitHub](https://github.com/njapke/overlayfs-in-firecracker)
- [Firecracker Containerd](https://github.com/firecracker-microvm/firecracker-containerd)
