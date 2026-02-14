#!/usr/bin/env rust-script
// -*- mode: rust; -*-

// Firecracker Integration
//
// This module handles the actual Firecracker VM spawning using the HTTP API over Unix sockets.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tracing::{debug, info};

use crate::vm::config::VmConfig;
use crate::vm::hypervisor::{Hypervisor, InstanceStart, VmInstance};

/// Hypervisor implementation for Firecracker
pub struct FirecrackerHypervisor;

#[async_trait]
impl Hypervisor for FirecrackerHypervisor {
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>> {
        let process = start_firecracker(config).await?;
        Ok(Box::new(process))
    }
}

/// Firecracker process handle
pub struct FirecrackerProcess {
    pub id: String,
    pub pid: u32,
    pub socket_path: String,
    pub child_process: Option<Child>,
    pub spawn_time_ms: f64,
}

#[async_trait]
impl VmInstance for FirecrackerProcess {
    fn id(&self) -> &str {
        &self.id
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn spawn_time_ms(&self) -> f64 {
        self.spawn_time_ms
    }

    async fn stop(&mut self) -> Result<()> {
        debug!("Stopping Firecracker VM {}", self.id);

        // 1. Send InstanceStart::Stop via API (if we had a stop command)
        // For now, we just kill the process as it's ephemeral

        // 2. Kill the process if it's still running
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill().await;
        }

        // 3. Cleanup the socket file
        if Path::new(&self.socket_path).exists() {
            let _ = tokio::fs::remove_file(&self.socket_path).await;
        }

        Ok(())
    }
}

/// Firecracker API Client
pub struct FirecrackerClient {
    #[cfg(unix)]
    stream: UnixStream,
}

impl FirecrackerClient {
    /// Create a new Firecracker API client connected to the given Unix socket
    pub async fn new(socket_path: &str) -> Result<Self> {
        #[cfg(unix)]
        {
            let stream = UnixStream::connect(socket_path)
                .await
                .context("Failed to connect to Firecracker socket")?;
            Ok(Self { stream })
        }
        #[cfg(not(unix))] {
            let _ = socket_path;
            Err(anyhow!("Firecracker is only supported on Unix systems"))
        }
    }

    /// Send a request to the Firecracker API
    async fn send_request<T: Serialize>(&mut self, method: &str, path: &str, body: Option<&T>) -> Result<()> {
        #[cfg(unix)]
        {
            let io = TokioIo::new(&mut self.stream);
            let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

            tokio::spawn(async move {
                if let Err(err) = conn.await {
                    debug!("Connection failed: {:?}", err);
                }
            });

            let mut req_builder = Request::builder()
                .method(method)
                .uri(path)
                .header("Host", "localhost")
                .header("Accept", "application/json");

            let request = if let Some(body_data) = body {
                req_builder = req_builder.header("Content-Type", "application/json");
                let json = serde_json::to_string(body_data)?;
                req_builder.body(Full::new(Bytes::from(json)))? 
            } else {
                req_builder.body(Full::new(Bytes::new()))? 
            };

            let response = sender.send_request(request).await?;

            if response.status().is_success() || response.status() == StatusCode::NO_CONTENT {
                Ok(())
            } else {
                let status = response.status();
                let body_bytes = response.collect().await?.to_bytes();
                let error_msg = String::from_utf8_lossy(&body_bytes);
                Err(anyhow!("Firecracker API error ({}): {}", status, error_msg))
            }
        }
        #[cfg(not(unix))] {
            let _ = (method, path, body);
            Err(anyhow!("Firecracker is only supported on Unix systems"))
        }
    }
}

/// Start a Firecracker VM process
pub async fn start_firecracker(config: &VmConfig) -> Result<FirecrackerProcess> {
    let start_time = Instant::now();

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
    let mut client = match FirecrackerClient::new(&socket_path).await {
        Ok(client) => client,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    };

    if let Err(e) = configure_vm(&mut client, config).await {
        let _ = child.kill().await;
        return Err(e);
    }

    // 6. Start the instance
    if let Err(e) = start_instance(&mut client).await {
        let _ = child.kill().await;
        return Err(e);
    }

    let elapsed = start_time.elapsed();
    let spawn_time_ms = elapsed.as_secs_f64() * 1000.0;
    info!("VM {} started in {:.2}ms", config.vm_id, spawn_time_ms);

    Ok(FirecrackerProcess {
        id: config.vm_id.clone(),
        pid,
        socket_path,
        child_process: Some(child),
        spawn_time_ms,
    })
}

/// Stop a Firecracker VM process
pub async fn stop_firecracker(mut process: FirecrackerProcess) -> Result<()> {
    process.stop().await
}

// Helper functions for API interaction

async fn configure_vm(client: &mut FirecrackerClient, config: &VmConfig) -> Result<()> {
    // 1. Set Boot Source
    let boot_source = BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: Some(config.boot_args.clone()),
    };
    client.send_request("PUT", "/boot-source", Some(&boot_source)).await?;

    // 2. Set Root Drive
    let drive = Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: config.rootfs_path.clone(),
        is_root_device: true,
        is_read_only: true,
    };
    client.send_request("PUT", "/drives/rootfs", Some(&drive)).await?;

    // 3. Set Machine Configuration
    let machine_config = MachineConfiguration {
        vcpu_count: config.vcpu_count,
        mem_size_mib: config.mem_size_mib,
        ht_enabled: false,
    };
    client.send_request("PUT", "/machine-config", Some(&machine_config)).await?;

    Ok(())
}

async fn start_instance(client: &mut FirecrackerClient) -> Result<()> {
    let action = Action {
        action_type: "InstanceStart".to_string(),
    };
    client.send_request("PUT", "/actions", Some(&action)).await
}

// Serialization structs for Firecracker API

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
    vcpu_count: u32,
    mem_size_mib: u32,
    ht_enabled: bool,
}

#[derive(Serialize)]
struct Action {
    action_type: String,
}

#[cfg(test)]
fn create_rootfs_drive(path: &str) -> Drive {
    Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: path.to_string(),
        is_root_device: true,
        is_read_only: true,
    }
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
            ..VmConfig::new("test-missing-kernel".to_string())
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
        // Find a path that exists for kernel but not for rootfs
        // We use /dev/null for kernel as it exists on unix
        #[cfg(unix)]
        let kernel = "/dev/null";
        #[cfg(not(unix))] 
        let kernel = "C:\\Windows\\System32\\drivers\\etc\\hosts";

        let config = VmConfig {
            kernel_path: kernel.to_string(),
            rootfs_path: "/non/existent/rootfs".to_string(),
            ..VmConfig::new("test-missing-rootfs".to_string())
        };
        let result = start_firecracker(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Root filesystem not found"));
    }
}
