// Firecracker Integration
//
// This module handles the actual Firecracker VM spawning using the HTTP API over Unix sockets.
// It manages the lifecycle of the Firecracker process, including starting, configuring,
// and stopping the VM.

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
use crate::vm::firecracker_types::{Action, BootSource, Drive, MachineConfiguration};

// Type aliases to simplify complex hyper types
type HttpSendRequest = hyper::client::conn::http1::SendRequest<Full<Bytes>>;
type HttpConnection = hyper::client::conn::http1::Connection<TokioIo<UnixStream>, Full<Bytes>>;

/// Firecracker VM process manager
///
/// Holds the state of a running Firecracker process.
#[derive(Debug)]
pub struct FirecrackerProcess {
    /// Process ID of the Firecracker binary
    pub pid: u32,
    /// Path to the API socket
    pub socket_path: String,
    /// Handle to the child process
    pub child_process: Option<Child>,
    /// Time taken to spawn the VM in milliseconds
    pub spawn_time_ms: f64,
    /// Path to the temporary seccomp filter file (if any)
    pub seccomp_path: Option<PathBuf>,
}

/// Start a Firecracker VM process
///
/// This function:
/// 1. Validates resources (kernel, rootfs)
/// 2. Prepares the API socket and seccomp filter
/// 3. Spawns the Firecracker process
/// 4. Configures the VM via the HTTP API
/// 5. Starts the instance
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

    // 3. Prepare seccomp filter (if configured)
    let mut seccomp_path_buf = None;
    if let Some(ref filter) = config.seccomp_filter {
        let filter_json = filter
            .to_firecracker_json()
            .context("Failed to serialize seccomp filter")?;
        let path = PathBuf::from(format!("/tmp/ironclaw/seccomp/{}.json", config.vm_id));

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create seccomp directory")?;
        }

        tokio::fs::write(&path, filter_json)
            .await
            .context("Failed to write seccomp filter file")?;
        seccomp_path_buf = Some(path);
        debug!("Seccomp filter written to: {:?}", seccomp_path_buf);
    }

    // 4. Spawn Firecracker process
    let mut command = Command::new("firecracker");
    command.arg("--api-sock").arg(&socket_path);

    // Apply seccomp filter if present
    if let Some(ref path) = seccomp_path_buf {
        command.arg("--seccomp-filter").arg(path);
    }

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

    // 5. Wait for socket to be ready (retry loop)
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
        // Cleanup seccomp file if process failed
        if let Some(path) = &seccomp_path_buf {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(anyhow!("Firecracker API socket did not appear in time"));
    }

    // 6. Connect to API and configure VM
    if let Err(e) = configure_vm(&socket_path, config).await {
        let _ = child.kill().await;
        if let Some(path) = &seccomp_path_buf {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(e);
    }

    // 7. Start the instance
    if let Err(e) = start_instance(&socket_path).await {
        let _ = child.kill().await;
        if let Some(path) = &seccomp_path_buf {
            let _ = tokio::fs::remove_file(path).await;
        }
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
        seccomp_path: seccomp_path_buf,
    })
}

/// Stop a Firecracker VM process
///
/// Terminates the process and cleans up resources (socket, seccomp file).
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

    // Cleanup seccomp file
    if let Some(path) = &process.seccomp_path {
        if path.exists() {
            if let Err(e) = tokio::fs::remove_file(path).await {
                warn!("Failed to remove seccomp filter file: {}", e);
            } else {
                debug!("Removed seccomp filter file: {:?}", path);
            }
        }
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
        ht_enabled: Some(false),
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
}
