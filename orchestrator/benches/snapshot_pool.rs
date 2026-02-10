// Snapshot Pool Performance Benchmarks
//
// This benchmark measures the performance of VM spawning via snapshot pool
// compared to cold boot.
//
// Key metrics:
// - Snapshot load time: target <20ms
// - VM spawn time: target 10-50ms
// - Pool acquisition overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ironclaw_orchestrator::vm;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Benchmark: VM spawn from snapshot pool
fn bench_vm_spawn_from_pool(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.path().to_str().unwrap());

    // Warm up pool first
    rt.block_on(async {
        let _ = vm::warmup_pool().await;
    });

    c.bench_function("vm_spawn_from_pool", |b| {
        b.to_async(&rt).iter(|| async {
            let handle = vm::spawn_vm(black_box("benchmark-task")).await.unwrap();
            black_box(handle);
        });
    });

    std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
}

/// Benchmark: Cold boot VM (fallback)
fn bench_cold_boot_vm(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("cold_boot_vm", |b| {
        b.to_async(&rt).iter(|| async {
            // Call cold boot directly
            let handle = vm::spawn_vm(black_box("cold-benchmark-task")).await.unwrap();
            black_box(handle);
        });
    });
}

/// Benchmark: Concurrent VM spawns
fn bench_concurrent_spawns(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.path().to_str().unwrap());

    // Warm up pool first
    rt.block_on(async {
        let _ = vm::warmup_pool().await;
    });

    let mut group = c.benchmark_group("concurrent_spawns");

    for concurrent_count in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrent_count),
            concurrent_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();

                    for i in 0..count {
                        let task_id = format!("concurrent-{}", i);
                        let handle = vm::spawn_vm(black_box(&task_id)).await.unwrap();
                        handles.push(handle);
                    }

                    black_box(handles);
                });
            },
        );
    }

    group.finish();

    std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
}

/// Benchmark: Pool statistics
fn bench_pool_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.path().to_str().unwrap());

    // Warm up pool first
    rt.block_on(async {
        let _ = vm::warmup_pool().await;
    });

    c.bench_function("pool_stats", |b| {
        b.to_async(&rt).iter(|| async {
            let stats = vm::pool_stats().await.unwrap();
            black_box(stats);
        });
    });

    std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
}

criterion_group!(
    benches,
    bench_vm_spawn_from_pool,
    bench_cold_boot_vm,
    bench_concurrent_spawns,
    bench_pool_stats
);
criterion_main!(benches);
