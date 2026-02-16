// Chaos Engineering Test Runner
//
// This binary runs comprehensive chaos testing for Week 5-6 of the
// performance validation plan: resilience under VM kills, network partitions,
// CPU throttling, memory pressure, and mixed chaos scenarios.
//
// Usage:
//   cargo run --release --bin run_chaos_tests -- [results_path]
//
// Example:
//   cargo run --release --bin run_chaos_tests -- .beads/metrics/performance

use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

use luminaguard_orchestrator::vm::chaos::ChaosTestHarness;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .init();

    // Parse arguments
    let args: Vec<String> = env::args().collect();
    let results_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from(".beads/metrics/performance")
    };

    // Create results directory if it doesn't exist
    fs::create_dir_all(&results_path)?;

    info!("=== LuminaGuard Chaos Engineering Test Suite ===");
    info!("Testing: Resilience under VM kills, network partitions, CPU throttling, memory pressure");
    info!("Results directory: {:?}", results_path);
    info!("");

    // Create test harness
    let start_time = std::time::Instant::now();

    info!("Initializing chaos test harness...");
    let harness = ChaosTestHarness::new(
        Default::default(),
        Default::default(),
        results_path.clone(),
    )?;

    info!("Running chaos engineering tests...");
    let results = harness.run_all_tests().await?;

    let elapsed = start_time.elapsed();
    info!("");
    info!("All chaos tests completed in {:.2}s", elapsed.as_secs_f64());
    info!("");

    // Display summary
    println!("\n{}", "=".repeat(80));
    println!("CHAOS ENGINEERING TEST REPORT");
    println!("{}", "=".repeat(80));

    let total_tests = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total_tests - passed;

    println!("Total Tests:           {}", total_tests);
    println!("Passed:                {}", passed);
    println!("Failed:                {}", failed);
    println!("Total Duration:        {:.2}s", elapsed.as_secs_f64());
    println!("{}", "=".repeat(80));

    // Detailed results by test type
    println!("\nDETAILED RESULTS:\n");
    for result in &results {
        let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
        println!("{} {}", status, result.test_name);
        println!("  Type:                 {:?}", result.test_type);
        println!("  Duration:             {:.0}ms", result.duration_ms);
        println!("  MTTR:                 {:.0}ms (Mean Time To Recovery)", result.mttr_ms);
        println!("  Success Rate:         {:.1}%", result.success_rate);
        println!("  Cascade Failures:     {}", result.cascade_failures);
        println!("  Graceful Degradation: {}", result.graceful_degradation);

        println!("  Metrics:");
        println!(
            "    Operations:         {}/{} successful",
            result.metrics.successful_operations, result.metrics.total_operations
        );
        println!(
            "    Recovery Events:    {} (avg: {:.0}ms, max: {:.0}ms, min: {:.0}ms)",
            result.metrics.recovery_events,
            result.metrics.avg_recovery_time_ms,
            result.metrics.max_recovery_time_ms,
            result.metrics.min_recovery_time_ms
        );
        println!("    Chaos Events:       {}", result.metrics.chaos_events);
        println!(
            "    Resource Pressure:  {} events",
            result.metrics.resource_pressure_events
        );

        if let Some(err) = &result.error_message {
            println!("  Error: {}", err);
        }
        println!();
    }

    // Save report to JSON
    let report_json = serde_json::to_string_pretty(&results)?;
    let report_path = results_path.join("chaos_engineering_report.json");
    fs::write(&report_path, report_json)?;
    info!("Report saved to: {:?}", report_path);

    // Save summary to text file
    let mut summary = format!(
        "LuminaGuard Chaos Engineering Test Report\n\
         ==========================================\n\
         Generated: {}\n\
         \n\
         Test Results:\n\
         - Total Tests: {}\n\
         - Passed: {}\n\
         - Failed: {}\n\
         - Total Time: {:.2}s\n\
         \n\
         Test Details:\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        total_tests,
        passed,
        failed,
        elapsed.as_secs_f64()
    );

    for result in &results {
        summary.push_str(&format!(
            "\n{}: {}\n",
            if result.passed { "✓" } else { "✗" },
            result.test_name
        ));
        summary.push_str(&format!("  Type: {:?}\n", result.test_type));
        summary.push_str(&format!("  Duration: {:.0}ms\n", result.duration_ms));
        summary.push_str(&format!("  MTTR: {:.0}ms\n", result.mttr_ms));
        summary.push_str(&format!("  Success Rate: {:.1}%\n", result.success_rate));
        summary.push_str(&format!("  Cascade Failures: {}\n", result.cascade_failures));
        summary.push_str(&format!(
            "  Operations: {}/{}\n",
            result.metrics.successful_operations, result.metrics.total_operations
        ));
        if let Some(err) = &result.error_message {
            summary.push_str(&format!("  Error: {}\n", err));
        }
    }

    let summary_path = results_path.join("chaos_engineering_summary.txt");
    fs::write(&summary_path, summary)?;
    info!("Summary saved to: {:?}", summary_path);

    // Determine exit code
    let exit_code = if failed > 0 {
        println!("\n❌ {} test(s) failed", failed);
        1
    } else {
        println!("\n✅ All tests passed!");
        0
    };

    println!();
    std::process::exit(exit_code);
}
