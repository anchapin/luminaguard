// Firecracker Integration
//
// This module will handle the actual Firecracker VM spawning.
// Placeholder for Phase 2 implementation.

use anyhow::Result;

/// Firecracker VM process manager
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
    pub seccomp_path: String,
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
///
/// # TODO (Phase 2)
///
/// This will be implemented in Phase 2 when we integrate Firecracker.
/// For now, it's a placeholder to satisfy the compiler.
pub async fn start_firecracker(_config: &str) -> Result<FirecrackerProcess> {
    // TODO: Phase 2 implementation
    // 1. Create API socket
    // 2. Start firecracker process
    // 3. Configure VM via API
    // 4. Return process handle

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

    #[tokio::test]
    async fn test_firecracker_placeholder() {
        // Placeholder test - will be replaced with real tests in Phase 2
        let result = start_firecracker("{}").await;
        assert!(result.is_ok());
    }
}
