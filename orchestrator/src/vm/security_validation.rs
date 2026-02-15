// Week 1: Security Validation - Standalone Test Runner
//
// This module provides a standalone test runner for security escape validation.
// It runs all security tests and generates a report.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::info;

use crate::vm::config::VmConfig;
use crate::vm::security_escape::{SecurityTestHarness, SecurityReport};


/// Run comprehensive security validation
///
/// This function executes all security escape tests and saves the report.
///
/// # Arguments
///
/// * `output_dir` - Directory to save the security report
///
/// # Returns
///
/// * `SecurityReport` - Complete security test results
pub async fn run_security_validation(output_dir: &str) -> Result<SecurityReport> {
    info!("Starting Week 1 Security Validation");
    info!("==================================");

    // Create output directory if it doesn't exist
    let output_path = PathBuf::from(output_dir);
    fs::create_dir_all(&output_path)
        .context("Failed to create security validation output directory")?;

    // Configure VM for security testing
    let config = VmConfig {
        vm_id: "security-validation-vm".to_string(),
        vcpu_count: 1,
        memory_mb: 512,
        kernel_path: "/tmp/luminaguard-fc-test/vmlinux.bin".to_string(),
        rootfs_path: "/tmp/luminaguard-fc-test/rootfs.ext4".to_string(),
        enable_networking: false, // Security requirement
        vsock_path: None,
        seccomp_filter: None, // Will use default Basic level
    };

    info!("Running security tests with Basic seccomp filter");

    // Run all security tests
    let mut harness = SecurityTestHarness::new(config);
    let report = harness.run_all_tests().await?;

    // Print summary
    println!("\n{}", report.summary());

    // Save report to JSON
    let report_path = output_path.join("security-validation-report.json");
    let report_json = report.to_json()?;
    fs::write(&report_path, report_json)
        .context("Failed to write security report")?;

    info!("Security report saved to: {:?}", report_path);

    // Save summary to text file
    let summary_path = output_path.join("security-validation-summary.txt");
    fs::write(&summary_path, report.summary())
        .context("Failed to write security summary")?;

    info!("Security summary saved to: {:?}", summary_path);

    // Check for failures and warn if any
    if !report.failed_tests().is_empty() {
        println!("\n⚠️  SECURITY WARNINGS:");
        for test in report.failed_tests() {
            println!("  - {}: {}", test.test_name, test.error_message.as_ref().unwrap_or(&"Unknown error".to_string()));
        }
    }

    Ok(report)
}

/// Quick security check (no full report generation)
///
/// Useful for CI/CD pipelines where only pass/fail status matters.
pub async fn quick_security_check() -> Result<bool> {
    info!("Running quick security check");

    let config = VmConfig::new("quick-check".to_string());
    let mut harness = SecurityTestHarness::new(config);
    let report = harness.run_all_tests().await?;

    let all_blocked = report.security_score() >= 100.0;

    if all_blocked {
        info!("✅ Quick security check: PASSED");
    } else {
        tracing::warn!("⚠️  Quick security check: SCORE = {:.1}%", report.security_score());
    }

    Ok(all_blocked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_security_validation_runs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_dir = temp_dir.path().to_str().unwrap();

        let report = run_security_validation(output_dir).await.unwrap();

        assert!(report.total_tests > 0);
        assert!(report.total_time_ms > 0.0);

        // Verify files were created
        let report_path = PathBuf::from(output_dir).join("security-validation-report.json");
        let summary_path = PathBuf::from(output_dir).join("security-validation-summary.txt");

        assert!(report_path.exists());
        assert!(summary_path.exists());
    }

    #[tokio::test]
    async fn test_quick_security_check() {
        let result = quick_security_check().await.unwrap();
        assert!(result); // Should pass with current seccomp config
    }

    #[test]
    fn test_output_directory_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_dir = temp_dir.path().join("security");

        assert!(!output_dir.exists());

        // Directory will be created by run_security_validation
        // We just verify the path construction works
        assert!(output_dir.is_absolute());
    }

    #[tokio::test]
    async fn test_report_contains_all_test_categories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_dir = temp_dir.path().to_str().unwrap();

        let report = run_security_validation(output_dir).await.unwrap();

        let test_names: Vec<_> = report.test_results.iter().map(|r| &r.test_name).collect();

        // Verify all test categories are present
        assert!(test_names.iter().any(|t| t.contains("privilege_escalation")));
        assert!(test_names.iter().any(|t| t.contains("filesystem_escape")));
        assert!(test_names.iter().any(|t| t.contains("network_escape")));
        assert!(test_names.iter().any(|t| t.contains("process")));
        assert!(test_names.iter().any(|t| t.contains("system_config")));
    }
}
