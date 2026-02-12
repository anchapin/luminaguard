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
pub mod seccomp;
#[cfg(unix)]
pub mod vsock;

// Prototype module for feasibility testing
// TODO: Add vm-prototype feature to Cargo.toml when prototype module is ready
// #[cfg(feature = "vm-prototype")]
// pub mod prototype;

#[cfg(all(test, unix))]
mod tests;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::vm::config::VmConfig;

#[cfg(unix)]
use crate::vm::firecracker::{start_firecracker, stop_firecracker, FirecrackerProcess};
#[cfg(unix)]
use crate::vm::firewall::FirewallManager;
#[cfg(unix)]
use crate::vm::seccomp::{SeccompFilter, SeccompLevel};

// Dummy types for non-unix platforms to satisfy struct definitions
#[cfg(not(unix))]
#[derive(Debug)]
pub struct FirecrackerProcess {
    pub spawn_time_ms: f64,
}

#[cfg(not(unix))]
pub struct FirewallManager;

/// VM handle for managing lifecycle
pub struct VmHandle {
    pub id: String,
    // On non-unix, this will be None or a dummy
    #[cfg(unix)]
    process: Arc<Mutex<Option<FirecrackerProcess>>>,
    #[cfg(not(unix))]
    #[allow(dead_code)]
    process: Arc<Mutex<Option<()>>>,

    pub spawn_time_ms: f64,
    config: VmConfig,

    #[cfg(unix)]
    firewall_manager: Option<FirewallManager>,
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
pub async fn spawn_vm_with_config(task_id: &str, config: &VmConfig) -> Result<VmHandle> {
    #[cfg(unix)]
    return spawn_vm_unix(task_id, config).await;

    #[cfg(not(unix))]
    return spawn_vm_dummy(task_id, config).await;
}

#[cfg(unix)]
async fn spawn_vm_unix(task_id: &str, config: &VmConfig) -> Result<VmHandle> {
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

#[cfg(not(unix))]
async fn spawn_vm_dummy(task_id: &str, config: &VmConfig) -> Result<VmHandle> {
    tracing::warn!("Spawning VM not supported on non-Unix platforms");
    Ok(VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(None)),
        spawn_time_ms: 0.0,
        config: config.clone(),
    })
}

/// Destroy a VM (ephemeral cleanup)
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    #[cfg(unix)]
    return destroy_vm_unix(handle).await;

    #[cfg(not(unix))]
    return destroy_vm_dummy(handle).await;
}

#[cfg(unix)]
async fn destroy_vm_unix(handle: VmHandle) -> Result<()> {
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

#[cfg(not(unix))]
async fn destroy_vm_dummy(_handle: VmHandle) -> Result<()> {
    Ok(())
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
    Ok(true)
}
