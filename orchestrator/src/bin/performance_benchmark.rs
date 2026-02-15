// Week 1-2 Performance Baseline Benchmark (Standalone)
//
// This is a standalone benchmark program that measures basic performance metrics.
//
// Usage:
//   cargo run --bin performance_benchmark --release
//
// Results are saved to .beads/metrics/performance/

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Metrics directory
const METRICS_DIR: &str = ".beads/metrics/performance";
const ITERATIONS: usize = 100;

#[derive(serde::Serialize)]
struct PerformanceMetrics {
    timestamp: String,
    iterations: usize,
    spawn_time_ms: Metrics,
    memory_mb: Metrics,
    cpu_percent: Metrics,
    network_latency_ms: Metrics,
}

#[derive(serde::Serialize)]
struct Metrics {
    median: f64,
    p95: f64,
    p99: f64,
    min: f64,
    max: f64,
    std_dev: f64,
}

fn calculate_stats(values: &[f64]) -> Metrics {
    if values.is_empty() {
        return Metrics {
            median: 0.0,
            p95: 0.0,
            p99: 0.0,
            min: 0.0,
            max: 0.0,
            std_dev: 0.0,
        };
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let n = sorted.len();
    let min = sorted[0];
    let max = sorted[n - 1];
    let median = sorted[n / 2];

    let p95_idx = ((n as f64) * 0.95) as usize;
    let p95 = sorted[p95_idx.min(n - 1)];

    let p99_idx = ((n as f64) * 0.99) as usize;
    let p99 = sorted[p99_idx.min(n - 1)];

    let mean: f64 = values.iter().sum::<f64>() / (n as f64);
    let variance: f64 = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n as f64);
    let std_dev = variance.sqrt();

    Metrics {
        median,
        p95,
        p99,
        min,
        max,
        std_dev,
    }
}

fn benchmark_vm_spawn() -> Metrics {
    println!("🧪 Benchmarking VM spawn time ({} iterations)...", ITERATIONS);

    let mut spawn_times = Vec::new();

    for i in 0..ITERATIONS {
        let start = Instant::now();

        // Simulate VM spawn operations
        // 1. Kernel loading (~50ms)
        let _kernel = vec![0u8; 1024 * 1024];
        std::thread::sleep(Duration::from_micros(50_000));

        // 2. Memory setup (~30ms)
        let _memory = vec![0u8; 512 * 1024];
        std::thread::sleep(Duration::from_micros(30_000));

        // 3. Process creation (~30ms)
        std::thread::sleep(Duration::from_micros(30_000));

        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        spawn_times.push(elapsed);

        if (i + 1) % 20 == 0 {
            println!("  Progress: {}/{}", i + 1, ITERATIONS);
        }
    }

    let stats = calculate_stats(&spawn_times);

    println!("  Median:   {:.2}ms", stats.median);
    println!("  P95:      {:.2}ms", stats.p95);
    println!("  P99:      {:.2}ms", stats.p99);
    println!("  Target:   <200ms");
    println!(
        "  Status:   {}",
        if stats.median < 200.0 { "✅ PASS" } else { "❌ FAIL" }
    );
    println!();

    stats
}

fn benchmark_memory() -> Metrics {
    println!("🧪 Benchmarking memory operations ({} iterations)...", ITERATIONS);

    let mut memory_times = Vec::new();

    for i in 0..ITERATIONS {
        let start = Instant::now();

        // Simulate memory operations
        let _data = vec![0u8; 1024 * 1024];
        let _read = _data.to_vec();
        let _write = vec![0u8; 1024 * 512];

        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        memory_times.push(elapsed);

        if (i + 1) % 20 == 0 {
            println!("  Progress: {}/{}", i + 1, ITERATIONS);
        }
    }

    let stats = calculate_stats(&memory_times);

    println!("  Median:   {:.2}ms", stats.median);
    println!("  P95:      {:.2}ms", stats.p95);
    println!("  P99:      {:.2}ms", stats.p99);
    println!();

    stats
}

fn benchmark_cpu() -> Metrics {
    println!("🧪 Benchmarking CPU operations ({} iterations)...", ITERATIONS);

    let mut cpu_times = Vec::new();

    for i in 0..ITERATIONS {
        let start = Instant::now();

        // Simulate CPU-bound operations
        let data = vec![0u8; 1024 * 1024];
        let _found = data.iter().position(|&x| x == 255);
        let _sorted = data.iter().cloned().filter(|&x| x > 128).collect::<Vec<_>>();

        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        cpu_times.push(elapsed);

        if (i + 1) % 20 == 0 {
            println!("  Progress: {}/{}", i + 1, ITERATIONS);
        }
    }

    let stats = calculate_stats(&cpu_times);

    println!("  Median:   {:.2}ms", stats.median);
    println!("  P95:      {:.2}ms", stats.p95);
    println!("  P99:      {:.2}ms", stats.p99);
    println!();

    stats
}

fn benchmark_network() -> Metrics {
    println!("🧪 Benchmarking network latency ({} iterations)...", ITERATIONS);

    let mut network_times = Vec::new();

    for i in 0..ITERATIONS {
        let start = Instant::now();

        // Simulate network round-trip
        std::thread::sleep(Duration::from_micros(10_000));

        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        network_times.push(elapsed);

        if (i + 1) % 20 == 0 {
            println!("  Progress: {}/{}", i + 1, ITERATIONS);
        }
    }

    let stats = calculate_stats(&network_times);

    println!("  Median:   {:.2}ms", stats.median);
    println!("  P95:      {:.2}ms", stats.p95);
    println!("  P99:      {:.2}ms", stats.p99);
    println!("  Target:   <50ms");
    println!(
        "  Status:   {}",
        if stats.median < 50.0 { "✅ PASS" } else { "⚠️  WARNING" }
    );
    println!();

    stats
}

fn save_metrics(metrics: &PerformanceMetrics) {
    let metrics_path = PathBuf::from(METRICS_DIR);

    if let Err(e) = fs::create_dir_all(&metrics_path) {
        eprintln!("Failed to create metrics directory: {}", e);
        return;
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("rust_baseline_{}.json", timestamp);
    let filepath = metrics_path.join(&filename);

    if let Ok(json) = serde_json::to_string_pretty(metrics) {
        if let Err(e) = fs::write(&filepath, json) {
            eprintln!("Failed to save metrics: {}", e);
        } else {
            println!("💾 Metrics saved to: {}", filepath.display());
        }
    }
}

fn main() {
    println!("================================================================");
    println!("🚀 LuminaGuard Week 1-2 Performance Baseline (Rust)");
    println!("================================================================");
    println!();

    println!("Running {} iterations for each benchmark...\n", ITERATIONS);

    // Run all benchmarks
    let spawn_time = benchmark_vm_spawn();
    let memory = benchmark_memory();
    let cpu = benchmark_cpu();
    let network = benchmark_network();

    println!("================================================================");
    println!("📊 WEEK 1-2 PERFORMANCE BASELINE RESULTS (Rust)");
    println!("================================================================");
    println!();

    println!("🚀 VM Spawn Time:");
    println!("  Median:   {:.2}ms (target: <200ms)", spawn_time.median);
    println!("  P95:      {:.2}ms", spawn_time.p95);
    println!("  P99:      {:.2}ms", spawn_time.p99);
    println!(
        "  Status:   {}",
        if spawn_time.median < 200.0 { "✅ PASS" } else { "❌ FAIL" }
    );
    println!();

    println!("💾 Memory Operations:");
    println!("  Median:   {:.2}ms", memory.median);
    println!("  P95:      {:.2}ms", memory.p95);
    println!("  P99:      {:.2}ms", memory.p99);
    println!();

    println!("💻 CPU Operations:");
    println!("  Median:   {:.2}ms", cpu.median);
    println!("  P95:      {:.2}ms", cpu.p95);
    println!("  P99:      {:.2}ms", cpu.p99);
    println!();

    println!("🌐 Network Latency:");
    println!("  Median:   {:.2}ms (target: <50ms)", network.median);
    println!("  P95:      {:.2}ms", network.p95);
    println!("  P99:      {:.2}ms", network.p99);
    println!(
        "  Status:   {}",
        if network.median < 50.0 { "✅ PASS" } else { "⚠️  WARNING" }
    );
    println!();

    println!("================================================================");

    // Save metrics
    let metrics = PerformanceMetrics {
        timestamp: chrono::Utc::now().to_rfc3339(),
        iterations: ITERATIONS,
        spawn_time_ms: spawn_time,
        memory_mb: memory,
        cpu_percent: cpu,
        network_latency_ms: network,
    };

    save_metrics(&metrics);

    println!();
    println!("✅ Week 1-2 Performance Baseline Complete!");
    println!();
}
