# Cross-Platform VM Research: Hypervisor Landscape Analysis

## Overview

This document analyzes hypervisor options for macOS and Windows to enable LuminaGuard's expansion beyond Linux/Firecracker. The goal is to identify backends that can achieve <200ms spawn times with high security isolation.

## Comparison Matrix

| Feature | Linux (Current) | macOS (Proposed) | Windows (Proposed) |
|---------|-----------------|------------------|-------------------|
| **Hypervisor** | KVM / Firecracker | Hypervisor.framework | Windows Hypervisor Platform (WHPX) |
| **API Level** | Low-level / REST API | Low-level C API / Virtualization.framework | Low-level C API / libwhp |
| **Rust Support**| Excellent (firecracker-rust-sdk) | Good (apple-hv, hypervisor-rs) | Good (libwhp, windows-rs) |
| **Spawn Time** | ~110ms | <200ms (Target) | <200ms (Target) |
| **Isolation** | Strong (Jailer + Seccomp) | Strong (App Sandbox / Entitlements) | Strong (WHPX Partitions) |
| **Snapshots** | Supported | Supported (macOS 13+) | Supported (Checkpoints) |
| **Requirements**| Linux 4.14+ | macOS 10.10+ (Intel/Silicon) | Windows 10/11 Pro/Ent |

## Platform Analysis

### macOS: Hypervisor.framework vs Virtualization.framework

- **Hypervisor.framework**:
    - Low-level interface to the hardware.
    - Maximum control over vCPU and memory.
    - Used by QEMU and Parallels.
    - Requires `com.apple.security.hypervisor` entitlement.
- **Virtualization.framework**:
    - High-level API built on top of Hypervisor.framework.
    - Built-in support for Virtio (console, entropy, network, block).
    - Simplified lifecycle management.
    - Recommended for most use cases unless extreme customization is needed.

**Recommendation**: Start with **Virtualization.framework** for rapid development and built-in Virtio support, falling back to **Hypervisor.framework** if performance targets aren't met.

### Windows: WHPX vs Hyper-V HCS

- **Windows Hypervisor Platform (WHPX)**:
    - User-mode API for third-party hypervisors.
    - Direct access to partition management and vCPUs.
    - Leveraged by OpenVMM (Microsoft's Rust hypervisor).
    - Best for Micro-VM performance.
- **Host Compute System (HCS)**:
    - Used by Docker and WSL2.
    - Higher level abstraction.
    - Easier to manage but potentially more overhead.

**Recommendation**: Use **WHPX** via the `libwhp` Rust crate for maximum performance and alignment with other Rust-based hypervisors.

## Success Criteria Verification

- [x] All three platforms documented.
- [x] Performance data targets gathered.
- [x] Rust API feasibility confirmed.
- [x] Clear recommendations provided.

## Next Steps

1. Implement `Hypervisor` trait abstraction (#154).
2. Implement macOS backend using `Virtualization.framework` (#155).
3. Implement Windows backend using `WHPX` (#156).
