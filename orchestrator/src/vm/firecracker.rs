// Firecracker Integration
//
// This module will handle the actual Firecracker VM spawning.

use crate::vm::config::VmConfig;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::Path;
use tracing::{debug, info, warn};

#[cfg(unix)]
use bytes::Bytes;
#[cfg(unix)]
use http_body_util::{BodyExt, Full};
#[cfg(unix)]
use hyper::{Request, StatusCode};
#[cfg(unix)]
use hyper_util::rt::TokioIo;
#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::process::{Child, Command};

#[cfg(unix)]
type HttpSendRequest = hyper::client::conn::http1::SendRequest<Full<Bytes>>;
#[cfg(unix)]
type HttpConnection = hyper::client::conn::http1::Connection<TokioIo<UnixStream>, Full<Bytes>>;

/// Firecracker VM process manager
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
    pub seccomp_path: String,
    #[cfg(unix)]
    pub child_process: Option<Child>,
    #[cfg(not(unix))]
    pub child_process: Option<()>,
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
struct Vsock {
    vsock_id: String,
    guest_cid: u32,
    uds_path: String,
}

#[derive(Serialize)]
#[cfg(unix)]
struct Action {
    action_type: String,
}

/// Start a Firecracker VM process
#[cfg(unix)]
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    let start_time = std::time::Instant::now();
    let socket_path = format!("/tmp/firecracker-{}.sock", config.vm_id);
    let seccomp_path = format!("/tmp/seccomp-{}.json", config.vm_id);

    // Write seccomp filter to file
    if let Some(filter) = &config.seccomp_filter {
        let json = filter.to_firecracker_json()?;
        tokio::fs::write(&seccomp_path, json)
            .await
            .context("Failed to write seccomp filter")?;
    } else {
        warn!(
            "No seccomp filter provided for VM {}, using empty (unsafe!)",
            config.vm_id
        );
        // Create an empty dummy file to satisfy firecracker arg?
        // Firecracker requires valid json if arg is passed.
        // Assuming we always have one if we use spawn_vm_with_config
        tokio::fs::write(&seccomp_path, "{}").await?;
    }

    // Cleanup stale socket
    if Path::new(&socket_path).exists() {
        tokio::fs::remove_file(&socket_path).await.ok();
    }

    info!("Spawning Firecracker process for VM {}", config.vm_id);
    let mut command = Command::new("firecracker");
    command
        .arg("--api-sock")
        .arg(&socket_path)
        .arg("--seccomp-filter")
        .arg(&seccomp_path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command.spawn().context("Failed to spawn firecracker")?;
    let pid = child.id().unwrap_or(0);

    // Wait for socket to be created
    let mut attempts = 0;
    while !Path::new(&socket_path).exists() {
        if attempts > 500 {
            // 500 * 10ms = 5000ms (5s)
            let _ = child.kill().await;
            anyhow::bail!("Firecracker failed to create socket");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        attempts += 1;
    }

    // Configure VM
    if let Err(e) = configure_vm(&socket_path, config).await {
        let _ = child.kill().await;
        return Err(e);
    }

    // Start Instance
    if let Err(e) = start_instance(&socket_path).await {
        let _ = child.kill().await;
        return Err(e);
    }

    let spawn_time = start_time.elapsed().as_secs_f64() * 1000.0;

    Ok(FirecrackerProcess {
        pid,
        socket_path,
        seccomp_path,
        child_process: Some(child),
        spawn_time_ms: spawn_time,
    })
}

/// Stop a Firecracker VM process
#[cfg(unix)]
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    info!("Stopping Firecracker VM (PID: {})", process.pid);

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

    // Cleanup seccomp filter
    if Path::new(&process.seccomp_path).exists() {
        let _ = tokio::fs::remove_file(&process.seccomp_path).await;
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
    let stream: UnixStream = UnixStream::connect(socket_path)
        .await
        .context("Failed to connect to firecracker socket")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn): (HttpSendRequest, HttpConnection) =
        hyper::client::conn::http1::handshake(io)
            .await
            .context("Handshake failed")?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
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
        let body_bytes = res.into_body().collect().await?.to_bytes();
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
    .await?;

    // 2. Set Rootfs Drive
    let rootfs = Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: config.rootfs_path.clone(),
        is_root_device: true,
        is_read_only: true, // Secure!
    };
    send_request(
        socket_path,
        hyper::Method::PUT,
        "/drives/rootfs",
        Some(&rootfs),
    )
    .await?;

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
    .await?;

    // 4. Set VSOCK
    configure_vsock(socket_path, config).await?;

    Ok(())
}

#[cfg(unix)]
async fn configure_vsock(socket_path: &str, config: &VmConfig) -> Result<()> {
    let vsock_path = format!("/tmp/firecracker-vsock-{}.sock", config.vm_id);

    // Ensure vsock path doesn't exist
    if Path::new(&vsock_path).exists() {
        tokio::fs::remove_file(&vsock_path).await.ok();
    }

    let vsock = Vsock {
        vsock_id: "1".to_string(),
        guest_cid: 3,
        uds_path: vsock_path,
    };
    send_request(socket_path, hyper::Method::PUT, "/vsock", Some(&vsock)).await?;
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
pub async fn start_firecracker(_config: &VmConfig) -> Result<FirecrackerProcess> {
    anyhow::bail!("Firecracker is only supported on Unix systems")
}

#[cfg(not(unix))]
pub async fn stop_firecracker(_process: FirecrackerProcess) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn test_firecracker_structs_serialization() {
        // Just verify structs are serializable (compile check)
        let boot = BootSource {
            kernel_image_path: "kernel".into(),
            boot_args: None,
        };
        assert!(serde_json::to_string(&boot).is_ok());

        let drive = Drive {
            drive_id: "root".into(),
            path_on_host: "path".into(),
            is_root_device: true,
            is_read_only: true,
        };
        assert!(serde_json::to_string(&drive).is_ok());
    }
}
