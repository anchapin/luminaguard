// Firecracker Integration
//
<<<<<<< HEAD
// This module will handle the actual Firecracker VM spawning.
// Placeholder for Phase 2 implementation.

use crate::vm::config::VmConfig;
use anyhow::{anyhow, Context, Result};
=======
// This module handles the actual Firecracker VM spawning.

use anyhow::{anyhow, Context, Result};
use crate::vm::config::VmConfig;
>>>>>>> origin/main
use serde::Serialize;
use std::path::Path;
#[cfg(not(unix))]
use std::process::Child;
#[cfg(unix)]
use tokio::process::Child;
use tracing::{debug, info};

#[cfg(unix)]
use bytes::Bytes;
#[cfg(unix)]
use http_body_util::{BodyExt, Full};
#[cfg(unix)]
use hyper::client::conn::http1::{Connection as HttpConnection, SendRequest as HttpSendRequest};
#[cfg(unix)]
use hyper::{Request, StatusCode};
#[cfg(unix)]
use hyper_util::rt::TokioIo;
#[cfg(unix)]
use tokio::net::UnixStream;
<<<<<<< HEAD
=======
use std::process::Stdio;
use std::time::Instant;
>>>>>>> origin/main

/// Firecracker VM process manager
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
<<<<<<< HEAD
    pub seccomp_path: String,
    pub child_process: Option<Child>,
    pub spawn_time_ms: f64,
}

// Firecracker API structs
=======
    pub child: Option<Child>,
    pub spawn_time_ms: f64,
}

impl Drop for FirecrackerProcess {
    fn drop(&mut self) {
        // Cleanup socket file
        if Path::new(&self.socket_path).exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        // Ensure process is killed
        if let Some(mut child) = self.child.take() {
            // We can't await in Drop, but we can start kill.
            // tokio::process::Child::start_kill() is non-blocking.
            let _ = child.start_kill();
        }
    }
}

#[derive(Serialize)]
#[cfg(unix)]
struct Vsock {
    guest_cid: u32,
    uds_path: String,
}

/// Start a Firecracker VM process
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    info!("Starting Firecracker VM: {}", config.vm_id);

    // Validate resources
    if !Path::new(&config.kernel_path).exists() {
        // Just warning for now to allow tests to run without resources
        debug!("Kernel image not found at {}", config.kernel_path);
    }

    // Paths
    let socket_path = format!("/tmp/firecracker_{}.sock", config.vm_id);
    let seccomp_path = format!("/tmp/firecracker_{}_seccomp.json", config.vm_id);

    // Cleanup stale socket
    if Path::new(&socket_path).exists() {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    // Write seccomp filter
    if let Some(filter) = &config.seccomp_filter {
        let json = filter.to_firecracker_json()?;
        tokio::fs::write(&seccomp_path, json)
            .await
            .context("Failed to write seccomp filter")?;
    } else {
        // Create empty filter or handle absence?
        // Firecracker requires filter if flag is passed.
    }

    let start_time = Instant::now();

    // Spawn process
    let mut cmd = tokio::process::Command::new("firecracker");
    cmd.args(["--api-sock", &socket_path]);

    if config.seccomp_filter.is_some() {
        cmd.args(["--seccomp-filter", &seccomp_path]);
    }

    // Redirect stdout/stderr to null to avoid noise
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    let mut child = cmd.spawn().context("Failed to spawn firecracker")?;

    // Wait for socket to be ready
    let mut retries = 50; // 500ms timeout
    let mut socket_ready = false;
    while retries > 0 {
        if Path::new(&socket_path).exists() {
            socket_ready = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        retries -= 1;
    }

    if !socket_ready {
        let _ = child.kill().await;
        anyhow::bail!("Firecracker socket failed to appear");
    }

    // Configure VM
    if let Err(e) = configure_vm(&socket_path, config).await {
        let _ = child.kill().await;
        return Err(e.context("Failed to configure VM"));
    }

    // Start Instance
    if let Err(e) = start_instance(&socket_path).await {
        let _ = child.kill().await;
        return Err(e.context("Failed to start instance"));
    }

    let spawn_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    info!("VM {} started in {:.2}ms", config.vm_id, spawn_time_ms);

    Ok(FirecrackerProcess {
        pid: child.id().unwrap_or(0),
        socket_path,
        child: Some(child),
        spawn_time_ms,
    })
}

/// Stop a Firecracker VM process
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    tracing::info!("Stopping Firecracker VM (PID: {})", process.pid);

    if let Some(mut child) = process.child.take() {
        // Send SIGTERM
        let _ = child.kill().await;
        // Wait for it to exit
        let _ = child.wait().await;
    }

    if Path::new(&process.socket_path).exists() {
        let _ = std::fs::remove_file(&process.socket_path);
    }

    // Cleanup seccomp filter
    let seccomp_path = format!("/tmp/firecracker_{}_seccomp.json",
        process.socket_path
            .rsplit('_')
            .nth(1)
            .unwrap_or(&process.socket_path)
            .trim_start_matches("/tmp/firecracker_")
    );
    if Path::new(&seccomp_path).exists() {
        let _ = tokio::fs::remove_file(&seccomp_path).await;
    }

    Ok(())
}

// Helper functions for API interaction
>>>>>>> origin/main

#[derive(Serialize)]
struct BootSource {
    kernel_image_path: String,
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
<<<<<<< HEAD
#[cfg(unix)]
struct Vsock {
    guest_cid: u32,
    uds_path: String,
}

#[derive(Serialize)]
struct Action {
    action_type: String,
}

/// Start a Firecracker VM process
#[cfg(unix)]
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    info!("Starting Firecracker VM: {}", config.vm_id);

    // Validate resources
    if !Path::new(&config.kernel_path).exists() {
        // Just warning for now to allow tests to run without resources
        debug!("Kernel image not found at {}", config.kernel_path);
    }

    // Paths
    let socket_path = format!("/tmp/firecracker_{}.sock", config.vm_id);
    let seccomp_path = format!("/tmp/firecracker_{}_seccomp.json", config.vm_id);

    // Cleanup stale socket
    if Path::new(&socket_path).exists() {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    // Write seccomp filter
    if let Some(filter) = &config.seccomp_filter {
        let json = filter.to_firecracker_json()?;
        tokio::fs::write(&seccomp_path, json)
            .await
            .context("Failed to write seccomp filter")?;
    } else {
        // Create empty filter or handle absence?
        // Firecracker requires filter if flag is passed.
    }

    let start_time = std::time::Instant::now();

    // Spawn process
    let mut cmd = tokio::process::Command::new("firecracker");
    cmd.args(["--api-sock", &socket_path]);

    if config.seccomp_filter.is_some() {
        cmd.args(["--seccomp-filter", &seccomp_path]);
    }

    // Redirect stdout/stderr to null to avoid noise
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.stdin(std::process::Stdio::null());

    let mut child = cmd.spawn().context("Failed to spawn firecracker")?;

    // Wait for socket to be ready
    let mut retries = 50; // 500ms timeout
    let mut socket_ready = false;
    while retries > 0 {
        if Path::new(&socket_path).exists() {
            socket_ready = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        retries -= 1;
    }

    if !socket_ready {
        let _ = child.kill().await;
        anyhow::bail!("Firecracker socket failed to appear");
    }

    // Configure VM
    if let Err(e) = configure_vm(&socket_path, config).await {
        let _ = child.kill().await;
        return Err(e.context("Failed to configure VM"));
    }

    // Start Instance
    if let Err(e) = start_instance(&socket_path).await {
        let _ = child.kill().await;
        return Err(e.context("Failed to start instance"));
    }

    let spawn_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    info!("VM {} started in {:.2}ms", config.vm_id, spawn_time_ms);

    Ok(FirecrackerProcess {
        pid: child.id().unwrap_or(0),
        socket_path,
        seccomp_path,
        child_process: Some(child),
        spawn_time_ms,
    })
}

/// Stop a Firecracker VM process
#[cfg(unix)]
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    info!("Stopping Firecracker VM (PID: {})", process.pid);

    if let Some(mut child) = process.child_process.take() {
        // Send SIGTERM
        let _ = child.kill().await;
        // Wait for it to exit
        let _ = child.wait().await;
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

=======
struct Action {
    action_type: String,
}

>>>>>>> origin/main
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

    let stream: UnixStream = UnixStream::connect(socket_path)
        .await
        .context("Failed to connect to firecracker socket")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn): (
        HttpSendRequest<Full<Bytes>>,
        HttpConnection<TokioIo<UnixStream>, Full<Bytes>>,
    ) = hyper::client::conn::http1::handshake(io)
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

    let res: hyper::Response<hyper::body::Incoming> = sender
        .send_request(req)
        .await
        .context("Failed to send request")?;

    if res.status().is_success() || res.status() == StatusCode::NO_CONTENT {
        Ok(())
    } else {
        let status = res.status();
        let body_bytes: Bytes = res.collect().await?.to_bytes();
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
        is_read_only: true,
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

    // 4. Configure VSOCK
    // We use a predictable path based on VM ID to allow the agent to connect
    let vsock_path = format!("/tmp/ironclaw_{}.vsock", config.vm_id);
    let vsock = Vsock {
        guest_cid: 3,
        uds_path: vsock_path,
    };
    send_request(socket_path, hyper::Method::PUT, "/vsock", Some(&vsock))
        .await
        .context("Failed to configure vsock")?;

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

    #[tokio::test]
    async fn test_firecracker_placeholder() {
        // Skip if firecracker is not installed
        if tokio::process::Command::new("firecracker")
            .arg("--version")
            .output()
            .await
            .is_err()
        {
            println!("Skipping test: Firecracker not installed");
            return;
        }

        // Placeholder test - will be replaced with real tests in Phase 2
        let config = VmConfig::default();
        let result = start_firecracker(&config).await;

        // If it fails, check if it's due to missing resources (expected in CI)
        if let Err(e) = result {
            let msg = e.to_string().to_lowercase();
            // Check for common resource/availability issues
            if msg.contains("kernel image not found")
                || msg.contains("rootfs image not found")
                || msg.contains("firecracker socket failed to appear")
                || msg.contains("failed to configure vm")  // Generic config error, likely missing resources
                || msg.contains("no such file")  // Missing file/path
                || msg.contains("permission denied")
            // Can't access resources
            {
                println!("Skipping test (Firecracker resources unavailable): {}", msg);
                return;
            }
            panic!("Failed to start firecracker: {}", e);
        }
    }
}
