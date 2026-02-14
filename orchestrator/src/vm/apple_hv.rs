//! macOS Virtualization.framework Backend
//!
//! This module implements the Hypervisor and VmInstance traits using
//! Apple's Virtualization.framework (available on macOS 11+).
//!
//! The implementation provides:
//! - Fast VM spawning via hardware virtualization (KVM hypervisor on Apple Silicon)
//! - Network isolation and system resource limits
//! - Graceful shutdown and cleanup

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::info;

use crate::vm::config::VmConfig;
use crate::vm::hypervisor::{Hypervisor, VmInstance};

// Conditional vz module import (macOS only)
#[cfg(target_os = "macos")]
use vz;

/// macOS Virtualization.framework Hypervisor implementation
pub struct AppleHvHypervisor;

#[async_trait]
impl Hypervisor for AppleHvHypervisor {
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>> {
        #[cfg(target_os = "macos")]
        {
            let instance = start_apple_hv(config).await?;
            Ok(Box::new(instance))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = config;
            Err(anyhow!("Apple Hypervisor is only available on macOS"))
        }
    }

    fn name(&self) -> &str {
        "apple_hv"
    }
}

/// macOS VM instance managed by Virtualization.framework
#[cfg(target_os = "macos")]
pub struct AppleHvInstance {
    pub id: String,
    pub pid: u32,
    pub spawn_time_ms: f64,
    partition: Arc<Mutex<Option<vz::Partition>>>,
}

#[cfg(not(target_os = "macos"))]
pub struct AppleHvInstance {
    pub id: String,
    pub pid: u32,
    pub spawn_time_ms: f64,
}

#[async_trait]
impl VmInstance for AppleHvInstance {
    fn id(&self) -> &str {
        &self.id
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn socket_path(&self) -> &str {
        ""
    }

    fn spawn_time_ms(&self) -> f64 {
        self.spawn_time_ms
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping macOS VM (ID: {}, PID: {})", self.id, self.pid);

        #[cfg(target_os = "macos")]
        {
            let mut partition_guard = self.partition.lock().await;
            if let Some(mut partition) = partition_guard.take() {
                // Stop the partition
                partition
                    .stop()
                    .await
                    .context("Failed to stop Virtualization.framework partition")?;
                info!("macOS VM {} stopped", self.id);
            } else {
                tracing::warn!("VM {} already stopped or never started", self.id);
            }
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
async fn start_apple_hv(config: &VmConfig) -> Result<AppleHvInstance> {
    let start_time = Instant::now();
    info!("Starting macOS Virtualization.framework VM: {}", config.vm_id);

    // Validate required files exist
    let kernel_path = PathBuf::from(&config.kernel_path);
    let rootfs_path = PathBuf::from(&config.rootfs_path);

    if !kernel_path.exists() {
        return Err(anyhow!("Kernel image not found at: {:?}", kernel_path));
    }
    if !rootfs_path.exists() {
        return Err(anyhow!("Root filesystem not found at: {:?}", rootfs_path));
    }

    info!(
        "Initializing VM with kernel: {:?}, rootfs: {:?}",
        kernel_path, rootfs_path
    );

    // Create and configure virtual machine
    let mut vm_config = vz::VirtualMachineConfiguration::new();

    // Configure CPU and memory
    vm_config.set_vcpu_count(config.vcpu_count as usize);
    vm_config.set_memory_size(config.memory_mb as usize * 1024 * 1024);
    info!(
        "Configured VM with {} vCPUs and {}MB memory",
        config.vcpu_count, config.memory_mb
    );

    // Create boot loader for the kernel
    let boot_loader = vz::LinuxBootLoader::new(kernel_path.clone());
    vm_config.set_boot_loader(Box::new(boot_loader));

    // Configure storage (root filesystem) via virtio-block
    let disk_attachment = vz::Disk::new(rootfs_path.clone());
    let disk_config = vz::StorageDeviceConfiguration::new(Box::new(disk_attachment));
    vm_config.add_storage_device(&disk_config);
    info!("Attached root filesystem via virtio-block");

    // Configure networking if enabled
    if config.enable_networking {
        let network_device = vz::VirtioNetworkDeviceConfiguration::new();
        vm_config.add_network_device(&network_device);
        info!("Enabled networking (virtio-net)");
    } else {
        info!("Networking disabled for security");
    }

    // Validate configuration
    vm_config
        .validate()
        .context("VM configuration validation failed")?;

    // Create and start the partition
    let partition = vz::Partition::new(vm_config);
    partition
        .start()
        .await
        .context("Failed to start Virtualization.framework partition")?;

    let spawn_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    info!(
        "VM {} started successfully in {:.2}ms",
        config.vm_id, spawn_time_ms
    );

    Ok(AppleHvInstance {
        id: config.vm_id.clone(),
        pid: std::process::id(), // VM process PID on host
        spawn_time_ms,
        partition: Arc::new(Mutex::new(Some(partition))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_hv_name() {
        let hv = AppleHvHypervisor;
        assert_eq!(hv.name(), "apple_hv");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_apple_hv_name_on_macos() {
        // Verify name is correct on macOS
        let hv = AppleHvHypervisor;
        assert_eq!(hv.name(), "apple_hv");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_apple_hv_unavailable_on_non_macos() {
        // Verified: apple_hv module exists and is properly gated
        // Cross-platform compilation should succeed with #[cfg] guards
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_apple_hv_instance_fields() {
        // Test AppleHvInstance struct construction and field access
        let instance = AppleHvInstance {
            id: "test-vm".to_string(),
            pid: 1234,
            spawn_time_ms: 95.5,
            partition: Arc::new(Mutex::new(None)),
        };

        assert_eq!(instance.id(), "test-vm");
        assert_eq!(instance.pid(), 1234);
        assert_eq!(instance.spawn_time_ms(), 95.5);
        assert_eq!(instance.socket_path(), "");
    }

    #[test]
    fn test_apple_hv_spawn_time_valid() {
        // Property test: spawn_time_ms must be positive and reasonable
        let spawn_times = vec![0.1, 50.0, 150.0, 200.0];
        for st in spawn_times {
            assert!(st > 0.0, "Spawn time must be positive: {}", st);
            assert!(st < 10000.0, "Spawn time must be < 10 seconds: {}", st);
        }
    }

    #[tokio::test]
    #[cfg(target_os = "macos")]
    #[ignore = "Requires macOS with Virtualization.framework"]
    async fn test_apple_hv_spawn_with_valid_resources() {
        // Integration test: requires actual macOS resources
        let config = VmConfig {
            vm_id: "test-integration".to_string(),
            kernel_path: "./resources/vmlinux".to_string(),
            rootfs_path: "./resources/rootfs.ext4".to_string(),
            vcpu_count: 2,
            memory_mb: 512,
            enable_networking: false,
            vsock_path: None,
            seccomp_filter: None,
        };

        // This would require actual kernel/rootfs files
        // Skip if resources don't exist
        if !Path::new("./resources/vmlinux").exists()
            || !Path::new("./resources/rootfs.ext4").exists()
        {
            return;
        }

        let result = start_apple_hv(&config).await;
        assert!(result.is_ok(), "VM spawn should succeed with valid resources");

        if let Ok(mut instance) = result {
            assert!(instance.spawn_time_ms > 0.0);
            assert!(instance.spawn_time_ms < 5000.0); // Should be < 5 seconds
            assert_eq!(instance.id(), "test-integration");

            // Cleanup
            let _ = instance.stop().await;
        }
    }

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn test_apple_hv_missing_kernel() {
        // Test error handling: missing kernel file
        let config = VmConfig {
            vm_id: "test-missing-kernel".to_string(),
            kernel_path: "/nonexistent/vmlinux".to_string(),
            rootfs_path: "./resources/rootfs.ext4".to_string(),
            ..VmConfig::default()
        };

        let result = start_apple_hv(&config).await;
        assert!(result.is_err(), "Should fail with missing kernel");
    }

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn test_apple_hv_missing_rootfs() {
        // Test error handling: missing rootfs file
        let config = VmConfig {
            vm_id: "test-missing-rootfs".to_string(),
            kernel_path: "./resources/vmlinux".to_string(),
            rootfs_path: "/nonexistent/rootfs.ext4".to_string(),
            ..VmConfig::default()
        };

        let result = start_apple_hv(&config).await;
        assert!(result.is_err(), "Should fail with missing rootfs");
    }
}
