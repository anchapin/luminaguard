#![cfg(unix)]
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
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Child;
use tokio::process::Command;
use tracing::{debug, info};

use crate::vm::config::VmConfig;
use crate::vm::hypervisor::{Hypervisor, VmInstance};
use crate::vm::jailer::config::JailerConfig;

/// Jailer Hypervisor implementation
pub struct JailerHypervisor {
    pub jailer_config_factory: Box<dyn Fn(&VmConfig) -> JailerConfig + Send + Sync>,
}

#[async_trait]
impl Hypervisor for JailerHypervisor {
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>> {
        let jailer_config = (self.jailer_config_factory)(config);
        let process = start_jailed_firecracker(config, &jailer_config).await?;
        Ok(Box::new(process))
    }

    fn name(&self) -> &str {
        "jailer"
    }
}

/// Jailer process handle
#[derive(Debug)]
pub struct JailerProcess {
    pub id: String,
    pub pid: u32,
    pub socket_path: String,
    pub child_process: Option<Child>,
    pub spawn_time_ms: f64,
    pub chroot_dir: PathBuf,
}

#[async_trait]
impl VmInstance for JailerProcess {
    fn id(&self) -> &str {
        &self.id
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn socket_path(&self) -> &str {
        &self.socket_path
    }

    fn spawn_time_ms(&self) -> f64 {
        self.spawn_time_ms
    }

    async fn stop(&mut self) -> Result<()> {
        info!(
            "Stopping jailed Firecracker VM (ID: {}, PID: {})",
            self.id, self.pid
        );

        // Kill the jailer process (which will also kill Firecracker)
        if let Some(mut child) = self.child_process.take() {
            child
                .kill()
                .await
                .context("Failed to kill jailer process")?;
        }

        // Cleanup socket
        if Path::new(&self.socket_path).exists() {
            let _ = tokio::fs::remove_file(&self.socket_path).await;
        }

        // Cleanup hard links to kernel and rootfs
        let _ =
            tokio::fs::remove_file(self.chroot_dir.join("run").join("firecracker.socket")).await;
        let _ = tokio::fs::remove_file(
            self.chroot_dir
                .join("run")
                .join(format!("{}.json", self.pid)),
        )
        .await;

        Ok(())
    }
}

/// Start Firecracker via Jailer
pub async fn start_jailed_firecracker(
    vm_config: &VmConfig,
    jailer_config: &JailerConfig,
) -> Result<JailerProcess> {
    let start_time = Instant::now();
    info!("Starting jailed Firecracker VM: {}", jailer_config.id);

    // 0. Verify jailer binary is executable
    verify_jailer_executable()?;

    // 1. Validate jailer configuration
    jailer_config
        .validate()
        .context("Invalid jailer configuration")?;

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

    // Create hard links or copies
    match tokio::fs::hard_link(&kernel_path, &jailed_kernel_path).await {
        Ok(_) => {
            debug!("Created hard link to kernel in chroot");
        }
        Err(_) => {
            tokio::fs::copy(&kernel_path, &jailed_kernel_path).await?;
            debug!("Copied kernel to chroot");
        }
    }

    match tokio::fs::hard_link(&rootfs_path, &jailed_rootfs_path).await {
        Ok(_) => {
            debug!("Created hard link to rootfs in chroot");
        }
        Err(_) => {
            tokio::fs::copy(&rootfs_path, &jailed_rootfs_path).await?;
            debug!("Copied rootfs to chroot");
        }
    }

    // 5. Build jailer command
    let mut jailer_cmd = Command::new("jailer");
    let mut args = jailer_config.build_args();
    let socket_name = "firecracker.socket";
    args.extend(vec![
        format!("--api-sock=/run/{}", socket_name),
        "--config-file".to_string(),
        format!("/run/{}.json", jailer_config.id),
    ]);

    jailer_cmd.args(&args);
    jailer_cmd.stdout(std::process::Stdio::piped());
    jailer_cmd.stderr(std::process::Stdio::piped());

    // 6. Spawn jailer process
    let mut child = jailer_cmd
        .spawn()
        .context("Failed to spawn jailer process")?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("Failed to get jailer PID"))?;

    // 7. Wait for socket to be ready
    let jailed_socket_path = chroot_dir.join("run").join(socket_name);
    let mut retries = 0;
    let max_retries = 50;
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
        let _ = child.kill().await;
        return Err(anyhow!(
            "Jailed Firecracker API socket did not appear in time"
        ));
    }

    let socket_path = jailed_socket_path.to_str().unwrap().to_string();

    // 8. Configure VM via API
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

    Ok(JailerProcess {
        id: vm_config.vm_id.clone(),
        pid,
        socket_path,
        child_process: Some(child),
        spawn_time_ms,
        chroot_dir,
    })
}

/// Stop a jailed Firecracker VM process
pub async fn stop_jailed_firecracker(mut process: JailerProcess) -> Result<()> {
    process.stop().await
}

// Helper functions for API interaction

async fn configure_jailed_vm(socket_path: &str, config: &VmConfig) -> Result<()> {
    use bytes::Bytes;
    use http_body_util::Full;
    use hyper::{Request, StatusCode};
    use hyper_util::rt::TokioIo;

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

    let stream = tokio::net::UnixStream::connect(socket_path).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        let _ = conn.await;
    });

    let boot_source = BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off".to_string()),
    };
    let json = serde_json::to_string(&boot_source)?;
    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/boot-source")
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))?;
    let res = sender.send_request(req).await?;
    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to configure boot source"));
    }

    let stream = tokio::net::UnixStream::connect(socket_path).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        let _ = conn.await;
    });

    let rootfs = Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: config.rootfs_path.clone(),
        is_root_device: true,
        is_read_only: false,
    };
    let json = serde_json::to_string(&rootfs)?;
    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/drives/rootfs")
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))?;
    let res = sender.send_request(req).await?;
    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to configure rootfs"));
    }

    let stream = tokio::net::UnixStream::connect(socket_path).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        let _ = conn.await;
    });

    let machine_config = MachineConfiguration {
        vcpu_count: config.vcpu_count,
        mem_size_mib: config.memory_mb,
    };
    let json = serde_json::to_string(&machine_config)?;
    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/machine-config")
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))?;
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

    let stream = tokio::net::UnixStream::connect(socket_path).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::task::spawn(async move {
        let _ = conn.await;
    });

    let action = Action {
        action_type: "InstanceStart".to_string(),
    };
    let json = serde_json::to_string(&action)?;
    let req = Request::builder()
        .method(hyper::Method::PUT)
        .uri("http://localhost/actions")
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))?;
    let res = sender.send_request(req).await?;
    if !res.status().is_success() && res.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to start instance"));
    }

    Ok(())
}

fn verify_jailer_executable() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let jailer_path = Path::new("/usr/local/bin/jailer");
    if !jailer_path.exists() {
        anyhow::bail!("Jailer binary not found");
    }
    let metadata = jailer_path.metadata()?;
    if metadata.permissions().mode() & 0o111 == 0 {
        anyhow::bail!("Jailer binary is not executable");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jailer_process_struct() {
        let process = JailerProcess {
            id: "test".to_string(),
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
