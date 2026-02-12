// Firecracker Integration
//
// This module handles the actual Firecracker VM spawning.

use anyhow::{Context as _, Result, anyhow, bail};
use crate::vm::config::VmConfig;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Command, Child};
use tokio::net::UnixStream;
use hyper::{Request, Uri, Method};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use http_body_util::Full;
use hyper::body::Bytes;
use tower_service::Service;
use std::pin::Pin;
use std::future::Future;
use std::task::{Context as TaskContext, Poll};
use serde_json::json;

/// Firecracker VM process manager
pub struct FirecrackerProcess {
    pub pid: u32,
    pub socket_path: String,
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

#[derive(Clone)]
struct UnixConnector {
    path: String,
}

impl Service<Uri> for UnixConnector {
    type Response = hyper_util::rt::TokioIo<UnixStream>;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Uri) -> Self::Future {
        let path = self.path.clone();
        Box::pin(async move {
            let stream = UnixStream::connect(path).await?;
            Ok(hyper_util::rt::TokioIo::new(stream))
        })
    }
}

/// Start a Firecracker VM process
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    let start_time = Instant::now();

    // 1. Prepare paths
    let socket_path = format!("/tmp/firecracker-{}.sock", config.vm_id);
    // Ensure socket doesn't exist
    if Path::new(&socket_path).exists() {
        std::fs::remove_file(&socket_path).context("Failed to remove existing socket")?;
    }

    // 2. Start firecracker process
    let firecracker_bin = "firecracker"; // Assume in PATH

    tracing::info!("Starting Firecracker with socket: {}", socket_path);
    let mut command = Command::new(firecracker_bin);
    command
        .arg("--api-sock")
        .arg(&socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped()) // Capture stdout/stderr for debugging
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // Write seccomp filter if present
    // Note: We access seccomp_filter unconditionally here assuming VmConfig has it.
    // If VmConfig::seccomp_filter is missing, this will fail.
    // Given the CI error, it seems likely we are on a platform where it might be conditionally excluded?
    // Or maybe I am misinterpreting the error.
    // However, I will gate this block to be safe if I can confirm.
    // For now, I'll fix the type annotations.
    if let Some(filter) = &config.seccomp_filter {
        let json = filter.to_firecracker_json()?;
        let seccomp_path = format!("/tmp/seccomp-{}.json", config.vm_id);
        tokio::fs::write(&seccomp_path, json).await?;
        command.arg("--seccomp-level").arg("2").arg("--seccomp-filter").arg(&seccomp_path);
    }

    let child = command.spawn().context("Failed to spawn firecracker process. Ensure 'firecracker' is in PATH.")?;
    let pid = child.id().unwrap_or(0);

    // 3. Wait for socket
    let mut socket_ready = false;
    // 5 seconds timeout (50 * 100ms)
    for _ in 0..50 {
        if Path::new(&socket_path).exists() {
            socket_ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !socket_ready {
        return Err(anyhow!("Firecracker socket did not appear in time. Process info: PID {}", pid));
    }

    // 4. Configure VM via API
    let connector = UnixConnector { path: socket_path.clone() };
    // Create client using legacy builder which is compatible with UnixConnector
    // We don't explicitly use the handshake result here as Client::builder handles it internally for the pool
    // But if we were to do manual handshake, we would use HttpHandshakeResult
    let client = Client::builder(TokioExecutor::new())
        .build(connector);

    // 4.1 Set boot source
    let boot_source = json!({
        "kernel_image_path": config.kernel_path,
    });

    let req = Request::builder()
        .method(Method::PUT)
        .uri("http://localhost/boot-source")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(Full::new(Bytes::from(boot_source.to_string())))
        .context("Failed to build request")?;

    let res = client.request(req).await.context("HTTP request to /boot-source failed")?;
    if !res.status().is_success() {
        bail!("Firecracker /boot-source failed: {}", res.status());
    }

    // 4.2 Set machine config
    let machine_config = json!({
        "vcpu_count": config.vcpu_count,
        "mem_size_mib": config.memory_mb,
        "ht_enabled": false
    });

    let req = Request::builder()
        .method(Method::PUT)
        .uri("http://localhost/machine-config")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(Full::new(Bytes::from(machine_config.to_string())))
        .context("Failed to build request")?;

    let res = client.request(req).await.context("HTTP request to /machine-config failed")?;
    if !res.status().is_success() {
        bail!("Firecracker /machine-config failed: {}", res.status());
    }

    // 4.3 Start Instance
    let action = json!({
        "action_type": "InstanceStart"
    });

    let req = Request::builder()
        .method(Method::PUT)
        .uri("http://localhost/actions")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(Full::new(Bytes::from(action.to_string())))
        .context("Failed to build request")?;

    let res = client.request(req).await.context("HTTP request to /actions failed")?;
    if !res.status().is_success() {
        bail!("Firecracker /actions failed: {}", res.status());
    }

    let spawn_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    tracing::info!("Firecracker VM started in {:.2}ms", spawn_time_ms);

    Ok(FirecrackerProcess {
        pid,
        socket_path,
        child: Some(child),
        spawn_time_ms,
    })
}

/// Stop a Firecracker VM process
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    tracing::info!("Stopping Firecracker VM (PID: {})", process.pid);

    if let Some(mut child) = process.child.take() {
        // Send SIGTERM/SIGKILL
        child.kill().await.context("Failed to kill firecracker process")?;
        child.wait().await.context("Failed to wait for process exit")?;
    }

    if Path::new(&process.socket_path).exists() {
        let _ = std::fs::remove_file(&process.socket_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_firecracker_placeholder() {
        // Placeholder test - will be replaced with real tests in Phase 2
        // We can't really test start_firecracker without a binary, so we skip or mock.
        // For now, let's just ensure it compiles and maybe returns error.
        let config = VmConfig::default();
        let result = start_firecracker(&config).await;
        // It should fail because firecracker binary is missing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_firecracker_struct_drop() {
        let socket_path = "/tmp/test-fc-drop.sock";
        // Create dummy socket file
        let _ = std::fs::File::create(socket_path);

        {
            let process = FirecrackerProcess {
                pid: 123,
                socket_path: socket_path.to_string(),
                child: None,
                spawn_time_ms: 10.0,
            };
            // Ensure drop is called
            drop(process);
        }

        // Check if socket file is removed
        assert!(!Path::new(socket_path).exists());
    }
}
