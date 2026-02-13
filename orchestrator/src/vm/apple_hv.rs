// macOS Virtualization.framework Backend
//
// This module implements the Hypervisor and VmInstance traits using 
// Apple's Virtualization.framework (via the `vz` crate).

use anyhow::{anyhow, Result};
#[cfg(target_os = "macos")]
use anyhow::Context;
use async_trait::async_trait;
#[cfg(target_os = "macos")]
use std::time::Instant;
use tracing::info;

use crate::vm::config::VmConfig;
use crate::vm::hypervisor::{Hypervisor, VmInstance};

#[cfg(target_os = "macos")]
use vz::{
    LinuxBootLoader, VirtualMachine, VirtualMachineConfiguration,
    VirtioBlockDeviceConfiguration, DiskImageStorageDeviceAttachment,
    VirtioConsoleDeviceSerialPortConfiguration, FileHandleSerialPortAttachment,
};

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
pub struct AppleHvInstance {
    pub id: String,
    pub spawn_time_ms: f64,
    #[cfg(target_os = "macos")]
    pub vm: VirtualMachine,
}

#[async_trait]
impl VmInstance for AppleHvInstance {
    fn id(&self) -> &str {
        &self.id
    }

    fn pid(&self) -> u32 {
        // macOS Virtualization.framework manages the process internally
        0
    }

    fn socket_path(&self) -> &str {
        // Apple HV doesn't use a Unix socket for API like Firecracker
        ""
    }

    fn spawn_time_ms(&self) -> f64 {
        self.spawn_time_ms
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping macOS VM (ID: {})", self.id);
        
        #[cfg(target_os = "macos")]
        {
            self.vm.stop().await.context("Failed to stop macOS VM")?;
        }
        
        Ok(())
    }
}

#[cfg(target_os = "macos")]
async fn start_apple_hv(config: &VmConfig) -> Result<AppleHvInstance> {
    let start_time = Instant::now();
    info!("Starting macOS Virtualization.framework VM: {}", config.vm_id);

    // 1. Configure Bootloader (Linux)
    let mut bootloader = LinuxBootLoader::new(&config.kernel_path);
    bootloader.set_command_line("console=hvc0 root=/dev/vda rw");

    // 2. Machine Configuration
    let mut vz_config = VirtualMachineConfiguration::new(
        bootloader, 
        config.vcpu_count as usize, 
        config.memory_mb as u64 * 1024 * 1024
    );

    // 3. Storage (Rootfs)
    let attachment = DiskImageStorageDeviceAttachment::new(&config.rootfs_path, true)
        .context("Failed to create storage attachment")?;
    let block_device = VirtioBlockDeviceConfiguration::new(attachment);
    vz_config.add_storage_device(block_device);

    // 4. Console (Stdout for now)
    let serial_port = VirtioConsoleDeviceSerialPortConfiguration::new(
        FileHandleSerialPortAttachment::stdout().context("Failed to attach serial port to stdout")?
    );
    vz_config.add_serial_port(serial_port);

    // 5. Start VM
    let vm = VirtualMachine::new(vz_config).context("Failed to create VirtualMachine")?;
    vm.start().await.context("Failed to start VirtualMachine")?;

    let elapsed = start_time.elapsed();
    let spawn_time_ms = elapsed.as_secs_f64() * 1000.0;
    info!("macOS VM {} started in {:.2}ms", config.vm_id, spawn_time_ms);

    Ok(AppleHvInstance {
        id: config.vm_id.clone(),
        spawn_time_ms,
        vm,
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

    #[tokio::test]
    async fn test_spawn_on_unsupported_platform() {
        #[cfg(not(target_os = "macos"))]
        {
            let hv = AppleHvHypervisor;
            let config = VmConfig::new("test".to_string());
            let result = hv.spawn(&config).await;
            assert!(result.is_err());
            if let Err(e) = result {
                assert!(e.to_string().contains("only available on macOS"));
            } else {
                panic!("Expected an error, but got Ok");
            }
        }
    }
}
