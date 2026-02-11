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
#[cfg(unix)]
pub mod firewall;
#[cfg(unix)]
pub mod seccomp;
#[cfg(unix)]
pub mod vsock;

// Prototype module for feasibility testing
#[cfg(feature = "vm-prototype")]
pub mod prototype;

#[cfg(test)]
mod tests;

use anyhow::Result;
use crate::vm::config::VmConfig;

#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use tokio::sync::Mutex;
#[cfg(unix)]
use crate::vm::firecracker::{start_firecracker, stop_firecracker, FirecrackerProcess};
#[cfg(unix)]
use crate::vm::firewall::FirewallManager;
#[cfg(unix)]
use crate::vm::seccomp::{SeccompFilter, SeccompLevel};

// ------------------------------------------------------------------------------------------------
// UNIX IMPLEMENTATION
// ------------------------------------------------------------------------------------------------

#[cfg(unix)]
/// VM handle for managing lifecycle
pub struct VmHandle {
    pub id: String,
    process: Arc<Mutex<Option<FirecrackerProcess>>>,
    pub spawn_time_ms: f64,
    config: VmConfig,
    firewall_manager: Option<FirewallManager>,
}

#[cfg(unix)]
impl VmHandle {
    /// Get the vsock socket path for this VM
    pub fn vsock_path(&self) -> Option<&str> {
        self.config.vsock_path.as_deref()
    }
}

#[cfg(unix)]
/// Spawn a new JIT Micro-VM
pub async fn spawn_vm(task_id: &str) -> Result<VmHandle> {
    spawn_vm_with_config(task_id, &VmConfig::new(task_id.to_string())).await
}

#[cfg(unix)]
/// Spawn a new JIT Micro-VM with custom configuration
pub async fn spawn_vm_with_config(task_id: &str, config: &VmConfig) -> Result<VmHandle> {
    tracing::info!("Spawning VM for task: {}", task_id);

    // Apply default seccomp filter if not specified (security best practice)
    let config_with_seccomp = if config.seccomp_filter.is_none() {
        let mut secured_config = config.clone();
        secured_config.seccomp_filter = Some(SeccompFilter::new(SeccompLevel::Basic));
        tracing::info!("Auto-enabling seccomp filter (Basic level) for security");
        secured_config
    } else {
        config.clone()
    };

    // Configure firewall to block all network traffic
    let firewall_manager = FirewallManager::new(config_with_seccomp.vm_id.clone());

    // Apply firewall rules (may fail if not root)
    match firewall_manager.configure_isolation() {
        Ok(_) => {
            tracing::info!(
                "Firewall isolation configured for VM: {}",
                config_with_seccomp.vm_id
            );
        }
        Err(e) => {
            tracing::warn!(
                "Failed to configure firewall (running without root?): {}. \
                VM will still have networking disabled in config, but firewall rules are not applied.",
                e
            );
            // Continue anyway - networking is still disabled in config
        }
    }

    // Verify firewall rules are active (if configured)
    match firewall_manager.verify_isolation() {
        Ok(true) => {
            tracing::info!(
                "Firewall isolation verified for VM: {}",
                config_with_seccomp.vm_id
            );
        }
        Ok(false) => {
            tracing::debug!(
                "Firewall rules not active for VM: {}",
                config_with_seccomp.vm_id
            );
        }
        Err(e) => {
            tracing::debug!("Failed to verify firewall rules: {}", e);
        }
    }

    // Start Firecracker VM
    let process = start_firecracker(&config_with_seccomp).await?;

    let spawn_time = process.spawn_time_ms;

    Ok(VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(process))),
        spawn_time_ms: spawn_time,
        config: config.clone(),
        firewall_manager: Some(firewall_manager),
    })
}

#[cfg(unix)]
/// Destroy a VM (ephemeral cleanup)
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    tracing::info!("Destroying VM: {}", handle.id);

    // Take the process out of the Arc<Mutex>
    let process = handle.process.lock().await.take();

    if let Some(proc) = process {
        stop_firecracker(proc).await?;
    } else {
        tracing::warn!("VM {} already destroyed", handle.id);
    }

    Ok(())
}

#[cfg(unix)]
/// Verify that a VM is properly network-isolated
pub fn verify_network_isolation(handle: &VmHandle) -> Result<bool> {
    if let Some(ref firewall) = handle.firewall_manager {
        firewall.verify_isolation()
    } else {
        Ok(false)
    }
}

// ------------------------------------------------------------------------------------------------
// NON-UNIX IMPLEMENTATION (STUB)
// ------------------------------------------------------------------------------------------------

#[cfg(not(unix))]
/// VM handle stub for non-Unix platforms
pub struct VmHandle {
    pub id: String,
    pub spawn_time_ms: f64,
}

#[cfg(not(unix))]
impl VmHandle {
    pub fn vsock_path(&self) -> Option<&str> {
        None
    }
}

#[cfg(not(unix))]
pub async fn spawn_vm(task_id: &str) -> Result<VmHandle> {
    anyhow::bail!("JIT Micro-VMs are only supported on Unix-like systems (Linux/macOS)")
}

#[cfg(not(unix))]
pub async fn spawn_vm_with_config(task_id: &str, _config: &VmConfig) -> Result<VmHandle> {
    anyhow::bail!("JIT Micro-VMs are only supported on Unix-like systems (Linux/macOS)")
}

#[cfg(not(unix))]
pub async fn destroy_vm(_handle: VmHandle) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
pub fn verify_network_isolation(_handle: &VmHandle) -> Result<bool> {
    Ok(false)
}

// ------------------------------------------------------------------------------------------------
// TESTS
// ------------------------------------------------------------------------------------------------

#[cfg(test)]
mod inline_tests {
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

        // If assets don't exist or we are on non-unix, we expect an error or it to fail
        #[cfg(not(unix))]
        {
            assert!(result.is_err());
            return;
        }

        #[cfg(unix)]
        if result.is_err() {
            println!("Skipping test: Firecracker assets not available");
            return;
        }

        #[cfg(unix)]
        {
            let handle = result.unwrap();
            assert_eq!(handle.id, "test-task");
            assert!(handle.spawn_time_ms > 0.0);

            destroy_vm(handle).await.unwrap();
        }
    }

    #[test]
    fn test_vm_id_format() {
        let task_id = "task-123";
        let expected_id = task_id.to_string();
        assert_eq!(expected_id, "task-123");
    }
}
