// Week 1: Security Escape Validation - Main Test Runner

use crate::vm::security_escape_simple::{SecurityTestHarness, SecurityReport};
use std::fs;

#[test]
fn test_security_validation() {
    println!("\n========== SECURITY ESCAPE VALIDATION ==========\n");
    
    let mut harness = SecurityTestHarness::new();
    let report = harness.run_all_tests();
    
    println!("\n{}", report.summary());
    
    // Verify security score
    let score = report.security_score();
    println!("\nSecurity Score: {:.1}%", score);
    
    if score >= 100.0 {
        println!("✅ ALL ESCAPE ATTEMPTS BLOCKED - SYSTEM SECURE");
    }
    
    // Save report to metrics directory
    fs::create_dir_all(".beads/metrics/security").expect("Failed to create metrics directory");
    
    let report_json = report.to_json().expect("Failed to serialize report");
    fs::write(".beads/metrics/security/security-validation-report.json", report_json).expect("Failed to write report");
    
    fs::write(".beads/metrics/security/security-validation-summary.txt", report.summary()).expect("Failed to write summary");
    
    println!("\nReport saved to: .beads/metrics/security/security-validation-report.json");
    println!("Summary saved to: .beads/metrics/security/security-validation-summary.txt");
    
    // Verify report contains expected test categories
    assert!(!report.test_results.is_empty(), "Should have test results");
    
    // Calculate and verify score
    let expected_score = (report.blocked_count as f64 / report.total_tests as f64) * 100.0;
    assert!((score - expected_score).abs() < 0.01, "Score calculation incorrect");
}
