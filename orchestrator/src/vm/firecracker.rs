// JIT Micro-VM - Firecracker Process Management
//
// This module handles the actual Firecracker VM spawning using the HTTP API over Unix sockets.

#![cfg(target_os = "linux")]

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
use tracing::{debug, info, warn};

use crate::vm::config::VmConfig;

/// Represents a running Firecracker process
pub struct FirecrackerProcess {
    child_process: Option<Child>,
    vm_id: String,
    pub spawn_time_ms: f64,
}

/// Firecracker boot source configuration
#[derive(Serialize)]
struct BootSource {
    kernel_image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    boot_args: Option<String>,
}

/// Firecracker drive configuration
#[derive(Serialize)]
struct Drive {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

/// Firecracker machine configuration
#[derive(Serialize)]
struct MachineConfiguration {
    vcpu_count: u8,
    mem_size_mib: u32,
}

/// Start a Firecracker VM
///
/// # Arguments
///
/// * `config` - VM configuration
///
/// # Returns
///
/// * `FirecrackerProcess` - Running process handle
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    let start_time = Instant::now();

    // 1. Verify resources exist
    if !Path::new(&config.kernel_path).exists() {
        return Err(anyhow!(
            "Kernel image not found at: {}",
            config.kernel_path
        ));
    }
    if !Path::new(&config.rootfs_path).exists() {
        return Err(anyhow!("Root filesystem not found at: {}", config.rootfs_path));
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

    // Redirect stdout/stderr to null to avoid cluttering logs
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());

    let mut child = command
        .spawn()
        .context("Failed to spawn firecracker process")?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("Failed to get firecracker PID"))?;

    debug!("Firecracker process started with PID: {}", pid);

    // 4. Wait for socket to be ready
    let mut retries = 0;
    while !Path::new(&socket_path).exists() {
        if retries > 50 {
            let _ = child.kill();
            return Err(anyhow!("Timed out waiting for firecracker socket"));
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        retries += 1;
    }

    // 5. Configure VM via HTTP API
    if let Err(e) = configure_vm(&socket_path, config).await {
        let _ = child.kill().await;
        return Err(e.context("Failed to configure VM"));
    }

    // 6. Start VM
    if let Err(e) = send_action(&socket_path, "InstanceStart").await {
        let _ = child.kill().await;
        return Err(e.context("Failed to start VM instance"));
    }

    let spawn_time = start_time.elapsed().as_secs_f64() * 1000.0;
    info!(
        "VM {} started successfully in {:.2}ms",
        config.vm_id, spawn_time
    );

    Ok(FirecrackerProcess {
        child_process: Some(child),
        vm_id: config.vm_id.clone(),
        spawn_time_ms: spawn_time,
    })
}

/// Stop a Firecracker VM
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    info!("Stopping VM: {}", process.vm_id);

    // Try graceful shutdown via API (optional)
    // For now, we just kill the process as these are ephemeral VMs
    // Or just kill the process. Firecracker usually handles SIGTERM gracefully.

    if let Some(mut child) = process.child_process.take() {
        child
            .kill()
            .await
            .context("Failed to kill firecracker process")?;
    }

    // Cleanup socket
    let socket_path = format!("/tmp/firecracker-{}.socket", process.vm_id);
    if Path::new(&socket_path).exists() {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    Ok(())
}

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
            warn!("Connection failed: {:?}", err);
        }
    });

    let req_body = if let Some(b) = body {
        let json = serde_json::to_string(b).context("Failed to serialize body")?;
        Full::new(Bytes::from(json))
    } else {
        Full::new(Bytes::new())
    };

    let req = Request::builder()
        .method(method)
        .uri(format!("http://localhost{}", path))
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
        Err(anyhow!(
            "Request failed with status {}: {}",
            status,
            body_str
        ))
    }
}

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

async fn send_action(socket_path: &str, action_type: &str) -> Result<()> {
    #[derive(Serialize)]
    struct Action {
        action_type: String,
    }

    let action = Action {
        action_type: action_type.to_string(),
    };
    send_request(
        socket_path,
        hyper::Method::PUT,
        "/actions",
        Some(&action),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firecracker_structs_serialization() {
        let boot = BootSource {
            kernel_image_path: "/tmp/vmlinux".to_string(),
            boot_args: None,
        };
        let json = serde_json::to_string(&boot).unwrap();
        assert!(json.contains("kernel_image_path"));

        let drive = Drive {
            drive_id: "1".to_string(),
            path_on_host: "/tmp/disk".to_string(),
            is_root_device: true,
            is_read_only: false,
        };
        let json = serde_json::to_string(&drive).unwrap();
        assert!(json.contains("drive_id"));
    }

    #[tokio::test]
    async fn test_missing_kernel_image() {
        let config = VmConfig {
            kernel_path: "/nonexistent/kernel".to_string(),
            ..Default::default()
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
        // Create dummy kernel file so we pass the first check
        let kernel_path = "/tmp/dummy_kernel_for_test";
        let _ = std::fs::write(kernel_path, "dummy");

        let config = VmConfig {
            kernel_path: kernel_path.to_string(),
            rootfs_path: "/nonexistent/rootfs".to_string(),
            ..Default::default()
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
