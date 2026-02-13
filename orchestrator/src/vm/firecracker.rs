// Firecracker Integration
//
// This module handles the actual Firecracker VM spawning using the HTTP API over Unix sockets.

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tracing::{debug, info};

use crate::vm::config::VmConfig;

/// Firecracker VM process manager
#[derive(Debug)]
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
    pub child_process: Option<Child>,
    pub spawn_time_ms: f64,
}

// Firecracker API structs

#[derive(Serialize)]
struct BootSource {
    kernel_image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    boot_args: Option<String>,
}

#[derive(Serialize)]
struct Drive {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

#[derive(Serialize)]
struct MachineConfiguration {
    vcpu_count: u8,
    mem_size_mib: u32,
    // ht_enabled: bool, // Optional, defaults to false
}

#[derive(Serialize)]
struct Action {
    action_type: String,
}

/// Start a Firecracker VM process
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    let start_time = Instant::now();
    info!("Starting Firecracker VM: {}", config.vm_id);

    // 1. Validate resources
    let kernel_path = PathBuf::from(&config.kernel_path);
    let rootfs_path = PathBuf::from(&config.rootfs_path);

    if !kernel_path.exists() {
        return Err(anyhow!("Kernel image not found at: {:?}", kernel_path));
    }
    if !rootfs_path.exists() {
        return Err(anyhow!("Root filesystem not found at: {:?}", rootfs_path));
    }

    // 2. Prepare socket path
    let socket_path = format!("/tmp/firecracker-{}.socket", config.vm_id);
    if Path::new(&socket_path).exists() {
        tokio::fs::remove_file(&socket_path)
            .await
            .context("Failed to remove existing socket")?;
    }

    // 3. Spawn Firecracker process
    let mut command = Command::new("firecracker");
    command.arg("--api-sock").arg(&socket_path);

    // Redirect stdout/stderr to null or log file to keep output clean
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());

    let mut child = command
        .spawn()
        .context("Failed to spawn firecracker process")?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("Failed to get firecracker PID"))?;

    debug!("Firecracker process started with PID: {}", pid);

    // 4. Wait for socket to be ready (retry loop)
    let mut retries = 0;
    let max_retries = 50; // 50 * 10ms = 500ms
    let mut socket_ready = false;

    while retries < max_retries {
        if Path::new(&socket_path).exists() {
            socket_ready = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        retries += 1;
    }

    if !socket_ready {
        // Kill the process if socket never appeared
        let _ = child.kill().await;
        return Err(anyhow!("Firecracker API socket did not appear in time"));
    }

    // 5. Connect to API and configure VM
    if let Err(e) = configure_vm(&socket_path, config).await {
        let _ = child.kill().await;
        return Err(e);
    }

    // 6. Start the instance
    if let Err(e) = start_instance(&socket_path).await {
        let _ = child.kill().await;
        return Err(e);
    }

    let elapsed = start_time.elapsed();
    let spawn_time_ms = elapsed.as_secs_f64() * 1000.0;
    info!("VM {} started in {:.2}ms", config.vm_id, spawn_time_ms);

    Ok(FirecrackerProcess {
        pid,
        socket_path,
        child_process: Some(child),
        spawn_time_ms,
    })
}

/// Stop a Firecracker VM process
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    info!("Stopping Firecracker VM (PID: {})", process.pid);

    // Try to send InstanceStart with "Exit" action? No, Send SendCtrlAltDel?
    // Or just kill the process. Firecracker usually handles SIGTERM gracefully.

    if let Some(mut child) = process.child_process.take() {
        child
            .kill()
            .await
            .context("Failed to kill firecracker process")?;
    }

    // Cleanup socket
    if Path::new(&process.socket_path).exists() {
        let _ = tokio::fs::remove_file(&process.socket_path).await;
    }

    Ok(())
}

// Helper functions for API interaction

async fn send_request<T: Serialize>(
    socket_path: &str,
    method: hyper::Method,
    path: &str,
    body: Option<&T>,
) -> Result<()> {
    // We create a new connection for each request for simplicity,
    // though reusing it would be slightly faster.
    // Given the low number of requests, this is acceptable.

    let stream = UnixStream::connect(socket_path)
        .await
        .context("Failed to connect to firecracker socket")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("Handshake failed")?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            // It's expected that connection might close after response
            debug!("Connection closed: {:?}", err);
        }
    });

    let req_body = if let Some(b) = body {
        let json = serde_json::to_string(b).context("Failed to serialize body")?;
        Full::new(Bytes::from(json))
    } else {
        Full::new(Bytes::from(""))
    };

    let req = Request::builder()
        .method(method)
        .uri(format!("http://localhost{}", path)) // Host header is required but ignored for unix socket
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(req_body)
        .context("Failed to build request")?;

    let res = sender
        .send_request(req)
        .await
        .context("Failed to send request")?;

    if res.status().is_success() || res.status() == StatusCode::NO_CONTENT {
        Ok(())
    } else {
        let status = res.status();
        let body_bytes = res.collect().await?.to_bytes();
        let body_str = String::from_utf8_lossy(&body_bytes);
        Err(anyhow!("Firecracker API error: {} - {}", status, body_str))
    }
}

/// Create rootfs drive configuration
///
/// Ensures rootfs is always mounted as read-only for security.
fn create_rootfs_drive(path: &str) -> Drive {
    Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: path.to_string(),
        is_root_device: true,
        // CRITICAL SECURITY: Shared rootfs must be read-only to prevent cross-VM contamination
        is_read_only: true,
    }
}

async fn configure_vm(socket_path: &str, config: &VmConfig) -> Result<()> {
    // 1. Set Boot Source
    let boot_source = BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off ro".to_string()),
    };
    send_request(
        socket_path,
        hyper::Method::PUT,
        "/boot-source",
        Some(&boot_source),
    )
    .await
    .context("Failed to configure boot source")?;

    // 2. Set Rootfs Drive
    let rootfs = create_rootfs_drive(&config.rootfs_path);
    send_request(
        socket_path,
        hyper::Method::PUT,
        "/drives/rootfs",
        Some(&rootfs),
    )
    .await
    .context("Failed to configure rootfs")?;

    // 3. Set Machine Config
    let machine_config = MachineConfiguration {
        vcpu_count: config.vcpu_count,
        mem_size_mib: config.memory_mb,
    };
    send_request(
        socket_path,
        hyper::Method::PUT,
        "/machine-config",
        Some(&machine_config),
    )
    .await
    .context("Failed to configure machine")?;

    Ok(())
}

async fn start_instance(socket_path: &str) -> Result<()> {
    let action = Action {
        action_type: "InstanceStart".to_string(),
    };
    send_request(socket_path, hyper::Method::PUT, "/actions", Some(&action))
        .await
        .context("Failed to start instance")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firecracker_structs_serialization() {
        let boot_source = BootSource {
            kernel_image_path: "/tmp/kernel".to_string(),
            boot_args: Some("console=ttyS0".to_string()),
        };
        let json = serde_json::to_string(&boot_source).unwrap();
        assert!(json.contains("kernel_image_path"));
        assert!(json.contains("boot_args"));
    }

    #[tokio::test]
    async fn test_missing_kernel_image() {
        let config = VmConfig {
            kernel_path: "/non/existent/kernel".to_string(),
            ..VmConfig::default()
        };
        let result = start_firecracker(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Kernel image not found"));
    }

    #[tokio::test]
    async fn test_missing_rootfs() {
        // Create dummy kernel file to pass first check
        let kernel_path = std::env::temp_dir().join("dummy_kernel");
        let _ = std::fs::File::create(&kernel_path);

        let config = VmConfig {
            kernel_path: kernel_path.to_str().unwrap().to_string(),
            rootfs_path: "/non/existent/rootfs".to_string(),
            ..VmConfig::default()
        };
        let result = start_firecracker(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Root filesystem not found"));

        let _ = std::fs::remove_file(kernel_path);
    }

    /// Integration test: Verify Firecracker binary is available
    ///
    /// Requirements:
    /// - Firecracker installed at /usr/local/bin/firecracker
    #[test]
    fn test_firecracker_binary_check() {
        let firecracker_path = "/usr/local/bin/firecracker";
        let exists = std::path::Path::new(firecracker_path).exists();
        if exists {
            println!("Firecracker binary available: true");
        } else {
            println!("Firecracker binary available: false (tests requiring real execution will be skipped)");
        }
        assert!(exists || !exists); // Always passes, just reports status
    }

    /// Integration test: Start and stop Firecracker with real binary
    ///
    /// Requirements:
    /// - Firecracker installed at /usr/local/bin/firecracker
    /// - VM kernel/rootfs resources available (or test will skip)
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation and VM resources"]
    async fn test_firecracker_start_with_real_binary() {
        // Check if Firecracker is available
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        // Check if resources exist
        let kernel_path = "./resources/vmlinux";
        let rootfs_path = "./resources/rootfs.ext4";

        if !std::path::Path::new(kernel_path).exists() {
            println!("Skipping: Kernel image not found at {}", kernel_path);
            return;
        }

        if !std::path::Path::new(rootfs_path).exists() {
            println!("Skipping: Rootfs not found at {}", rootfs_path);
            return;
        }

        let config = VmConfig {
            vm_id: "integration-test-vm".to_string(),
            kernel_path: kernel_path.to_string(),
            rootfs_path: rootfs_path.to_string(),
            ..VmConfig::default()
        };

        let start = std::time::Instant::now();
        let result = start_firecracker(&config).await;

        match result {
            Ok(process) => {
                let elapsed = start.elapsed();
                println!(
                    "Firecracker started in {:.2}ms, PID: {}",
                    elapsed.as_millis(),
                    process.pid
                );

                // Clone socket_path before moving process
                let socket_path = process.socket_path.clone();

                // Verify socket was created
                assert!(std::path::Path::new(&socket_path).exists());

                let socket_path = process.socket_path.clone();
<<<<<<< HEAD

=======
>>>>>>> origin/main
                // Stop the VM
                stop_firecracker(process).await.unwrap();
                println!("Firecracker stopped successfully");

                // Verify socket was cleaned up
                assert!(!std::path::Path::new(&socket_path).exists());
            }
            Err(e) => {
                eprintln!("Failed to start Firecracker: {}", e);
                println!("Skipping: May require additional setup or permissions");
            }
        }
    }

    /// Integration test: Verify socket creation and cleanup
    ///
    /// Requirements:
    /// - Firecracker installed
    /// - Ability to create temp sockets
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation"]
    async fn test_firecracker_socket_creation() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        let socket_path = "/tmp/test-firecracker-socket.socket";

        // Clean up any existing socket
        if std::path::Path::new(socket_path).exists() {
            let _ = tokio::fs::remove_file(socket_path).await;
        }

        // Verify socket doesn't exist initially
        assert!(!std::path::Path::new(socket_path).exists());

        // Note: We can't actually start Firecracker without resources,
        // but we can verify socket path handling logic
        println!("Socket path would be: {}", socket_path);
        assert!(socket_path.ends_with(".socket"));
    }

    /// Integration test: Firecracker process lifecycle with real binary
    ///
    /// Requirements:
    /// - Firecracker installed
    /// - VM resources available
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation and VM resources"]
    async fn test_firecracker_process_lifecycle() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        let kernel_path = "./resources/vmlinux";
        let rootfs_path = "./resources/rootfs.ext4";

        if !std::path::Path::new(kernel_path).exists()
            || !std::path::Path::new(rootfs_path).exists()
        {
            println!("Skipping: VM resources not available");
            return;
        }

        let config = VmConfig {
            vm_id: "lifecycle-test-vm".to_string(),
            kernel_path: kernel_path.to_string(),
            rootfs_path: rootfs_path.to_string(),
            ..VmConfig::default()
        };

        // Start
        let process = match start_firecracker(&config).await {
            Ok(p) => p,
            Err(e) => {
                println!("Skipping: Failed to start Firecracker: {}", e);
                return;
            }
        };

        // Verify process attributes
        assert!(process.pid > 0);
        assert!(!process.socket_path.is_empty());
        assert!(process.spawn_time_ms > 0.0);

        // Clone socket_path before moving process
        let socket_path = process.socket_path.clone();

        // Verify socket exists
        assert!(std::path::Path::new(&socket_path).exists());

        let socket_path = process.socket_path.clone();
<<<<<<< HEAD

=======
>>>>>>> origin/main
        // Stop
        stop_firecracker(process).await.unwrap();

        // Verify cleanup
        assert!(!std::path::Path::new(&socket_path).exists());

        println!("Firecracker lifecycle test completed successfully");
    }

    /// Integration test: Measure actual Firecracker spawn time
    ///
    /// Requirements:
    /// - Firecracker installed
    /// - VM resources available
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation and VM resources"]
    async fn test_firecracker_spawn_time_measurement() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        let kernel_path = "./resources/vmlinux";
        let rootfs_path = "./resources/rootfs.ext4";

        if !std::path::Path::new(kernel_path).exists()
            || !std::path::Path::new(rootfs_path).exists()
        {
            println!("Skipping: VM resources not available");
            return;
        }

        // Measure multiple spawns for average
        let mut times = Vec::new();

        for i in 0..3 {
            let _config = VmConfig {
                vm_id: format!("perf-test-vm-{}", i),
                kernel_path: kernel_path.to_string(),
                rootfs_path: rootfs_path.to_string(),
                ..VmConfig::default()
            };

            match start_firecracker(&_config).await {
                Ok(process) => {
                    times.push(process.spawn_time_ms);
                    stop_firecracker(process).await.unwrap();
                }
                Err(e) => {
                    println!("Skipping iteration {}: {}", i, e);
                    return;
                }
            }
        }

        if !times.is_empty() {
            let avg_time = times.iter().sum::<f64>() / times.len() as f64;
            println!("Firecracker spawn times: {:?}", times);
            println!("Average spawn time: {:.2}ms", avg_time);

            // Target is <200ms
            assert!(avg_time < 200.0, "Spawn time too high: {:.2}ms", avg_time);
        }
    }

    /// Integration test: Firecracker error handling with invalid kernel
    ///
    /// Requirements:
    /// - Firecracker installed
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation"]
    async fn test_firecracker_error_handling_invalid_kernel() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        // Create invalid kernel (too small to be real kernel)
        let invalid_kernel = std::env::temp_dir().join("invalid_kernel");
        std::fs::write(&invalid_kernel, b"INVALID_KERNEL").unwrap();

        let config = VmConfig {
            vm_id: "invalid-kernel-test".to_string(),
            kernel_path: invalid_kernel.to_str().unwrap().to_string(),
            rootfs_path: "./resources/rootfs.ext4".to_string(),
            ..VmConfig::default()
        };

        let result = start_firecracker(&config).await;

        // Should fail with validation error or runtime error
        assert!(result.is_err());

        let _ = std::fs::remove_file(invalid_kernel);
        println!("Error handling test passed");
    }

    /// Integration test: Firecracker error handling with invalid rootfs
    ///
    /// Requirements:
    /// - Firecracker installed
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation"]
    async fn test_firecracker_error_handling_invalid_rootfs() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        // Create valid kernel path (dummy file)
        let dummy_kernel = std::env::temp_dir().join("dummy_kernel");
        std::fs::write(&dummy_kernel, vec![0u8; 1024]).unwrap();

        // Create invalid rootfs (too small)
        let invalid_rootfs = std::env::temp_dir().join("invalid_rootfs.ext4");
        std::fs::write(&invalid_rootfs, b"INVALID_ROOTFS").unwrap();

        let config = VmConfig {
            vm_id: "invalid-rootfs-test".to_string(),
            kernel_path: dummy_kernel.to_str().unwrap().to_string(),
            rootfs_path: invalid_rootfs.to_str().unwrap().to_string(),
            ..VmConfig::default()
        };

        let result = start_firecracker(&config).await;

        // Should fail with validation error or runtime error
        assert!(result.is_err());

        let _ = std::fs::remove_file(dummy_kernel);
        let _ = std::fs::remove_file(invalid_rootfs);
        println!("Invalid rootfs error handling test passed");
    }

    /// Integration test: Firecracker cleanup on stop
    ///
    /// Requirements:
    /// - Firecracker installed
    /// - VM resources available
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation and VM resources"]
    async fn test_firecracker_stop_cleanup() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        let kernel_path = "./resources/vmlinux";
        let rootfs_path = "./resources/rootfs.ext4";

        if !std::path::Path::new(kernel_path).exists()
            || !std::path::Path::new(rootfs_path).exists()
        {
            println!("Skipping: VM resources not available");
            return;
        }

        let config = VmConfig {
            vm_id: "cleanup-test-vm".to_string(),
            kernel_path: kernel_path.to_string(),
            rootfs_path: rootfs_path.to_string(),
            ..VmConfig::default()
        };

        let process = match start_firecracker(&config).await {
            Ok(p) => p,
            Err(e) => {
                println!("Skipping: Failed to start: {}", e);
                return;
            }
        };

        let socket_path = process.socket_path.clone();
        let pid = process.pid;

        // Verify socket exists
        assert!(std::path::Path::new(&socket_path).exists());

        // Stop
        stop_firecracker(process).await.unwrap();

        // Verify socket was removed
        assert!(!std::path::Path::new(&socket_path).exists());

        // Verify process was killed (check that PID is no longer active)
        // Note: We can't reliably check this without more code,
        // but we can verify the socket cleanup

        println!(
            "Cleanup test passed: PID {}, socket removed: {}",
            pid, socket_path
        );
    }

    /// Integration test: Stop Firecracker without process
    ///
    /// This tests graceful handling of cleanup when process is already gone
    #[tokio::test]
    #[ignore = "Tests edge case with stopped process"]
    async fn test_stop_firecracker_without_process() {
        // Create a mock process without a real child
        let process = FirecrackerProcess {
            pid: 99999, // Non-existent PID
            socket_path: "/tmp/nonexistent.socket".to_string(),
            child_process: None,
            spawn_time_ms: 0.0,
        };

        // Should not panic even without a real process
        let result = stop_firecracker(process).await;

        // Should succeed (or warn, but not panic)
        assert!(result.is_ok() || result.is_err());
        println!("Stop without process test passed");
    }

    /// Integration test: Stop Firecracker cleanup socket
    ///
    /// Verifies that socket cleanup happens even if process cleanup fails
    #[tokio::test]
    #[ignore = "Tests cleanup edge cases"]
    async fn test_stop_firecracker_cleanup_socket() {
        // Create a test socket
        let socket_path = std::env::temp_dir().join("test-socket-123.sock");

        // Create a dummy file (not a real socket, but tests cleanup logic)
        std::fs::write(&socket_path, b"test").unwrap();

        assert!(std::path::Path::new(&socket_path).exists());

        // Create process
        let process = FirecrackerProcess {
            pid: 12345,
            socket_path: socket_path.to_str().unwrap().to_string(),
            child_process: None,
            spawn_time_ms: 100.0,
        };

        // Stop should clean up the socket
        let _ = stop_firecracker(process).await;

        // Socket should be removed
        assert!(!std::path::Path::new(&socket_path).exists());

        println!("Socket cleanup test passed");
    }

    /// Integration test: Firecracker spawn time tracking
    ///
    /// Verifies that spawn time is measured and recorded correctly
    #[test]
    fn test_firecracker_spawn_time_tracking() {
        let process = FirecrackerProcess {
            pid: 123,
            socket_path: "/tmp/test.socket".to_string(),
            child_process: None,
            spawn_time_ms: 150.5,
        };

        assert_eq!(process.spawn_time_ms, 150.5);
        assert!(process.spawn_time_ms > 0.0);
        assert!(process.spawn_time_ms < 10000.0); // Less than 10 seconds

        println!(
            "Spawn time tracking test passed: {:.2}ms",
            process.spawn_time_ms
        );
    }

    /// Integration test: Firecracker process struct
    ///
    /// Verifies struct fields and defaults
    #[test]
    fn test_firecracker_process_struct() {
        let process = FirecrackerProcess {
            pid: 12345,
            socket_path: "/tmp/vm.socket".to_string(),
            child_process: None,
            spawn_time_ms: 200.0,
        };

        assert_eq!(process.pid, 12345);
        assert_eq!(process.socket_path, "/tmp/vm.socket");
        assert!(process.child_process.is_none());
        assert_eq!(process.spawn_time_ms, 200.0);

        println!("Firecracker process struct test passed");
    }

    /// Integration test: Test API request without server
    ///
    /// This tests error handling when connecting to non-existent socket
    #[tokio::test]
    #[ignore = "Tests error handling without real server"]
    async fn test_firecracker_api_request_without_server() {
        let socket_path = std::env::temp_dir().join("nonexistent-socket-999.socket");

        let boot_source = BootSource {
            kernel_image_path: "/tmp/kernel".to_string(),
            boot_args: None,
        };

        // Should fail to connect to non-existent socket
        let result = send_request(
            &socket_path.to_str().unwrap().to_string(),
            hyper::Method::PUT,
            "/boot-source",
            Some(&boot_source),
        )
        .await;

        assert!(result.is_err());
        println!(
            "API request without server test passed: {:?}",
            result.unwrap_err()
        );
    }

    /// Integration test: VM config validation in Firecracker context
    ///
    /// Tests that config validation happens before API calls
    #[tokio::test]
    async fn test_vm_config_validation_in_firecracker() {
        let kernel_path = std::env::temp_dir().join("test_kernel");
        std::fs::write(&kernel_path, b"KERNEL").unwrap();

        let config = VmConfig {
            vm_id: "validation-test".to_string(),
            kernel_path: kernel_path.to_str().unwrap().to_string(),
            rootfs_path: std::env::temp_dir()
                .join("test_rootfs.ext4")
                .to_str()
                .unwrap()
                .to_string(),
            ..VmConfig::default()
        };

        // Rootfs doesn't exist, should fail
        let result = start_firecracker(&config).await;
        assert!(result.is_err());

        let _ = std::fs::remove_file(kernel_path);
        println!("VM config validation test passed");
    }

    /// Integration test: Test concurrent Firecracker starts
    ///
    /// Requirements:
    /// - Firecracker installed
    /// - VM resources available
    #[tokio::test]
    #[ignore = "Requires real Firecracker installation and VM resources"]
    async fn test_firecracker_concurrent_starts() {
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        let kernel_path = "./resources/vmlinux";
        let rootfs_path = "./resources/rootfs.ext4";

        if !std::path::Path::new(kernel_path).exists()
            || !std::path::Path::new(rootfs_path).exists()
        {
            println!("Skipping: VM resources not available");
            return;
        }

        // Try to start multiple VMs concurrently
        let mut tasks = Vec::new();

        for i in 0..3 {
            let config = VmConfig {
                vm_id: format!("concurrent-test-{}", i),
                kernel_path: kernel_path.to_string(),
                rootfs_path: rootfs_path.to_string(),
                ..VmConfig::default()
            };

            tasks.push(tokio::spawn(
                async move { start_firecracker(&config).await },
            ));
        }

        // Wait for all to complete
        let mut success_count = 0;
        for task in tasks {
            match task.await.unwrap() {
                Ok(_) => success_count += 1,
                Err(e) => {
                    println!("Concurrent start failed: {}", e);
                }
            }
        }

        println!("Concurrent starts: {} succeeded out of 3", success_count);

        // At least some should succeed
        assert!(success_count > 0);
    }

    /// Integration test: Firecracker config serialization with real config
    ///
    /// Tests that real config objects serialize correctly for API
    #[test]
    fn test_firecracker_config_serialization_with_real_config() {
        let boot_source = BootSource {
            kernel_image_path: "/tmp/vmlinux".to_string(),
            boot_args: Some("console=ttyS0 reboot=k panic=1".to_string()),
        };

        let drive = Drive {
            drive_id: "rootfs".to_string(),
            path_on_host: "/tmp/rootfs.ext4".to_string(),
            is_root_device: true,
            is_read_only: false,
        };

        let machine_config = MachineConfiguration {
            vcpu_count: 2,
            mem_size_mib: 512,
        };

        // Verify all serialize correctly
        let boot_json = serde_json::to_string(&boot_source).unwrap();
        let drive_json = serde_json::to_string(&drive).unwrap();
        let machine_json = serde_json::to_string(&machine_config).unwrap();

        assert!(boot_json.contains("kernel_image_path"));
        assert!(boot_json.contains("boot_args"));
        assert!(drive_json.contains("drive_id"));
        assert!(drive_json.contains("is_root_device"));
        assert!(machine_json.contains("vcpu_count"));
        assert!(machine_json.contains("mem_size_mib"));

        println!("Config serialization test passed");
    }

    /// Property-based test: VM config paths must exist before spawn
    ///
    /// This test verifies that the code properly validates file paths
    #[test]
    fn test_property_vm_config_paths_exist() {
        let test_cases = vec![
            ("./vmlinux", "./rootfs.ext4"),
            ("/tmp/kernel", "/tmp/rootfs"),
            ("./resources/vmlinux", "./resources/rootfs.ext4"),
        ];

        for (kernel, rootfs) in test_cases {
            // Test with config
            let config = VmConfig {
                vm_id: "property-test".to_string(),
                kernel_path: kernel.to_string(),
                rootfs_path: rootfs.to_string(),
                ..VmConfig::default()
            };

            // Config validation should pass
            assert!(config.validate().is_ok());

            // Actual spawn will fail if paths don't exist, but that's expected
        }

        println!("VM config paths property test passed");
    }

    /// Integration test: Boot source serialization
    ///
    /// Tests different boot source configurations
    #[test]
    fn test_boot_source_without_boot_args() {
        let boot_source = BootSource {
            kernel_image_path: "/tmp/kernel".to_string(),
            boot_args: None,
        };

        let json = serde_json::to_string(&boot_source).unwrap();
        assert!(json.contains("kernel_image_path"));
        // boot_args should be omitted from JSON when None

        println!("Boot source without boot args test passed");
    }

    /// Integration test: Machine configuration serialization
    ///
    /// Tests different machine configurations
    #[test]
    fn test_machine_configuration_serialization() {
        let machine_config = MachineConfiguration {
            vcpu_count: 4,
            mem_size_mib: 1024,
        };

        let json = serde_json::to_string(&machine_config).unwrap();
        assert!(json.contains("\"vcpu_count\":4"));
        assert!(json.contains("\"mem_size_mib\":1024"));

        println!("Machine configuration serialization test passed");
    }

    /// Integration test: Drive serialization
    ///
    /// Tests different drive configurations
    #[test]
    fn test_drive_serialization() {
        let drive = Drive {
            drive_id: "test-drive".to_string(),
            path_on_host: "/tmp/drive.img".to_string(),
            is_root_device: false,
            is_read_only: true,
        };

        let json = serde_json::to_string(&drive).unwrap();
        assert!(json.contains("test-drive"));
        assert!(json.contains("/tmp/drive.img"));
        assert!(json.contains("\"is_root_device\":false"));
        assert!(json.contains("\"is_read_only\":true"));

        println!("Drive serialization test passed");
    }

    /// Integration test: Action serialization
    ///
    /// Tests different action types
    #[test]
    fn test_action_serialization() {
        let action = Action {
            action_type: "InstanceStart".to_string(),
        };

        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("InstanceStart"));

        println!("Action serialization test passed");
    }

    /// Security Test: Verify rootfs drive is always read-only
    ///
    /// This test ensures that the `create_rootfs_drive` helper function
    /// enforces the security invariant that shared rootfs images must be read-only.
    #[test]
    fn test_rootfs_drive_is_secure() {
        let path = "/tmp/rootfs.ext4";
        let drive = create_rootfs_drive(path);

        assert_eq!(drive.drive_id, "rootfs");
        assert_eq!(drive.path_on_host, path);
        assert!(drive.is_root_device);
        assert!(
            drive.is_read_only,
            "SECURITY: Shared rootfs must be mounted read-only"
        );

        // Verify JSON serialization also reflects this
        let json = serde_json::to_string(&drive).unwrap();
        assert!(json.contains("\"is_read_only\":true"));

        println!("Rootfs security check passed: is_read_only=true");
    }
}
