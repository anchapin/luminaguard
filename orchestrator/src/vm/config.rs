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
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.vcpu_count == 0 {
            anyhow::bail!("vCPU count must be > 0");
        }
        if self.memory_mb < 128 {
            anyhow::bail!("Memory must be at least 128 MB");
        }
        if self.enable_networking {
            anyhow::bail!("Networking MUST be disabled for security. VMs should use vsock-only communication.");
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
    use crate::vm::seccomp::SeccompLevel;

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
    fn test_config_validation_low_memory() {
        let mut config = VmConfig::default();
        config.memory_mb = 64; // Below 128 MB minimum
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 128 MB"));
    }

    #[test]
    fn test_config_validation_networking_disabled() {
        let mut config = VmConfig::default();
        config.enable_networking = true;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("MUST be disabled for security"));
    }

    #[test]
    fn test_new_config_generates_vsock_path() {
        let config = VmConfig::new("my-vm-123".to_string());
        assert!(config.vsock_path.is_some());
        assert!(config
            .vsock_path
            .unwrap()
            .contains("/tmp/ironclaw/vsock/my-vm-123.sock"));
    }

    #[test]
    fn test_to_json() {
        let config = VmConfig::new("test-vm".to_string());
        let json = config.to_firecracker_json();
        assert!(json.contains("boot-source"));
        assert!(json.contains("machine-config"));
    }

    #[test]
    fn test_to_json_contains_config_values() {
        let mut config = VmConfig::new("test-vm".to_string());
        config.vcpu_count = 4;
        config.memory_mb = 2048;
        config.kernel_path = "/custom/kernel".to_string();

        let json = config.to_firecracker_json();
        assert!(json.contains("/custom/kernel"));
        assert!(json.contains("\"vcpu_count\": 4"));
        assert!(json.contains("\"mem_size_mib\": 2048"));
    }

    #[test]
    fn test_config_with_seccomp_filter() {
        let filter = SeccompFilter::new(SeccompLevel::Minimal);
        let mut config = VmConfig::default();
        config.seccomp_filter = Some(filter);

        assert!(config.seccomp_filter.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_vm_id_uniqueness() {
        let config1 = VmConfig::new("vm-1".to_string());
        let config2 = VmConfig::new("vm-2".to_string());

        assert_ne!(config1.vm_id, config2.vm_id);
        assert_ne!(config1.vsock_path, config2.vsock_path);
    }
}
