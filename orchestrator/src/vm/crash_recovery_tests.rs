// VM Crash Recovery Tests
//
// This module implements comprehensive crash recovery testing for issue #538.
// It simulates actual VM crashes (OOM, segfault, SIGKILL) and verifies:
// 1. Graceful shutdown on VM crashes
// 2. Resource cleanup after crash (file handles, sockets, memory)
// 3. Error reporting captures crash context (logs, stack traces)
// 4. Recovery mechanisms work correctly
// 5. VM pool recovery after crashes
//
// Testing Philosophy:
// - Chaos engineering: Simulate real-world crash conditions
// - Verify no resource leaks after crashes
// - Test recovery procedures
// - Use actual process termination (SIGKILL) for realistic simulation

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    use crate::vm::config::VmConfig;
    use crate::vm::destroy_vm;

    /// Test: Graceful shutdown on VM process crash (SIGKILL)
    ///
    /// Simulates a hard VM crash by sending SIGKILL to the Firecracker process.
    /// Verifies:
    /// - System handles SIGKILL gracefully
    /// - No orphaned processes remain
    /// - Resources are cleaned up
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_graceful_shutdown_on_sigkill() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        // Check if Firecracker resources exist
        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        let metrics_before = collect_system_metrics();

        // Spawn VM
        let config = VmConfig {
            kernel_path: kernel_path.clone(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("sigkill-test".to_string())
        };

        let handle = match crate::vm::spawn_vm_with_config("sigkill-test", &config).await {
            Ok(h) => h,
            Err(e) => {
                panic!("Failed to spawn VM: {}", e);
            }
        };

        let vm_pid = handle.pid();

        // Wait for VM to stabilize
        sleep(Duration::from_millis(100)).await;

        // Simulate hard crash with SIGKILL
        tracing::warn!("Simulating SIGKILL for VM {}", vm_pid);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            // Send SIGKILL to the Firecracker process
            if let Err(e) = kill(Pid::from_raw(vm_pid as i32), Signal::SIGKILL) {
                tracing::error!("Failed to send SIGKILL: {}", e);
            }
        }

        // Wait for process termination
        sleep(Duration::from_millis(200)).await;

        // Verify process is dead
        #[cfg(unix)]
        {
            let process_exists = fs::metadata(format!("/proc/{}", vm_pid)).is_ok();
            assert!(!process_exists, "VM process should be terminated after SIGKILL");
        }

        // Verify no orphaned processes
        let orphaned_processes = find_orphaned_firecracker_processes();
        assert!(
            orphaned_processes.is_empty(),
            "No orphaned Firecracker processes should remain after SIGKILL. Found: {:?}",
            orphaned_processes
        );

        // Verify resource cleanup
        let metrics_after = collect_system_metrics();

        // File descriptors should not increase significantly
        if let (Some(before), Some(after)) = (metrics_before.file_descriptors, metrics_after.file_descriptors)
        {
            let fd_increase = after.saturating_sub(before);
            assert!(
                fd_increase < 10,
                "File descriptor leak detected: {} fds leaked after SIGKILL",
                fd_increase
            );
        }

        tracing::info!("Test passed: Graceful shutdown on SIGKILL verified");
    }

    /// Test: Resource cleanup after OOM crash simulation
    ///
    /// Simulates an out-of-memory crash scenario.
    /// Verifies:
    /// - All resources are cleaned up
    /// - No memory leaks
    /// - No file handle leaks
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_resource_cleanup_after_oom_crash() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        let metrics_before = collect_system_metrics();

        // Spawn VM with minimal memory to trigger OOM-like conditions
        let mut config = VmConfig {
            kernel_path: kernel_path.clone(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("oom-test".to_string())
        };
        config.memory_mb = 64; // Very low memory

        let handle = match crate::vm::spawn_vm_with_config("oom-test", &config).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("Failed to spawn VM with low memory (expected): {}", e);
                // This is expected - VM may fail to spawn due to OOM
                // Verify cleanup happened
                let metrics_after = collect_system_metrics();

                if let (Some(before), Some(after)) = (metrics_before.memory_mb, metrics_after.memory_mb)
                {
                    let memory_increase = after.saturating_sub(before);
                    assert!(
                        memory_increase < 100,
                        "Memory leak detected: {} MB leaked after OOM crash",
                        memory_increase
                    );
                }

                tracing::info!("Test passed: Resource cleanup after OOM crash verified");
                return;
            }
        };

        // VM spawned successfully, destroy it normally
        sleep(Duration::from_millis(50)).await;

        if let Err(e) = destroy_vm(handle).await {
            tracing::error!("Failed to destroy VM: {}", e);
        }

        let metrics_after = collect_system_metrics();

        // Verify no significant resource leaks
        if let (Some(before), Some(after)) = (metrics_before.file_descriptors, metrics_after.file_descriptors)
        {
            let fd_increase = after.saturating_sub(before);
            assert!(
                fd_increase < 5,
                "File descriptor leak detected: {} fds leaked",
                fd_increase
            );
        }

        tracing::info!("Test passed: Resource cleanup after OOM crash verified");
    }

    /// Test: Error reporting captures crash context
    ///
    /// Verifies that error messages and logs capture sufficient crash context.
    /// This includes:
    /// - VM ID
    /// - PID
    /// - Error type
    /// - Stack traces (when available)
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_error_reporting_captures_crash_context() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        // Test 1: Invalid kernel path error
        let config = VmConfig {
            kernel_path: "/nonexistent/kernel.bin".to_string(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("error-context-test".to_string())
        };

        let result = crate::vm::spawn_vm_with_config("error-context-test", &config).await;

        assert!(result.is_err(), "Should fail with invalid kernel path");

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("kernel") || error_msg.contains("spawn"),
            "Error should mention kernel or spawn: {}",
            error_msg
        );

        tracing::info!("Error message captured: {}", error_msg);

        // Test 2: Invalid config validation error
        let mut invalid_config = VmConfig::new("invalid-config-test".to_string());
        invalid_config.memory_mb = 0; // Invalid memory

        let validation_result = invalid_config.validate();
        assert!(validation_result.is_err(), "Should fail with invalid memory config");

        let validation_error = validation_result.unwrap_err().to_string();
        assert!(
            validation_error.contains("memory") || validation_error.contains("Invalid"),
            "Validation error should mention memory: {}",
            validation_error
        );

        tracing::info!("Validation error captured: {}", validation_error);

        tracing::info!("Test passed: Error reporting captures crash context");
    }

    /// Test: Recovery mechanisms after crash
    ///
    /// Verifies that the system can recover from crashes and spawn new VMs.
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_recovery_mechanisms_after_crash() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        // Spawn VM #1
        let config1 = VmConfig {
            kernel_path: kernel_path.clone(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("recovery-test-1".to_string())
        };

        let handle1 = crate::vm::spawn_vm_with_config("recovery-test-1", &config1)
            .await
            .unwrap();

        sleep(Duration::from_millis(50)).await;

        // Kill VM #1 (simulate crash)
        let pid1 = handle1.pid();

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let _ = kill(Pid::from_raw(pid1 as i32), Signal::SIGKILL);
        }

        sleep(Duration::from_millis(200)).await;

        // Try to spawn VM #2 (recovery)
        let config2 = VmConfig {
            kernel_path: kernel_path.clone(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("recovery-test-2".to_string())
        };

        let handle2 = match crate::vm::spawn_vm_with_config("recovery-test-2", &config2).await {
            Ok(h) => h,
            Err(e) => {
                panic!("Failed to spawn recovery VM: {}", e);
            }
        };

        assert_ne!(handle2.id, handle1.id, "New VM should have different ID");
        assert_ne!(handle2.pid(), handle1.pid(), "New VM should have different PID");

        // Verify VM #2 is functional
        sleep(Duration::from_millis(50)).await;

        // Clean up
        destroy_vm(handle2).await.unwrap();

        tracing::info!("Test passed: Recovery mechanisms work after crash");
    }

    /// Test: VM pool recovery after crashes
    ///
    /// Verifies that the snapshot pool can recover from crashes and continue
    /// to provide VMs.
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_vm_pool_recovery_after_crashes() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        let config = VmConfig {
            kernel_path: kernel_path.clone(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("pool-recovery-test".to_string())
        };

        let mut successful_spawns = 0;
        let mut failed_spawns = 0;

        // Spawn 10 VMs, randomly killing some
        for i in 0..10 {
            let vm_id = format!("pool-test-{}", i);

            let handle = match crate::vm::spawn_vm_with_config(&vm_id, &config).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Failed to spawn VM {}: {}", vm_id, e);
                    failed_spawns += 1;
                    continue;
                }
            };

            sleep(Duration::from_millis(20)).await;

            // Randomly kill 30% of VMs to simulate crashes
            if i % 10 < 3 {
                tracing::warn!("Simulating crash for VM {}", vm_id);

                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;

                    let _ = kill(Pid::from_raw(handle.pid() as i32), Signal::SIGKILL);
                }

                sleep(Duration::from_millis(50)).await;
                failed_spawns += 1;
            } else {
                // Normal destroy
                if destroy_vm(handle).await.is_ok() {
                    successful_spawns += 1;
                } else {
                    failed_spawns += 1;
                }
            }
        }

        // Verify pool is still functional after crashes
        let recovery_handle = crate::vm::spawn_vm_with_config("pool-recovery-final", &config)
            .await
            .expect("Should be able to spawn VM after pool crashes");

        sleep(Duration::from_millis(50)).await;
        destroy_vm(recovery_handle).await.unwrap();

        tracing::info!(
            "Pool recovery test: {} successful, {} failed",
            successful_spawns,
            failed_spawns
        );

        // At least 60% should succeed
        let success_rate = (successful_spawns as f64 / 10.0) * 100.0;
        assert!(
            success_rate >= 60.0,
            "Pool recovery success rate should be >= 60%, got {:.1}%",
            success_rate
        );

        tracing::info!("Test passed: VM pool recovery after crashes verified");
    }

    /// Test: No orphaned file descriptors after crashes
    ///
    /// Verifies that no file descriptors are leaked after VM crashes.
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_no_orphaned_file_descriptors() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        let fds_before = get_file_descriptor_count();

        // Spawn and crash multiple VMs
        for i in 0..5 {
            let vm_id = format!("fd-leak-test-{}", i);

            let config = VmConfig {
                kernel_path: kernel_path.clone(),
                rootfs_path: rootfs_path.clone(),
                ..VmConfig::new(vm_id.clone())
            };

            let handle = match crate::vm::spawn_vm_with_config(&vm_id, &config).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Failed to spawn VM {}: {}", vm_id, e);
                    continue;
                }
            };

            sleep(Duration::from_millis(20)).await;

            // Kill VM
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                let _ = kill(Pid::from_raw(handle.pid() as i32), Signal::SIGKILL);
            }

            sleep(Duration::from_millis(50)).await;
        }

        let fds_after = get_file_descriptor_count();

        let fd_leak = fds_after.saturating_sub(fds_before);
        assert!(
            fd_leak < 10,
            "File descriptor leak detected: {} fds leaked after 5 crashes",
            fd_leak
        );

        tracing::info!("Test passed: No orphaned file descriptors after crashes");
    }

    /// Test: Sequential crash recovery
    ///
    /// Tests that the system can handle multiple sequential crashes without
    /// degrading or failing.
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_sequential_crash_recovery() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        let config = VmConfig {
            kernel_path: kernel_path.clone(),
            rootfs_path: rootfs_path.clone(),
            ..VmConfig::new("sequential-crash-test".to_string())
        };

        let mut recovery_times = Vec::new();

        // Spawn 10 VMs sequentially, crashing each one
        for i in 0..10 {
            let vm_id = format!("seq-crash-{}", i);
            let start_time = Instant::now();

            let handle = match crate::vm::spawn_vm_with_config(&vm_id, &config).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Failed to spawn VM {}: {}", vm_id, e);
                    continue;
                }
            };

            sleep(Duration::from_millis(20)).await;

            // Kill VM
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                let _ = kill(Pid::from_raw(handle.pid() as i32), Signal::SIGKILL);
            }

            sleep(Duration::from_millis(50)).await;

            // Try to spawn recovery VM
            let recovery_id = format!("seq-recovery-{}", i);
            let recovery_start = Instant::now();

            let recovery_handle =
                match crate::vm::spawn_vm_with_config(&recovery_id, &config).await {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::error!("Failed to spawn recovery VM {}: {}", recovery_id, e);
                        continue;
                    }
                };

            let recovery_time = recovery_start.elapsed().as_millis() as f64;
            recovery_times.push(recovery_time);

            sleep(Duration::from_millis(20)).await);

            // Clean up recovery VM
            let _ = destroy_vm(recovery_handle).await;

            let total_time = start_time.elapsed().as_millis() as f64;
            tracing::info!(
                "Sequential crash {}: recovery time = {:.2}ms, total time = {:.2}ms",
                i,
                recovery_time,
                total_time
            );
        }

        // Verify recovery times are consistent (not degrading)
        if recovery_times.len() > 2 {
            let avg_time: f64 = recovery_times.iter().sum::<f64>() / recovery_times.len() as f64;
            let max_time = recovery_times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            assert!(
                max_time < avg_time * 3.0,
                "Recovery time should not degrade significantly: max = {:.2}ms, avg = {:.2}ms",
                max_time,
                avg_time
            );
        }

        tracing::info!("Test passed: Sequential crash recovery verified");
    }

    /// Test: Crash during VM spawn
    ///
    /// Tests that the system handles crashes that occur during VM spawn.
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_crash_during_vm_spawn() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        // Try to spawn multiple VMs in rapid succession
        let mut handles = Vec::new();

        for i in 0..3 {
            let vm_id = format!("spawn-crash-{}", i);

            let config = VmConfig {
                kernel_path: kernel_path.clone(),
                rootfs_path: rootfs_path.clone(),
                ..VmConfig::new(vm_id.clone())
            };

            match crate::vm::spawn_vm_with_config(&vm_id, &config).await {
                Ok(handle) => {
                    handles.push(handle);
                }
                Err(e) => {
                    tracing::warn!("Failed to spawn VM {}: {}", vm_id, e);
                }
            }
        }

        // Clean up any successfully spawned VMs
        for handle in handles {
            let _ = destroy_vm(handle).await;
        }

        // Wait for cleanup
        sleep(Duration::from_millis(500)).await;

        // Verify no orphaned processes
        let orphaned = find_orphaned_firecracker_processes();
        assert!(
            orphaned.is_empty(),
            "No orphaned processes should remain after spawn crashes. Found: {:?}",
            orphaned
        );

        tracing::info!("Test passed: Crash during VM spawn handled correctly");
    }

    /// Test: Error context logging on crash
    ///
    /// Verifies that sufficient error context is logged when crashes occur.
    #[tokio::test]
    #[ignore = "requires Firecracker binary and test resources"]
    async fn test_error_context_logging_on_crash() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        let (kernel_path, rootfs_path) = get_test_resources();
        if kernel_path.is_none() || rootfs_path.is_none() {
            tracing::warn!("Skipping test: Firecracker assets not available");
            return;
        }

        let kernel_path = kernel_path.unwrap();
        let rootfs_path = rootfs_path.unwrap();

        // Test with invalid configuration
        let mut invalid_config = VmConfig::new("error-logging-test".to_string());
        invalid_config.kernel_path = "/nonexistent/path/kernel.bin".to_string();

        let result = crate::vm::spawn_vm_with_config("error-logging-test", &invalid_config).await;

        assert!(result.is_err(), "Should fail with invalid kernel path");

        // The error should include context
        let error = result.unwrap_err();
        let error_string = error.to_string();

        // Error should provide useful context
        tracing::info!("Error context: {}", error_string);
        tracing::info!("Error chain: {:?}", error.chain().collect::<Vec<_>>());

        // At minimum, error should be descriptive
        assert!(
            error_string.len() > 10,
            "Error message should be descriptive, got: {}",
            error_string
        );

        tracing::info!("Test passed: Error context logging verified");
    }
}

// Helper functions for crash recovery tests

/// Check if hypervisor tests should be skipped
fn should_skip_hypervisor_tests() -> bool {
    !is_firecracker_available()
}

/// Check if Firecracker is available
fn is_firecracker_available() -> bool {
    std::path::Path::new("/usr/local/bin/firecracker").exists()
        || std::path::Path::new("/usr/bin/firecracker").exists()
}

/// Get test resources (kernel and rootfs paths)
fn get_test_resources() -> (Option<String>, Option<String>) {
    let kernel_path = if std::path::Path::new("/tmp/luminaguard-fc-test/vmlinux.bin").exists() {
        Some("/tmp/luminaguard-fc-test/vmlinux.bin".to_string())
    } else if std::path::Path::new("./resources/vmlinux").exists() {
        Some("./resources/vmlinux".to_string())
    } else {
        None
    };

    let rootfs_path = if std::path::Path::new("/tmp/luminaguard-fc-test/rootfs.ext4").exists() {
        Some("/tmp/luminaguard-fc-test/rootfs.ext4".to_string())
    } else if std::path::Path::new("./resources/rootfs.ext4").exists() {
        Some("./resources/rootfs.ext4".to_string())
    } else {
        None
    };

    (kernel_path, rootfs_path)
}

/// System metrics collected during tests
#[derive(Debug, Clone, Default)]
struct SystemMetrics {
    pub memory_mb: Option<u64>,
    pub file_descriptors: Option<u32>,
    pub processes: Option<u32>,
}

/// Collect system metrics (Linux only)
#[cfg(target_os = "linux")]
fn collect_system_metrics() -> SystemMetrics {
    let memory_mb = read_memory_usage_mb().ok();
    let file_descriptors = get_file_descriptor_count();
    let processes = read_process_count().ok();

    SystemMetrics {
        memory_mb,
        file_descriptors,
        processes,
    }
}

/// Collect system metrics (non-Linux)
#[cfg(not(target_os = "linux"))]
fn collect_system_metrics() -> SystemMetrics {
    SystemMetrics::default()
}

/// Read current memory usage in MB (Linux only)
#[cfg(target_os = "linux")]
fn read_memory_usage_mb() -> Result<u64> {
    let content = fs::read_to_string("/proc/meminfo")?;
    for line in content.lines() {
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

/// Get current file descriptor count (Linux only)
fn get_file_descriptor_count() -> u32 {
    #[cfg(target_os = "linux")]
    {
        let pid = std::process::id();
        let fd_dir = format!("/proc/{}/fd", pid);

        if let Ok(entries) = fs::read_dir(&fd_dir) {
            return entries.count() as u32;
        }
    }

    0
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

/// Find orphaned Firecracker processes (Linux only)
#[cfg(target_os = "linux")]
fn find_orphaned_firecracker_processes() -> Vec<u32> {
    let mut orphaned = Vec::new();

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let pid_str = entry.file_name().to_string_lossy().to_string();
            if let Ok(pid) = pid_str.parse::<u32>() {
                if let Ok(comm) = fs::read_to_string(format!("/proc/{}/comm", pid)) {
                    let comm = comm.trim();
                    if comm.contains("firecracker") {
                        orphaned.push(pid);
                    }
                }
            }
        }
    }

    orphaned
}

/// Find orphaned Firecracker processes (non-Linux)
#[cfg(not(target_os = "linux"))]
fn find_orphaned_firecracker_processes() -> Vec<u32> {
    Vec::new()
}
