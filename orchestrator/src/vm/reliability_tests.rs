// VM Reliability Integration Tests
//
// These tests verify the VM crash testing harness works correctly
// and can be run as part of the test suite.

#[cfg(test)]
mod tests {
    /// Test that crash test harness can be created
    #[test]
    fn test_crash_harness_creation() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let results_path = temp_dir.path().join("results");

        let harness = crate::vm::reliability::CrashTestHarness::new(
            "/tmp/kernel".to_string(),
            "/tmp/rootfs".to_string(),
            results_path,
        );

        assert!(harness.is_ok());
    }

    /// Test crash test result serialization
    #[test]
    fn test_crash_test_result_serialization() {
        use crate::vm::reliability::{CrashTestMetrics, CrashTestResult, CrashTestType};

        let result = CrashTestResult {
            test_name: "test_crash".to_string(),
            test_type: CrashTestType::FileOperation,
            passed: true,
            duration_ms: 150.5,
            cleanup_success: true,
            data_corrupted: false,
            restart_success: true,
            error_message: None,
            metrics: CrashTestMetrics::default(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let decoded: CrashTestResult = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.test_name, "test_crash");
        assert!(decoded.passed);
        assert_eq!(decoded.test_type, CrashTestType::FileOperation);
        assert_eq!(decoded.duration_ms, 150.5);
    }

    /// Test all crash test types serialize correctly
    #[test]
    fn test_all_crash_test_types() {
        use crate::vm::reliability::CrashTestType;

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

    /// Test crash test metrics with values
    #[test]
    fn test_crash_test_metrics_with_values() {
        use crate::vm::reliability::CrashTestMetrics;

        let metrics = CrashTestMetrics {
            vm_spawn_time_ms: 110.5,
            vm_lifecycle_time_ms: 250.0,
            kill_to_cleanup_time_ms: 15.5,
            memory_before_mb: Some(4096),
            memory_after_mb: Some(4080),
            file_descriptors_before: Some(100),
            file_descriptors_after: Some(95),
            processes_before: Some(200),
            processes_after: Some(198),
        };

        assert_eq!(metrics.vm_spawn_time_ms, 110.5);
        assert_eq!(metrics.memory_before_mb, Some(4096));
        assert_eq!(metrics.memory_after_mb, Some(4080));
        assert_eq!(metrics.file_descriptors_before, Some(100));
        assert_eq!(metrics.file_descriptors_after, Some(95));
    }

    /// Test summary generation with mixed results
    #[test]
    fn test_summary_generation_with_mixed_results() {
        use crate::vm::reliability::{
            CrashTestHarness, CrashTestMetrics, CrashTestResult, CrashTestType,
        };

        let temp_dir = tempfile::TempDir::new().unwrap();
        let results_path = temp_dir.path().join("results");

        let harness = CrashTestHarness::new(
            "/tmp/kernel".to_string(),
            "/tmp/rootfs".to_string(),
            results_path,
        )
        .unwrap();

        let results = vec![
            CrashTestResult {
                test_name: "file_op_test".to_string(),
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
                test_name: "network_test".to_string(),
                test_type: CrashTestType::NetworkOperation,
                passed: false,
                duration_ms: 150.0,
                cleanup_success: false,
                data_corrupted: false,
                restart_success: true,
                error_message: Some("Network error".to_string()),
                metrics: CrashTestMetrics::default(),
            },
            CrashTestResult {
                test_name: "tool_exec_test".to_string(),
                test_type: CrashTestType::ToolExecution,
                passed: true,
                duration_ms: 120.0,
                cleanup_success: true,
                data_corrupted: false,
                restart_success: true,
                error_message: None,
                metrics: CrashTestMetrics::default(),
            },
        ];

        let summary = harness.generate_summary(&results);

        assert!(summary.contains("Total Tests: 3"));
        assert!(summary.contains("Passed: 2"));
        assert!(summary.contains("Failed: 1"));
        assert!(summary.contains("66.7%")); // 2/3 * 100
        assert!(summary.contains("file_op_test"));
        assert!(summary.contains("network_test"));
        assert!(summary.contains("tool_exec_test"));
    }

    /// Test summary generation with all passing tests
    #[test]
    fn test_summary_generation_all_passing() {
        use crate::vm::reliability::{
            CrashTestHarness, CrashTestMetrics, CrashTestResult, CrashTestType,
        };

        let temp_dir = tempfile::TempDir::new().unwrap();
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
                passed: true,
                duration_ms: 110.0,
                cleanup_success: true,
                data_corrupted: false,
                restart_success: true,
                error_message: None,
                metrics: CrashTestMetrics::default(),
            },
        ];

        let summary = harness.generate_summary(&results);

        assert!(summary.contains("Total Tests: 2"));
        assert!(summary.contains("Passed: 2"));
        assert!(summary.contains("Failed: 0"));
        assert!(summary.contains("100.0%"));
        assert!(summary.contains("MET")); // Target met
    }

    /// Test summary generation with target not met
    #[test]
    fn test_summary_generation_target_not_met() {
        use crate::vm::reliability::{
            CrashTestHarness, CrashTestMetrics, CrashTestResult, CrashTestType,
        };

        let temp_dir = tempfile::TempDir::new().unwrap();
        let results_path = temp_dir.path().join("results");

        let harness = CrashTestHarness::new(
            "/tmp/kernel".to_string(),
            "/tmp/rootfs".to_string(),
            results_path,
        )
        .unwrap();

        // Create 10 tests where only 8 pass (80% < 95% target)
        let results: Vec<CrashTestResult> = (0..10)
            .map(|i| CrashTestResult {
                test_name: format!("test{}", i),
                test_type: CrashTestType::FileOperation,
                passed: i < 8, // First 8 pass, last 2 fail
                duration_ms: 100.0,
                cleanup_success: i < 8,
                data_corrupted: false,
                restart_success: true,
                error_message: if i < 8 {
                    None
                } else {
                    Some("Failed".to_string())
                },
                metrics: CrashTestMetrics::default(),
            })
            .collect();

        let summary = harness.generate_summary(&results);

        assert!(summary.contains("Total Tests: 10"));
        assert!(summary.contains("Passed: 8"));
        assert!(summary.contains("Failed: 2"));
        assert!(summary.contains("80.0%"));
        assert!(summary.contains("NOT MET")); // Target not met
    }

    /// Test that results can be saved to file
    #[test]
    fn test_save_results_to_file() {
        use crate::vm::reliability::{
            CrashTestHarness, CrashTestMetrics, CrashTestResult, CrashTestType,
        };

        let temp_dir = tempfile::TempDir::new().unwrap();
        let results_path = temp_dir.path().join("results");

        let harness = CrashTestHarness::new(
            "/tmp/kernel".to_string(),
            "/tmp/rootfs".to_string(),
            results_path.clone(),
        )
        .unwrap();

        let results = vec![CrashTestResult {
            test_name: "test_save".to_string(),
            test_type: CrashTestType::FileOperation,
            passed: true,
            duration_ms: 100.0,
            cleanup_success: true,
            data_corrupted: false,
            restart_success: true,
            error_message: None,
            metrics: CrashTestMetrics::default(),
        }];

        let result = harness.save_results(&results);
        assert!(result.is_ok());

        // Check that file was created
        let entries: Vec<_> = std::fs::read_dir(&results_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert!(!entries.is_empty(), "Results file should be created");

        // Verify JSON can be parsed
        let entry = &entries[0];
        let content = std::fs::read_to_string(entry.path()).unwrap();
        let loaded: Vec<CrashTestResult> = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].test_name, "test_save");
    }

    /// Test default crash test metrics
    #[test]
    fn test_default_crash_test_metrics() {
        use crate::vm::reliability::CrashTestMetrics;

        let metrics = CrashTestMetrics::default();

        assert_eq!(metrics.vm_spawn_time_ms, 0.0);
        assert_eq!(metrics.vm_lifecycle_time_ms, 0.0);
        assert_eq!(metrics.kill_to_cleanup_time_ms, 0.0);
        assert!(metrics.memory_before_mb.is_none());
        assert!(metrics.memory_after_mb.is_none());
        assert!(metrics.file_descriptors_before.is_none());
        assert!(metrics.file_descriptors_after.is_none());
        assert!(metrics.processes_before.is_none());
        assert!(metrics.processes_after.is_none());
    }

    /// Test crash test result with error message
    #[test]
    fn test_crash_test_result_with_error() {
        use crate::vm::reliability::{CrashTestMetrics, CrashTestResult, CrashTestType};

        let result = CrashTestResult {
            test_name: "test_error".to_string(),
            test_type: CrashTestType::ToolExecution,
            passed: false,
            duration_ms: 200.0,
            cleanup_success: false,
            data_corrupted: true,
            restart_success: false,
            error_message: Some("VM failed to spawn: kernel not found".to_string()),
            metrics: CrashTestMetrics::default(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let decoded: CrashTestResult = serde_json::from_str(&json).unwrap();

        assert!(!decoded.passed);
        assert!(decoded.data_corrupted);
        assert!(!decoded.restart_success);
        assert_eq!(
            decoded.error_message,
            Some("VM failed to spawn: kernel not found".to_string())
        );
    }

    /// Test that sequential crash test type exists
    #[test]
    fn test_sequential_crash_test_type() {
        use crate::vm::reliability::CrashTestType;

        let test_type = CrashTestType::SequentialCrash;

        let json = serde_json::to_string(&test_type).unwrap();
        let decoded: CrashTestType = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, CrashTestType::SequentialCrash);
        assert!(json.contains("SequentialCrash"));
    }

    /// Test rapid spawn kill test type
    #[test]
    fn test_rapid_spawn_kill_test_type() {
        use crate::vm::reliability::CrashTestType;

        let test_type = CrashTestType::RapidSpawnKill;

        let json = serde_json::to_string(&test_type).unwrap();
        let decoded: CrashTestType = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, CrashTestType::RapidSpawnKill);
        assert!(json.contains("RapidSpawnKill"));
    }

    /// Test memory pressure test type
    #[test]
    fn test_memory_pressure_test_type() {
        use crate::vm::reliability::CrashTestType;

        let test_type = CrashTestType::MemoryPressure;

        let json = serde_json::to_string(&test_type).unwrap();
        let decoded: CrashTestType = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, CrashTestType::MemoryPressure);
        assert!(json.contains("MemoryPressure"));
    }
}
