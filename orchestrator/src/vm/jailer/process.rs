// Jailer Process Management
//
// Handles spawning and managing Firecracker processes via Jailer
//
// Real Jailer Execution Flow:
// 1. Verify jailer binary is executable
// 2. Validate configuration (VM resources, jailer settings)
// 3. Create chroot directory structure
// 4. Prepare resources in chroot (hard links to kernel/rootfs)
// 5. Build and execute jailer command with all arguments
// 6. Wait for API socket to be created
// 7. Configure VM via Firecracker API (boot source, drives, machine config)
// 8. Start the VM instance
//
// Security Layers:
// - chroot: Filesystem isolation
// - cgroups: Resource limits (CPU, memory)
// - namespaces: Process, network, mount isolation
// - UID/GID: Privilege separation

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Child;
use tokio::process::Command;
use tracing::{debug, info};

use crate::vm::jailer::config::JailerConfig;
use crate::vm::config::VmConfig;

/// Jailer process handle
#[derive(Debug)]
pub struct JailerProcess {
    pub pid: u32,
    pub socket_path: String,
    pub child_process: Option<Child>,
    pub spawn_time_ms: f64,
    pub chroot_dir: PathBuf,
}

/// Start Firecracker via Jailer
///
/// This function performs real jailer binary execution, which requires:
/// - Root privileges (CAP_SYS_ADMIN for namespace/cgroup operations)
/// - Jailer binary at `/usr/local/bin/jailer`
/// - Firecracker binary (path specified in jailer_config.exec_file)
/// - Valid kernel and rootfs files
/// - Proper cgroup setup (v1 or v2)
///
/// # Arguments
///
/// * `vm_config` - VM configuration (kernel, rootfs, memory, CPU)
/// * `jailer_config` - Jailer sandbox configuration (chroot, cgroups, namespaces)
///
/// # Returns
///
/// * `JailerProcess` - Handle for managing the jailed Firecracker process
///
/// # Example
///
/// ```no_run
/// use ironclaw_orchestrator::vm::{config::VmConfig, jailer::{JailerConfig, start_jailed_firecracker}};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let vm_config = VmConfig::new("my-vm".to_string());
///     let jailer_config = JailerConfig::new("my-vm".to_string())
///         .with_user(1000, 1000);
///
///     let jailer_process = start_jailed_firecracker(&vm_config, &jailer_config).await?;
///     println!("Jailed VM spawned: PID={}, Socket={}", jailer_process.pid, jailer_process.socket_path);
///     Ok(())
/// }
/// ```
pub async fn start_jailed_firecracker(
    vm_config: &VmConfig,
    jailer_config: &JailerConfig,
) -> Result<JailerProcess> {
    let start_time = Instant::now();
    info!(
        "Starting jailed Firecracker VM: {}",
        jailer_config.id
    );

    // 0. Verify jailer binary is executable
    verify_jailer_executable()?;

    // 1. Validate jailer configuration
    jailer_config.validate().context("Invalid jailer configuration")?;

    // 2. Validate VM resources
    let kernel_path = PathBuf::from(&vm_config.kernel_path);
    let rootfs_path = PathBuf::from(&vm_config.rootfs_path);

    if !kernel_path.exists() {
        return Err(anyhow!(
            "Kernel image not found at: {:?}. \
            Download from https://github.com/firecracker-microvm/firecracker/blob/main/docs/rootfs-and-kernel-setup.md",
            kernel_path
        ));
    }
    if !rootfs_path.exists() {
        return Err(anyhow!(
            "Root filesystem not found at: {:?}. \
            Download from https://github.com/firecracker-microvm/firecracker/blob/main/docs/rootfs-and-kernel-setup.md",
            rootfs_path
        ));
    }

    // 3. Prepare chroot directory structure
    let chroot_dir = jailer_config.chroot_dir();

    // Create chroot base directory if it doesn't exist
    if let Some(parent) = chroot_dir.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create chroot base directory. This may require root privileges for /srv/jailer")?;
        }
    }

    // 4. Prepare resources in chroot directory
    // The jailer will copy the Firecracker binary, but we need to prepare
    // the kernel and rootfs as hard links or copies

    let jailed_kernel_path = chroot_dir.join(
        kernel_path
            .file_name()
            .ok_or_else(|| anyhow!("Invalid kernel path"))?,
    );

    let jailed_rootfs_path = chroot_dir.join(
        rootfs_path
            .file_name()
            .ok_or_else(|| anyhow!("Invalid rootfs path"))?,
    );

    // Create hard links to kernel and rootfs in chroot
    // Hard links are preferred over copies to avoid duplicating large files
    // This requires both source and destination to be on the same filesystem
    match tokio::fs::hard_link(&kernel_path, &jailed_kernel_path).await {
        Ok(_) => {
            debug!(
                "Created hard link to kernel in chroot: {:?}",
                jailed_kernel_path
            );
        }
        Err(e) => {
            // If hard link fails, fall back to copy
            debug!(
                "Hard link failed (different filesystems?), copying kernel: {:?}",
                e
            );
            tokio::fs::copy(&kernel_path, &jailed_kernel_path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to copy kernel to chroot: {:?} -> {:?}",
                        kernel_path, jailed_kernel_path
                    )
                })?;
            debug!(
                "Copied kernel to chroot: {:?} ({} bytes)",
                jailed_kernel_path,
                kernel_path.metadata().map(|m| m.len()).unwrap_or(0)
            );
        }
    }

    match tokio::fs::hard_link(&rootfs_path, &jailed_rootfs_path).await {
        Ok(_) => {
            debug!(
                "Created hard link to rootfs in chroot: {:?}",
                jailed_rootfs_path
            );
        }
        Err(e) => {
            // If hard link fails, fall back to copy
            debug!(
                "Hard link failed (different filesystems?), copying rootfs: {:?}",
                e
            );
            tokio::fs::copy(&rootfs_path, &jailed_rootfs_path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to copy rootfs to chroot: {:?} -> {:?}",
                        rootfs_path, jailed_rootfs_path
                    )
                })?;
            debug!(
                "Copied rootfs to chroot: {:?} ({} bytes)",
                jailed_rootfs_path,
                rootfs_path.metadata().map(|m| m.len()).unwrap_or(0)
            );
        }
    }

    debug!(
        "VM resources prepared in chroot directory: {:?}",
        chroot_dir
    );

    // 5. Build jailer command
    let mut jailer_cmd = Command::new("jailer");

    // Add jailer arguments
    let mut args = jailer_config.build_args();

    // Add Firecracker arguments after the separator
    // The API socket path will be relative to chroot
    let socket_name = "firecracker.socket";
    args.extend(vec![
        format!("--api-sock=/run/{}", socket_name),
        "--config-file".to_string(),
        format!("/run/{}.json", jailer_config.id),
    ]);

    jailer_cmd.args(&args);

    // Redirect stdout/stderr (capture stderr for debugging errors)
    jailer_cmd.stdout(std::process::Stdio::piped());
    jailer_cmd.stderr(std::process::Stdio::piped());

    debug!("Jailer command: jailer {}", args.join(" "));

    // 6. Spawn jailer process
    let mut child = jailer_cmd
        .spawn()
        .context("Failed to spawn jailer process. Ensure jailer is installed at /usr/local/bin/jailer and is executable.")?;

    let pid = child
        .id()
        .ok_or_else(|| anyhow!("Failed to get jailer PID"))?;

    debug!("Jailer process started with PID: {}", pid);

    // 7. Wait for socket to be ready (retry loop)
    // The socket will be created inside the chroot at /run/firecracker.socket
    // But we access it via the chroot path
    let jailed_socket_path = chroot_dir.join("run").join(socket_name);

    let mut retries = 0;
    let max_retries = 50; // 50 * 10ms = 500ms
    let mut socket_ready = false;

    while retries < max_retries {
        if jailed_socket_path.exists() {
            socket_ready = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        retries += 1;
    }

    if !socket_ready {
        // Kill the process if socket never appeared
        let _ = child.kill().await;

        // Try to read stderr for error details
        if let Some(mut stderr) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let mut error_buf = vec![];
            if stderr.read_to_end(&mut error_buf).await.is_ok() {
                let error_output = String::from_utf8_lossy(&error_buf);
                if !error_output.trim().is_empty() {
                    return Err(anyhow!(
                        "Jailed Firecracker API socket did not appear in time: {:?}\n\
                        Jailer stderr output: {}",
                        jailed_socket_path,
                        error_output
                    ));
                }
            }
        }

        return Err(anyhow!(
            "Jailed Firecracker API socket did not appear in time: {:?}\n\
            This may indicate:\n\
            1. Kernel or rootfs resources are missing or inaccessible\n\
            2. Insufficient permissions (need CAP_SYS_ADMIN for namespaces)\n\
            3. Cgroup v2 not configured properly\n\
            4. Firewall or security policies blocking execution",
            jailed_socket_path
        ));
    }

    let socket_path = jailed_socket_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid socket path"))?
        .to_string();

    info!(
        "Jailed Firecracker API socket ready at: {}",
        socket_path
    );

    // 8. Configure VM via API
    // Note: We need to use paths relative to chroot in the API calls
    let config_for_api = VmConfig {
        kernel_path: format!("/{}", kernel_path.file_name().unwrap().to_str().unwrap()),
        rootfs_path: format!("/{}", rootfs_path.file_name().unwrap().to_str().unwrap()),
        ..vm_config.clone()
    };

    if let Err(e) = configure_jailed_vm(&socket_path, &config_for_api).await {
        let _ = child.kill().await;
        return Err(e);
    }

    // 9. Start the instance
    if let Err(e) = start_instance(&socket_path).await {
        let _ = child.kill().await;
        return Err(e);
    }

    let elapsed = start_time.elapsed();
    let spawn_time_ms = elapsed.as_secs_f64() * 1000.0;

    info!(
        "Jailed VM {} started in {:.2}ms (PID: {})",
        jailer_config.id, spawn_time_ms, pid
    );

    Ok(JailerProcess {
        pid,
        socket_path,
        child_process: Some(child),
        spawn_time_ms,
        chroot_dir,
    })
}

/// Stop a jailed Firecracker VM process
pub async fn stop_jailed_firecracker(mut process: JailerProcess) -> Result<()> {
    info!(
        "Stopping jailed Firecracker VM (PID: {})",
        process.pid
    );

    // Kill the jailer process (which will also kill Firecracker)
    if let Some(mut child) = process.child_process.take() {
        child
            .kill()
            .await
            .context("Failed to kill jailer process")?;
    }

    // Cleanup socket
    if Path::new(&process.socket_path).exists() {
        let _ = tokio::fs::remove_file(&process.socket_path).await;
    }

    // Cleanup hard links to kernel and rootfs
    // Note: We should be careful not to delete the original files
    let _ = tokio::fs::remove_file(process.chroot_dir.join("run").join("firecracker.socket")).await;
    let _ = tokio::fs::remove_file(process.chroot_dir.join("run").join(format!("{}.json", process.pid))).await;

    // Optionally cleanup entire chroot directory
    // This should be done carefully to avoid race conditions
    // let _ = tokio::fs::remove_dir_all(&process.chroot_dir).await;

    Ok(())
}

// Helper functions for API interaction

async fn configure_jailed_vm(socket_path: &str, config: &VmConfig) -> Result<()> {
    use bytes::Bytes;
    use http_body_util::Full;
    use hyper::{Request, StatusCode};
    use hyper_util::rt::TokioIo;

    // Reuse API helpers from firecracker module
    // For now, we'll duplicate the minimal logic here

    #[derive(serde::Serialize)]
    struct BootSource {
        kernel_image_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        boot_args: Option<String>,
    }

    #[derive(serde::Serialize)]
    struct Drive {
        drive_id: String,
        path_on_host: String,
        is_root_device: bool,
        is_read_only: bool,
    }

    #[derive(serde::Serialize)]
    struct MachineConfiguration {
        vcpu_count: u8,
        mem_size_mib: u32,
    }

    let stream = tokio::net::UnixStream::connect(socket_path)
        .await
        .context("Failed to connect to jailed Firecracker socket")?;

    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("Handshake failed")?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            debug!("Connection closed: {:?}", err);
        }
    });

    // Configure boot source
    let boot_source = BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off".to_string()),
    };

    let json = serde_json::to_string(&boot_source)?;
    let req_body = Full::new(Bytes::from(json));

    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/boot-source")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(req_body)
        .context("Failed to build request")?;

    let res = sender.send_request(req).await?;

    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to configure boot source"));
    }

    // Configure rootfs
    let rootfs = Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: config.rootfs_path.clone(),
        is_root_device: true,
        is_read_only: false,
    };

    // Need to reconnect for second request (simplified)
    let stream = tokio::net::UnixStream::connect(socket_path)
        .await
        .context("Failed to reconnect to socket")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("Handshake failed")?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            debug!("Connection closed: {:?}", err);
        }
    });

    let json = serde_json::to_string(&rootfs)?;
    let req_body = Full::new(Bytes::from(json));

    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/drives/rootfs")
        .header("Content-Type", "application/json")
        .body(req_body)
        .context("Failed to build request")?;

    let res = sender.send_request(req).await?;

    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to configure rootfs"));
    }

    // Configure machine
    let machine_config = MachineConfiguration {
        vcpu_count: config.vcpu_count,
        mem_size_mib: config.memory_mb,
    };

    let stream = tokio::net::UnixStream::connect(socket_path)
        .await
        .context("Failed to reconnect to socket")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("Handshake failed")?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            debug!("Connection closed: {:?}", err);
        }
    });

    let json = serde_json::to_string(&machine_config)?;
    let req_body = Full::new(Bytes::from(json));

    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/machine-config")
        .header("Content-Type", "application/json")
        .body(req_body)
        .context("Failed to build request")?;

    let res = sender.send_request(req).await?;

    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to configure machine"));
    }

    Ok(())
}

async fn start_instance(socket_path: &str) -> Result<()> {
    use bytes::Bytes;
    use http_body_util::Full;
    use hyper::{Request, StatusCode};
    use hyper_util::rt::TokioIo;

    #[derive(serde::Serialize)]
    struct Action {
        action_type: String,
    }

    let stream = tokio::net::UnixStream::connect(socket_path)
        .await
        .context("Failed to connect to socket")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("Handshake failed")?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            debug!("Connection closed: {:?}", err);
        }
    });

    let action = Action {
        action_type: "InstanceStart".to_string(),
    };

    let json = serde_json::to_string(&action)?;
    let req_body = Full::new(Bytes::from(json));

    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/actions")
        .header("Content-Type", "application/json")
        .body(req_body)
        .context("Failed to build request")?;

    let res = sender.send_request(req).await?;

    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to start instance"));
    }

    Ok(())
}

/// Verify that the jailer binary is executable
///
/// This function checks that:
/// 1. The jailer binary exists at the expected path
/// 2. The binary has execute permissions
/// 3. Can be executed (basic sanity check)
///
/// # Returns
///
/// * `Ok(())` - Jailer binary is executable
/// * `Err(_)` - Jailer binary is missing or not executable
fn verify_jailer_executable() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    let jailer_path = Path::new("/usr/local/bin/jailer");

    if !jailer_path.exists() {
        anyhow::bail!(
            "Jailer binary not found at {:?}. \
            \nTo install Firecracker, follow the official guide: \
            \nhttps://github.com/firecracker-microvm/firecracker/blob/main/docs/getting-started.md \
            \n\nOr download pre-built binaries: \
            \nhttps://github.com/firecracker-microvm/firecracker/releases",
            jailer_path
        );
    }

    // Check if file is executable
    let metadata = jailer_path.metadata().context("Failed to read jailer binary metadata")?;
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    if mode & 0o111 == 0 {
        anyhow::bail!(
            "Jailer binary is not executable: {:?}. \
            \nRun: chmod +x /usr/local/bin/jailer",
            jailer_path
        );
    }

    // Verify the binary can actually execute by checking --version
    let version_check = Command::new("jailer")
        .arg("--version")
        .output()
        .context("Failed to execute jailer binary");

    match version_check {
        Ok(output) => {
            if !output.status.success() {
                anyhow::bail!(
                    "Jailer binary exists but --version check failed. \
                    \nStderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            debug!(
                "Jailer binary verified: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }
        Err(e) => {
            anyhow::bail!(
                "Jailer binary exists but cannot be executed: {}. \
                \nThis may indicate a corrupted binary or missing dependencies.",
                e
            );
        }
    }

    debug!("Jailer binary verified as executable and functional: {:?}", jailer_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jailer_process_struct() {
        let process = JailerProcess {
            pid: 1234,
            socket_path: "/tmp/test.socket".to_string(),
            child_process: None,
            spawn_time_ms: 100.0,
            chroot_dir: PathBuf::from("/srv/jailer/firecracker/test/root"),
        };

        assert_eq!(process.pid, 1234);
        assert_eq!(process.socket_path, "/tmp/test.socket");
    }
}
