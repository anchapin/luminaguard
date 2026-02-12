// Firecracker Integration
//
// This module will handle the actual Firecracker VM spawning.

use crate::vm::config::VmConfig;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info};

#[cfg(unix)]
use http_body_util::{BodyExt, Full}; // BodyExt for collect()
#[cfg(unix)]
use hyper::body::Bytes;
#[cfg(unix)]
use hyper::client::conn::http1::{Connection, SendRequest};
#[cfg(unix)]
use hyper::{Request, StatusCode};
#[cfg(unix)]
use hyper_util::rt::TokioIo;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::process::{Child, Command};

// Type aliases for cleaner signatures
#[cfg(unix)]
type HttpSendRequest = SendRequest<Full<Bytes>>;
#[cfg(unix)]
type HttpConnection = Connection<TokioIo<UnixStream>, Full<Bytes>>;

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

/// Start a Firecracker VM process (Unix implementation)
#[cfg(unix)]
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    let start_time = Instant::now();
    let socket_path = format!("/tmp/firecracker-{}.socket", config.vm_id);
    let seccomp_path = format!("/tmp/firecracker-{}.seccomp", config.vm_id);

    // Ensure socket doesn't exist
    if Path::new(&socket_path).exists() {
        tokio::fs::remove_file(&socket_path).await?;
    }

    // Write seccomp filter if present
    if let Some(filter) = &config.seccomp_filter {
        let json = filter.to_firecracker_json()?;
        tokio::fs::write(&seccomp_path, json)
            .await
            .context("Failed to write seccomp filter")?;
    }

    info!("Spawning Firecracker process for VM: {}", config.vm_id);

    // Spawn firecracker process
    let mut command = Command::new("firecracker");
    command.arg("--api-sock").arg(&socket_path);

    if config.seccomp_filter.is_some() {
        command.arg("--seccomp-filter").arg(&seccomp_path);
    }

    // Redirect stdout/stderr to avoid polluting orchestrator logs
    // command.stdout(std::process::Stdio::null());
    // command.stderr(std::process::Stdio::null());

    let child = command
        .spawn()
        .context("Failed to spawn firecracker binary. Is it installed?")?;

    let pid = child.id().unwrap_or(0);

    // Wait for socket to be created (with timeout)
    let mut retries = 0;
    while !Path::new(&socket_path).exists() {
        if retries > 20 {
            // 2 seconds
            // Kill process if it failed to start
            // child.kill().await?; // Can't kill easily if we moved child. But we haven't moved it yet.
            // Actually we can drop child handle if we don't return it? No, it might zombie.
            anyhow::bail!("Firecracker socket was not created in time");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        retries += 1;
    }

    // Configure VM via API
    configure_vm(&socket_path, config).await?;

    // Start Instance
    start_instance(&socket_path).await?;

    let duration = start_time.elapsed();
    let spawn_time_ms = duration.as_secs_f64() * 1000.0;

    info!(
        "Firecracker VM started (PID: {}) in {:.2}ms",
        pid, spawn_time_ms
    );

    Ok(FirecrackerProcess {
        pid,
        socket_path,
        seccomp_path,
        child_process: Some(child),
        spawn_time_ms,
    })
}

/// Start a Firecracker VM process (Non-Unix implementation)
#[cfg(not(unix))]
pub async fn start_firecracker(_config: &VmConfig) -> Result<FirecrackerProcess> {
    anyhow::bail!("Firecracker is only supported on Unix systems")
}

/// Stop a Firecracker VM process
#[cfg(unix)]
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    info!("Stopping Firecracker VM (PID: {})", process.pid);

    if let Some(mut child) = process.child_process.take() {
        // Try graceful shutdown first?
        // For now, just kill
        child.kill().await.ok(); // Ignore error if already dead
        child.wait().await.ok();
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

#[cfg(not(unix))]
pub async fn stop_firecracker(_process: FirecrackerProcess) -> Result<()> {
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
        is_read_only: true, // Fixed: Security requirement (was false)
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
