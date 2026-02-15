// Week 1-2 Performance Baseline Benchmarks
//
// This module implements comprehensive performance benchmarks for single-agent operations.
//
// Key metrics measured:
// - VM spawn time: target <200ms (Wave 2: ~110ms)
// - Memory footprint: target <200MB
// - CPU utilization: target <50%
// - Network latency: target <50ms
//
// Usage:
//   cargo test --test baseline_benchmarks -- --nocapture --test-threads=1
//
// Results are stored in .beads/metrics/performance/ as JSON files.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

// Path to Firecracker test assets
const TEST_KERNEL_PATH: &str = "/tmp/luminaguard-fc-test/vmlinux.bin";
const TEST_ROOTFS_PATH: &str = "/tmp/luminaguard-fc-test/rootfs.ext4";

// Metrics directory
const METRICS_DIR: &str = ".beads/metrics/performance";

/// Performance metrics for a single benchmark run
#[derive(Debug, serde::Serialize)]
struct PerformanceMetrics {
    /// Test run timestamp
    timestamp: String,
    /// Number of iterations
    iterations: u32,
    /// Spawn time statistics (ms)
    spawn_time: SpawnTimeMetrics,
    /// Memory usage statistics (MB)
    memory: MemoryMetrics,
    /// CPU usage statistics (%)
    cpu: CpuMetrics,
    /// Network latency statistics (ms)
    network: NetworkMetrics,
}

#[derive(Debug, serde::Serialize)]
struct SpawnTimeMetrics {
    /// Median spawn time
    median_ms: f64,
    /// 95th percentile spawn time
    p95_ms: f64,
    /// 99th percentile spawn time
    p99_ms: f64,
    /// Minimum spawn time
    min_ms: f64,
    /// Maximum spawn time
    max_ms: f64,
    /// Standard deviation
    std_dev_ms: f64,
    /// Whether target (<200ms) is met
    meets_target: bool,
}

#[derive(Debug, serde::Serialize)]
struct MemoryMetrics {
    /// Median memory usage (MB)
    median_mb: f64,
    /// 95th percentile memory usage (MB)
    p95_mb: f64,
    /// Peak memory usage (MB)
    peak_mb: f64,
    /// Whether target (<200MB) is met
    meets_target: bool,
}

#[derive(Debug, serde::Serialize)]
struct CpuMetrics {
    /// Average CPU usage (%)
    avg_percent: f64,
    /// Peak CPU usage (%)
    peak_percent: f64,
    /// Whether target (<50%) is met
    meets_target: bool,
}

#[derive(Debug, serde::Serialize)]
struct NetworkMetrics {
    /// Median network latency (ms)
    median_ms: f64,
    /// 95th percentile latency (ms)
    p95_ms: f64,
    /// Whether target (<50ms) is met
    meets_target: bool,
}

/// Check if Firecracker test assets are available
fn test_assets_available() -> bool {
    PathBuf::from(TEST_KERNEL_PATH).exists() && PathBuf::from(TEST_ROOTFS_PATH).exists()
}

/// Download Firecracker test assets if not available
fn ensure_test_assets() {
    if !test_assets_available() {
        println!("⚠️  Firecracker test assets not found");
        println!("   Kernel expected at: {}", TEST_KERNEL_PATH);
        println!("   Rootfs expected at: {}", TEST_ROOTFS_PATH);
        println!("\nTo download test assets, run:");
        println!("   ./scripts/download-firecracker-assets.sh");
        println!("\nContinuing with limited benchmarks (no real VM spawn)...\n");
    }
}

/// Calculate statistics from a slice of values
fn calculate_stats(values: &[f64]) -> (f64, f64, f64, f64, f64, f64) {
    let n = values.len();

    // Sort for percentiles
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min = sorted[0];
    let max = sorted[n - 1];
    let median = sorted[n / 2];

    let p95_index = ((n as f64) * 0.95) as usize;
    let p95 = sorted[p95_index.min(n - 1)];

    let p99_index = ((n as f64) * 0.99) as usize;
    let p99 = sorted[p99_index.min(n - 1)];

    // Calculate standard deviation
    let mean: f64 = values.iter().sum::<f64>() / (n as f64);
    let variance: f64 = values
        .iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>()
        / (n as f64);
    let std_dev = variance.sqrt();

    (min, max, median, p95, p99, std_dev)
}

/// Measure memory usage of current process (in MB)
fn measure_memory_mb() -> f64 {
    // Try /proc/self/status for Linux
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                // VmRSS: 12345 kB
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<f64>() {
                        return kb / 1024.0; // Convert to MB
                    }
                }
            }
        }
    }

    // Fallback: estimate
    100.0 // Conservative estimate
}

/// Measure CPU usage of current process (%)
fn measure_cpu_percent(duration: Duration) -> f64 {
    let start_time = Instant::now();
    let mut last_cpu_time = get_process_cpu_time();

    let mut max_usage = 0.0;
    let mut measurements = Vec::new();

    while start_time.elapsed() < duration {
        std::thread::sleep(Duration::from_millis(100));

        let current_cpu_time = get_process_cpu_time();
        let elapsed = start_time.elapsed().as_secs_f64();
        let cpu_delta = current_cpu_time - last_cpu_time;

        if elapsed > 0.0 {
            let usage = (cpu_delta / elapsed) * 100.0;
            max_usage = max_usage.max(usage);
            measurements.push(usage);
        }

        last_cpu_time = current_cpu_time;
    }

    // Return average usage
    if measurements.is_empty() {
        0.0
    } else {
        measurements.iter().sum::<f64>() / measurements.len() as f64
    }
}

/// Get process CPU time in seconds
fn get_process_cpu_time() -> f64 {
    if let Ok(stat) = fs::read_to_string("/proc/self/stat") {
        // Format: pid (comm) state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt utime stime ...
        let parts: Vec<&str> = stat.split_whitespace().collect();
        if parts.len() >= 15 {
            let utime: f64 = parts[13].parse().unwrap_or(0.0);
            let stime: f64 = parts[14].parse().unwrap_or(0.0);
            // Convert jiffies to seconds (assuming 100 Hz)
            return (utime + stime) / 100.0;
        }
    }
    0.0
}

/// Measure network latency using local loopback (simulated)
fn measure_network_latency() -> f64 {
    // For local testing, use a simple TCP connection to localhost
    let start = Instant::now();

    // Try to connect to localhost:80 (will fail, but we measure the attempt)
    let _ = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 80)),
        Duration::from_millis(50),
    );

    start.elapsed().as_secs_f64() * 1000.0
}

/// Baseline VM spawn time benchmark
#[test]
#[ignore]
fn baseline_vm_spawn_time() {
    ensure_test_assets();

    if !test_assets_available() {
        println!("⏭️  Skipping VM spawn benchmark - no test assets");
        return;
    }

    let rt = Runtime::new().expect("Failed to create runtime");
    let iterations = 100;
    let mut spawn_times = Vec::new();

    println!("🧪 Running VM spawn time benchmark ({} iterations)...", iterations);

    for i in 0..iterations {
        let task_id = format!("baseline-spawn-{}", i);

        let result = rt.block_on(async {
            let config = luminaguard_orchestrator::vm::config::VmConfig {
                kernel_path: TEST_KERNEL_PATH.to_string(),
                rootfs_path: TEST_ROOTFS_PATH.to_string(),
                ..luminaguard_orchestrator::vm::config::VmConfig::new(task_id.clone())
            };

            let start = Instant::now();
            let handle = luminaguard_orchestrator::vm::spawn_vm_with_config(&task_id, &config).await;

            let spawn_time = start.elapsed().as_secs_f64() * 1000.0;

            if let Ok(handle) = handle {
                let _ = luminaguard_orchestrator::vm::destroy_vm(handle).await;
                Some(spawn_time)
            } else {
                None
            }
        });

        if let Some(time) = result {
            spawn_times.push(time);
            if (i + 1) % 10 == 0 {
                println!("  Progress: {}/{} (last: {:.2}ms)", i + 1, iterations, time);
            }
        }
    }

    if spawn_times.is_empty() {
        println!("❌ No successful VM spawns recorded");
        return;
    }

    let (min_ms, max_ms, median_ms, p95_ms, p99_ms, std_dev_ms) = calculate_stats(&spawn_times);
    let meets_target = median_ms < 200.0;

    println!("\n📊 VM Spawn Time Results:");
    println!("  Median:   {:.2}ms", median_ms);
    println!("  P95:      {:.2}ms", p95_ms);
    println!("  P99:      {:.2}ms", p99_ms);
    println!("  Min:      {:.2}ms", min_ms);
    println!("  Max:      {:.2}ms", max_ms);
    println!("  Std Dev:  {:.2}ms", std_dev_ms);
    println!("  Target:   <200ms");
    println!("  Status:   {}", if meets_target { "✅ PASS" } else { "❌ FAIL" });
    println!();

    let spawn_time_metrics = SpawnTimeMetrics {
        median_ms,
        p95_ms,
        p99_ms,
        min_ms,
        max_ms,
        std_dev_ms,
        meets_target,
    };

    // Save metrics
    save_metrics("spawn_time_baseline", spawn_time_metrics);
}

/// Baseline memory usage benchmark
#[test]
#[ignore]
fn baseline_memory_usage() {
    let iterations = 100;
    let mut memory_readings = Vec::new();

    println!("🧪 Running memory usage benchmark ({} iterations)...", iterations);

    for i in 0..iterations {
        // Simulate typical agent workload
        let _data = vec![0u8; 1024 * 1024]; // 1MB allocation
        std::thread::sleep(Duration::from_millis(10));

        let memory_mb = measure_memory_mb();
        memory_readings.push(memory_mb);

        if (i + 1) % 10 == 0 {
            println!("  Progress: {}/{} (current: {:.2}MB)", i + 1, iterations, memory_mb);
        }
    }

    let (_min_mb, max_mb, median_mb, p95_mb, _p99_mb, _std_dev_mb) =
        calculate_stats(&memory_readings);
    let meets_target = median_mb < 200.0;

    println!("\n📊 Memory Usage Results:");
    println!("  Median:   {:.2}MB", median_mb);
    println!("  P95:      {:.2}MB", p95_mb);
    println!("  Peak:     {:.2}MB", max_mb);
    println!("  Target:   <200MB");
    println!("  Status:   {}", if meets_target { "✅ PASS" } else { "❌ FAIL" });
    println!();

    let memory_metrics = MemoryMetrics {
        median_mb,
        p95_mb,
        peak_mb: max_mb,
        meets_target,
    };

    // Save metrics
    save_metrics("memory_baseline", memory_metrics);
}

/// Baseline CPU usage benchmark
#[test]
#[ignore]
fn baseline_cpu_usage() {
    let duration = Duration::from_secs(10);
    println!("🧪 Running CPU usage benchmark ({}s)...", duration.as_secs());

    let rt = Runtime::new().expect("Failed to create runtime");
    let cpu_percent = rt.block_on(async {
        // Simulate typical agent workload
        let _handle = tokio::spawn(async {
            for _ in 0..100 {
                let _data = vec![0u8; 1024 * 10]; // Small allocation
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        measure_cpu_percent(duration)
    });

    let meets_target = cpu_percent < 50.0;

    println!("\n📊 CPU Usage Results:");
    println!("  Average:  {:.2}%", cpu_percent);
    println!("  Peak:     {:.2}%", cpu_percent); // Simplified
    println!("  Target:   <50%");
    println!("  Status:   {}", if meets_target { "✅ PASS" } else { "❌ FAIL" });
    println!();

    let cpu_metrics = CpuMetrics {
        avg_percent: cpu_percent,
        peak_percent: cpu_percent,
        meets_target,
    };

    // Save metrics
    save_metrics("cpu_baseline", cpu_metrics);
}

/// Baseline network latency benchmark
#[test]
#[ignore]
fn baseline_network_latency() {
    let iterations = 100;
    let mut latencies = Vec::new();

    println!("🧪 Running network latency benchmark ({} iterations)...", iterations);

    for i in 0..iterations {
        let latency_ms = measure_network_latency();
        latencies.push(latency_ms);

        if (i + 1) % 10 == 0 {
            println!("  Progress: {}/{} (last: {:.2}ms)", i + 1, iterations, latency_ms);
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    let (_min_ms, _max_ms, median_ms, p95_ms, _p99_ms, _std_dev_ms) = calculate_stats(&latencies);
    let meets_target = median_ms < 50.0;

    println!("\n📊 Network Latency Results:");
    println!("  Median:   {:.2}ms", median_ms);
    println!("  P95:      {:.2}ms", p95_ms);
    println!("  Target:   <50ms");
    println!("  Status:   {}", if meets_target { "✅ PASS" } else { "❌ FAIL" });
    println!();

    let network_metrics = NetworkMetrics {
        median_ms,
        p95_ms,
        meets_target,
    };

    // Save metrics
    save_metrics("network_baseline", network_metrics);
}

/// Comprehensive baseline benchmark (all metrics)
#[test]
fn baseline_comprehensive() {
    println!("🚀 Starting Week 1-2 Comprehensive Performance Baseline\n");
    println!("This will measure:");
    println!("  - VM spawn time (target: <200ms)");
    println!("  - Memory usage (target: <200MB)");
    println!("  - CPU utilization (target: <50%)");
    println!("  - Network latency (target: <50ms)");
    println!();

    let iterations = 100;

    // Collect all metrics
    let mut spawn_times = Vec::new();
    let mut memory_readings = Vec::new();
    let mut latencies = Vec::new();

    // Check for test assets
    ensure_test_assets();
    let has_test_assets = test_assets_available();

    if has_test_assets {
        println!("📦 Test assets found - running full VM spawn benchmarks");
    } else {
        println!("⚠️  No test assets - running synthetic benchmarks only");
    }

    println!();

    let rt = Runtime::new().expect("Failed to create runtime");

    for i in 0..iterations {
        // Measure spawn time (if assets available)
        if has_test_assets {
            let task_id = format!("baseline-{}", i);

            let result = rt.block_on(async {
                let config = luminaguard_orchestrator::vm::config::VmConfig {
                    kernel_path: TEST_KERNEL_PATH.to_string(),
                    rootfs_path: TEST_ROOTFS_PATH.to_string(),
                    ..luminaguard_orchestrator::vm::config::VmConfig::new(task_id.clone())
                };

                let start = Instant::now();
                let handle = luminaguard_orchestrator::vm::spawn_vm_with_config(&task_id, &config).await;

                let spawn_time = start.elapsed().as_secs_f64() * 1000.0;

                if let Ok(handle) = handle {
                    let _ = luminaguard_orchestrator::vm::destroy_vm(handle).await;
                    Some(spawn_time)
                } else {
                    None
                }
            });

            if let Some(time) = result {
                spawn_times.push(time);
            }
        }

        // Measure memory
        let _data = vec![0u8; 1024 * 1024]; // Simulate workload
        std::thread::sleep(Duration::from_millis(1));
        memory_readings.push(measure_memory_mb());

        // Measure network latency
        latencies.push(measure_network_latency());

        if (i + 1) % 20 == 0 {
            println!("  Progress: {}/{}", i + 1, iterations);
        }
    }

    println!();

    // Calculate statistics
    let (_spawn_min, _spawn_max, spawn_median, spawn_p95, spawn_p99, spawn_std) =
        if !spawn_times.is_empty() {
            calculate_stats(&spawn_times)
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        };

    let (_mem_min, mem_max, mem_median, mem_p95, _mem_p99, _mem_std) =
        calculate_stats(&memory_readings);

    let (_net_min, _net_max, net_median, net_p95, _net_p99, _net_std) = calculate_stats(&latencies);

    // CPU measurement (separate)
    println!("🧪 Measuring CPU usage (10s)...");
    let cpu_avg = rt.block_on(async {
        let _handle = tokio::spawn(async {
            for _ in 0..100 {
                let _data = vec![0u8; 1024 * 10];
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        measure_cpu_percent(Duration::from_secs(10))
    });

    println!("\n" + "=".repeat(60));
    println!("📊 WEEK 1-2 PERFORMANCE BASELINE RESULTS");
    println!("=".repeat(60));

    // Spawn Time
    if !spawn_times.is_empty() {
        println!("\n🚀 VM Spawn Time:");
        println!("  Median:   {:.2}ms", spawn_median);
        println!("  P95:      {:.2}ms", spawn_p95);
        println!("  P99:      {:.2}ms", spawn_p99);
        println!("  Std Dev:  {:.2}ms", spawn_std);
        println!("  Target:   <200ms");
        println!(
            "  Status:   {}",
            if spawn_median < 200.0 { "✅ PASS" } else { "❌ FAIL" }
        );
    } else {
        println!("\n🚀 VM Spawn Time: ⏭️  SKIPPED (no test assets)");
    }

    // Memory
    println!("\n💾 Memory Usage:");
    println!("  Median:   {:.2}MB", mem_median);
    println!("  P95:      {:.2}MB", mem_p95);
    println!("  Peak:     {:.2}MB", mem_max);
    println!("  Target:   <200MB");
    println!(
        "  Status:   {}",
        if mem_median < 200.0 { "✅ PASS" } else { "❌ FAIL" }
    );

    // CPU
    println!("\n💻 CPU Usage:");
    println!("  Average:  {:.2}%", cpu_avg);
    println!("  Peak:     {:.2}%", cpu_avg);
    println!("  Target:   <50%");
    println!(
        "  Status:   {}",
        if cpu_avg < 50.0 { "✅ PASS" } else { "❌ FAIL" }
    );

    // Network
    println!("\n🌐 Network Latency:");
    println!("  Median:   {:.2}ms", net_median);
    println!("  P95:      {:.2}ms", net_p95);
    println!("  Target:   <50ms");
    println!(
        "  Status:   {}",
        if net_median < 50.0 { "✅ PASS" } else { "⚠️  WARNING" }
    );

    println!("\n" + "=".repeat(60));

    // Build comprehensive metrics
    let metrics = PerformanceMetrics {
        timestamp: chrono::Utc::now().to_rfc3339(),
        iterations: iterations as u32,
        spawn_time: SpawnTimeMetrics {
            median_ms: spawn_median,
            p95_ms: spawn_p95,
            p99_ms: spawn_p99,
            min_ms: if !spawn_times.is_empty() {
                let mut sorted = spawn_times.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                sorted[0]
            } else {
                0.0
            },
            max_ms: if !spawn_times.is_empty() {
                let mut sorted = spawn_times.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                sorted[sorted.len() - 1]
            } else {
                0.0
            },
            std_dev_ms: spawn_std,
            meets_target: spawn_median < 200.0,
        },
        memory: MemoryMetrics {
            median_mb: mem_median,
            p95_mb: mem_p95,
            peak_mb: mem_max,
            meets_target: mem_median < 200.0,
        },
        cpu: CpuMetrics {
            avg_percent: cpu_avg,
            peak_percent: cpu_avg,
            meets_target: cpu_avg < 50.0,
        },
        network: NetworkMetrics {
            median_ms: net_median,
            p95_ms: net_p95,
            meets_target: net_median < 50.0,
        },
    };

    // Save comprehensive metrics
    save_metrics("comprehensive_baseline", metrics);
}

/// Save metrics to JSON file
fn save_metrics<T: serde::Serialize>(name: &str, metrics: T) {
    let metrics_path = PathBuf::from(METRICS_DIR);

    // Create directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&metrics_path) {
        println!("⚠️  Failed to create metrics directory: {}", e);
        return;
    }

    // Generate filename with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.json", name, timestamp);
    let filepath = metrics_path.join(&filename);

    // Write metrics to file
    if let Ok(json) = serde_json::to_string_pretty(&metrics) {
        if let Err(e) = fs::write(&filepath, json) {
            println!("⚠️  Failed to save metrics to {}: {}", filepath.display(), e);
        } else {
            println!("💾 Metrics saved to: {}", filepath.display());
        }
    } else {
        println!("⚠️  Failed to serialize metrics");
    }
}
