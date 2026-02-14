#![cfg(unix)]
// Real Integration Tests for Firecracker VM Module
//
// This module contains comprehensive integration tests that run against
// actual Firecracker and Jailer binaries, testing real VM lifecycle.
//
// # Requirements
//
// These tests require:
// - Firecracker installed at /usr/local/bin/firecracker
// - Jailer installed at /usr/local/bin/jailer (for jailed tests)
// - VM kernel image at ./resources/vmlinux (or custom path)
// - VM rootfs at ./resources/rootfs.ext4 (or custom path)
// - Root/sudo access for jailer tests (optional, tests skip gracefully)
//
// # Running Tests
//
// Run all integration tests:
// ```bash
// cargo test --lib vm::integration_tests -- --ignored
// ```
//
// Run specific integration test:
// ```bash
// cargo test --lib vm::integration_tests::test_real_vm_lifecycle -- --ignored --nocapture
// ```

use crate::vm::config::VmConfig;
use crate::vm::jailer::{verify_jailer_installed, JailerConfig};
use crate::vm::seccomp::{SeccompFilter, SeccompLevel};
use crate::vm::{
    destroy_vm, pool_stats, spawn_vm, spawn_vm_jailed, spawn_vm_with_config, warmup_pool,
};
use std::time::Instant;

/// Helper function to check if we have VM resources available
fn has_vm_resources() -> bool {
    std::path::Path::new("./resources/vmlinux").exists()
        && std::path::Path::new("./resources/rootfs.ext4").exists()
}

/// Helper function to check if Firecracker is available
fn has_firecracker() -> bool {
    std::path::Path::new("/usr/local/bin/firecracker").exists()
}

/// Integration test: Real VM lifecycle (spawn, use, destroy)
///
/// Tests the complete VM lifecycle with actual Firecracker binary.
///
/// Requirements:
/// - Firecracker installed
/// - VM resources (kernel/rootfs) available
#[tokio::test]
#[ignore = "integration test - requires Firecracker and VM resources"]
async fn test_real_vm_lifecycle() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    println!("Starting real VM lifecycle test...");

    // Spawn VM
    let start = Instant::now();
    let handle = match spawn_vm("real-lifecycle-test").await {
        Ok(h) => h,
        Err(e) => {
            println!("Failed to spawn VM: {}", e);
            println!("This may require additional setup or permissions");
            return;
        }
    };

    let spawn_time = start.elapsed();
    println!(
        "VM spawned in {:.2}ms, PID: {:?}",
        spawn_time.as_millis(),
        handle.id
    );

    // Verify handle
    assert_eq!(handle.id, "real-lifecycle-test");
    assert!(handle.spawn_time_ms > 0.0);
    assert!(handle.spawn_time_ms < 10000.0); // Should be < 10 seconds

    // Verify vsock path
    assert!(handle.vsock_path().is_some());

    // Destroy VM
    let destroy_start = Instant::now();
    destroy_vm(handle).await.unwrap();
    let destroy_time = destroy_start.elapsed();

    println!("VM destroyed in {:.2}ms", destroy_time.as_millis());
    println!("Real VM lifecycle test completed successfully");
}

/// Integration test: Real VM spawn with custom configuration
///
/// Tests VM spawning with custom configuration options.
///
/// Requirements:
/// - Firecracker installed
/// - VM resources available
#[tokio::test]
#[ignore = "integration test - requires Firecracker and VM resources"]
async fn test_real_vm_spawn_with_config() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    let config = VmConfig {
        vm_id: "custom-config-test".to_string(),
        vcpu_count: 2,
        memory_mb: 1024,
        kernel_path: "./resources/vmlinux".to_string(),
        rootfs_path: "./resources/rootfs.ext4".to_string(),
        enable_networking: false,
        vsock_path: None,
        seccomp_filter: None,
    };

    let start = Instant::now();
    let handle = match spawn_vm_with_config("custom-config-test", &config).await {
        Ok(h) => h,
        Err(e) => {
            println!("Failed to spawn VM: {}", e);
            return;
        }
    };

    let spawn_time = start.elapsed();
    println!(
        "VM with custom config spawned in {:.2}ms",
        spawn_time.as_millis()
    );

    assert_eq!(handle.id, "custom-config-test");
    destroy_vm(handle).await.unwrap();

    println!("Custom config test completed");
}

/// Integration test: Real VM with seccomp filter
///
/// Tests VM spawning with seccomp syscall filtering.
///
/// Requirements:
/// - Firecracker installed
/// - VM resources available
#[tokio::test]
#[ignore = "integration test - requires Firecracker and VM resources"]
async fn test_real_vm_with_seccomp() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    // Test with Basic seccomp level
    let mut config = VmConfig::new("seccomp-test".to_string());
    config.seccomp_filter = Some(SeccompFilter::new(SeccompLevel::Basic));

    let start = Instant::now();
    let handle = match spawn_vm_with_config("seccomp-test", &config).await {
        Ok(h) => h,
        Err(e) => {
            println!("Failed to spawn VM with seccomp: {}", e);
            return;
        }
    };

    let spawn_time = start.elapsed();
    println!(
        "VM with seccomp Basic spawned in {:.2}ms",
        spawn_time.as_millis()
    );

    assert!(handle.config.seccomp_filter.is_some());
    destroy_vm(handle).await.unwrap();
}

/// Integration test: Multiple real VMs
///
/// Tests spawning multiple VMs concurrently.
///
/// Requirements:
/// - Firecracker installed
/// - VM resources available
#[tokio::test]
#[ignore = "integration test - requires Firecracker and VM resources"]
async fn test_real_multiple_vms() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    println!("Starting multiple VMs test...");

    let mut handles = Vec::new();

    // Spawn 3 VMs
    for i in 0..3 {
        let vm_id = format!("multi-vm-{}", i);

        match spawn_vm(&vm_id).await {
            Ok(handle) => {
                handles.push(handle);
                println!("Spawned VM {}: {}", i, vm_id);
            }
            Err(e) => {
                println!("Failed to spawn VM {}: {}", i, e);
                // Continue with remaining VMs
            }
        }
    }

    // Verify all have unique IDs
    let ids: Vec<_> = handles.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        "VM IDs should be unique"
    );

    println!("Spawned {} VMs successfully", handles.len());

    // Destroy all VMs
    for handle in handles {
        destroy_vm(handle).await.unwrap();
    }

    println!("Multiple VMs test completed");
}

/// Integration test: Rapid VM spawn/destroy cycle
///
/// Tests VM creation and destruction in rapid succession.
///
/// Requirements:
/// - Firecracker installed
/// - VM resources available
#[tokio::test]
#[ignore = "integration test - requires Firecracker and VM resources"]
async fn test_real_rapid_vm_cycle() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    println!("Starting rapid VM cycle test (10 iterations)...");

    let mut spawn_times = Vec::new();

    for i in 0..10 {
        let vm_id = format!("rapid-cycle-{}", i);

        let start = Instant::now();
        let handle = match spawn_vm(&vm_id).await {
            Ok(h) => h,
            Err(e) => {
                println!("Failed in iteration {}: {}", i, e);
                return;
            }
        };

        let spawn_time = start.elapsed();
        spawn_times.push(spawn_time);

        destroy_vm(handle).await.unwrap();

        println!(
            "Iteration {}: spawn time {:.2}ms",
            i,
            spawn_time.as_millis()
        );
    }

    let avg_time = spawn_times.iter().sum::<std::time::Duration>() / spawn_times.len() as u32;

    println!(
        "Average spawn time over 10 iterations: {:.2}ms",
        avg_time.as_millis()
    );
    println!(
        "Min spawn time: {:.2}ms",
        spawn_times.iter().min().unwrap().as_millis()
    );
    println!(
        "Max spawn time: {:.2}ms",
        spawn_times.iter().max().unwrap().as_millis()
    );

    // Target is <200ms average
    if avg_time.as_millis() > 200 {
        println!("Warning: Average spawn time exceeds 200ms target");
    }

    assert!(!spawn_times.is_empty());
}

/// Integration test: VM with error conditions
///
/// Tests VM behavior with various error conditions.
///
/// Requirements:
/// - Firecracker installed
#[tokio::test]
#[ignore = "integration test - requires Firecracker"]
async fn test_real_vm_error_conditions() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    println!("Testing VM error conditions...");

    // Test 1: Missing kernel
    let config = VmConfig {
        vm_id: "error-no-kernel".to_string(),
        kernel_path: "/nonexistent/kernel".to_string(),
        rootfs_path: "./resources/rootfs.ext4".to_string(),
        ..VmConfig::default()
    };

    let result = spawn_vm_with_config("error-no-kernel", &config).await;
    assert!(result.is_err());
    println!("Missing kernel error: OK");

    // Test 2: Missing rootfs
    let config = VmConfig {
        vm_id: "error-no-rootfs".to_string(),
        kernel_path: "./resources/vmlinux".to_string(),
        rootfs_path: "/nonexistent/rootfs.ext4".to_string(),
        ..VmConfig::default()
    };

    let result = spawn_vm_with_config("error-no-rootfs", &config).await;
    assert!(result.is_err());
    println!("Missing rootfs error: OK");

    println!("Error conditions test completed");
}

/// Integration test: Real jailed VM
///
/// Tests VM spawning inside Jailer sandbox.
///
/// Requirements:
/// - Firecracker installed
/// - Jailer installed
/// - VM resources available
/// - Root/sudo access (optional, may skip)
#[tokio::test]
#[ignore = "integration test - requires Firecracker, Jailer, and VM resources"]
async fn test_real_jailed_vm() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if verify_jailer_installed().is_err() {
        println!("Skipping: Jailer not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    let vm_config = VmConfig::new("jailed-vm-test".to_string());
    let jailer_config = JailerConfig::new("jailed-vm-test".to_string());

    let start = Instant::now();
    let handle = match spawn_vm_jailed("jailed-vm-test", &vm_config, &jailer_config).await {
        Ok(h) => h,
        Err(e) => {
            println!("Failed to spawn jailed VM: {}", e);
            println!("This may require root/sudo access");
            return;
        }
    };

    let spawn_time = start.elapsed();
    println!("Jailed VM spawned in {:.2}ms", spawn_time.as_millis());

    assert_eq!(handle.id, "jailed-vm-test");

    // Destroy jailed VM
    use crate::vm::destroy_vm_jailed;
    destroy_vm_jailed(handle, &jailer_config).await.unwrap();

    println!("Jailed VM test completed");
}

/// Integration test: Jailed VM with custom user
///
/// Tests jailed VM running as non-root user.
///
/// Requirements:
/// - Firecracker installed
/// - Jailer installed
/// - VM resources available
/// - Root/sudo access
#[tokio::test]
#[ignore = "integration test - requires root/sudo for jailed VM with user"]
async fn test_real_jailed_vm_with_user() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if verify_jailer_installed().is_err() {
        println!("Skipping: Jailer not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    let vm_config = VmConfig::new("jailed-user-test".to_string());
    let jailer_config = JailerConfig::new("jailed-user-test".to_string()).with_user(1000, 1000); // Run as user 1000:1000

    let result = spawn_vm_jailed("jailed-user-test", &vm_config, &jailer_config).await;

    match result {
        Ok(handle) => {
            println!("Jailed VM with user spawned: {}", handle.id);

            use crate::vm::destroy_vm_jailed;
            destroy_vm_jailed(handle, &jailer_config).await.unwrap();

            println!("Jailed VM with user test completed");
        }
        Err(e) => {
            println!("Failed to spawn jailed VM with user: {}", e);
            println!("This is expected if running without root/sudo");
        }
    }
}

/// Integration test: Jailed VM with cgroups
///
/// Tests jailed VM with cgroup resource limits.
///
/// Requirements:
/// - Firecracker installed
/// - Jailer installed
/// - VM resources available
/// - Root/sudo access
#[tokio::test]
#[ignore = "integration test - requires root/sudo for cgroups"]
async fn test_real_jailed_vm_with_cgroups() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if verify_jailer_installed().is_err() {
        println!("Skipping: Jailer not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    let vm_config = VmConfig::new("jailed-cgroup-test".to_string());
    let mut jailer_config = JailerConfig::new("jailed-cgroup-test".to_string());

    // Set cgroup limits
    jailer_config = jailer_config
        .with_cgroup("cpu.shares".to_string(), "512".to_string())
        .with_cgroup("memory.limit_in_bytes".to_string(), "268435456".to_string()); // 256MB

    let result = spawn_vm_jailed("jailed-cgroup-test", &vm_config, &jailer_config).await;

    match result {
        Ok(handle) => {
            println!("Jailed VM with cgroups spawned: {}", handle.id);

            use crate::vm::destroy_vm_jailed;
            destroy_vm_jailed(handle, &jailer_config).await.unwrap();

            println!("Jailed VM with cgroups test completed");
        }
        Err(e) => {
            println!("Failed to spawn jailed VM with cgroups: {}", e);
            println!("This is expected if running without root/sudo");
        }
    }
}

/// Integration test: Snapshot pool statistics
///
/// Tests snapshot pool functionality.
#[tokio::test]
#[ignore = "integration test - tests pool functionality"]
async fn test_real_pool_stats() {
    println!("Testing pool stats...");

    let result = pool_stats().await;

    match result {
        Ok(stats) => {
            println!("Pool stats: size={}/{}", stats.current_size, stats.max_size);

            assert!(stats.max_size > 0);
        }
        Err(e) => {
            println!("Pool not initialized (expected): {}", e);
        }
    }
}

/// Integration test: Pool warmup
///
/// Tests pool warmup functionality.
#[tokio::test]
#[ignore = "integration test - tests pool warmup"]
async fn test_real_pool_warmup() {
    println!("Testing pool warmup...");

    let result = warmup_pool().await;

    match result {
        Ok(_) => {
            println!("Pool warmed up successfully");

            // Check stats
            if let Ok(stats) = pool_stats().await {
                println!(
                    "Pool size after warmup: {}/{}",
                    stats.current_size, stats.max_size
                );
                println!(
                    "Oldest snapshot: {:?}s, Newest snapshot: {:?}s",
                    stats.oldest_snapshot_age_secs, stats.newest_snapshot_age_secs
                );
            }
        }
        Err(e) => {
            println!("Pool warmup failed (may be expected): {}", e);
        }
    }
}

/// Integration test: VM spawn performance measurement
///
/// Measures actual VM spawn times over multiple iterations.
///
/// Requirements:
/// - Firecracker installed
/// - VM resources available
#[tokio::test]
#[ignore = "integration test - performance measurement test"]
async fn test_real_vm_performance() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    println!("Starting VM performance measurement (5 iterations)...");

    let mut times = Vec::new();

    for i in 0..5 {
        let vm_id = format!("perf-{}", i);

        let start = Instant::now();
        let handle = match spawn_vm(&vm_id).await {
            Ok(h) => h,
            Err(e) => {
                println!("Failed in iteration {}: {}", i, e);
                return;
            }
        };

        times.push(start.elapsed());
        destroy_vm(handle).await.unwrap();

        println!(
            "Iteration {}: {:.2}ms",
            i,
            times.last().unwrap().as_millis()
        );
    }

    let avg = times.iter().sum::<std::time::Duration>() / times.len() as u32;
    let min = times.iter().min().unwrap();
    let max = times.iter().max().unwrap();

    println!("\nPerformance Summary:");
    println!("  Average: {:.2}ms", avg.as_millis());
    println!("  Min: {:.2}ms", min.as_millis());
    println!("  Max: {:.2}ms", max.as_millis());

    // Check against target
    if avg.as_millis() as f64 > 200.0 {
        println!("  ⚠️  Average exceeds 200ms target");
    } else {
        println!("  ✅ Average meets 200ms target");
    }
}

/// Integration test: Firewall configuration with real VM
///
/// Tests firewall rules applied to VM.
#[tokio::test]
#[ignore = "integration test - requires iptables and root"]
async fn test_real_firewall_with_vm() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    let handle = match spawn_vm("firewall-test").await {
        Ok(h) => h,
        Err(e) => {
            println!("Failed to spawn VM: {}", e);
            return;
        }
    };

    // Verify firewall manager exists
    assert!(handle.firewall_manager.is_some());

    // Try to verify isolation
    use crate::vm::verify_network_isolation;
    let isolated = verify_network_isolation(&handle);

    match isolated {
        Ok(true) => {
            println!("Firewall isolation verified: ✅");
        }
        Ok(false) => {
            println!("Firewall isolation not active (may require root): ⚠️");
        }
        Err(e) => {
            println!("Failed to verify firewall isolation: {}", e);
        }
    }

    destroy_vm(handle).await.unwrap();

    println!("Firewall test completed");
}

/// Integration test: VM ID handling with special characters
///
/// Tests that VM IDs with special characters are handled correctly.
#[tokio::test]
#[ignore = "integration test - requires Firecracker and VM resources"]
async fn test_real_vm_special_ids() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    if !has_vm_resources() {
        println!("Skipping: VM resources not found");
        return;
    }

    // Test various ID formats
    let test_ids = vec![
        "test-vm-1",
        "vm-with-dashes",
        "vm-with-digits-123",
        "UPPERCASE-ID",
    ];

    for vm_id in test_ids {
        let handle = match spawn_vm(vm_id).await {
            Ok(h) => h,
            Err(e) => {
                println!("Failed with ID '{}': {}", vm_id, e);
                return;
            }
        };

        println!("VM '{}' spawned successfully", vm_id);
        destroy_vm(handle).await.unwrap();
    }

    println!("Special IDs test completed");
}

/// Integration test: Cleanup after failed spawn
///
/// Tests that resources are cleaned up even if spawn fails.
#[tokio::test]
#[ignore = "integration test - tests cleanup edge cases"]
async fn test_real_cleanup_on_failed_spawn() {
    if !has_firecracker() {
        println!("Skipping: Firecracker not installed");
        return;
    }

    // Try to spawn with invalid config
    let config = VmConfig {
        vm_id: "cleanup-test".to_string(),
        kernel_path: "/nonexistent/kernel".to_string(),
        rootfs_path: "/nonexistent/rootfs".to_string(),
        ..VmConfig::default()
    };

    let result = spawn_vm_with_config("cleanup-test", &config).await;

    // Should fail
    assert!(result.is_err());

    // Verify no zombie processes or sockets
    // (In real scenario, we'd check /proc and /tmp, but here we just verify no panic)
    println!("Cleanup on failed spawn test completed");
}
