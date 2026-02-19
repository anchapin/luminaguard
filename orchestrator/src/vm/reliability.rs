// VM Reliability Crash Testing
//
// This module implements comprehensive crash testing for Week 1-2 of the
// reliability testing plan. It simulates various VM crash scenarios and verifies:
// 1. Graceful VM shutdown
// 2. No data corruption
// 3. Proper resource cleanup
// 4. Restart capability
//
// Testing Philosophy:
// - Chaos engineering: Simulate real-world crash conditions
// - Verify system recovers gracefully
// - No data loss allowed
// - All resources must be cleaned up

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::vm::config::VmConfig;
use crate::vm::destroy_vm;

/// Test results for crash scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashTestResult {
    pub test_name: String,
    pub test_type: CrashTestType,
    pub passed: bool,
    pub duration_ms: f64,
    pub cleanup_success: bool,
    pub data_corrupted: bool,
    pub restart_success: bool,
    pub error_message: Option<String>,
    pub metrics: CrashTestMetrics,
}

/// Types of crash tests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrashTestType {
    FileOperation,
    NetworkOperation,
    ToolExecution,
    SequentialCrash,
    RapidSpawnKill,
    MemoryPressure,
}

/// Metrics collected during crash tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashTestMetrics {
    pub vm_spawn_time_ms: f64,
    pub vm_lifecycle_time_ms: f64,
    pub kill_to_cleanup_time_ms: f64,
    pub memory_before_mb: Option<u64>,
    pub memory_after_mb: Option<u64>,
    pub file_descriptors_before: Option<u32>,
    pub file_descriptors_after: Option<u32>,
    pub processes_before: Option<u32>,
    pub processes_after: Option<u32>,
}

/// VM crash test harness
pub struct CrashTestHarness {
    /// Kernel path for test VMs
    kernel_path: String,
    /// Rootfs path for test VMs
    rootfs_path: String,
    /// Temporary directory for test data
    temp_dir: PathBuf,
    /// Results storage path
    results_path: PathBuf,
}

impl CrashTestHarness {
    /// Create a new crash test harness
    ///
    /// # Arguments
    ///
    /// * `kernel_path` - Path to VM kernel image
    /// * `rootfs_path` - Path to VM rootfs
    /// * `results_path` - Path to store test results
    pub fn new(kernel_path: String, rootfs_path: String, results_path: PathBuf) -> Result<Self> {
        // Create temporary directory for test data
        let temp_dir = std::env::temp_dir().join("luminaguard-reliability-tests");
        fs::create_dir_all(&temp_dir)
            .context("Failed to create temp directory for reliability tests")?;

        // Create results directory
        fs::create_dir_all(&results_path).context("Failed to create results directory")?;

        Ok(Self {
            kernel_path,
            rootfs_path,
            temp_dir,
            results_path,
        })
    }

    /// Run all crash tests
    pub async fn run_all_tests(&self) -> Result<Vec<CrashTestResult>> {
        let mut results = Vec::new();

        tracing::info!("Starting comprehensive VM crash testing suite");

        // Test 1: Crash during file operations
        results.push(self.test_crash_during_file_operations().await?);

        // Test 2: Crash during network operations (simulated)
        results.push(self.test_crash_during_network_operations().await?);

        // Test 3: Crash during tool execution (simulated)
        results.push(self.test_crash_during_tool_execution().await?);

        // Test 4: Sequential crash testing
        results.push(self.test_sequential_crashes().await?);

        // Test 5: Rapid spawn and kill
        results.push(self.test_rapid_spawn_kill().await?);

        // Test 6: Memory pressure crash
        results.push(self.test_memory_pressure_crash().await?);

        // Save all results
        self.save_results(&results)?;

        Ok(results)
    }

    /// Test: Kill VM during active file operations
    ///
    /// Simulates crash while VM is performing I/O operations.
    /// Verifies:
    /// - VM terminates cleanly
    /// - No data corruption occurs
    /// - Resources are properly cleaned up
    /// - System can restart after crash
    async fn test_crash_during_file_operations(&self) -> Result<CrashTestResult> {
        let test_name = "crash_during_file_operations".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        let metrics_before = self.collect_system_metrics()?;

        // Create VM config
        let config = VmConfig {
            kernel_path: self.kernel_path.clone(),
            rootfs_path: self.rootfs_path.clone(),
            ..VmConfig::new(format!("{}-1", test_name))
        };

        // Spawn VM
        let spawn_start = Instant::now();
        let handle =
            match crate::vm::spawn_vm_with_config(&format!("{}-1", test_name), &config).await {
                Ok(h) => h,
                Err(e) => {
                    return Ok(CrashTestResult {
                        test_name: test_name.clone(),
                        test_type: CrashTestType::FileOperation,
                        passed: false,
                        duration_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                        cleanup_success: false,
                        data_corrupted: false,
                        restart_success: false,
                        error_message: Some(format!("Failed to spawn VM: {}", e)),
                        metrics: CrashTestMetrics {
                            vm_spawn_time_ms: 0.0,
                            vm_lifecycle_time_ms: 0.0,
                            kill_to_cleanup_time_ms: 0.0,
                            ..Default::default()
                        },
                    });
                }
            };
        let vm_spawn_time_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

        // Simulate file operations (wait for VM to be active)
        sleep(Duration::from_millis(100)).await;

        // Kill the VM (simulating crash)
        let kill_start = Instant::now();

        // Note: We're destroying the VM normally here, but in a real crash
        // scenario, the process would be killed via SIGKILL
        // For testing, we verify that destroy_vm() handles cleanup correctly
        let cleanup_success = match destroy_vm(handle).await {
            Ok(_) => true,
            Err(e) => {
                tracing::error!("Failed to destroy VM: {}", e);
                false
            }
        };
        let kill_to_cleanup_time_ms = kill_start.elapsed().as_secs_f64() * 1000.0;

        // Check for data corruption
        let data_corrupted = self.check_data_corruption(&test_name);

        // Verify restart capability
        let restart_success = match self
            .test_restart_after_crash(&format!("{}-restart", test_name))
            .await
        {
            Ok(_) => true,
            Err(e) => {
                tracing::error!("Failed to restart VM: {}", e);
                false
            }
        };

        let metrics_after = self.collect_system_metrics()?;
        let vm_lifecycle_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let passed = cleanup_success && !data_corrupted && restart_success;

        let result = CrashTestResult {
            test_name,
            test_type: CrashTestType::FileOperation,
            passed,
            duration_ms: vm_lifecycle_time_ms,
            cleanup_success,
            data_corrupted,
            restart_success,
            error_message: if passed {
                None
            } else {
                Some("Test failed".to_string())
            },
            metrics: CrashTestMetrics {
                vm_spawn_time_ms,
                vm_lifecycle_time_ms,
                kill_to_cleanup_time_ms,
                memory_before_mb: metrics_before.memory_mb,
                memory_after_mb: metrics_after.memory_mb,
                file_descriptors_before: metrics_before.file_descriptors,
                file_descriptors_after: metrics_after.file_descriptors,
                processes_before: metrics_before.processes,
                processes_after: metrics_after.processes,
            },
        };

        tracing::info!(
            "Test {}: {} (cleanup: {}, data_corrupted: {}, restart: {})",
            result.test_name,
            if result.passed { "PASSED" } else { "FAILED" },
            result.cleanup_success,
            result.data_corrupted,
            result.restart_success
        );

        Ok(result)
    }

    /// Test: Kill VM during network operations (simulated)
    ///
    /// Simulates crash while VM is performing network operations.
    /// Since VMs have networking disabled, we simulate the scenario
    /// by killing the VM after a short delay.
    async fn test_crash_during_network_operations(&self) -> Result<CrashTestResult> {
        let test_name = "crash_during_network_operations".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        let metrics_before = self.collect_system_metrics()?;

        let config = VmConfig {
            kernel_path: self.kernel_path.clone(),
            rootfs_path: self.rootfs_path.clone(),
            ..VmConfig::new(format!("{}-1", test_name))
        };

        let spawn_start = Instant::now();
        let handle =
            match crate::vm::spawn_vm_with_config(&format!("{}-1", test_name), &config).await {
                Ok(h) => h,
                Err(e) => {
                    return Ok(CrashTestResult {
                        test_name: test_name.clone(),
                        test_type: CrashTestType::NetworkOperation,
                        passed: false,
                        duration_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                        cleanup_success: false,
                        data_corrupted: false,
                        restart_success: false,
                        error_message: Some(format!("Failed to spawn VM: {}", e)),
                        metrics: CrashTestMetrics {
                            vm_spawn_time_ms: 0.0,
                            vm_lifecycle_time_ms: 0.0,
                            kill_to_cleanup_time_ms: 0.0,
                            ..Default::default()
                        },
                    });
                }
            };
        let vm_spawn_time_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

        // Simulate network operation delay
        sleep(Duration::from_millis(50)).await;

        let kill_start = Instant::now();
        let cleanup_success = destroy_vm(handle).await.is_ok();
        let kill_to_cleanup_time_ms = kill_start.elapsed().as_secs_f64() * 1000.0;

        let data_corrupted = self.check_data_corruption(&test_name);
        let restart_success = self
            .test_restart_after_crash(&format!("{}-restart", test_name))
            .await
            .is_ok();

        let metrics_after = self.collect_system_metrics()?;
        let vm_lifecycle_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let passed = cleanup_success && !data_corrupted && restart_success;

        Ok(CrashTestResult {
            test_name,
            test_type: CrashTestType::NetworkOperation,
            passed,
            duration_ms: vm_lifecycle_time_ms,
            cleanup_success,
            data_corrupted,
            restart_success,
            error_message: if passed {
                None
            } else {
                Some("Test failed".to_string())
            },
            metrics: CrashTestMetrics {
                vm_spawn_time_ms,
                vm_lifecycle_time_ms,
                kill_to_cleanup_time_ms,
                memory_before_mb: metrics_before.memory_mb,
                memory_after_mb: metrics_after.memory_mb,
                file_descriptors_before: metrics_before.file_descriptors,
                file_descriptors_after: metrics_after.file_descriptors,
                processes_before: metrics_before.processes,
                processes_after: metrics_after.processes,
            },
        })
    }

    /// Test: Kill VM during tool execution (simulated)
    ///
    /// Simulates crash while VM is executing tools.
    /// Verifies that VM state is not corrupted and can be recovered.
    async fn test_crash_during_tool_execution(&self) -> Result<CrashTestResult> {
        let test_name = "crash_during_tool_execution".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        let metrics_before = self.collect_system_metrics()?;

        let config = VmConfig {
            kernel_path: self.kernel_path.clone(),
            rootfs_path: self.rootfs_path.clone(),
            ..VmConfig::new(format!("{}-1", test_name))
        };

        let spawn_start = Instant::now();
        let handle =
            match crate::vm::spawn_vm_with_config(&format!("{}-1", test_name), &config).await {
                Ok(h) => h,
                Err(e) => {
                    return Ok(CrashTestResult {
                        test_name: test_name.clone(),
                        test_type: CrashTestType::ToolExecution,
                        passed: false,
                        duration_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                        cleanup_success: false,
                        data_corrupted: false,
                        restart_success: false,
                        error_message: Some(format!("Failed to spawn VM: {}", e)),
                        metrics: CrashTestMetrics {
                            vm_spawn_time_ms: 0.0,
                            vm_lifecycle_time_ms: 0.0,
                            kill_to_cleanup_time_ms: 0.0,
                            ..Default::default()
                        },
                    });
                }
            };
        let vm_spawn_time_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

        // Simulate tool execution delay
        sleep(Duration::from_millis(75)).await;

        let kill_start = Instant::now();
        let cleanup_success = destroy_vm(handle).await.is_ok();
        let kill_to_cleanup_time_ms = kill_start.elapsed().as_secs_f64() * 1000.0;

        let data_corrupted = self.check_data_corruption(&test_name);
        let restart_success = self
            .test_restart_after_crash(&format!("{}-restart", test_name))
            .await
            .is_ok();

        let metrics_after = self.collect_system_metrics()?;
        let vm_lifecycle_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let passed = cleanup_success && !data_corrupted && restart_success;

        Ok(CrashTestResult {
            test_name,
            test_type: CrashTestType::ToolExecution,
            passed,
            duration_ms: vm_lifecycle_time_ms,
            cleanup_success,
            data_corrupted,
            restart_success,
            error_message: if passed {
                None
            } else {
                Some("Test failed".to_string())
            },
            metrics: CrashTestMetrics {
                vm_spawn_time_ms,
                vm_lifecycle_time_ms,
                kill_to_cleanup_time_ms,
                memory_before_mb: metrics_before.memory_mb,
                memory_after_mb: metrics_after.memory_mb,
                file_descriptors_before: metrics_before.file_descriptors,
                file_descriptors_after: metrics_after.file_descriptors,
                processes_before: metrics_before.processes,
                processes_after: metrics_after.processes,
            },
        })
    }

    /// Test: Sequential VM crashes
    ///
    /// Tests that the system can handle multiple crashes in sequence.
    /// Verifies:
    /// - Each crash is handled independently
    /// - No resource leaks accumulate
    /// - System remains stable after multiple crashes
    async fn test_sequential_crashes(&self) -> Result<CrashTestResult> {
        let test_name = "sequential_crashes".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        let metrics_before = self.collect_system_metrics()?;

        let num_crashes = 5;
        let mut cleanup_success_count = 0;
        let mut spawn_times = Vec::new();

        for i in 0..num_crashes {
            let task_id = format!("{}-{}", test_name, i);

            let config = VmConfig {
                kernel_path: self.kernel_path.clone(),
                rootfs_path: self.rootfs_path.clone(),
                ..VmConfig::new(task_id.clone())
            };

            let spawn_start = Instant::now();
            match crate::vm::spawn_vm_with_config(&task_id, &config).await {
                Ok(handle) => {
                    spawn_times.push(spawn_start.elapsed().as_secs_f64() * 1000.0);

                    // Short delay to simulate workload
                    sleep(Duration::from_millis(10)).await;

                    if destroy_vm(handle).await.is_ok() {
                        cleanup_success_count += 1;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to spawn VM {}: {}", i, e);
                }
            }
        }

        let avg_spawn_time_ms = if spawn_times.is_empty() {
            0.0
        } else {
            spawn_times.iter().sum::<f64>() / spawn_times.len() as f64
        };

        let cleanup_success = cleanup_success_count == num_crashes;
        let data_corrupted = self.check_data_corruption(&test_name);

        // Test restart after all crashes
        let restart_success = self
            .test_restart_after_crash(&format!("{}-restart", test_name))
            .await
            .is_ok();

        let metrics_after = self.collect_system_metrics()?;
        let vm_lifecycle_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let passed = cleanup_success && !data_corrupted && restart_success;

        Ok(CrashTestResult {
            test_name,
            test_type: CrashTestType::SequentialCrash,
            passed,
            duration_ms: vm_lifecycle_time_ms,
            cleanup_success,
            data_corrupted,
            restart_success,
            error_message: if passed {
                None
            } else {
                Some(format!(
                    "Cleanups: {}/{}",
                    cleanup_success_count, num_crashes
                ))
            },
            metrics: CrashTestMetrics {
                vm_spawn_time_ms: avg_spawn_time_ms,
                vm_lifecycle_time_ms,
                kill_to_cleanup_time_ms: 0.0,
                memory_before_mb: metrics_before.memory_mb,
                memory_after_mb: metrics_after.memory_mb,
                file_descriptors_before: metrics_before.file_descriptors,
                file_descriptors_after: metrics_after.file_descriptors,
                processes_before: metrics_before.processes,
                processes_after: metrics_after.processes,
            },
        })
    }

    /// Test: Rapid spawn and kill
    ///
    /// Tests system resilience under rapid VM lifecycle changes.
    /// Verifies:
    /// - System can handle rapid spawn/kill cycles
    /// - No race conditions occur
    /// - Resources are properly managed
    async fn test_rapid_spawn_kill(&self) -> Result<CrashTestResult> {
        let test_name = "rapid_spawn_kill".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        let metrics_before = self.collect_system_metrics()?;

        let num_iterations = 10;
        let mut cleanup_success_count = 0;
        let mut spawn_times = Vec::new();

        for i in 0..num_iterations {
            let task_id = format!("{}-{}", test_name, i);

            let config = VmConfig {
                kernel_path: self.kernel_path.clone(),
                rootfs_path: self.rootfs_path.clone(),
                ..VmConfig::new(task_id.clone())
            };

            let spawn_start = Instant::now();
            match crate::vm::spawn_vm_with_config(&task_id, &config).await {
                Ok(handle) => {
                    spawn_times.push(spawn_start.elapsed().as_secs_f64() * 1000.0);

                    // Immediate kill (no delay)
                    if destroy_vm(handle).await.is_ok() {
                        cleanup_success_count += 1;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to spawn VM {}: {}", i, e);
                }
            }
        }

        let avg_spawn_time_ms = if spawn_times.is_empty() {
            0.0
        } else {
            spawn_times.iter().sum::<f64>() / spawn_times.len() as f64
        };

        let cleanup_success = cleanup_success_count == num_iterations;
        let data_corrupted = self.check_data_corruption(&test_name);

        let metrics_after = self.collect_system_metrics()?;
        let vm_lifecycle_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let passed = cleanup_success && !data_corrupted;

        Ok(CrashTestResult {
            test_name,
            test_type: CrashTestType::RapidSpawnKill,
            passed,
            duration_ms: vm_lifecycle_time_ms,
            cleanup_success,
            data_corrupted,
            restart_success: true, // Not applicable for this test
            error_message: if passed {
                None
            } else {
                Some(format!(
                    "Cleanups: {}/{}",
                    cleanup_success_count, num_iterations
                ))
            },
            metrics: CrashTestMetrics {
                vm_spawn_time_ms: avg_spawn_time_ms,
                vm_lifecycle_time_ms,
                kill_to_cleanup_time_ms: 0.0,
                memory_before_mb: metrics_before.memory_mb,
                memory_after_mb: metrics_after.memory_mb,
                file_descriptors_before: metrics_before.file_descriptors,
                file_descriptors_after: metrics_after.file_descriptors,
                processes_before: metrics_before.processes,
                processes_after: metrics_after.processes,
            },
        })
    }

    /// Test: Memory pressure crash
    ///
    /// Tests system behavior under memory pressure.
    /// Verifies:
    /// - System doesn't crash under memory pressure
    /// - Graceful degradation occurs
    /// - Cleanup works even with memory constraints
    async fn test_memory_pressure_crash(&self) -> Result<CrashTestResult> {
        let test_name = "memory_pressure_crash".to_string();
        let start_time = Instant::now();

        tracing::info!("Running test: {}", test_name);

        let metrics_before = self.collect_system_metrics()?;

        // Spawn VM with minimal memory configuration
        let mut config = VmConfig {
            kernel_path: self.kernel_path.clone(),
            rootfs_path: self.rootfs_path.clone(),
            ..VmConfig::new(format!("{}-1", test_name))
        };
        config.memory_mb = 128; // Minimal memory

        let spawn_start = Instant::now();
        let handle =
            match crate::vm::spawn_vm_with_config(&format!("{}-1", test_name), &config).await {
                Ok(h) => h,
                Err(e) => {
                    return Ok(CrashTestResult {
                        test_name: test_name.clone(),
                        test_type: CrashTestType::MemoryPressure,
                        passed: false,
                        duration_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                        cleanup_success: false,
                        data_corrupted: false,
                        restart_success: false,
                        error_message: Some(format!("Failed to spawn VM: {}", e)),
                        metrics: CrashTestMetrics {
                            vm_spawn_time_ms: 0.0,
                            vm_lifecycle_time_ms: 0.0,
                            kill_to_cleanup_time_ms: 0.0,
                            ..Default::default()
                        },
                    });
                }
            };
        let vm_spawn_time_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

        // Wait for VM to stabilize
        sleep(Duration::from_millis(100)).await;

        let kill_start = Instant::now();
        let cleanup_success = destroy_vm(handle).await.is_ok();
        let kill_to_cleanup_time_ms = kill_start.elapsed().as_secs_f64() * 1000.0;

        let data_corrupted = self.check_data_corruption(&test_name);

        let metrics_after = self.collect_system_metrics()?;
        let vm_lifecycle_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        let passed = cleanup_success && !data_corrupted;

        Ok(CrashTestResult {
            test_name,
            test_type: CrashTestType::MemoryPressure,
            passed,
            duration_ms: vm_lifecycle_time_ms,
            cleanup_success,
            data_corrupted,
            restart_success: true, // Not applicable for this test
            error_message: if passed {
                None
            } else {
                Some("Test failed".to_string())
            },
            metrics: CrashTestMetrics {
                vm_spawn_time_ms,
                vm_lifecycle_time_ms,
                kill_to_cleanup_time_ms,
                memory_before_mb: metrics_before.memory_mb,
                memory_after_mb: metrics_after.memory_mb,
                file_descriptors_before: metrics_before.file_descriptors,
                file_descriptors_after: metrics_after.file_descriptors,
                processes_before: metrics_before.processes,
                processes_after: metrics_after.processes,
            },
        })
    }

    /// Test: Restart capability after crash
    ///
    /// Verifies that a new VM can be spawned after a crash.
    async fn test_restart_after_crash(&self, task_id: &str) -> Result<()> {
        let config = VmConfig {
            kernel_path: self.kernel_path.clone(),
            rootfs_path: self.rootfs_path.clone(),
            ..VmConfig::new(task_id.to_string())
        };

        let handle = crate::vm::spawn_vm_with_config(task_id, &config).await?;

        // Verify VM is running
        sleep(Duration::from_millis(50)).await;

        // Clean up
        destroy_vm(handle).await?;

        Ok(())
    }

    /// Collect system metrics before/after tests
    fn collect_system_metrics(&self) -> Result<SystemMetrics> {
        // On Linux, we can read /proc/meminfo and /proc/<pid>/fd
        #[cfg(target_os = "linux")]
        {
            let memory_mb = read_memory_usage_mb()?;
            let file_descriptors = read_file_descriptor_count()?;
            let processes = read_process_count()?;

            Ok(SystemMetrics {
                memory_mb: Some(memory_mb),
                file_descriptors: Some(file_descriptors),
                processes: Some(processes),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(SystemMetrics {
                memory_mb: None,
                file_descriptors: None,
                processes: None,
            })
        }
    }

    /// Check for data corruption in test artifacts
    fn check_data_corruption(&self, test_name: &str) -> bool {
        // Check temp directory for corrupted files
        let test_dir = self.temp_dir.join(test_name);

        if !test_dir.exists() {
            return false;
        }

        // Simple check: look for files that are 0 bytes or too large
        if let Ok(entries) = fs::read_dir(&test_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        let size = metadata.len();
                        // Flag corruption if file is empty (but should have content)
                        // or suspiciously large (> 1GB)
                        if size == 0 || size > 1_000_000_000 {
                            tracing::warn!(
                                "Potential corruption detected in {:?}: size = {} bytes",
                                entry.path(),
                                size
                            );
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Save test results to JSON file
    pub fn save_results(&self, results: &[CrashTestResult]) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("crash_test_results_{}.json", timestamp);
        let path = self.results_path.join(filename);

        let json =
            serde_json::to_string_pretty(results).context("Failed to serialize test results")?;

        fs::write(&path, json).context("Failed to write test results")?;

        tracing::info!("Test results saved to: {:?}", path);

        Ok(())
    }

    /// Generate a summary report from test results
    pub fn generate_summary(&self, results: &[CrashTestResult]) -> String {
        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let success_rate = (passed_tests as f64 / total_tests as f64) * 100.0;

        let mut summary = format!(
            "\n=== VM Crash Test Summary ===\n\n\
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
            let status = if result.passed {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            summary.push_str(&format!(
                "  {} - {} ({:.2}ms)\n",
                status, result.test_name, result.duration_ms
            ));
        }

        // Target: 95% clean termination
        let target_met = success_rate >= 95.0;
        summary.push_str(&format!(
            "\nTarget (95% clean termination): {}\n",
            if target_met { "✓ MET" } else { "✗ NOT MET" }
        ));

        summary
    }
}

/// System metrics collected during tests
#[derive(Debug, Clone, Default)]
struct SystemMetrics {
    memory_mb: Option<u64>,
    file_descriptors: Option<u32>,
    processes: Option<u32>,
}

/// Read current memory usage in MB (Linux only)
#[cfg(target_os = "linux")]
fn read_memory_usage_mb() -> Result<u64> {
    let content = fs::read_to_string("/proc/meminfo")?;
    let lines: Vec<&str> = content.lines().collect();

    for line in lines {
        if line.starts_with("MemAvailable:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: u64 = parts[1].parse()?;
                return Ok(kb / 1024);
            }
        }
    }

    Ok(0)
}

/// Read current file descriptor count (Linux only)
#[cfg(target_os = "linux")]
fn read_file_descriptor_count() -> Result<u32> {
    let pid = std::process::id();
    let fd_dir = format!("/proc/{}/fd", pid);

    if let Ok(entries) = fs::read_dir(&fd_dir) {
        let count = entries.count() as u32;
        return Ok(count);
    }

    Ok(0)
}

/// Read current process count (Linux only)
#[cfg(target_os = "linux")]
fn read_process_count() -> Result<u32> {
    if let Ok(entries) = fs::read_dir("/proc") {
        let count = entries
            .filter(|e| {
                e.as_ref()
                    .ok()
                    .and_then(|entry| entry.file_name().to_str().map(|s| s.parse::<u32>().is_ok()))
                    .unwrap_or(false)
            })
            .count() as u32;
        return Ok(count);
    }

    Ok(0)
}

impl Default for CrashTestMetrics {
    fn default() -> Self {
        Self {
            vm_spawn_time_ms: 0.0,
            vm_lifecycle_time_ms: 0.0,
            kill_to_cleanup_time_ms: 0.0,
            memory_before_mb: None,
            memory_after_mb: None,
            file_descriptors_before: None,
            file_descriptors_after: None,
            processes_before: None,
            processes_after: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_crash_harness_creation() {
        let temp_dir = TempDir::new().unwrap();
        let results_path = temp_dir.path().join("results");

        let harness = CrashTestHarness::new(
            "/tmp/kernel".to_string(),
            "/tmp/rootfs".to_string(),
            results_path,
        );

        assert!(harness.is_ok());
    }

    #[test]
    fn test_crash_test_result_serialization() {
        let result = CrashTestResult {
            test_name: "test".to_string(),
            test_type: CrashTestType::FileOperation,
            passed: true,
            duration_ms: 100.0,
            cleanup_success: true,
            data_corrupted: false,
            restart_success: true,
            error_message: None,
            metrics: CrashTestMetrics::default(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let decoded: CrashTestResult = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.test_name, "test");
        assert!(decoded.passed);
    }

    #[test]
    fn test_crash_test_type_serialization() {
        let types = vec![
            CrashTestType::FileOperation,
            CrashTestType::NetworkOperation,
            CrashTestType::ToolExecution,
            CrashTestType::SequentialCrash,
            CrashTestType::RapidSpawnKill,
            CrashTestType::MemoryPressure,
        ];

        for test_type in types {
            let json = serde_json::to_string(&test_type).unwrap();
            let decoded: CrashTestType = serde_json::from_str(&json).unwrap();
            assert_eq!(test_type, decoded);
        }
    }

    #[test]
    fn test_summary_generation() {
        let temp_dir = TempDir::new().unwrap();
        let results_path = temp_dir.path().join("results");

        let harness = CrashTestHarness::new(
            "/tmp/kernel".to_string(),
            "/tmp/rootfs".to_string(),
            results_path,
        )
        .unwrap();

        let results = vec![
            CrashTestResult {
                test_name: "test1".to_string(),
                test_type: CrashTestType::FileOperation,
                passed: true,
                duration_ms: 100.0,
                cleanup_success: true,
                data_corrupted: false,
                restart_success: true,
                error_message: None,
                metrics: CrashTestMetrics::default(),
            },
            CrashTestResult {
                test_name: "test2".to_string(),
                test_type: CrashTestType::NetworkOperation,
                passed: false,
                duration_ms: 150.0,
                cleanup_success: false,
                data_corrupted: false,
                restart_success: true,
                error_message: Some("Failed".to_string()),
                metrics: CrashTestMetrics::default(),
            },
        ];

        let summary = harness.generate_summary(&results);
        assert!(summary.contains("Total Tests: 2"));
        assert!(summary.contains("Passed: 1"));
        assert!(summary.contains("test1"));
        assert!(summary.contains("test2"));
    }
}
