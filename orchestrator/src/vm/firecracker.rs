// Firecracker Integration
//
// This module will handle the actual Firecracker VM spawning.
// Placeholder for Phase 2 implementation.

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
#[cfg(unix)]
use serde::Serialize;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::Instant;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::process::{Child, Command};
#[cfg(unix)]
use tracing::{debug, info};

use crate::vm::config::VmConfig;

/// Firecracker VM process manager
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
    #[cfg(unix)]
    pub child_process: Option<Child>,
    #[cfg(not(unix))]
    pub child_process: Option<()>, // Dummy for non-unix
    pub spawn_time_ms: f64,
}

// Firecracker API structs

#[derive(Serialize)]
#[cfg(unix)]
struct BootSource {
    kernel_image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    boot_args: Option<String>,
}

#[derive(Serialize)]
#[cfg(unix)]
struct Drive {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

#[derive(Serialize)]
#[cfg(unix)]
struct MachineConfiguration {
    vcpu_count: u8,
    mem_size_mib: u32,
    // ht_enabled: bool, // Optional, defaults to false
}

#[derive(Serialize)]
#[cfg(unix)]
struct Action {
    action_type: String,
}

/// Start a Firecracker VM process
#[cfg(unix)]
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
        pid: 0,
        socket_path: "/tmp/firecracker.sock".to_string(),
    })
}

/// Stop a Firecracker VM process
#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
async fn configure_vm(socket_path: &str, config: &VmConfig) -> Result<()> {
    // 1. Set Boot Source
    let boot_source = BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off".to_string()),
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
    let rootfs = Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: config.rootfs_path.clone(),
        is_root_device: true,
        is_read_only: false,
    };
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

#[cfg(unix)]
async fn start_instance(socket_path: &str) -> Result<()> {
    let action = Action {
        action_type: "InstanceStart".to_string(),
    };
    send_request(socket_path, hyper::Method::PUT, "/actions", Some(&action))
        .await
        .context("Failed to start instance")?;
    Ok(())
}

// Dummy implementations for non-unix systems (Windows)
#[cfg(not(unix))]
pub async fn start_firecracker(_config: &VmConfig) -> anyhow::Result<FirecrackerProcess> {
    anyhow::bail!("Firecracker is only supported on Unix systems")
}

#[cfg(not(unix))]
pub async fn stop_firecracker(_process: FirecrackerProcess) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
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
    #[cfg(unix)]
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
    #[cfg(unix)]
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
}
