// Firecracker API Types
//
// This module contains the data structures used to interact with the Firecracker API.
// These structs are serialized to JSON and sent to the Firecracker process.

use serde::{Deserialize, Serialize};

/// Boot source configuration
///
/// Configures the kernel image and boot arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootSource {
    /// Path to the kernel image file
    pub kernel_image_path: String,
    /// Boot arguments (e.g., "console=ttyS0 reboot=k panic=1 pci=off")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_args: Option<String>,
}

/// Drive configuration
///
/// Configures a block device (e.g., root filesystem).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    /// Unique identifier for the drive
    pub drive_id: String,
    /// Path to the disk image on the host
    pub path_on_host: String,
    /// Whether this is the root device
    pub is_root_device: bool,
    /// Whether the drive is read-only
    pub is_read_only: bool,
}

/// Machine configuration
///
/// Configures the VM's resources (vCPUs, memory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfiguration {
    /// Number of vCPUs
    pub vcpu_count: u8,
    /// Memory size in MiB
    pub mem_size_mib: u32,
    /// Whether hyperthreading is enabled (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ht_enabled: Option<bool>,
}

/// Action request
///
/// Used to trigger actions like starting the VM or sending Ctrl+Alt+Del.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Type of action (e.g., "InstanceStart", "SendCtrlAltDel")
    pub action_type: String,
}

/// Full Firecracker Configuration
///
/// Represents the complete configuration for a Firecracker VM,
/// compatible with the `--config-file` argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirecrackerConfig {
    /// Boot source configuration
    #[serde(rename = "boot-source")]
    pub boot_source: BootSource,
    /// List of drives
    pub drives: Vec<Drive>,
    /// Machine configuration
    #[serde(rename = "machine-config")]
    pub machine_config: MachineConfiguration,
    // Add other fields as needed (network-interfaces, logger, metrics, etc.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_source_serialization() {
        let boot_source = BootSource {
            kernel_image_path: "/path/to/kernel".to_string(),
            boot_args: Some("console=ttyS0".to_string()),
        };
        let json = serde_json::to_string(&boot_source).unwrap();
        assert!(json.contains("kernel_image_path"));
        assert!(json.contains("boot_args"));
    }

    #[test]
    fn test_drive_serialization() {
        let drive = Drive {
            drive_id: "rootfs".to_string(),
            path_on_host: "/path/to/rootfs".to_string(),
            is_root_device: true,
            is_read_only: false,
        };
        let json = serde_json::to_string(&drive).unwrap();
        assert!(json.contains("drive_id"));
        assert!(json.contains("path_on_host"));
        assert!(json.contains("is_root_device"));
    }

    #[test]
    fn test_machine_config_serialization() {
        let config = MachineConfiguration {
            vcpu_count: 2,
            mem_size_mib: 1024,
            ht_enabled: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("vcpu_count"));
        assert!(json.contains("mem_size_mib"));
        assert!(!json.contains("ht_enabled"));
    }

    #[test]
    fn test_firecracker_config_serialization() {
        let config = FirecrackerConfig {
            boot_source: BootSource {
                kernel_image_path: "kernel".to_string(),
                boot_args: None,
            },
            drives: vec![Drive {
                drive_id: "rootfs".to_string(),
                path_on_host: "rootfs".to_string(),
                is_root_device: true,
                is_read_only: false,
            }],
            machine_config: MachineConfiguration {
                vcpu_count: 1,
                mem_size_mib: 128,
                ht_enabled: Some(false),
            },
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("boot-source"));
        assert!(json.contains("machine-config"));
        assert!(json.contains("drives"));
    }
}
