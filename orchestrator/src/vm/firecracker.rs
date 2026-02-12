// Firecracker Integration
//
// This module will handle the actual Firecracker VM spawning.
// Placeholder for Phase 2 implementation.

use anyhow::{anyhow, Context, Result};
use tracing::{debug, info};

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use tokio::process::Child;
#[cfg(unix)]
use hyper::{Request, StatusCode};
#[cfg(unix)]
use http_body_util::{Full, BodyExt};
#[cfg(unix)]
use bytes::Bytes;
#[cfg(unix)]
use hyper_util::rt::TokioIo;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use serde::Serialize;

use crate::vm::config::VmConfig;

/// Firecracker VM process manager
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
    pub seccomp_path: String,
    #[cfg(unix)]
    pub child_process: Option<Child>,
    #[cfg(not(unix))]
    pub child_process: Option<()>, // Dummy for non-unix
    pub spawn_time_ms: f64,
}

// Firecracker API structs (Unix only)

#[cfg(unix)]
#[derive(Serialize)]
#[allow(dead_code)]
struct BootSource {
    kernel_image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    boot_args: Option<String>,
}

#[cfg(unix)]
#[derive(Serialize)]
#[allow(dead_code)]
struct Drive {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

#[cfg(unix)]
#[derive(Serialize)]
#[allow(dead_code)]
struct MachineConfiguration {
    vcpu_count: u8,
    mem_size_mib: u32,
    // ht_enabled: bool, // Optional, defaults to false
}

#[cfg(unix)]
#[derive(Serialize)]
#[allow(dead_code)]
struct Action {
    action_type: String,
}

/// Start a Firecracker VM process
///
/// # TODO (Phase 2)
///
/// This will be implemented in Phase 2 when we integrate Firecracker.
/// For now, it's a placeholder to satisfy the compiler.
#[cfg(unix)]
pub async fn start_firecracker(_config: &VmConfig) -> Result<FirecrackerProcess> {
    // TODO: Phase 2 implementation
    // 1. Create API socket
    // 2. Start firecracker process
    // 3. Configure VM via API
    // 4. Return process handle

    Ok(FirecrackerProcess {
        pid: 0,
        socket_path: "/tmp/firecracker.sock".to_string(),
        seccomp_path: "/tmp/seccomp.json".to_string(),
        child_process: None,
        spawn_time_ms: 0.0,
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

    // Cleanup seccomp filter
    if Path::new(&process.seccomp_path).exists() {
        let _ = tokio::fs::remove_file(&process.seccomp_path).await;
    }

    Ok(())
}

// Helper functions for API interaction

#[cfg(unix)]
#[allow(dead_code)]
type HttpSendRequest = hyper::client::conn::http1::SendRequest<Full<Bytes>>;
#[cfg(unix)]
#[allow(dead_code)]
type HttpConnection = hyper::client::conn::http1::Connection<TokioIo<UnixStream>, Full<Bytes>>;

#[cfg(unix)]
#[allow(dead_code)]
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
    let (mut sender, conn): (HttpSendRequest, HttpConnection) =
        hyper::client::conn::http1::handshake(io)
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    Err(anyhow!("Firecracker is only supported on Unix systems"))
}

#[cfg(not(unix))]
pub async fn stop_firecracker(_process: FirecrackerProcess) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_firecracker_placeholder() {
        // Placeholder test - will be replaced with real tests in Phase 2
        // We need a dummy config
        let config = VmConfig::new("test".to_string());
        let result = start_firecracker(&config).await;

        #[cfg(unix)]
        assert!(result.is_ok());
        #[cfg(not(unix))]
        assert!(result.is_err());
    }
}
