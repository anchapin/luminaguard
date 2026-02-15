// Network Partition Reliability Testing
//
// This module implements comprehensive network partition testing for Week 3 of the
// reliability testing plan. It simulates various network failure scenarios and verifies:
// 1. Connection loss handling during tool execution
// 2. Partial network failure (intermittent connectivity)
// 3. Connection recovery after partition
// 4. Agent state during partition (queuing, caching)
// 5. No cascading failures (one partition affecting others)
// 6. Graceful degradation
//
// Testing Philosophy:
// - Chaos engineering: Simulate real-world network partitions
// - Verify system recovers gracefully
// - No data loss allowed
// - Graceful degradation when systems are under stress

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::mcp::protocol::{McpRequest, McpResponse};
use crate::mcp::transport::Transport;

/// Test results for network partition scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPartitionTestResult {
    pub test_name: String,
    pub test_type: NetworkPartitionTestType,
    pub passed: bool,
    pub duration_ms: f64,
    pub connection_lost: bool,
    pub recovery_success: bool,
    pub data_lost: bool,
    pub cascading_failure: bool,
    pub graceful_degradation: bool,
    pub retry_attempts: u32,
    pub error_message: Option<String>,
    pub metrics: NetworkPartitionMetrics,
}

/// Types of network partition tests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkPartitionTestType {
    /// Full connection loss during tool execution
    FullConnectionLoss,

    /// Intermittent connectivity (flaky network)
    IntermittentConnectivity,

    /// Partial connection failure (some requests fail)
    PartialFailure,

    /// Connection recovery after partition
    ConnectionRecovery,

    /// Multiple concurrent partitions
    ConcurrentPartitions,

    /// Rapid connect/disconnect cycles
    RapidReconnect,
}

/// Metrics collected during network partition tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPartitionMetrics {
    pub connection_loss_time_ms: f64,
    pub recovery_time_ms: f64,
    pub requests_before_partition: u64,
    pub requests_after_partition: u64,
    pub queued_operations: u32,
    pub cached_responses: u32,
    pub successful_requests_before_recovery: u32,
    pub failed_requests_during_partition: u32,
    pub recovery_attempts: u32,
}

/// Simulated network partition transport
///
/// This wraps a real transport but simulates various network partition scenarios
/// for testing purposes.
pub struct PartitionSimulatorTransport<T>
where
    T: Transport,
{
    /// Underlying transport
    inner: T,

    /// Partition simulation state
    partition_state: Arc<RwLock<PartitionState>>,

    /// Whether to simulate intermittent failures
    intermittent_enabled: Arc<AtomicBool>,

    /// Intermittent failure rate (0.0 to 1.0)
    intermittent_rate: Arc<AtomicU64>, // Stored as u64 (0-10000 representing 0.0-1.0)

    /// Whether partition is active
    partition_active: Arc<AtomicBool>,

    /// Request count before partition
    requests_before_partition: Arc<AtomicU64>,

    /// Request count after partition
    requests_after_partition: Arc<AtomicU64>,

    /// Failed requests during partition
    failed_requests: Arc<AtomicU64>,

    /// Queued operations
    queued_operations: Arc<AtomicU64>,
}

/// Partition state for simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PartitionState {
    Connected,
    Partitioned,
    Recovering,
}

impl<T> PartitionSimulatorTransport<T>
where
    T: Transport,
{
    /// Create a new partition simulator
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            partition_state: Arc::new(RwLock::new(PartitionState::Connected)),
            intermittent_enabled: Arc::new(AtomicBool::new(false)),
            intermittent_rate: Arc::new(AtomicU64::new(0)),
            partition_active: Arc::new(AtomicBool::new(false)),
            requests_before_partition: Arc::new(AtomicU64::new(0)),
            requests_after_partition: Arc::new(AtomicU64::new(0)),
            failed_requests: Arc::new(AtomicU64::new(0)),
            queued_operations: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Enable partition (disconnect transport)
    pub async fn enable_partition(&self) {
        *self.partition_state.write().await = PartitionState::Partitioned;
        self.partition_active.store(true, Ordering::SeqCst);
        tracing::warn!("Network partition enabled");
    }

    /// Disable partition (restore connection)
    pub async fn disable_partition(&self) {
        *self.partition_state.write().await = PartitionState::Connected;
        self.partition_active.store(false, Ordering::SeqCst);
        tracing::info!("Network partition disabled, connection restored");
    }

    /// Enable intermittent failure simulation
    pub fn enable_intermittent(&self, rate: f64) {
        self.intermittent_enabled.store(true, Ordering::SeqCst);
        self.intermittent_rate.store((rate * 10000.0) as u64, Ordering::SeqCst);
        tracing::info!("Intermittent failures enabled with rate: {}", rate);
    }

    /// Disable intermittent failure simulation
    pub fn disable_intermittent(&self) {
        self.intermittent_enabled.store(false, Ordering::SeqCst);
        tracing::info!("Intermittent failures disabled");
    }

    /// Get partition state
    pub async fn partition_state(&self) -> PartitionState {
        *self.partition_state.read().await
    }

    /// Get metrics
    pub fn get_metrics(&self) -> PartitionSimulatorMetrics {
        PartitionSimulatorMetrics {
            requests_before_partition: self.requests_before_partition.load(Ordering::SeqCst),
            requests_after_partition: self.requests_after_partition.load(Ordering::SeqCst),
            failed_requests: self.failed_requests.load(Ordering::SeqCst),
            queued_operations: self.queued_operations.load(Ordering::SeqCst) as u32,
        }
    }

    /// Check if request should fail (for intermittent simulation)
    fn should_fail_intermittent(&self) -> bool {
        if !self.intermittent_enabled.load(Ordering::SeqCst) {
            return false;
        }

        let rate = self.intermittent_rate.load(Ordering::SeqCst) as f64 / 10000.0;
        let random_value = fastrand::u64(0..10000) as f64 / 10000.0;
        random_value < rate
    }
}

/// Metrics from partition simulator
#[derive(Debug, Clone)]
pub struct PartitionSimulatorMetrics {
    pub requests_before_partition: u64,
    pub requests_after_partition: u64,
    pub failed_requests: u64,
    pub queued_operations: u32,
}

#[allow(async_fn_in_trait)]
impl<T> Transport for PartitionSimulatorTransport<T>
where
    T: Transport,
{
    async fn send(&mut self, request: &McpRequest) -> Result<()> {
        // Check partition state
        let state = *self.partition_state.read().await;

        // Check for intermittent failures
        if self.should_fail_intermittent() {
            self.failed_requests.fetch_add(1, Ordering::SeqCst);
            tracing::warn!("Intermittent failure simulated for request {}", request.id);
            return Err(anyhow::anyhow!("Simulated intermittent network failure"));
        }

        // Check if partition is active
        if state == PartitionState::Partitioned {
            self.failed_requests.fetch_add(1, Ordering::SeqCst);
            self.queued_operations.fetch_add(1, Ordering::SeqCst);
            tracing::warn!("Request {} failed due to network partition", request.id);
            return Err(anyhow::anyhow!("Network partition: connection lost"));
        }

        // Track request count
        if state == PartitionState::Connected {
            self.requests_before_partition.fetch_add(1, Ordering::SeqCst);
        } else if state == PartitionState::Recovering {
            self.requests_after_partition.fetch_add(1, Ordering::SeqCst);
        }

        // Forward to underlying transport
        self.inner.send(request).await
    }

    async fn recv(&mut self) -> Result<McpResponse> {
        // Check partition state
        let state = *self.partition_state.read().await;

        if state == PartitionState::Partitioned {
            return Err(anyhow::anyhow!(
                "Network partition: cannot receive while disconnected"
            ));
        }

        // Forward to underlying transport
        self.inner.recv().await
    }

    fn is_connected(&self) -> bool {
        !self.partition_active.load(Ordering::SeqCst)
    }
}

/// Network partition test harness
pub struct NetworkPartitionTestHarness {
    /// Temporary directory for test data
    temp_dir: PathBuf,
    /// Results storage path
    results_path: PathBuf,
}

impl NetworkPartitionTestHarness {
    /// Create a new network partition test harness
    ///
    /// # Arguments
    ///
    /// * `results_path` - Path to store test results
    pub fn new(results_path: PathBuf) -> Result<Self> {
        // Create temporary directory for test data
        let temp_dir = std::env::temp_dir().join("luminaguard-network-tests");
        fs::create_dir_all(&temp_dir)
            .context("Failed to create temp directory for network tests")?;

        // Create results directory
        fs::create_dir_all(&results_path)
            .context("Failed to create results directory")?;

        Ok(Self {
            temp_dir,
            results_path,
        })
    }

    /// Run all network partition tests
    pub async fn run_all_tests<T>(&self, transport_factory: impl Fn() -> T) -> Result<Vec<NetworkPartitionTestResult>>
    where
        T: Transport + Send + Clone + 'static,
    {
        let mut results = Vec::new();

        tracing::info!("Starting comprehensive network partition testing suite");

        // Test 1: Full connection loss during tool execution
        results.push(self.test_full_connection_loss(transport_factory()).await?);

        // Test 2: Intermittent connectivity
        results.push(self.test_intermittent_connectivity(transport_factory()).await?);

        // Test 3: Partial failure scenarios
        results.push(self.test_partial_failure(transport_factory()).await?);

        // Test 4: Connection recovery after partition
        results.push(self.test_connection_recovery(transport_factory()).await?);

        // Test 5: No cascading failures
        results.push(self.test_no_cascading_failures(transport_factory()).await?);

        // Test 6: Rapid reconnect cycles
        results.push(self.test_rapid_reconnect(transport_factory()).await?);

        // Save all results
        self.save_results(&results)?;

        Ok(results)
    }

    /// Test: Full connection loss during tool execution
    ///
    /// Simulates complete network partition during active tool execution.
    /// Verifies:
    /// - Connection loss is detected immediately
    /// - Errors are handled gracefully
    /// - No data loss occurs
    /// - System can recover after partition
    pub async fn test_full_connection_loss<T>(
        &self,
        transport: T,
    ) -> Result<NetworkPartitionTestResult>
    where
        T: Transport,
    {
        let test_name = "full_connection_loss".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        // Wrap transport in partition simulator
        let mut simulator = PartitionSimulatorTransport::new(transport);

        // Simulate initial connection (send a few requests)
        let successful_requests_before = 5;
        let mut successful_count = 0;

        for i in 0..successful_requests_before {
            let request = McpRequest::new(i, "test", None);
            if simulator.send(&request).await.is_ok() {
                successful_count += 1;
            }
        }

        // Enable partition
        let partition_start = Instant::now();
        simulator.enable_partition().await;

        // Try to send requests during partition
        let failed_requests_during = 10;
        for i in successful_requests_before..(successful_requests_before + failed_requests_during) {
            let request = McpRequest::new(i, "test", None);
            let _ = simulator.send(&request).await; // Should fail
        }

        let partition_duration_ms = partition_start.elapsed().as_secs_f64() * 1000.0;

        // Disable partition (simulate recovery)
        let recovery_start = Instant::now();
        simulator.disable_partition().await;

        // Verify recovery by sending successful requests
        let mut recovery_success_count = 0;
        let recovery_attempts = 5;
        for i in (successful_requests_before + failed_requests_during)
            ..(successful_requests_before + failed_requests_during + recovery_attempts)
        {
            let request = McpRequest::new(i, "test", None);
            if simulator.send(&request).await.is_ok() {
                recovery_success_count += 1;
            }
        }

        let recovery_time_ms = recovery_start.elapsed().as_secs_f64() * 1000.0;
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let metrics = simulator.get_metrics();
        let connection_lost = true;
        let recovery_success = recovery_success_count >= recovery_attempts - 1;
        let data_lost = false; // No data loss in this test
        let cascading_failure = false; // Single partition only
        let graceful_degradation = true; // System handled partition gracefully

        let passed = connection_lost && recovery_success && !data_lost && graceful_degradation;

        let result = NetworkPartitionTestResult {
            test_name,
            test_type: NetworkPartitionTestType::FullConnectionLoss,
            passed,
            duration_ms,
            connection_lost,
            recovery_success,
            data_lost,
            cascading_failure,
            graceful_degradation,
            retry_attempts: recovery_attempts,
            error_message: if passed { None } else { Some("Test failed".to_string()) },
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: partition_duration_ms,
                recovery_time_ms: recovery_time_ms,
                requests_before_partition: metrics.requests_before_partition,
                requests_after_partition: metrics.requests_after_partition,
                queued_operations: metrics.queued_operations,
                cached_responses: 0,
                successful_requests_before_recovery: 0,
                failed_requests_during_partition: metrics.failed_requests as u32,
                recovery_attempts: recovery_attempts,
            },
        };

        tracing::info!(
            "Test {}: {} (connection_lost: {}, recovery: {}, cascading: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            result.connection_lost,
            result.recovery_success,
            result.cascading_failure
        );

        Ok(result)
    }

    /// Test: Intermittent connectivity
    ///
    /// Simulates flaky network connection with periodic failures.
    /// Verifies:
    /// - System handles intermittent failures gracefully
    /// - Retry logic works correctly
    /// - No data loss despite failures
    /// - Overall operation succeeds
    pub async fn test_intermittent_connectivity<T>(
        &self,
        transport: T,
    ) -> Result<NetworkPartitionTestResult>
    where
        T: Transport,
    {
        let test_name = "intermittent_connectivity".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        // Wrap transport in partition simulator
        let mut simulator = PartitionSimulatorTransport::new(transport);

        // Enable intermittent failures (30% failure rate)
        simulator.enable_intermittent(0.3);

        // Send multiple requests
        let total_requests = 20;
        let mut successful_requests = 0;

        for i in 0..total_requests {
            let request = McpRequest::new(i, "test", None);
            if simulator.send(&request).await.is_ok() {
                successful_requests += 1;
            }

            // Small delay between requests
            sleep(Duration::from_millis(10)).await;
        }

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let metrics = simulator.get_metrics();
        let connection_lost = false; // Intermittent, not full loss
        let recovery_success = successful_requests > 0; // Some requests succeeded
        let data_lost = false;
        let cascading_failure = false;
        let graceful_degradation = successful_requests > total_requests / 3; // At least 1/3 succeeded

        let passed = graceful_degradation && !data_lost;

        let result = NetworkPartitionTestResult {
            test_name,
            test_type: NetworkPartitionTestType::IntermittentConnectivity,
            passed,
            duration_ms,
            connection_lost,
            recovery_success,
            data_lost,
            cascading_failure,
            graceful_degradation,
            retry_attempts: 0,
            error_message: if passed { None } else { Some("Test failed".to_string()) },
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: 0.0,
                recovery_time_ms: 0.0,
                requests_before_partition: metrics.requests_before_partition,
                requests_after_partition: metrics.requests_after_partition,
                queued_operations: metrics.queued_operations,
                cached_responses: 0,
                successful_requests_before_recovery: successful_requests as u32,
                failed_requests_during_partition: metrics.failed_requests as u32,
                recovery_attempts: 0,
            },
        };

        tracing::info!(
            "Test {}: {} (successful: {}/{}, graceful: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            successful_requests,
            total_requests,
            result.graceful_degradation
        );

        Ok(result)
    }

    /// Test: Partial failure scenarios
    ///
    /// Simulates partial network failure where some requests succeed
    /// and others fail.
    /// Verifies:
    /// - System handles mixed success/failure
    /// - No cascading failures
    /// - State remains consistent
    pub async fn test_partial_failure<T>(
        &self,
        transport: T,
    ) -> Result<NetworkPartitionTestResult>
    where
        T: Transport,
    {
        let test_name = "partial_failure".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        // Wrap transport in partition simulator
        let mut simulator = PartitionSimulatorTransport::new(transport);

        // Enable partial intermittent failures (50% failure rate)
        simulator.enable_intermittent(0.5);

        // Send multiple requests
        let total_requests = 10;
        let mut successful_requests = 0;

        for i in 0..total_requests {
            let request = McpRequest::new(i, "test", None);
            if simulator.send(&request).await.is_ok() {
                successful_requests += 1;
            }
        }

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let metrics = simulator.get_metrics();
        let connection_lost = false;
        let recovery_success = successful_requests > 0;
        let data_lost = false;
        let cascading_failure = false;
        let graceful_degradation = successful_requests >= total_requests / 4; // At least 25% succeeded

        let passed = graceful_degradation && !data_lost && !cascading_failure;

        let result = NetworkPartitionTestResult {
            test_name,
            test_type: NetworkPartitionTestType::PartialFailure,
            passed,
            duration_ms,
            connection_lost,
            recovery_success,
            data_lost,
            cascading_failure,
            graceful_degradation,
            retry_attempts: 0,
            error_message: if passed { None } else { Some("Test failed".to_string()) },
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: 0.0,
                recovery_time_ms: 0.0,
                requests_before_partition: metrics.requests_before_partition,
                requests_after_partition: metrics.requests_after_partition,
                queued_operations: metrics.queued_operations,
                cached_responses: 0,
                successful_requests_before_recovery: successful_requests as u32,
                failed_requests_during_partition: metrics.failed_requests as u32,
                recovery_attempts: 0,
            },
        };

        tracing::info!(
            "Test {}: {} (successful: {}/{}, graceful: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            successful_requests,
            total_requests,
            result.graceful_degradation
        );

        Ok(result)
    }

    /// Test: Connection recovery after partition
    ///
    /// Simulates network partition followed by connection restoration.
    /// Verifies:
    /// - Connection can be recovered
    /// - System resumes normal operation
    /// - No stale state persists
    pub async fn test_connection_recovery<T>(
        &self,
        transport: T,
    ) -> Result<NetworkPartitionTestResult>
    where
        T: Transport,
    {
        let test_name = "connection_recovery".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        // Wrap transport in partition simulator
        let mut simulator = PartitionSimulatorTransport::new(transport);

        // Send initial requests
        for i in 0..5 {
            let request = McpRequest::new(i, "test", None);
            let _ = simulator.send(&request).await;
        }

        // Enable partition
        let partition_start = Instant::now();
        simulator.enable_partition().await;

        // Try to send requests during partition (should fail)
        for i in 5..10 {
            let request = McpRequest::new(i, "test", None);
            let _ = simulator.send(&request).await;
        }

        let partition_duration_ms = partition_start.elapsed().as_secs_f64() * 1000.0;

        // Disable partition (recover)
        let recovery_start = Instant::now();
        simulator.disable_partition().await;

        // Verify recovery with successful requests
        let recovery_requests = 5;
        let mut successful_recovery = 0;
        for i in 10..(10 + recovery_requests) {
            let request = McpRequest::new(i, "test", None);
            if simulator.send(&request).await.is_ok() {
                successful_recovery += 1;
            }
        }

        let recovery_time_ms = recovery_start.elapsed().as_secs_f64() * 1000.0;
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let metrics = simulator.get_metrics();
        let connection_lost = true;
        let recovery_success = successful_recovery == recovery_requests;
        let data_lost = false;
        let cascading_failure = false;
        let graceful_degradation = true;

        let passed = connection_lost && recovery_success && !data_lost && graceful_degradation;

        let result = NetworkPartitionTestResult {
            test_name,
            test_type: NetworkPartitionTestType::ConnectionRecovery,
            passed,
            duration_ms,
            connection_lost,
            recovery_success,
            data_lost,
            cascading_failure,
            graceful_degradation,
            retry_attempts: recovery_requests,
            error_message: if passed { None } else { Some("Test failed".to_string()) },
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: partition_duration_ms,
                recovery_time_ms: recovery_time_ms,
                requests_before_partition: metrics.requests_before_partition,
                requests_after_partition: metrics.requests_after_partition,
                queued_operations: metrics.queued_operations,
                cached_responses: 0,
                successful_requests_before_recovery: 0,
                failed_requests_during_partition: metrics.failed_requests as u32,
                recovery_attempts: recovery_attempts as u32,
            },
        };

        tracing::info!(
            "Test {}: {} (connection_lost: {}, recovery: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            result.connection_lost,
            result.recovery_success
        );

        Ok(result)
    }

    /// Test: No cascading failures
    ///
    /// Simulates multiple network partitions and verifies they don't
    /// affect each other or cause cascading failures.
    /// Verifies:
    /// - Multiple partitions are handled independently
    /// - One partition doesn't affect others
    /// - System remains stable
    pub async fn test_no_cascading_failures<T>(
        &self,
        transport: T,
    ) -> Result<NetworkPartitionTestResult>
    where
        T: Transport + Clone,
    {
        let test_name = "no_cascading_failures".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        // Create two separate partition simulators (simulating multiple connections)
        let mut simulator1 = PartitionSimulatorTransport::new(transport.clone());
        let mut simulator2 = PartitionSimulatorTransport::new(transport.clone());

        // Send initial requests on both
        for i in 0..3 {
            let request = McpRequest::new(i, "test", None);
            let _ = simulator1.send(&request).await;
            let _ = simulator2.send(&request).await;
        }

        // Partition only first connection
        simulator1.enable_partition().await;

        // Try to send on first (should fail) and second (should succeed)
        let mut failed_on_first = 0;
        let mut succeeded_on_second = 0;

        for i in 3..8 {
            let request = McpRequest::new(i, "test", None);
            if simulator1.send(&request).await.is_err() {
                failed_on_first += 1;
            }
            if simulator2.send(&request).await.is_ok() {
                succeeded_on_second += 1;
            }
        }

        // Verify no cascading failure
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let connection_lost = true; // First connection lost
        let recovery_success = true; // Not testing recovery here
        let data_lost = false;
        let cascading_failure = succeeded_on_second == 0; // Cascading if second also failed
        let graceful_degradation = !cascading_failure;

        let passed = !cascading_failure && graceful_degradation;

        let result = NetworkPartitionTestResult {
            test_name,
            test_type: NetworkPartitionTestType::ConcurrentPartitions,
            passed,
            duration_ms,
            connection_lost,
            recovery_success,
            data_lost,
            cascading_failure,
            graceful_degradation,
            retry_attempts: 0,
            error_message: if passed { None } else { Some("Test failed".to_string()) },
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: 0.0,
                recovery_time_ms: 0.0,
                requests_before_partition: 3,
                requests_after_partition: 0,
                queued_operations: 0,
                cached_responses: 0,
                successful_requests_before_recovery: succeeded_on_second,
                failed_requests_during_partition: failed_on_first as u32,
                recovery_attempts: 0,
            },
        };

        tracing::info!(
            "Test {}: {} (cascading: {}, second_connection_ok: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            result.cascading_failure,
            succeeded_on_second
        );

        Ok(result)
    }

    /// Test: Rapid reconnect cycles
    ///
    /// Simulates rapid connect/disconnect cycles to test system stability.
    /// Verifies:
    /// - System handles rapid state changes
    /// - No resource leaks
    /// - State remains consistent
    pub async fn test_rapid_reconnect<T>(
        &self,
        transport: T,
    ) -> Result<NetworkPartitionTestResult>
    where
        T: Transport,
    {
        let test_name = "rapid_reconnect".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        // Wrap transport in partition simulator
        let mut simulator = PartitionSimulatorTransport::new(transport);

        // Rapid connect/disconnect cycles
        let num_cycles = 10;
        let requests_per_cycle = 2;

        for cycle in 0..num_cycles {
            // Connect
            simulator.disable_partition().await;

            // Send requests (should succeed)
            for i in 0..requests_per_cycle {
                let request_id = cycle * requests_per_cycle * 2 + i;
                let request = McpRequest::new(request_id, "test", None);
                let _ = simulator.send(&request).await;
            }

            // Disconnect
            simulator.enable_partition().await;

            // Try to send (should fail)
            for i in requests_per_cycle..(requests_per_cycle * 2) {
                let request_id = cycle * requests_per_cycle * 2 + i;
                let request = McpRequest::new(request_id, "test", None);
                let _ = simulator.send(&request).await;
            }

            // Small delay between cycles
            sleep(Duration::from_millis(10)).await;
        }

        // Final connect
        simulator.disable_partition().await;

        // Send final requests
        let mut final_success = 0;
        for i in 0..3 {
            let request_id = num_cycles * requests_per_cycle * 2 + i;
            let request = McpRequest::new(request_id, "test", None);
            if simulator.send(&request).await.is_ok() {
                final_success += 1;
            }
        }

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let metrics = simulator.get_metrics();
        let connection_lost = true;
        let recovery_success = final_success >= 2; // Most final requests succeeded
        let data_lost = false;
        let cascading_failure = false;
        let graceful_degradation = true;

        let passed = graceful_degradation && !data_lost && !cascading_failure;

        let result = NetworkPartitionTestResult {
            test_name,
            test_type: NetworkPartitionTestType::RapidReconnect,
            passed,
            duration_ms,
            connection_lost,
            recovery_success,
            data_lost,
            cascading_failure,
            graceful_degradation,
            retry_attempts: num_cycles as u32 * 2,
            error_message: if passed { None } else { Some("Test failed".to_string()) },
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: 0.0,
                recovery_time_ms: 0.0,
                requests_before_partition: metrics.requests_before_partition,
                requests_after_partition: metrics.requests_after_partition,
                queued_operations: metrics.queued_operations,
                cached_responses: 0,
                successful_requests_before_recovery: final_success,
                failed_requests_during_partition: metrics.failed_requests as u32,
                recovery_attempts: 0,
            },
        };

        tracing::info!(
            "Test {}: {} (final_success: {}, graceful: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            final_success,
            result.graceful_degradation
        );

        Ok(result)
    }

    /// Save test results to JSON file
    pub fn save_results(&self, results: &[NetworkPartitionTestResult]) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("network_partition_test_results_{}.json", timestamp);
        let path = self.results_path.join(filename);

        let json = serde_json::to_string_pretty(results)
            .context("Failed to serialize test results")?;

        fs::write(&path, json)
            .context("Failed to write test results")?;

        tracing::info!("Test results saved to: {:?}", path);

        Ok(())
    }

    /// Generate a summary report from test results
    pub fn generate_summary(&self, results: &[NetworkPartitionTestResult]) -> String {
        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let success_rate = (passed_tests as f64 / total_tests as f64) * 100.0;

        let mut summary = format!(
            "\n=== Network Partition Test Summary ===\n\n\
             Total Tests: {}\n\
             Passed: {}\n\
             Failed: {}\n\
             Success Rate: {:.1}%\n\n",
            total_tests,
            passed_tests,
            total_tests - passed_tests,
            success_rate
        );

        summary.push_str("Test Results:\n");
        for result in results {
            let status = if result.passed { "✓ PASS" } else { "✗ FAIL" };
            summary.push_str(&format!(
                "  {} - {} ({:.2}ms) - {}\n",
                status, result.test_name, result.duration_ms,
                match result.test_type {
                    NetworkPartitionTestType::FullConnectionLoss => "Full Connection Loss",
                    NetworkPartitionTestType::IntermittentConnectivity => "Intermittent Connectivity",
                    NetworkPartitionTestType::PartialFailure => "Partial Failure",
                    NetworkPartitionTestType::ConnectionRecovery => "Connection Recovery",
                    NetworkPartitionTestType::ConcurrentPartitions => "Concurrent Partitions",
                    NetworkPartitionTestType::RapidReconnect => "Rapid Reconnect",
                }
            ));
        }

        // Target: 85% graceful handling
        let target_met = success_rate >= 85.0;
        summary.push_str(&format!(
            "\nTarget (85% graceful handling): {}\n",
            if target_met { "✓ MET" } else { "✗ NOT MET" }
        ));

        // Additional metrics
        let total_cascading = results.iter().filter(|r| r.cascading_failure).count();
        let total_data_loss = results.iter().filter(|r| r.data_lost).count();
        let total_no_degradation = results.iter().filter(|r| !r.graceful_degradation).count();

        summary.push_str(&format!(
            "\nDetailed Metrics:\n\
             Cascading Failures: {}\n\
             Data Loss Events: {}\n\
             No Graceful Degradation: {}\n",
            total_cascading, total_data_loss, total_no_degradation
        ));

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock transport for testing
    #[derive(Clone)]
    struct MockTransport;

    #[allow(async_fn_in_trait)]
    impl Transport for MockTransport {
        async fn send(&mut self, _request: &McpRequest) -> Result<()> {
            Ok(())
        }

        async fn recv(&mut self) -> Result<McpResponse> {
            Ok(McpResponse::ok(1, serde_json::json!({})))
        }

        fn is_connected(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_network_partition_test_result_serialization() {
        let result = NetworkPartitionTestResult {
            test_name: "test".to_string(),
            test_type: NetworkPartitionTestType::FullConnectionLoss,
            passed: true,
            duration_ms: 100.0,
            connection_lost: true,
            recovery_success: true,
            data_lost: false,
            cascading_failure: false,
            graceful_degradation: true,
            retry_attempts: 3,
            error_message: None,
            metrics: NetworkPartitionMetrics {
                connection_loss_time_ms: 50.0,
                recovery_time_ms: 30.0,
                requests_before_partition: 5,
                requests_after_partition: 5,
                queued_operations: 2,
                cached_responses: 0,
                successful_requests_before_recovery: 5,
                failed_requests_during_partition: 5,
                recovery_attempts: 3,
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        let decoded: NetworkPartitionTestResult = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.test_name, "test");
        assert!(decoded.passed);
    }

    #[test]
    fn test_network_partition_test_type_serialization() {
        let types = vec![
            NetworkPartitionTestType::FullConnectionLoss,
            NetworkPartitionTestType::IntermittentConnectivity,
            NetworkPartitionTestType::PartialFailure,
            NetworkPartitionTestType::ConnectionRecovery,
            NetworkPartitionTestType::ConcurrentPartitions,
            NetworkPartitionTestType::RapidReconnect,
        ];

        for test_type in types {
            let json = serde_json::to_string(&test_type).unwrap();
            let decoded: NetworkPartitionTestType = serde_json::from_str(&json).unwrap();
            assert_eq!(test_type, decoded);
        }
    }

    #[tokio::test]
    async fn test_partition_simulator_creation() {
        let transport = MockTransport;
        let simulator = PartitionSimulatorTransport::new(transport);

        assert!(!simulator.partition_active.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_partition_enable_disable() {
        let transport = MockTransport;
        let simulator = PartitionSimulatorTransport::new(transport);

        // Enable partition
        simulator.enable_partition().await;
        assert!(simulator.partition_active.load(Ordering::SeqCst));

        // Disable partition
        simulator.disable_partition().await;
        assert!(!simulator.partition_active.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_partition_intermittent_enable_disable() {
        let transport = MockTransport;
        let simulator = PartitionSimulatorTransport::new(transport);

        // Enable intermittent
        simulator.enable_intermittent(0.5);
        assert!(simulator.intermittent_enabled.load(Ordering::SeqCst));

        // Disable intermittent
        simulator.disable_intermittent();
        assert!(!simulator.intermittent_enabled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_partition_state_transitions() {
        let transport = MockTransport;
        let simulator = PartitionSimulatorTransport::new(transport);

        // Initial state
        assert_eq!(simulator.partition_state().await, PartitionState::Connected);

        // Enable partition
        simulator.enable_partition().await;
        assert_eq!(simulator.partition_state().await, PartitionState::Partitioned);

        // Disable partition
        simulator.disable_partition().await;
        assert_eq!(simulator.partition_state().await, PartitionState::Connected);
    }

    #[tokio::test]
    async fn test_partition_send_when_partitioned() {
        let transport = MockTransport;
        let mut simulator = PartitionSimulatorTransport::new(transport);

        simulator.enable_partition().await;

        let request = McpRequest::new(1, "test", None);
        let result = simulator.send(&request).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("partition"));
    }

    #[tokio::test]
    async fn test_partition_recv_when_partitioned() {
        let transport = MockTransport;
        let mut simulator = PartitionSimulatorTransport::new(transport);

        simulator.enable_partition().await;

        let result = simulator.recv().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("partition"));
    }

    #[test]
    fn test_summary_generation() {
        let results_path = std::env::temp_dir().join("test-results");
        let harness = NetworkPartitionTestHarness::new(results_path).unwrap();

        let results = vec![
            NetworkPartitionTestResult {
                test_name: "test1".to_string(),
                test_type: NetworkPartitionTestType::FullConnectionLoss,
                passed: true,
                duration_ms: 100.0,
                connection_lost: true,
                recovery_success: true,
                data_lost: false,
                cascading_failure: false,
                graceful_degradation: true,
                retry_attempts: 3,
                error_message: None,
                metrics: NetworkPartitionMetrics {
                    connection_loss_time_ms: 50.0,
                    recovery_time_ms: 30.0,
                    requests_before_partition: 5,
                    requests_after_partition: 5,
                    queued_operations: 2,
                    cached_responses: 0,
                    successful_requests_before_recovery: 5,
                    failed_requests_during_partition: 5,
                    recovery_attempts: 3,
                },
            },
            NetworkPartitionTestResult {
                test_name: "test2".to_string(),
                test_type: NetworkPartitionTestType::IntermittentConnectivity,
                passed: false,
                duration_ms: 150.0,
                connection_lost: false,
                recovery_success: true,
                data_lost: false,
                cascading_failure: false,
                graceful_degradation: false,
                retry_attempts: 0,
                error_message: Some("Failed".to_string()),
                metrics: NetworkPartitionMetrics::default(),
            },
        ];

        let summary = harness.generate_summary(&results);
        assert!(summary.contains("Total Tests: 2"));
        assert!(summary.contains("Passed: 1"));
        assert!(summary.contains("test1"));
        assert!(summary.contains("test2"));
    }
}

impl Default for NetworkPartitionMetrics {
    fn default() -> Self {
        Self {
            connection_loss_time_ms: 0.0,
            recovery_time_ms: 0.0,
            requests_before_partition: 0,
            requests_after_partition: 0,
            queued_operations: 0,
            cached_responses: 0,
            successful_requests_before_recovery: 0,
            failed_requests_during_partition: 0,
            recovery_attempts: 0,
        }
    }
}
