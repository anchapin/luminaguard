//! Mesh Configuration
//!
//! This module defines configuration for mesh operations including discovery,
//! messaging, and peer management settings.

use serde::{Deserialize, Serialize};

/// Mesh configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Name of the mesh (default: "LuminaGuard-Dev")
    #[serde(default = "default_mesh_name")]
    pub mesh_name: String,

    /// Service type for mDNS discovery (default: "_luminaguard._tcp")
    #[serde(default = "default_service_type")]
    pub service_type: String,

    /// Port for mesh communication (default: 45721)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Discovery interval in seconds (default: 60)
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval_secs: u64,

    /// Peer timeout in seconds (default: 60)
    #[serde(default = "default_peer_timeout")]
    pub peer_timeout_secs: u64,

    /// Suspect timeout in seconds (default: 30)
    #[serde(default = "default_suspect_timeout")]
    pub suspect_timeout_secs: u64,

    /// Maximum message size in bytes (default: 1 MB)
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Enable peer discovery
    pub enable_discovery: bool,

    /// Enable periodic announcements
    pub enable_announcements: bool,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            mesh_name: default_mesh_name(),
            service_type: default_service_type(),
            port: default_port(),
            discovery_interval_secs: default_discovery_interval(),
            peer_timeout_secs: default_peer_timeout(),
            suspect_timeout_secs: default_suspect_timeout(),
            max_message_size: default_max_message_size(),
            enable_discovery: true,
            enable_announcements: true,
        }
    }
}

fn default_mesh_name() -> String {
    "LuminaGuard-Dev".to_string()
}

fn default_service_type() -> String {
    "_luminaguard._tcp".to_string()
}

fn default_port() -> u16 {
    45721 // IANA unassigned
}

fn default_discovery_interval() -> u64 {
    60 // seconds
}

fn default_peer_timeout() -> u64 {
    60 // seconds
}

fn default_suspect_timeout() -> u64 {
    30 // seconds
}

fn default_max_message_size() -> usize {
    1 * 1024 * 1024 // 1 MB
}

impl MeshConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.mesh_name.is_empty() {
            return Err("mesh_name cannot be empty".to_string());
        }

        if self.port == 0 {
            return Err("port must be greater than 0".to_string());
        }

        if self.discovery_interval_secs < 10 {
            return Err("discovery_interval_secs must be at least 10 seconds".to_string());
        }

        if self.peer_timeout_secs < self.suspect_timeout_secs {
            return Err(
                "peer_timeout_secs must be greater than suspect_timeout_secs".to_string(),
            );
        }

        if self.max_message_size > 10 * 1024 * 1024 {
            return Err("max_message_size cannot exceed 10 MB".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MeshConfig::default();

        assert_eq!(config.mesh_name, "LuminaGuard-Dev");
        assert_eq!(config.service_type, "_luminaguard._tcp");
        assert_eq!(config.port, 45721);
        assert_eq!(config.discovery_interval_secs, 60);
        assert_eq!(config.peer_timeout_secs, 60);
        assert_eq!(config.suspect_timeout_secs, 30);
        assert_eq!(config.max_message_size, 1 * 1024 * 1024);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = MeshConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_mesh_name() {
        let mut config = MeshConfig::default();
        config.mesh_name = "".to_string();
        assert_eq!(
            config.validate().unwrap_err(),
            "mesh_name cannot be empty"
        );
    }

    #[test]
    fn test_config_validation_invalid_port() {
        let mut config = MeshConfig::default();
        config.port = 0;
        assert_eq!(
            config.validate().unwrap_err(),
            "port must be greater than 0"
        );
    }

    #[test]
    fn test_config_validation_invalid_timeout() {
        let mut config = MeshConfig::default();
        config.peer_timeout_secs = 10;
        config.suspect_timeout_secs = 30;
        assert_eq!(
            config.validate().unwrap_err(),
            "peer_timeout_secs must be greater than suspect_timeout_secs"
        );
    }
}
