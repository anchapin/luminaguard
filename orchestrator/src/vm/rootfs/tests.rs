// Root Filesystem Hardening Tests
//
// Comprehensive tests for read-only rootfs and overlay functionality

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::rootfs::{OverlayType, RootfsConfig, RootfsManager};

    #[test]
    fn test_overlay_type_default_is_tmpfs() {
        let overlay = OverlayType::default();
        assert_eq!(overlay, OverlayType::Tmpfs);
    }

    #[test]
    fn test_rootfs_config_default_security() {
        let config = RootfsConfig::default();
        assert!(
            config.read_only,
            "SECURITY: Rootfs MUST be read-only by default"
        );
        assert_eq!(config.overlay_type, OverlayType::Tmpfs);
    }

    #[test]
    fn test_rootfs_config_new_uses_tmpfs() {
        let config = RootfsConfig::new("/tmp/test.ext4".to_string());
        assert!(config.read_only);
        assert_eq!(config.overlay_type, OverlayType::Tmpfs);
        assert!(config.overlay_path.is_none());
    }

    #[test]
    fn test_rootfs_config_with_persistent_overlay() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );
        assert!(config.read_only);
        assert_eq!(config.overlay_type, OverlayType::Ext4);
        assert_eq!(
            config.overlay_path,
            Some("/tmp/overlay.ext4".to_string())
        );
        assert_eq!(config.overlay_size_mb, Some(512));
    }

    #[test]
    fn test_rootfs_validate_requires_readonly() {
        let mut config = RootfsConfig::default();
        config.read_only = false;
        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("MUST be read-only"),
            "Error should mention read-only requirement: {}",
            error_msg
        );
    }

    #[test]
    fn test_rootfs_validate_ext4_requires_path() {
        let mut config = RootfsConfig::default();
        config.overlay_type = OverlayType::Ext4;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("overlay_path"));
    }

    #[test]
    fn test_rootfs_validate_ext4_min_size() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            32, // Too small
        );
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 64 MB"));
    }

    #[test]
    fn test_rootfs_validate_ext4_max_size_warning() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            20480, // 20 GB - very large
        );
        // Should validate successfully but with warning (logged, not tested)
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_boot_args_tmpfs() {
        let config = RootfsConfig::new("/tmp/rootfs.ext4".to_string());
        let args = config.get_boot_args();
        assert!(args.contains("overlay_root=ram"));
        assert!(args.contains("init=/sbin/overlay-init"));
        assert!(args.contains("console=ttyS0"));
    }

    #[test]
    fn test_boot_args_ext4() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );
        let args = config.get_boot_args();
        assert!(args.contains("overlay_root=vdb"));
        assert!(args.contains("init=/sbin/overlay-init"));
        assert!(args.contains("console=ttyS0"));
    }

    #[test]
    fn test_needs_overlay_init_when_readonly() {
        let config = RootfsConfig::default();
        assert!(config.needs_overlay_init());
    }

    #[test]
    fn test_rootfs_manager_new() {
        let config = RootfsConfig::default();
        let manager = RootfsManager::new(config);
        assert_eq!(manager.config.rootfs_path, config.rootfs_path);
    }

    // Property-based tests

    #[test]
    fn test_property_all_configs_readonly() {
        // Security invariant: ALL rootfs configs must be read-only
        let configs = vec![
            RootfsConfig::default(),
            RootfsConfig::new("/tmp/test1.ext4".to_string()),
            RootfsConfig::with_persistent_overlay(
                "/tmp/test2.ext4".to_string(),
                "/tmp/overlay2.ext4".to_string(),
                512,
            ),
            RootfsConfig::with_persistent_overlay(
                "/tmp/test3.ext4".to_string(),
                "/tmp/overlay3.ext4".to_string(),
                1024,
            ),
        ];

        for config in configs {
            assert!(
                config.read_only,
                "SECURITY: All rootfs configs must enforce read-only"
            );
        }
    }

    #[test]
    fn test_property_overlay_size_limits() {
        // Test size boundaries for ext4 overlay
        let too_small = 32;
        let valid = 512;
        let large = 20480;

        let config_too_small = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            too_small,
        );
        assert!(config_too_small.validate().is_err());

        let config_valid = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            valid,
        );
        assert!(config_valid.validate().is_ok());

        let config_large = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            large,
        );
        // Should validate (with warning in logs)
        assert!(config_large.validate().is_ok());
    }

    #[test]
    fn test_property_boot_args_always_contain_overlay_init() {
        // All overlay types should use overlay-init
        let tmpfs_config = RootfsConfig::new("/tmp/rootfs.ext4".to_string());
        let ext4_config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );

        let tmpfs_args = tmpfs_config.get_boot_args();
        let ext4_args = ext4_config.get_boot_args();

        assert!(
            tmpfs_args.contains("init=/sbin/overlay-init"),
            "Tmpfs overlay should use overlay-init"
        );
        assert!(
            ext4_args.contains("init=/sbin/overlay-init"),
            "Ext4 overlay should use overlay-init"
        );
    }

    #[test]
    fn test_security_readonly_flag_prevents_modification() {
        // This is a compile-time check that the flag exists and is enforced
        let config = RootfsConfig::default();
        assert!(
            config.read_only,
            "Read-only flag must be set by default for security"
        );

        // Attempting to set it to false should fail validation
        let mut config = RootfsConfig::default();
        config.read_only = false;
        assert!(
            config.validate().is_err(),
            "Validation should reject writable rootfs"
        );
    }

    #[test]
    fn test_overlay_type_serialization() {
        // Verify OverlayType can be serialized/deserialized
        let tmpfs = OverlayType::Tmpfs;
        let ext4 = OverlayType::Ext4;

        let tmpfs_json = serde_json::to_string(&tmpfs).unwrap();
        let ext4_json = serde_json::to_string(&ext4).unwrap();

        assert!(tmpfs_json.contains("Tmpfs") || tmpfs_json.contains("0"));
        assert!(ext4_json.contains("Ext4") || ext4_json.contains("1"));

        // Test deserialization
        let tmpfs_deserialized: OverlayType =
            serde_json::from_str(&tmpfs_json).unwrap();
        let ext4_deserialized: OverlayType = serde_json::from_str(&ext4_json).unwrap();

        assert_eq!(tmpfs_deserialized, OverlayType::Tmpfs);
        assert_eq!(ext4_deserialized, OverlayType::Ext4);
    }

    #[test]
    fn test_rootfs_config_serialization() {
        // Verify RootfsConfig can be serialized/deserialized
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RootfsConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.rootfs_path, config.rootfs_path);
        assert_eq!(deserialized.read_only, config.read_only);
        assert_eq!(deserialized.overlay_type, config.overlay_type);
        assert_eq!(deserialized.overlay_path, config.overlay_path);
        assert_eq!(deserialized.overlay_size_mb, config.overlay_size_mb);
    }
}
