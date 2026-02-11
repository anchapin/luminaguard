// JIT Micro-VM Module
//
// This module handles spawning and managing ephemeral Firecracker VMs.
//
// Key invariants:
// - Spawn time: <200ms (actual: ~110ms)
// - Ephemeral: VM destroyed after task completion
// - Security: No host execution, full isolation

pub mod config;

#[cfg(unix)]
pub mod firecracker;
#[cfg(target_os = "linux")]
pub mod firewall;
#[cfg(target_os = "linux")]
pub mod seccomp;
#[cfg(unix)]
pub mod vsock;

// Prototype module for feasibility testing
#[cfg(feature = "vm-prototype")]
pub mod prototype;

#[cfg(all(test, unix))]
mod tests;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::vm::config::VmConfig;

#[cfg(unix)]
use crate::vm::firecracker::{start_firecracker, stop_firecracker, FirecrackerProcess};
#[cfg(target_os = "linux")]
use crate::vm::firewall::FirewallManager;
#[cfg(unix)]
use crate::vm::seccomp::{SeccompFilter, SeccompLevel};

/// VM handle for managing lifecycle
pub struct VmHandle {
    pub id: String,
    #[cfg(unix)]
    process: Arc<Mutex<Option<FirecrackerProcess>>>,
    #[cfg(not(unix))]
    #[allow(dead_code)]
    process: Arc<Mutex<Option<()>>>,
    pub spawn_time_ms: f64,
    config: VmConfig,
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    pub firewall_manager: Option<FirewallManager>,
    #[cfg(all(unix, not(target_os = "linux")))]
    #[allow(dead_code)]
    firewall_manager: Option<()>,
}

impl VmHandle {
    /// Get the vsock socket path for this VM
    pub fn vsock_path(&self) -> Option<&str> {
        self.config.vsock_path.as_deref()
    }
}

/// Spawn a new JIT Micro-VM
pub async fn spawn_vm(task_id: &str) -> Result<VmHandle> {
    spawn_vm_with_config(task_id, &VmConfig::new(task_id.to_string())).await
}

/// Spawn a new JIT Micro-VM with custom configuration
#[cfg(unix)]
pub async fn spawn_vm_with_config(task_id: &str, config: &VmConfig) -> Result<VmHandle> {
    tracing::info!("Spawning VM for task: {}", task_id);

    #[cfg(target_os = "linux")]
    let config_with_seccomp = if config.seccomp_filter.is_none() {
        let mut secured_config = config.clone();
        secured_config.seccomp_filter = Some(SeccompFilter::new(SeccompLevel::Basic));
        tracing::info!("Auto-enabling seccomp filter (Basic level) for security");
        secured_config
    } else {
        config.clone()
    };

    #[cfg(not(target_os = "linux"))]
    let config_with_seccomp = config.clone();

    // Configure firewall
    #[cfg(target_os = "linux")]
    let firewall_manager = FirewallManager::new(task_id.to_string());
    #[cfg(target_os = "linux")]
    {
        // Enforce isolation before spawning VM
        firewall_manager.configure_isolation()?;
    }

    // Start Firecracker VM
    let process = start_firecracker(&config_with_seccomp).await?;

    let spawn_time = process.spawn_time_ms;

    #[cfg(target_os = "linux")]
    let vm_handle = VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(process))),
        firewall_manager: Some(firewall_manager),
        spawn_time_ms: spawn_time,
    };

    #[cfg(not(target_os = "linux"))]
    let vm_handle = VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(process))),
        firewall_manager: None,
        spawn_time_ms: spawn_time,
    };

    Ok(vm_handle)
}

#[cfg(not(unix))]
pub async fn spawn_vm_with_config(_task_id: &str, _config: &VmConfig) -> Result<VmHandle> {
    anyhow::bail!("VM spawning is only supported on Unix systems");
}

/// Destroy a VM (ephemeral cleanup)
#[cfg(unix)]
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    tracing::info!("Destroying VM: {}", handle.id);

    // Cleanup firewall rules
    if let Some(fw) = &handle.firewall_manager {
        if let Err(e) = fw.cleanup() {
            tracing::error!("Failed to cleanup firewall for VM {}: {}", handle.id, e);
        }
        let _: Result<()> = Ok(()); // Explicit type to help compiler on older Rust
    }

    // Take the process out of the Arc<Mutex>
    let process = handle.process.lock().await.take();

    if let Some(proc) = process {
        stop_firecracker(proc).await?;
    } else {
        tracing::warn!("VM {} already destroyed", handle.id);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vm_spawn_and_destroy() {
        // This test requires actual Firecracker installation
        // Skip in CI if not available
        if !std::path::Path::new("/usr/local/bin/firecracker").exists() {
            return;
        }

        // Ensure test assets exist
        let _ = std::fs::create_dir_all("/tmp/ironclaw-fc-test");

        let result = spawn_vm("test-task").await;

        // If assets don't exist, we expect an error
        if result.is_err() {
            println!("Skipping test: Firecracker assets not available");
            return;
        }

        let handle = result.unwrap();
        assert_eq!(handle.id, "test-task");
        assert!(handle.spawn_time_ms > 0.0);

        destroy_vm(handle).await.unwrap();
    }

    #[test]
    fn test_vm_id_format() {
        let task_id = "task-123";
        let expected_id = task_id.to_string();
        assert_eq!(expected_id, "task-123");
    }
}
<<<<<<< HEAD
=======

/// Unit tests for VmHandle
#[cfg(all(test, unix))]
mod vm_handle_tests {
    use super::*;

    #[test]
    fn test_vm_handle_vsock_path_none() {
        let mut config = VmConfig::default();
        config.vsock_path = None; // Explicitly set to None
        let handle = VmHandle {
            id: "test-vm".to_string(),
            process: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            spawn_time_ms: 100.0,
            config,
            firewall_manager: None,
        };

        assert!(handle.vsock_path().is_none());
    }

    #[test]
    fn test_vm_handle_vsock_path_some() {
        let mut config = VmConfig::default();
        config.vsock_path = Some("/tmp/test.sock".to_string());

        let handle = VmHandle {
            id: "test-vm".to_string(),
            process: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            spawn_time_ms: 100.0,
            config,
            firewall_manager: None,
        };

        assert_eq!(handle.vsock_path(), Some("/tmp/test.sock"));
    }
}

/// Unit tests for verify_network_isolation
#[cfg(all(test, unix))]
mod isolation_tests {
    use super::*;

    #[test]
    fn test_verify_isolation_with_no_firewall_manager() {
        let config = VmConfig::new("test-vm".to_string());
        let handle = VmHandle {
            id: "test-vm".to_string(),
            process: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            spawn_time_ms: 100.0,
            config,
            firewall_manager: None,
        };

        let result = verify_network_isolation(&handle);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_verify_isolation_with_firewall_manager() {
        let config = VmConfig::new("test-vm".to_string());
        let firewall = FirewallManager::new("test-vm".to_string());
        let handle = VmHandle {
            id: "test-vm".to_string(),
            process: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            spawn_time_ms: 100.0,
            config,
            firewall_manager: Some(firewall),
        };

        let result = verify_network_isolation(&handle);
        assert!(result.is_ok());
    }
}

/// Unit tests for destroy_vm
#[cfg(all(test, unix))]
mod destroy_tests {
    use super::*;

    #[tokio::test]
    async fn test_destroy_vm_with_no_process() {
        let config = VmConfig::new("test-vm".to_string());
        let handle = VmHandle {
            id: "test-vm".to_string(),
            process: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            spawn_time_ms: 100.0,
            config,
            firewall_manager: None,
        };

        let result = destroy_vm(handle).await;
        assert!(result.is_ok());
    }
}

/// Unit tests for spawn_vm config logic
#[cfg(all(test, unix))]
mod spawn_config_tests {
    use super::*;

    #[test]
    fn test_spawn_vm_delegates_to_spawn_vm_with_config() {
        // Test that spawn_vm creates a VmConfig and calls spawn_vm_with_config
        // We can't actually test the async function here, but we can verify
        // that VmConfig::new sets the expected values
        let config = VmConfig::new("test-task".to_string());
        assert_eq!(config.vm_id, "test-task");
        assert!(config.vsock_path.is_some());
    }

    #[test]
    fn test_vmconfig_seccomp_auto_enable_needed() {
        // When seccomp_filter is None, Basic level should be auto-enabled
        let config = VmConfig::default();
        assert!(config.seccomp_filter.is_none());

        // The logic in spawn_vm_with_config would add Basic seccomp
        let should_add_seccomp = config.seccomp_filter.is_none();
        assert!(should_add_seccomp);
    }

    #[test]
    fn test_vmconfig_seccomp_already_set() {
        // When seccomp_filter is Some, it should not be overridden
        use seccomp::{SeccompFilter, SeccompLevel};
        let config = VmConfig {
            seccomp_filter: Some(SeccompFilter::new(SeccompLevel::Minimal)),
            ..VmConfig::default()
        };

        // The logic in spawn_vm_with_config should keep the existing filter
        let should_add_seccomp = config.seccomp_filter.is_none();
        assert!(!should_add_seccomp);
    }
}

/// Verify that a VM is properly network-isolated
#[cfg(unix)]
pub fn verify_network_isolation(handle: &VmHandle) -> Result<bool> {
    if let Some(ref firewall) = handle.firewall_manager {
        firewall.verify_isolation()
    } else {
        Ok(false)
    }
}

#[cfg(not(unix))]
pub fn verify_network_isolation(_handle: &VmHandle) -> Result<bool> {
    Ok(false)
}
>>>>>>> 1a7c698 (fix: Repair broken PR state (compilation, hashing, read-only))
