// JIT Micro-VM Configuration
//
// Firecracker VM configuration for secure agent execution

use crate::vm::seccomp::SeccompFilter;
use serde::{Deserialize, Serialize};

/// VM configuration for Firecracker
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
            kernel_path: "/path/to/vmlinux.bin".to_string(),
            rootfs_path: "/path/to/rootfs.ext4".to_string(),
            enable_networking: false,
            seccomp_filter: None,
        }
    }
}

impl VmConfig {
    /// Create a new VM config with defaults
    pub fn new(vm_id: String) -> Self {
        Self {
            vm_id,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.vcpu_count == 0 {
            anyhow::bail!("vCPU count must be > 0");
        }
        if self.memory_mb < 128 {
            anyhow::bail!("Memory must be at least 128 MB");
        }
        if self.enable_networking {
            anyhow::bail!("Networking MUST be disabled for security");
        }
        Ok(())
    }

    /// Convert to Firecracker JSON config
    pub fn to_firecracker_json(&self) -> String {
        // TODO: Implement actual Firecracker JSON format
        format!(
            r#"{{
  "boot-source": {{
    "kernel_image_path": "{}"
  }},
  "machine-config": {{
    "vcpu_count": {},
    "mem_size_mib": {},
    "ht_enabled": false
  }}
}}"#,
            self.kernel_path, self.vcpu_count, self.memory_mb
        )
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
    }
}
