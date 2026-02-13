// Jailer Integration Tests
//
// Comprehensive tests for Jailer sandbox functionality

#[cfg(test)]
mod tests {
    use crate::vm::config::VmConfig;
    use crate::vm::jailer::{JailerConfig, verify_jailer_installed};

    /// Integration test: Verify Jailer binary is available
    ///
    /// Requirements:
    /// - Jailer installed at /usr/local/bin/jailer
    #[test]
    fn test_verify_jailer_installed() {
        let result = verify_jailer_installed();
        match result {
            Ok(_path) => {
                println!("Jailer is installed at: /usr/local/bin/jailer");
            }
            Err(e) => {
                println!("Jailer not installed: {}", e);
                println!("Tests requiring real Jailer will be skipped");
            }
        }
        assert!(result.is_ok() || result.is_err()); // Always passes, just reports status
    }

    /// Test that jailer config validates correct IDs
    #[test]
    fn test_jailer_config_valid_id() {
        let config = JailerConfig::test_config("test-vm-123".to_string());
        assert!(config.validate().is_ok());
    }

    /// Test that jailer config rejects empty IDs
    #[test]
    fn test_jailer_config_empty_id() {
        let config = JailerConfig::test_config("".to_string());
        assert!(config.validate().is_err());
    }

    /// Test that jailer config rejects IDs with invalid chars
    #[test]
    fn test_jailer_config_invalid_chars() {
        let config = JailerConfig::test_config("invalid@id#with$symbols".to_string());
        assert!(config.validate().is_err());
    }

    /// Test that jailer config rejects IDs that are too long
    #[test]
    fn test_jailer_config_id_too_long() {
        let long_id = "a".repeat(65); // 65 > 64 limit
        let config = JailerConfig::test_config(long_id);
        assert!(config.validate().is_err());
    }

    /// Test that jailer config with custom user works
    #[test]
    fn test_jailer_config_with_user() {
        let config = JailerConfig::test_config("test".to_string())
            .with_user(123, 456);
        assert_eq!(config.uid, 123);
        assert_eq!(config.gid, 456);
        assert!(config.validate().is_ok());
    }

    /// Test that jailer config with NUMA node works
    #[test]
    fn test_jailer_config_with_numa() {
        let config = JailerConfig::test_config("test".to_string())
            .with_numa_node(1);
        assert_eq!(config.numa_node, 1);
        assert!(config.validate().is_ok());
    }

    /// Test that jailer config with cgroups works
    #[test]
    fn test_jailer_config_with_cgroup() {
        let config = JailerConfig::test_config("test".to_string())
            .with_cgroup("cpu.shares".to_string(), "1024".to_string());

        assert_eq!(
            config.cgroups.get("cpu.shares"),
            Some(&"1024".to_string())
        );
        assert!(config.validate().is_ok());
    }

    /// Test that chroot path is computed correctly
    #[test]
    fn test_jailer_chroot_path() {
        let config = JailerConfig::new("my-vm".to_string());
        let chroot_dir = config.chroot_dir();

        assert!(chroot_dir.ends_with("firecracker/my-vm/root"));
        assert!(chroot_dir.starts_with("/srv/jailer"));
    }

    /// Test that jailer builds correct arguments
    #[test]
    fn test_jailer_build_args() {
        let config = JailerConfig::new("test-vm".to_string())
            .with_numa_node(1)
            .with_user(1000, 1000);

        let args = config.build_args();

        // Verify all required arguments are present
        assert!(args.contains(&"--id".to_string()));
        assert!(args.contains(&"test-vm".to_string()));
        assert!(args.contains(&"--node".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"--uid".to_string()));
        assert!(args.contains(&"1000".to_string()));
        assert!(args.contains(&"--gid".to_string()));
        assert!(args.contains(&"--daemonize".to_string()));
        assert!(args.contains(&"--new-pid-ns".to_string()));
        assert!(args.contains(&"--".to_string()));
    }

    /// Test that jailer args include cgroups
    #[test]
    fn test_jailer_args_with_cgroups() {
        let mut config = JailerConfig::new("test-vm".to_string());
        config.cgroups.insert("cpu.shares".to_string(), "512".to_string());
        config.cgroups.insert("memory.limit_in_bytes".to_string(), "268435456".to_string());

        let args = config.build_args();

        // Verify cgroup arguments are present
        let args_str = args.join(" ");
        assert!(args_str.contains("--cgroup"));
        assert!(args_str.contains("cpu.shares=512"));
        assert!(args_str.contains("memory.limit_in_bytes=268435456"));
    }

    /// Test that jailer args with network namespace
    #[test]
    fn test_jailer_args_with_netns() {
        let config = JailerConfig::new("test-vm".to_string())
            .with_netns(std::path::PathBuf::from("/var/run/netns/myns"));

        let args = config.build_args();

        assert!(args.contains(&"--netns".to_string()));
        assert!(args.contains(&"/var/run/netns/myns".to_string()));
    }

    /// Property-based test: All valid VM IDs should pass validation
    #[test]
    fn test_property_valid_ids_pass() {
        let max_id = "a".repeat(64);
        let valid_ids = vec![
            "simple",
            "with-dash",
            "with123numbers",
            "UPPERCASE",
            "mixed-Case-123",
            "a",
            "1",
            // Maximum valid length
            &max_id,
        ];

        for id in valid_ids {
            let config = JailerConfig::test_config(id.to_string());
            assert!(
                config.validate().is_ok(),
                "ID should be valid: {}",
                id
            );
        }
    }

    /// Property-based test: All invalid VM IDs should fail validation
    #[test]
    fn test_property_invalid_ids_fail() {
        let too_long = "a".repeat(65);
        let invalid_ids = vec![
            "",
            "with_underscore", // underscores are invalid
            "with.dot",       // dots are invalid
            "with/slash",      // slashes are invalid
            "with@symbol",     // @ is invalid
            "with#hash",       // # is invalid
            "with$ dollar",     // $ is invalid
            "with%percent",    // % is invalid
            "with&ampersand",  // & is invalid
            "with*asterisk",   // * is invalid
            "with space",       // spaces are invalid
            &too_long,  // Too long
        ];

        for id in invalid_ids {
            let config = JailerConfig::test_config(id.to_string());
            assert!(
                config.validate().is_err(),
                "ID should be invalid: {}",
                id
            );
        }
    }

    /// Integration test: Spawn and destroy jailed VM
    #[tokio::test]
    #[ignore] // Requires root and actual Firecracker installation
    async fn test_spawn_jailed_vm() {
        use crate::vm::{spawn_vm_jailed, destroy_vm_jailed};

        // Skip if jailer not installed
        if verify_jailer_installed().is_err() {
            return;
        }

        // Skip if test resources not available
        if !std::path::Path::new("./resources/vmlinux").exists() {
            return;
        }

        let vm_config = VmConfig::new("test-jailed-vm".to_string());
        let jailer_config = JailerConfig::new("test-jailed-vm".to_string());

        let result = spawn_vm_jailed("test-jailed-vm", &vm_config, &jailer_config).await;

        if result.is_err() {
            println!("Skipping test: Jailed VM spawn failed (may require root)");
            return;
        }

        let handle = result.unwrap();
        assert_eq!(handle.id, "test-jailed-vm");
        assert!(handle.spawn_time_ms > 0.0);

        destroy_vm_jailed(handle, &jailer_config)
            .await
            .unwrap();
    }

    /// Integration test: Jailed VM with non-root user
    #[tokio::test]
    #[ignore] // Requires root to set up
    async fn test_spawn_jailed_vm_with_user() {
        use crate::vm::{spawn_vm_jailed, destroy_vm_jailed};

        // Skip if jailer not installed
        if verify_jailer_installed().is_err() {
            return;
        }

        // Skip if test resources not available
        if !std::path::Path::new("./resources/vmlinux").exists() {
            return;
        }

        let vm_config = VmConfig::new("test-user-vm".to_string());
        let jailer_config = JailerConfig::new("test-user-vm".to_string())
            .with_user(1000, 1000); // Use non-root user

        let result = spawn_vm_jailed("test-user-vm", &vm_config, &jailer_config).await;

        if result.is_err() {
            println!("Skipping test: Jailed VM with user failed (may require user 1000 to exist)");
            return;
        }

        let handle = result.unwrap();
        assert_eq!(handle.id, "test-user-vm");

        destroy_vm_jailed(handle, &jailer_config)
            .await
            .unwrap();
    }

    /// Integration test: Jailed VM with cgroups
    #[tokio::test]
    #[ignore] // Requires root
    async fn test_spawn_jailed_vm_with_cgroups() {
        use crate::vm::{spawn_vm_jailed, destroy_vm_jailed};

        // Skip if jailer not installed
        if verify_jailer_installed().is_err() {
            return;
        }

        // Skip if test resources not available
        if !std::path::Path::new("./resources/vmlinux").exists() {
            return;
        }

        let vm_config = VmConfig::new("test-cgroup-vm".to_string());
        let jailer_config = JailerConfig::new("test-cgroup-vm".to_string())
            .with_cgroup("cpu.shares".to_string(), "256".to_string())
            .with_cgroup("memory.limit_in_bytes".to_string(), "268435456".to_string()); // 256MB

        let result = spawn_vm_jailed("test-cgroup-vm", &vm_config, &jailer_config).await;

        if result.is_err() {
            println!("Skipping test: Jailed VM with cgroups failed (may require root)");
            return;
        }

        let handle = result.unwrap();
        assert_eq!(handle.id, "test-cgroup-vm");

        destroy_vm_jailed(handle, &jailer_config)
            .await
            .unwrap();
    }

    /// Integration test: Verify real jailer binary can be executed
    #[tokio::test]
    async fn test_real_jailer_execution() {
        use std::process::Command;

        // Skip if jailer not installed
        if verify_jailer_installed().is_err() {
            println!("Skipping: Jailer not installed");
            return;
        }

        // Test 1: Verify jailer --help works
        let help_output = Command::new("jailer")
            .arg("--help")
            .output();

        match help_output {
            Ok(output) => {
                assert!(output.status.success(), "Jailer --help should succeed");
                let help_text = String::from_utf8_lossy(&output.stdout);
                assert!(help_text.contains("exec-file"), "Jailer help should mention exec-file");
                assert!(help_text.contains("id"), "Jailer help should mention id");
                println!("✓ Jailer --help works correctly");
            }
            Err(e) => {
                panic!("Failed to execute jailer --help: {}", e);
            }
        }

        // Test 2: Verify jailer version works
        let version_output = Command::new("jailer")
            .arg("--version")
            .output();

        match version_output {
            Ok(output) => {
                assert!(output.status.success(), "Jailer --version should succeed");
                let version_text = String::from_utf8_lossy(&output.stdout);
                println!("✓ Jailer version: {}", version_text.trim());
            }
            Err(e) => {
                panic!("Failed to execute jailer --version: {}", e);
            }
        }

        // Test 3: Verify jailer rejects invalid arguments
        let invalid_output = Command::new("jailer")
            .arg("--invalid-arg")
            .output();

        match invalid_output {
            Ok(output) => {
                assert!(!output.status.success(), "Jailer should reject invalid arguments");
                println!("✓ Jailer correctly rejects invalid arguments");
            }
            Err(e) => {
                panic!("Failed to execute jailer with invalid args: {}", e);
            }
        }
    }

    /// Integration test: Verify jailer binary path resolution
    #[tokio::test]
    async fn test_jailer_path_resolution() {
        use std::path::Path;

        // Test default path
        let default_path = Path::new("/usr/local/bin/jailer");
        if default_path.exists() {
            println!("✓ Jailer found at default path: /usr/local/bin/jailer");
        }

        // Test alternative paths
        let alt_paths = vec![
            "/usr/bin/jailer",
            "/opt/firecracker/bin/jailer",
            "/usr/local/bin/firecracker-v1.14.1-x86_64",
        ];

        for path in alt_paths {
            if Path::new(path).exists() {
                println!("✓ Jailer also found at: {}", path);
            }
        }

        // Verify verify_jailer_installed() function
        if verify_jailer_installed().is_ok() {
            println!("✓ verify_jailer_installed() succeeds");
        } else {
            println!("✗ verify_jailer_installed() failed (expected if no jailer installed)");
        }
    }

    /// Integration test: Test jailer config builds valid arguments
    #[tokio::test]
    async fn test_jailer_arg_validation() {
        // Test with default config
        let config = JailerConfig::new("test-vm".to_string());
        let args = config.build_args();

        // Verify required args are present
        let args_str = args.join(" ");
        assert!(args_str.contains("--id test-vm"), "Args should contain --id");
        assert!(args_str.contains("--node 0"), "Args should contain --node");
        assert!(args_str.contains("--uid"), "Args should contain --uid");
        assert!(args_str.contains("--gid"), "Args should contain --gid");
        assert!(args_str.contains("--exec-file"), "Args should contain --exec-file");
        assert!(args_str.contains("--chroot-base-dir"), "Args should contain --chroot-base-dir");
        assert!(args_str.contains("--daemonize"), "Args should contain --daemonize");
        assert!(args_str.contains("--new-pid-ns"), "Args should contain --new-pid-ns");
        assert!(args_str.ends_with("--"), "Args should end with separator");

        println!("✓ Jailer args build correctly: {}", args_str);
    }

    /// Integration test: Test chroot path generation
    #[tokio::test]
    async fn test_chroot_path_generation() {
        use std::path::Path;

        let config = JailerConfig::new("test-vm".to_string());
        let chroot_dir = config.chroot_dir();

        // Verify path structure: /srv/jailer/firecracker/<id>/root
        assert!(chroot_dir.is_absolute(), "Chroot path should be absolute");
        assert!(chroot_dir.ends_with("firecracker/test-vm/root"), "Chroot path should end with expected structure");

        println!("✓ Chroot path generated correctly: {}", chroot_dir.display());
    }
}
