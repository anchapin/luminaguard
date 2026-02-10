# Snapshot Pool Feature

**Status**: Phase 1 Complete (Prototype) | Phase 2 In Progress (Firecracker Integration)

## What is the Snapshot Pool?

The snapshot pool enables fast VM spawning (target: 10-50ms) by maintaining a pool of pre-created Firecracker VM snapshots. This is a significant improvement over cold boot times (~110ms).

## Quick Start

```rust
use ironclaw_orchestrator::vm;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Optional: Warm up pool on startup
    vm::warmup_pool().await?;

    // Spawn VM (automatically uses pool if available)
    let handle = vm::spawn_vm("my-task").await?;

    // Use VM for your task...

    // Destroy VM when done
    vm::destroy_vm(handle).await?;

    Ok(())
}
```

## Configuration

Set environment variables to customize the pool:

```bash
export IRONCLAW_POOL_SIZE=5                    # Number of snapshots (1-20)
export IRONCLAW_SNAPSHOT_REFRESH_SECS=3600     # Refresh interval (seconds)
export IRONCLAW_SNAPSHOT_PATH=/var/lib/ironclaw/snapshots
```

## Run Demo

```bash
cd orchestrator
cargo run --example snapshot_pool_demo
```

Expected output:
```
=== Snapshot Pool Demo ===

1. Warming up snapshot pool...
   ✓ Pool warmed up in 178.742µs

2. Pool Statistics:
   - Current size: 3/3
   - Oldest snapshot: 0s old
   - Newest snapshot: 0s old

3. Spawning VMs from pool:
   - VM 1: vm-from-snapshot-pool-snapshot-abc (spawned in 15.2µs)
   - VM 2: vm-from-snapshot-pool-snapshot-def (spawned in 12.8µs)
   ...
```

## Documentation

- **[Integration Guide](snapshot-pool-guide.md)** - Complete usage guide
- **[PR Summary](snapshot-pool-pr-summary.md)** - Implementation details

## Testing

```bash
# Run all VM module tests
cargo test --lib vm::

# Run benchmarks
cargo bench --bench snapshot_pool

# Check code quality
cargo clippy -- -D warnings
cargo fmt
```

## Architecture

```
┌─────────────────────────────────────────┐
│           spawn_vm() API                │
│  ┌───────────────────────────────────┐  │
│  │  Try pool (10-50ms)              │  │
│  │  ↓ Fallback                      │  │
│  │  Cold boot (110ms)               │  │
│  └───────────────────────────────────┘  │
└──────────────┬──────────────────────────┘
               │
     ┌─────────▼─────────┐
     │  Snapshot Pool    │
     │  (5 VMs ready)    │
     └─────────┬─────────┘
               │
     ┌─────────▼─────────┐
     │  Snapshot Module │
     │  - create()      │
     │  - load()        │
     │  - delete()      │
     └─────────┬─────────┘
               │
     ┌─────────▼─────────┐
     │  Disk Storage    │
     │  /var/lib/...    │
     └──────────────────┘
```

## Performance Targets

| Metric | Target | Phase 1 | Phase 2 |
|--------|--------|---------|---------|
| Snapshot load | <20ms | Placeholder | 🔄 In Progress |
| VM spawn from pool | 10-50ms | N/A | 🔄 In Progress |
| Cold boot fallback | ~110ms | ✅ Baseline | ✅ Complete |
| Pool warmup | <30s | ✅ <1ms | 🔄 Measuring |
| Memory overhead | <100MB/snapshot | 🔄 TBD | 🔄 In Progress |

## Current Status

### ✅ Phase 1 Complete (Prototype)

- [x] Snapshot module with create/load/delete APIs
- [x] Pool module with round-robin allocation
- [x] Automatic refresh of stale snapshots
- [x] Environment variable configuration
- [x] Comprehensive unit tests (23 tests, all passing)
- [x] Zero clippy warnings
- [x] Documentation and examples

### 🔄 Phase 2 In Progress (Firecracker Integration)

- [ ] Real snapshot creation via Firecracker API
- [ ] Real snapshot loading via Firecracker API
- [ ] Performance benchmarks with real VMs
- [ ] Memory usage tracking
- [ ] Prometheus metrics

## How It Works

1. **Initialization**: Pool creates 5 snapshots on startup (configurable)
2. **Acquisition**: `spawn_vm()` tries pool first, falls back to cold boot
3. **Allocation**: Round-robin ensures fair distribution
4. **Refresh**: Stale snapshots (>1 hour) automatically refreshed
5. **Cleanup**: VMs are ephemeral, destroyed after use

## Files Added

```
orchestrator/
├── src/vm/
│   ├── snapshot.rs      # Snapshot operations (365 lines)
│   ├── pool.rs          # Pool management (415 lines)
│   └── mod.rs           # Updated with pool integration (284 lines)
├── benches/
│   └── snapshot_pool.rs # Performance benchmarks
├── examples/
│   └── snapshot_pool_demo.rs
└── Cargo.toml           # Updated dependencies

docs/
├── snapshot-pool-readme.md       # This file
├── snapshot-pool-guide.md        # Integration guide
└── snapshot-pool-pr-summary.md   # Implementation details
```

## Contributing

When modifying the snapshot pool:

1. Run tests: `cargo test --lib vm::`
2. Run benchmarks: `cargo bench --bench snapshot_pool`
3. Check coverage: Ensure >90% coverage
4. Zero clippy warnings: `cargo clippy -- -D warnings`
5. Update docs: Document any API changes

## License

MIT License - See LICENSE file for details.

## Next Steps

See [snapshot-pool-guide.md](snapshot-pool-guide.md) for detailed implementation guide and Phase 2 roadmap.
