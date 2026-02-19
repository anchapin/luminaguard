// JIT Micro-VM Configuration
//
// Firecracker VM configuration for secure agent execution

use crate::vm::rootfs::RootfsConfig;
use crate::vm::seccomp::SeccompFilter;
use serde::{Deserialize, Serialize};
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
    /// Returns a JSON string that can be sent to the Firecracker API.
    /// Format follows Firecracker v1 API specification.
    pub fn to_firecracker_json(&self) -> String {
        use serde_json::json;

        // Build drives array - start with rootfs drive
        let mut drives = vec![json!({
            "drive_id": "rootfs",
            "path_on_host": self.effective_rootfs_path(),
            "is_root_device": true,
            "is_read_only": false
        })];

        // Add overlay drive if configured (push to array, not nest)
        if let Some(overlay_config) = self.get_overlay_drive() {
            drives.push(json!({
                "drive_id": overlay_config.drive_id,
                "path_on_host": overlay_config.path_on_host,
                "is_root_device": overlay_config.is_root_device,
                "is_read_only": overlay_config.is_read_only
            }));
        }

        // Build the full config
        let mut config = json!({
            "boot-source": {
                "kernel_image_path": self.kernel_path,
                "boot_args": self.get_boot_args()
            },
            "machine-config": {
                "vcpu_count": self.vcpu_count,
                "mem_size_mib": self.memory_mb,
                "ht_enabled": false
            },
            "drives": drives,
            "vsock": {
                "guest_cid": 3,
                "socket_path": self.vsock_path.clone().unwrap_or_default()
            },
            "network-interfaces": json!([])
        });

        // Add seccomp filter under correct key (not "kernel")
        // Firecracker v1 uses top-level "seccomp" key
        if let Some(ref seccomp) = self.seccomp_filter {
            // Get the seccomp filter JSON from the filter itself
            if let Ok(seccomp_json) = seccomp.to_firecracker_json() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&seccomp_json) {
                    config["seccomp"] = parsed;
                }
            }
        }

        // Serialize to string
        serde_json::to_string_pretty(&config)
            .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize config: {}\"}}", e))
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
        let config = VmConfig {
            vcpu_count: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_fails_networking() {
        let config = VmConfig {
            enable_networking: true,
            ..Default::default()
        };
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
