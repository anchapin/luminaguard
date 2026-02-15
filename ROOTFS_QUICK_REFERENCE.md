# Rootfs Hardening - Quick Reference

## Overview

LuminaGuard uses read-only root filesystems with writable overlay layers to prevent agents from modifying system files while providing a workspace for operations.

## Key Concepts

### Read-Only Rootfs
- Base filesystem is mounted read-only (SquashFS or ext4 with ro flag)
- Agents cannot modify system binaries, libraries, or configuration
- Provides protection against privilege escalation and persistent malware

### Overlay Layers
- **Tmpfs (default):** In-memory, ephemeral, fastest, data lost on shutdown
- **Ext4 (persistent):** On-disk, survives reboots, for debugging/long-running tasks

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

## Usage Examples

### Basic Usage (Default - Tmpfs Overlay)
```rust
use luminaguard_orchestrator::vm::{spawn_vm, config::VmConfig};

let config = VmConfig::new("my-task".to_string());
// Uses tmpfs overlay by default (ephemeral)
let handle = spawn_vm("my-task").await?;
```

### Persistent Ext4 Overlay
```rust
use luminaguard_orchestrator::vm::{spawn_vm_with_config, config::VmConfig};
use luminaguard_orchestrator::vm::rootfs::{RootfsConfig, OverlayType};

let rootfs_config = RootfsConfig::with_persistent_overlay(
    "./resources/rootfs.squashfs".to_string(),
    "./overlays/my-task-overlay.ext4".to_string(),
    512, // 512 MB
);

let config = VmConfig {
    rootfs_config: Some(rootfs_config),
    ..VmConfig::new("my-task".to_string())
};

let handle = spawn_vm_with_config("my-task", &config).await?;
```

### Custom Rootfs Path
```rust
let config = VmConfig {
    rootfs_config: Some(RootfsConfig::new(
        "/custom/path/rootfs.squashfs".to_string()
    )),
    ..VmConfig::new("my-task".to_string())
};
```

## Rootfs Management

### Create Minimal Alpine Linux Image
```rust
use luminaguard_orchestrator::vm::rootfs::RootfsManager;
use std::path::PathBuf;

let output_path = PathBuf::from("./rootfs-minimal.ext4");
RootfsManager::create_minimal_alpine(&output_path)?;
```

### Convert ext4 to SquashFS
```rust
use luminaguard_orchestrator::vm::rootfs::{RootfsConfig, RootfsManager};

let config = RootfsConfig::new("./rootfs.ext4".to_string());
let manager = RootfsManager::new(config);
let squashfs_path = manager.prepare()?; // Automatically converts
```

### Remove Unused Utilities
```rust
RootfsManager::remove_unused_utilities(&rootfs_path)?;
```

### Verify Rootfs Security
```rust
let is_secure = RootfsManager::verify_minimal_rootfs(&rootfs_path)?;
assert!(is_secure, "Rootfs failed security check");
```

## Seccomp Audit Whitelisting

### Basic Usage (Default Whitelist)
```rust
use luminaguard_orchestrator::vm::seccomp::{SeccompFilter, SeccompLevel};

let filter = SeccompFilter::new(SeccompLevel::Basic);
// Audits security-sensitive syscalls: execve, mount, setuid, etc.
```

### Add Custom Audit Syscalls
```rust
let filter = SeccompFilter::new(SeccompLevel::Basic)
    .add_audit_whitelist("open")
    .add_audit_whitelist("close");
```

### Audit All Blocked Syscalls
```rust
let filter = SeccompFilter::new(SeccompLevel::Basic)
    .audit_all_blocked(true); // Log EVERY blocked syscall
```

### Check if Syscall Should Be Audited
```rust
if filter.should_audit("execve") {
    // Log security event
}
```

### Get Complete Whitelist
```rust
let whitelist = filter.get_audit_whitelist();
// Includes defaults + custom additions
```

## Configuration Options

### RootfsConfig
```rust
pub struct RootfsConfig {
    pub rootfs_path: String,        // Path to rootfs image
    pub read_only: bool,             // MUST be true (security)
    pub overlay_type: OverlayType,    // Tmpfs or Ext4
    pub overlay_path: Option<String>, // For Ext4 overlay
    pub overlay_size_mb: Option<u32>, // For Ext4 overlay creation
}
```

### VmConfig Integration
```rust
pub struct VmConfig {
    pub rootfs_config: Option<RootfsConfig>,  // Optional rootfs config
    // ...
}
```

### Helper Methods
```rust
config.has_rootfs_hardening()           // Check if hardening enabled
config.effective_rootfs_path()          // Get rootfs path
config.get_boot_args()                  // Get kernel boot args
config.get_overlay_drive()              // Get overlay drive config (if ext4)
```

## Kernel Boot Parameters

### With Rootfs Hardening (Default)
```
console=ttyS0 reboot=k panic=1 pci=off overlay_root=ram init=/sbin/overlay-init
```

### With Persistent Ext4 Overlay
```
console=ttyS0 reboot=k panic=1 pci=off overlay_root=vdb init=/sbin/overlay-init
```

### Without Hardening
```
console=ttyS0 reboot=k panic=1 pci=off
```

## Security Guarantees

### What Agents CAN Do
- ✅ Write files to `/home/agent`, `/tmp` (in overlay)
- ✅ Execute code and create processes
- ✅ Modify configuration copies (in overlay)
- ✅ Install packages to writable directories

### What Agents CANNOT Do
- ❌ Modify system binaries (`/bin`, `/usr/bin`)
- ❌ Change system libraries (`/lib`, `/usr/lib`)
- ❌ Alter system configuration (`/etc` - only copies in overlay)
- ❌ Install persistent backdoors or rootkits
- ❌ Modify kernel or bootloader
- ❌ Make changes survive VM shutdown (with tmpfs)

## Performance

### Boot Time Impact
| Configuration | Boot Time | Notes |
|--------------|------------|-------|
| ext4 (rw) | ~110ms | Baseline |
| SquashFS (ro) + tmpfs | ~115ms | +5ms (default) |
| SquashFS (ro) + ext4 overlay | ~130ms | +20ms (persistent) |

### Disk Space
| Configuration | Disk Usage |
|--------------|-------------|
| ext4 (rw) | ~500 MB |
| SquashFS (ro) + tmpfs | ~200 MB |
| SquashFS (ro) + ext4 overlay | ~200 MB + overlay |

## Testing

### Run Rootfs Tests
```bash
cd orchestrator
cargo test --lib vm::rootfs
```

### Run Seccomp Tests
```bash
cargo test --lib vm::seccomp
```

### Run All VM Tests
```bash
cargo test --lib vm::
```

## Troubleshooting

### "Root filesystem not found"
- Ensure `rootfs_path` points to existing file
- Check file permissions
- Try absolute path

### "Root filesystem MUST be read-only"
- Security requirement: cannot disable read-only
- Always use `read_only: true`

### "Failed to mount ext4 image"
- Requires `sudo` for loop device mounting
- Check if `mksquashfs` is installed: `which mksquashfs`
- Install: `apt-get install squashfs-tools`

### "mksquashfs not found"
```bash
# Debian/Ubuntu
sudo apt-get install squashfs-tools

# Alpine
apk add squashfs-tools

# Fedora/RHEL
sudo dnf install squashfs-tools
```

### Overlay Not Working
- Check kernel boot args include `init=/sbin/overlay-init`
- Verify overlay-init script exists in rootfs
- Check overlay type: `overlay_root=ram` or `overlay_root=vdb`

## Best Practices

1. **Use tmpfs overlay** for standard agent tasks (ephemeral by design)
2. **Keep rootfs read-only** - security requirement
3. **Use SquashFS** for production (smaller, faster, read-only enforced)
4. **Remove unused utilities** to minimize attack surface
5. **Verify rootfs security** after creating images
6. **Enable audit logging** to monitor security events
7. **Use overlay only when needed** (debugging, long-running workloads)

## Migration Guide

### From ext4 to SquashFS
```rust
// Old way (still works for backward compatibility)
let config = VmConfig {
    rootfs_path: "./rootfs.ext4".to_string(),
    // ...
};

// New way (recommended)
let config = VmConfig {
    rootfs_config: Some(RootfsConfig::new(
        "./rootfs.squashfs".to_string()  // Use SquashFS
    )),
    // ...
};
```

### Enable Rootfs Hardening (Already Default)
```rust
// Already enabled by default in VmConfig::new()
let config = VmConfig::new("task".to_string());
assert!(config.has_rootfs_hardening()); // true
```

## Further Reading

- **Full Documentation:** `/docs/rootfs-hardening.md`
- **Implementation Summary:** `/IMPLEMENTATION_SUMMARY.md`
- **Architecture:** `/docs/architecture/architecture.md`
- **Overlay-Init Script:** `/orchestrator/resources/overlay-init`
