// Security Integration Test Runner
//
// This binary runs comprehensive security integration testing for Week 7-8 of the
// security validation plan: red-team simulation and chaos engineering tests.
//
// Usage:
//   cargo run --release --bin run_security_tests -- [results_path]
//
// Example:
//   cargo run --release --bin run_security_tests -- .beads/metrics/security

use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

use luminaguard_orchestrator::vm::security_integration_tests::IntegrationTestHarness;

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
        PathBuf::from(".beads/metrics/security")
    };

    // Create results directory if it doesn't exist
    fs::create_dir_all(&results_path)?;

    info!("=== LuminaGuard Security Integration Test Suite ===");
    info!("Testing: Red-Team Simulation & Chaos Engineering");
    info!("Results directory: {:?}", results_path);
    info!("");

    // Create and run test harness
    let start_time = std::time::Instant::now();

    info!("Initializing test harness...");
    let mut harness = IntegrationTestHarness::new();

    info!("Running all security integration tests...");
    let report = harness.run_all_tests();

    let elapsed = start_time.elapsed();
    info!("");
    info!("All tests completed in {:.2}s", elapsed.as_secs_f64());
    info!("");

    // Display summary
    println!("\n{}", "=".repeat(80));
    println!("SECURITY INTEGRATION TEST REPORT");
    println!("{}", "=".repeat(80));
    println!("Total Tests:       {}", report.total_tests);
    println!("Passed:            {}", report.passed_count);
    println!("Failed:            {}", report.failed_count);
    println!("Security Score:    {:.1}%", report.security_score);
    println!("Attack Block Rate: {:.1}%", report.attack_block_rate);
    println!("Total Time:        {:.2}ms", report.total_time_ms);
    println!("{}", "=".repeat(80));

    // Group results by category
    let mut categories: std::collections::HashMap<String, Vec<_>> =
        std::collections::HashMap::new();
    for result in &report.test_results {
        categories
            .entry(result.category.clone())
            .or_insert_with(Vec::new)
            .push(result);
    }

    println!("\nRESULTS BY CATEGORY:\n");
    for (category, results) in categories.iter() {
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;
        println!("{}:", category);
        println!("  Passed: {}/{}", passed, results.len());
        if failed > 0 {
            println!("  Failed: {}", failed);
        }
        for result in results {
            if !result.passed {
                println!("    - {} ({}ms)", result.test_name, result.execution_time_ms as i32);
                if let Some(err) = &result.error_message {
                    println!("      Error: {}", err);
                }
            }
        }
        println!();
    }

    // Save report to JSON
    let report_json = serde_json::to_string_pretty(&report)?;
    let report_path = results_path.join("security_integration_report.json");
    fs::write(&report_path, report_json)?;
    info!("Report saved to: {:?}", report_path);

    // Save summary to text file
    let mut summary = format!(
        "LuminaGuard Security Integration Test Report\n\
         =============================================\n\
         Generated: {}\n\
         \n\
         Test Results:\n\
         - Total Tests: {}\n\
         - Passed: {}\n\
         - Failed: {}\n\
         - Security Score: {:.1}%\n\
         - Attack Block Rate: {:.1}%\n\
         - Total Time: {:.2}ms\n\
         \n\
         Test Categories:\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        report.total_tests,
        report.passed_count,
        report.failed_count,
        report.security_score,
        report.attack_block_rate,
        report.total_time_ms
    );

    for (category, results) in categories.iter() {
        let passed = results.iter().filter(|r| r.passed).count();
        summary.push_str(&format!(
            "\n{}: {}/{} passed\n",
            category,
            passed,
            results.len()
        ));
        for result in results {
            summary.push_str(&format!(
                "  [{}] {} ({:.0}ms)\n",
                if result.passed { "✓" } else { "✗" },
                result.test_name,
                result.execution_time_ms
            ));
            if let Some(err) = &result.error_message {
                summary.push_str(&format!("      Error: {}\n", err));
            }
        }
    }

    let summary_path = results_path.join("security_integration_summary.txt");
    fs::write(&summary_path, summary)?;
    info!("Summary saved to: {:?}", summary_path);

    // Determine exit code
    let exit_code = if report.failed_count > 0 {
        println!("\n❌ {} test(s) failed", report.failed_count);
        1
    } else {
        println!("\n✅ All tests passed!");
        0
    };

    println!();
    std::process::exit(exit_code);
}
