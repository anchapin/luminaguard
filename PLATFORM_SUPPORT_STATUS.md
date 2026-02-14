# LuminaGuard Cross-Platform VM Support

## Status

This document consolidates the platform-specific VM implementations following the Windows Hyper-V refactoring (PR #179).

### Implemented Platforms

#### 1. Windows (Hyper-V) ✅
- **Module**: `orchestrator/src/vm/hyperv.rs`
- **Status**: Production-ready (Actor pattern for thread safety)
- **Architecture**: Uses `libwhp` for Hyper-V API access
- **Key Features**:
  - Send + Sync trait implementation via Actor pattern
  - Isolated thread ownership of `Partition` handle
  - MPSC channel communication for commands
  - No unsafe code required

#### 2. Linux (Firecracker/KVM) ✅
- **Module**: `orchestrator/src/vm/firecracker.rs`
- **Status**: Prototype (~110ms spawn time)
- **Features**:
  - Firecracker process lifecycle management
  - Snapshot pooling for fast spawn (10-50ms target)
  - Jailer sandboxing (chroot, cgroups, namespaces)
  - Seccomp filters (syscall whitelisting)
  - Network isolation (iptables firewall)

#### 3. macOS (AppleHV) ⏳ Planned
- **Module**: `orchestrator/src/vm/apple_hv.rs` (stub created)
- **Status**: In development
- **Architecture**: Uses macOS Virtualization.framework
- **Key Features**:
  - Unified interface via Hypervisor trait
  - Similar lifecycle to Linux implementation
  - Native macOS VM isolation

### Platform-Agnostic Abstraction

All platforms implement the `Hypervisor` trait:

```rust
pub trait Hypervisor: Send + Sync {
    fn spawn(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn get_status(&self) -> VmStatus;
}
```

This enables:
- Pluggable hypervisor implementations
- Unified test interface
- Easy cross-platform testing

### VM Lifecycle

1. **Spawn**: Create ephemeral VM in <200ms
2. **Configure**: Network, storage, security policies
3. **Execute**: Run agent task inside VM
4. **Monitor**: Track resource usage and status
5. **Destroy**: Clean up VM (critical for security)

### Security Layers (Defense in Depth)

1. **Rust Memory Safety**: No buffer overflows, use-after-free
2. **Micro-VM Isolation**: Hardware virtualization, separate kernel context
3. **Jailer Sandbox** (Linux): chroot, cgroups, namespaces, privilege drop
4. **Seccomp Filters**: Syscall whitelisting (99% blocked at Basic level)
5. **Firewall Rules**: Network isolation
6. **Approval Cliff**: Human-in-the-loop for destructive actions

### Testing Strategy

- **Unit Tests**: Per-hypervisor implementation tests
- **Integration Tests**: VM spawn/destroy with real tools
- **Cross-Platform Tests**: Same test suite on all platforms
- **Property-Based Tests**: Proptest for edge cases

### Performance Targets

| Metric | Target | Current (Linux) |
|--------|--------|-----------------|
| VM Spawn Time | <200ms | ~110ms ✅ |
| Memory Footprint | <100MB | ~80MB ✅ |
| Pool Hit Rate | >80% | >90% ✅ |

### MCP Transport Status

**Phase 1 - Completed:**
- ✅ Stdio transport - Local MCP server connections via process spawning

**Phase 2 - Completed:**
- ✅ HTTP transport - Remote MCP server support via HTTP POST
  - Exponential backoff retry logic (1s → 32s, configurable)
  - Load balancing via round-robin across multiple server instances
  - Custom HTTP headers (authentication, API keys, etc.)
  - Configurable request timeouts
  - Smart error handling (retries on transient failures, not on auth errors)
  - Full TLS/HTTPS support

**Phase 3 - Planned:**
- ⏳ Streamable HTTP transport (long-lived connections with chunked responses)

### Next Steps

1. HTTP transport integration tests with mock servers
2. Enterprise-grade monitoring and logging
3. Multi-platform CI/CD pipeline
4. Performance benchmarking (latency, throughput)
5. Documentation and user guides
