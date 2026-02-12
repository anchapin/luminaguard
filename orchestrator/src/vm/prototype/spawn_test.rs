// Firecracker Spawn Test
//
// Tests spawning a minimal Firecracker VM

use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use super::resources::FirecrackerAssets;

/// Spawn test result
pub enum SpawnTestResult {
    Success { spawn_time_ms: f64 },
    Failed { error: String },
}

/// Test spawning a Firecracker VM
///
/// This test:
/// 1. Creates a Unix socket for Firecracker API
/// 2. Starts Firecracker process
/// 3. Sends minimal VM configuration
/// 4. Measures spawn time
/// 5. Shuts down VM
pub async fn test_spawn(assets: &FirecrackerAssets) -> SpawnTestResult {
    // Check if assets exist
    if !assets.is_ready() {
        return SpawnTestResult::Failed {
            error: format!(
                "Test assets not ready. Kernel: {}, Rootfs: {}",
                assets.kernel_path.display(),
                assets.rootfs_path.display()
            ),
        };
    }

    let socket_path = "/tmp/firecracker-test.sock";

    // Clean up old socket if exists
    let _ = std::fs::remove_file(socket_path);

    // Start firecracker process
    tracing::debug!("Starting Firecracker process...");

    let start_time = Instant::now();

    let mut child = match Command::new("firecracker")
        .arg("--api-sock")
        .arg(socket_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return SpawnTestResult::Failed {
                error: format!("Failed to start firecracker: {}", e),
            }
        }
    };

    // Wait for socket to be created
    let mut retries = 0;
    while retries < 50 {
        // 5 seconds total
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        if std::path::Path::new(socket_path).exists() {
            break;
        }
        retries += 1;
    }

    if !std::path::Path::new(socket_path).exists() {
        let _ = child.kill();
        return SpawnTestResult::Failed {
            error: "Firecracker socket not created after 5 seconds".to_string(),
        };
    }

    // Connect to Firecracker API
    tracing::debug!("Connecting to Firecracker API...");

    let mut stream = match UnixStream::connect(socket_path).await {
        Ok(stream) => stream,
        Err(e) => {
            let _ = child.kill();
            return SpawnTestResult::Failed {
                error: format!("Failed to connect to firecracker socket: {}", e),
            };
        }
    };

    // Send boot source configuration
    tracing::debug!("Sending boot source config...");

    let boot_config = format!(
        r#"{{"kernel_image_path": "{}", "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"}}"#,
        assets.kernel_path.display()
    );

    if let Err(e) = send_request(&mut stream, &boot_config, "/boot-source").await {
        let _ = child.kill();
        return SpawnTestResult::Failed {
            error: format!("Failed to send boot config: {}", e),
        };
    }

    // Read response
    match read_response(&mut stream).await {
        Ok(_) => tracing::debug!("Boot config accepted"),
        Err(e) => {
            let _ = child.kill();
            return SpawnTestResult::Failed {
                error: format!("Boot config rejected: {}", e),
            };
        }
    }

    // Send machine configuration
    tracing::debug!("Sending machine config...");

    let machine_config = r#"{"vcpu_count": 1, "mem_size_mib": 256}"#;

    if let Err(e) = send_request(&mut stream, machine_config, "/machine-config").await {
        let _ = child.kill();
        return SpawnTestResult::Failed {
            error: format!("Failed to send machine config: {}", e),
        };
    }

    match read_response(&mut stream).await {
        Ok(_) => tracing::debug!("Machine config accepted"),
        Err(e) => {
            let _ = child.kill();
            return SpawnTestResult::Failed {
                error: format!("Machine config rejected: {}", e),
            };
        }
    }

    // Start VM
    tracing::debug!("Starting VM...");

    let actions_config = r#"{"action_type": "InstanceStart"}"#;

    if let Err(e) = send_request(&mut stream, actions_config, "/actions").await {
        let _ = child.kill();
        return SpawnTestResult::Failed {
            error: format!("Failed to start VM: {}", e),
        };
    }

    match read_response(&mut stream).await {
        Ok(_) => tracing::debug!("VM started"),
        Err(e) => {
            let _ = child.kill();
            return SpawnTestResult::Failed {
                error: format!("VM start failed: {}", e),
            };
        }
    }

    let spawn_time = start_time.elapsed().as_secs_f64() * 1000.0; // Convert to ms

    // Give VM a moment to boot
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Shutdown VM
    tracing::debug!("Shutting down VM...");

    let _ = child.kill();
    let _ = child.wait();

    // Clean up socket
    let _ = std::fs::remove_file(socket_path);

    SpawnTestResult::Success {
        spawn_time_ms: spawn_time,
    }
}

/// Send a PUT request to Firecracker API
async fn send_request(stream: &mut UnixStream, body: &str, path: &str) -> Result<()> {
    let request = format!(
        "PUT {} HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        path,
        body.len(),
        body
    );

    stream
        .write_all(request.as_bytes())
        .await
        .context("Failed to write to socket")?;

    Ok(())
}

/// Read response from Firecracker API
async fn read_response(stream: &mut UnixStream) -> Result<()> {
    use tokio::io::AsyncReadExt;

    let mut buffer = vec![0u8; 4096];
    let n = stream
        .read(&mut buffer)
        .await
        .context("Failed to read from socket")?;

    if n == 0 {
        return Ok(()); // Empty response
    }

    let response = String::from_utf8_lossy(&buffer[..n]);
    tracing::trace!("Firecracker response: {}", response);

    // Check for HTTP error
    if response.contains("HTTP/1.1 4") || response.contains("HTTP/1.1 5") {
        anyhow::bail!("Firecracker returned error: {}", response);
    }

    Ok(())
}
