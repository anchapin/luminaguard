// Network Partition Test Runner
//
// This module provides integration tests for network partition scenarios.
// It runs the full test suite and generates reports.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Instant;
use crate::mcp::transport::Transport;

use crate::vm::network_partition::{
    NetworkPartitionTestHarness, NetworkPartitionTestResult,
};

/// Run all network partition tests
///
/// This function executes the complete network partition test suite
/// and generates a summary report.
///
/// # Returns
///
/// * `Vec<NetworkPartitionTestResult>` - Test results for all scenarios
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm::run_network_partition_tests;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let results = run_network_partition_tests().await?;
///     println!("Tests completed: {}", results.len());
///     Ok(())
/// }
/// ```
pub async fn run_network_partition_tests() -> Result<Vec<NetworkPartitionTestResult>> {
    let start_time = Instant::now();

    tracing::info!("Starting Network Partition Test Suite (Week 3)");

    // Create test harness with results path
    let results_path = PathBuf::from(".beads/metrics/reliability");
    let harness = NetworkPartitionTestHarness::new(results_path.clone())
        .context("Failed to create network partition test harness")?;

    // Run all tests with a mock transport
    // In production, this would use real MCP transport
    let results: Vec<NetworkPartitionTestResult> = harness
        .run_all_tests(|| MockTransportForTesting)
        .await
        .context("Failed to run network partition tests")?;

    let total_duration = start_time.elapsed();

    // Generate and display summary
    let summary = harness.generate_summary(&results);
    println!("{}", summary);

    // Save detailed results
    harness.save_results(&results)?;

    tracing::info!(
        "Network partition test suite completed in {:.2}s",
        total_duration.as_secs_f64()
    );

    Ok(results)
}

/// Mock transport for testing network partition scenarios
///
/// This is a simple mock that simulates a working transport
/// for testing purposes. The actual partition simulation is handled
/// by the PartitionSimulatorTransport wrapper.
#[derive(Clone)]
pub struct MockTransportForTesting;

impl Transport for MockTransportForTesting {
    async fn send(
        &mut self,
        _request: &crate::mcp::protocol::McpRequest,
    ) -> Result<()> {
        // Simulate minimal delay
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        Ok(())
    }

    async fn recv(&mut self) -> Result<crate::mcp::protocol::McpResponse> {
        // Return a mock successful response
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        Ok(crate::mcp::protocol::McpResponse::ok(
            1,
            serde_json::json!({"status": "success"}),
        ))
    }

    fn is_connected(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_partition_test_runner() {
        // This test verifies the test runner works correctly
        // It's a meta-test that runs the full suite
        let results = run_network_partition_tests().await;

        assert!(results.is_ok());

        let results = results.unwrap();

        // Should have run all test types
        assert!(results.len() >= 5);

        // Check that at least some tests passed
        let passed_count = results.iter().filter(|r| r.passed).count();
        assert!(passed_count > 0);

        // Verify no cascading failures in all tests
        let cascading_count = results.iter().filter(|r| r.cascading_failure).count();
        assert_eq!(cascading_count, 0, "No tests should have cascading failures");

        // Verify no data loss in all tests
        let data_loss_count = results.iter().filter(|r| r.data_lost).count();
        assert_eq!(data_loss_count, 0, "No tests should have data loss");
    }

    #[tokio::test]
    async fn test_full_connection_loss_scenario() {
        // Test the full connection loss scenario specifically
        let results_path = PathBuf::from(".beads/metrics/reliability");
        let harness = NetworkPartitionTestHarness::new(results_path).unwrap();

        let result: NetworkPartitionTestResult = harness
            .test_full_connection_loss(MockTransportForTesting)
            .await
            .unwrap();

        // Verify test behavior
        assert!(result.connection_lost);
        assert_eq!(
            result.test_type,
            crate::vm::network_partition::NetworkPartitionTestType::FullConnectionLoss
        );
        assert!(!result.data_lost);
    }

    #[tokio::test]
    async fn test_intermittent_connectivity_scenario() {
        // Test the intermittent connectivity scenario specifically
        let results_path = PathBuf::from(".beads/metrics/reliability");
        let harness = NetworkPartitionTestHarness::new(results_path).unwrap();

        let result: NetworkPartitionTestResult = harness
            .test_intermittent_connectivity(MockTransportForTesting)
            .await
            .unwrap();

        // Verify test behavior
        assert_eq!(
            result.test_type,
            crate::vm::network_partition::NetworkPartitionTestType::IntermittentConnectivity
        );
        assert!(!result.data_lost);
    }

    #[tokio::test]
    async fn test_connection_recovery_scenario() {
        // Test the connection recovery scenario specifically
        let results_path = PathBuf::from(".beads/metrics/reliability");
        let harness = NetworkPartitionTestHarness::new(results_path).unwrap();

        let result: NetworkPartitionTestResult = harness
            .test_connection_recovery(MockTransportForTesting)
            .await
            .unwrap();

        // Verify test behavior
        assert!(result.connection_lost);
        assert!(result.recovery_success);
        assert_eq!(
            result.test_type,
            crate::vm::network_partition::NetworkPartitionTestType::ConnectionRecovery
        );
        assert!(!result.data_lost);
    }

    #[tokio::test]
    async fn test_no_cascading_failures_scenario() {
        // Test the no cascading failures scenario specifically
        let results_path = PathBuf::from(".beads/metrics/reliability");
        let harness = NetworkPartitionTestHarness::new(results_path).unwrap();

        let result: NetworkPartitionTestResult = harness
            .test_no_cascading_failures(MockTransportForTesting)
            .await
            .unwrap();

        // Verify no cascading failure occurred
        assert!(!result.cascading_failure);
        assert_eq!(
            result.test_type,
            crate::vm::network_partition::NetworkPartitionTestType::ConcurrentPartitions
        );
    }

    #[test]
    fn test_mock_transport_behavior() {
        // Verify mock transport behaves as expected
        let transport = MockTransportForTesting;

        // Should report as connected
        assert!(transport.is_connected());
    }
}
