// JIT Micro-VM Configuration
//
// Firecracker VM configuration for secure agent execution

use crate::vm::rootfs::RootfsConfig;
use crate::vm::seccomp::SeccompFilter;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

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

    /// Root filesystem path (deprecated, use rootfs_config instead)
    #[serde(default)]
    pub rootfs_path: String,

    /// Root filesystem configuration with hardening
    #[serde(default)]
    pub rootfs_config: Option<RootfsConfig>,

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
            rootfs_config: Some(RootfsConfig::default()),
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

        // Generate vsock path using UUID to ensure uniqueness and prevent path traversal
        config.vsock_path = Some(format!("/tmp/luminaguard/vsock/{}.sock", Uuid::new_v4()));

        config
    }

    /// Get the effective rootfs path (from rootfs_config if available, else rootfs_path)
    pub fn effective_rootfs_path(&self) -> &str {
        if let Some(ref rootfs_config) = self.rootfs_config {
            &rootfs_config.rootfs_path
        } else {
            &self.rootfs_path
        }
    }

    /// Check if rootfs hardening is enabled
    pub fn has_rootfs_hardening(&self) -> bool {
        self.rootfs_config.is_some()
            && self
                .rootfs_config
                .as_ref()
                .map(|c| c.read_only)
                .unwrap_or(false)
    }

    /// Get kernel boot arguments (including overlay init if rootfs hardening enabled)
    pub fn get_boot_args(&self) -> String {
        if let Some(ref rootfs_config) = self.rootfs_config {
            rootfs_config.get_boot_args()
        } else {
            // Default boot args without overlay
            "console=ttyS0 reboot=k panic=1 pci=off".to_string()
        }
    }

    /// Get overlay drive configuration (if using ext4 overlay)
    pub fn get_overlay_drive(&self) -> Option<OverlayDriveConfig> {
        if let Some(ref rootfs_config) = self.rootfs_config {
            if rootfs_config.overlay_type == crate::vm::rootfs::OverlayType::Ext4 {
                rootfs_config
                    .overlay_path
                    .as_ref()
                    .map(|path| OverlayDriveConfig {
                        drive_id: "overlayfs".to_string(),
                        path_on_host: path.clone(),
                        is_root_device: false,
                        is_read_only: false,
                    })
            } else {
                None
            }
        } else {
            None
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
    ///
    /// This implements the complete Firecracker API v1 format for VM configuration.
    /// See: https://github.com/firecracker-microvm/firecracker/blob/main/docs/api/v1.md
    pub fn to_firecracker_json(&self) -> String {
        let boot_args = self.get_boot_args();

        // Build the drives array - include rootfs if configured
        let mut drives = serde_json::json!([
            {
                "drive_id": "rootfs",
                "path_on_host": self.effective_rootfs_path(),
                "is_root_device": true,
                "is_read_only": false
            }
        ]);

        // Add overlay drive if configured
        if let Some(overlay_config) = self.get_overlay_drive() {
            drives = serde_json::json!([
                drives,
                {
                    "drive_id": overlay_config.drive_id,
                    "path_on_host": overlay_config.path_on_host,
                    "is_root_device": overlay_config.is_root_device,
                    "is_read_only": overlay_config.is_read_only
                }
            ]);
        }

        // Build vsock configuration if available
        let vsock_config = if let Some(ref vsock_path) = self.vsock_path {
            Some(serde_json::json!({
                "vsock_id": "vsock0",
                "guest_cid": 3,
                "uds_path": vsock_path
            }))
        } else {
            None
        };

        // Build the complete Firecracker configuration
        let mut config = serde_json::json!({
            "boot-source": {
                "kernel_image_path": self.kernel_path,
                "boot_args": boot_args
            },
            "machine-config": {
                "vcpu_count": self.vcpu_count,
                "mem_size_mib": self.memory_mb,
                "ht_enabled": false
            },
            "drives": drives
        });

        // Add vsock if configured
        if let Some(vsock) = vsock_config {
            config["vsock"] = vsock;
        }

        // Add network interfaces (currently empty for security - no networking)
        config["network-interfaces"] = serde_json::json!([]);

        // Add seccomp filter if configured
        if let Some(ref seccomp) = self.seccomp_filter {
            config["kernel"] = serde_json::json!({
                "seccomp_level": seccomp.level as u32
            });
        }

        serde_json::to_string_pretty(&config).unwrap_or_else(|_| {
            // Fallback to basic format if serialization fails
            format!(
                r#"{{
  "boot-source": {{
    "kernel_image_path": "{}",
    "boot_args": "{}"
  }},
  "machine-config": {{
    "vcpu_count": {},
    "mem_size_mib": {},
    "ht_enabled": false
  }}
}}"#,
                self.kernel_path, boot_args, self.vcpu_count, self.memory_mb
            )
        })
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
    fn test_config_validation_fails_networking() {
        let mut config = VmConfig::default();
        config.enable_networking = true;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Networking MUST be disabled"));
    }

    #[test]
    fn test_to_json() {
        let config = VmConfig::new("test-vm".to_string());
        let json = config.to_firecracker_json();
        assert!(json.contains("boot-source"));
        assert!(json.contains("machine-config"));
    }

    #[test]
    fn test_effective_rootfs_path_with_config() {
        let config = VmConfig::new("test-vm".to_string());
        assert_eq!(config.effective_rootfs_path(), "./resources/rootfs.ext4");
    }

    #[test]
    fn test_has_rootfs_hardening() {
        let config = VmConfig::new("test-vm".to_string());
        assert!(
            config.has_rootfs_hardening(),
            "Rootfs hardening should be enabled by default"
        );
    }

    #[test]
    fn test_get_boot_args_without_hardening() {
        use crate::vm::rootfs::RootfsConfig;
        let mut config = VmConfig::new("test-vm".to_string());
        config.rootfs_config = None;
        let args = config.get_boot_args();
        assert!(!args.contains("overlay_root"));
        assert!(!args.contains("init=/sbin/overlay-init"));
    }

    #[test]
    fn test_get_boot_args_with_hardening() {
        let config = VmConfig::new("test-vm".to_string());
        let args = config.get_boot_args();
        assert!(args.contains("overlay_root=ram"));
        assert!(args.contains("init=/sbin/overlay-init"));
    }
}

/// Overlay drive configuration for Firecracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayDriveConfig {
    /// Drive identifier
    pub drive_id: String,
    /// Path on host
    pub path_on_host: String,
    /// Is root device (always false for overlay)
    pub is_root_device: bool,
    /// Is read-only (always false for overlay)
    pub is_read_only: bool,
}
