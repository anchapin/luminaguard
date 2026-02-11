// JIT Micro-VM Configuration
//
// Firecracker VM configuration for secure agent execution.
// This module handles the configuration for individual Micro-VMs, including
// resource limits, kernel/rootfs paths, and security settings.

use crate::vm::firecracker_types::{BootSource, Drive, FirecrackerConfig, MachineConfiguration};
use crate::vm::seccomp::SeccompFilter;
use serde::{Deserialize, Serialize};

/// VM configuration for Firecracker
///
/// Contains all settings required to spawn a Firecracker VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// VM ID (unique identifier)
    pub vm_id: String,

    /// Number of vCPUs (default: 1)
    pub vcpu_count: u8,

    /// Memory size in MB (default: 512)
    pub memory_mb: u32,

    /// Kernel image path
    pub kernel_path: String,

    /// Root filesystem path
    pub rootfs_path: String,

    /// Enable networking (default: false for security)
    pub enable_networking: bool,

    /// vsock socket path (automatically generated)
    #[serde(skip)]
    pub vsock_path: Option<String>,

    /// Seccomp filter configuration
    #[serde(default)]
    pub seccomp_filter: Option<SeccompFilter>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            vm_id: "default".to_string(),
            vcpu_count: 1,
            memory_mb: 512,
            kernel_path: "./resources/vmlinux".to_string(),
            rootfs_path: "./resources/rootfs.ext4".to_string(),
            enable_networking: false,
            vsock_path: None,
            seccomp_filter: None,
        }
    }
}

impl VmConfig {
    /// Create a new VM config with defaults
    ///
    /// # Arguments
    ///
    /// * `vm_id` - Unique identifier for the VM
    pub fn new(vm_id: String) -> Self {
        let mut config = Self {
            vm_id,
            ..Default::default()
        };

        // Generate vsock path
        config.vsock_path = Some(format!("/tmp/ironclaw/vsock/{}.sock", config.vm_id));

        config
    }

    /// Validate configuration
    ///
    /// Checks that the configuration meets security and resource requirements.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.enable_networking {
            anyhow::bail!("Networking MUST be disabled for security");
        }
        if self.vcpu_count == 0 {
            anyhow::bail!("vCPU count must be > 0");
        }
        if self.memory_mb < 128 {
            anyhow::bail!("Memory must be at least 128 MB");
        }
        Ok(())
    }

    /// Convert to Firecracker JSON config
    ///
    /// Generates the JSON configuration expected by Firecracker's `--config-file` argument.
    pub fn to_firecracker_json(&self) -> String {
        let boot_source = BootSource {
            kernel_image_path: self.kernel_path.clone(),
            boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off".to_string()),
        };

        let drives = vec![Drive {
            drive_id: "rootfs".to_string(),
            path_on_host: self.rootfs_path.clone(),
            is_root_device: true,
            is_read_only: false,
        }];

        let machine_config = MachineConfiguration {
            vcpu_count: self.vcpu_count,
            mem_size_mib: self.memory_mb,
            ht_enabled: Some(false),
        };

        let config = FirecrackerConfig {
            boot_source,
            drives,
            machine_config,
        };

        serde_json::to_string_pretty(&config).expect("Failed to serialize Firecracker config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VmConfig::default();
        assert_eq!(config.vcpu_count, 1);
        assert_eq!(config.memory_mb, 512);
        assert!(!config.enable_networking);
    }

    #[test]
    fn test_config_validation() {
        let config = VmConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_fails() {
        let mut config = VmConfig::default();
        config.vcpu_count = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_to_json() {
        let config = VmConfig::new("test-vm".to_string());
        let json = config.to_firecracker_json();
        assert!(json.contains("boot-source"));
        assert!(json.contains("machine-config"));
        assert!(json.contains("drives"));
        assert!(json.contains("rootfs"));
    }
}
