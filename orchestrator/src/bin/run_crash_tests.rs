// VM Crash Test Runner
//
// This binary runs comprehensive crash testing for Week 1-2 of the
// reliability testing plan.
//
// Usage:
//   cargo run --bin run_crash_tests -- <kernel_path> <rootfs_path> [results_path]
//
// Example:
//   cargo run --bin run_crash_tests -- \
//     /tmp/luminaguard-fc-test/vmlinux.bin \
//     /tmp/luminaguard-fc-test/rootfs.ext4 \
//     .beads/metrics/reliability

use anyhow::Result;
use std::env;
use std::path::PathBuf;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use luminaguard_orchestrator::vm::reliability::CrashTestHarness;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!(
            "Usage: {} <kernel_path> <rootfs_path> [results_path]",
            args[0]
        );
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  kernel_path   Path to VM kernel image (vmlinux.bin)");
        eprintln!("  rootfs_path   Path to VM root filesystem (rootfs.ext4)");
        eprintln!(
            "  results_path  Path to store test results (default: .beads/metrics/reliability)"
        );
        eprintln!();
        eprintln!("Example:");
        eprintln!(
            "  {} /tmp/kernel /tmp/rootfs .beads/metrics/reliability",
            args[0]
        );
        std::process::exit(1);
    }

    let kernel_path = args[1].clone();
    let rootfs_path = args[2].clone();
    let results_path = if args.len() > 3 {
        PathBuf::from(&args[3])
    } else {
        PathBuf::from(".beads/metrics/reliability")
    };

    // Validate paths
    if !std::path::Path::new(&kernel_path).exists() {
        error!("Kernel not found at: {}", kernel_path);
        std::process::exit(1);
    }

    if !std::path::Path::new(&rootfs_path).exists() {
        error!("Rootfs not found at: {}", rootfs_path);
        std::process::exit(1);
    }

    info!("=== LuminaGuard VM Crash Testing Suite ===");
    info!("Kernel: {}", kernel_path);
    info!("Rootfs: {}", rootfs_path);
    info!("Results: {:?}", results_path);
    info!("");

    // Create test harness
    let harness = CrashTestHarness::new(kernel_path, rootfs_path, results_path)?;

    // Run all tests
    let start_time = std::time::Instant::now();

    let results = harness.run_all_tests().await?;

    let elapsed = start_time.elapsed();
    info!("");
    info!("All tests completed in {:?}", elapsed);
    info!("");

    // Generate summary
    let summary = harness.generate_summary(&results);
    println!("{}", summary);

    // Count failures
    let failed = results.iter().filter(|r| !r.passed).count();

    if failed > 0 {
        warn!("{} test(s) failed", failed);
        std::process::exit(1);
    }

    info!("All tests passed!");
    Ok(())
}
