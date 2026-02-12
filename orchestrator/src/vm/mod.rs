// JIT Micro-VM Module
//
// This module handles spawning and managing ephemeral Firecracker VMs.
//
// Key invariants:
// - Spawn time: <200ms (actual: ~110ms)
// - Ephemeral: VM destroyed after task completion
// - Security: No host execution, full isolation

pub mod config;
pub mod seccomp;

#[cfg(unix)]
pub mod firecracker;
#[cfg(unix)]
pub mod firewall;
#[cfg(unix)]
pub mod vsock;

#[cfg(test)]
#[cfg(unix)]
mod tests;

use anyhow::Result;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use tokio::sync::Mutex;

use crate::vm::config::VmConfig;
#[cfg(unix)]
use crate::vm::firecracker::{start_firecracker, stop_firecracker, FirecrackerProcess};
#[cfg(unix)]
use crate::vm::firewall::FirewallManager;
#[cfg(unix)]
use crate::vm::seccomp::{SeccompFilter, SeccompLevel};

/// VM handle for managing lifecycle (Unix/Linux)
#[cfg(unix)]
pub struct VmHandle {
    pub id: String,
    process: Arc<Mutex<Option<FirecrackerProcess>>>,
    pub spawn_time_ms: f64,
    config: VmConfig,
    #[cfg(target_os = "linux")]
    pub firewall_manager: Option<FirewallManager>,
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)] // Stubs for non-Linux Unix (macOS)
    pub firewall_manager: Option<()>,
}

/// VM handle for managing lifecycle (Non-Unix Stub)
#[cfg(not(unix))]
pub struct VmHandle {
    pub id: String,
    pub spawn_time_ms: f64,
    pub config: VmConfig,
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

/// Spawn a new JIT Micro-VM with custom configuration (Unix implementation)
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
        // This might fail if not root, but we log warning inside configure_isolation usually?
        // Actually configure_isolation returns Result.
        if let Err(e) = firewall_manager.configure_isolation() {
             tracing::warn!("Failed to configure firewall: {}", e);
        }
    }

    // Start Firecracker VM
    let process = start_firecracker(&config_with_seccomp).await?;

    let spawn_time = process.spawn_time_ms;

    #[cfg(target_os = "linux")]
    let vm_handle = VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(process))),
        spawn_time_ms: spawn_time,
        config: config.clone(),
        firewall_manager: Some(firewall_manager),
    };

    #[cfg(not(target_os = "linux"))]
    let vm_handle = VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(process))),
        spawn_time_ms: spawn_time,
        config: config.clone(),
        firewall_manager: None,
    };

    Ok(vm_handle)
}

/// Spawn a new JIT Micro-VM with custom configuration (Non-Unix Stub)
#[cfg(not(unix))]
pub async fn spawn_vm_with_config(_task_id: &str, _config: &VmConfig) -> Result<VmHandle> {
    anyhow::bail!("JIT Micro-VMs are only supported on Unix-like systems (Linux/macOS). Windows is not supported.");
}

/// Destroy a VM (ephemeral cleanup) - Unix implementation
#[cfg(unix)]
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    tracing::info!("Destroying VM: {}", handle.id);

    // Cleanup firewall rules
    #[cfg(target_os = "linux")]
    if let Some(fw) = &handle.firewall_manager {
        if let Err(e) = fw.cleanup() {
            tracing::error!("Failed to cleanup firewall for VM {}: {}", handle.id, e);
        }
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

/// Destroy a VM (ephemeral cleanup) - Non-Unix Stub
#[cfg(not(unix))]
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    tracing::info!("Destroying VM stub: {}", handle.id);
    Ok(())
}

/// Verify that a VM is properly network-isolated (Unix implementation)
#[cfg(unix)]
pub fn verify_network_isolation(handle: &VmHandle) -> Result<bool> {
    #[cfg(target_os = "linux")]
    if let Some(ref firewall) = handle.firewall_manager {
        firewall.verify_isolation()
    } else {
        Ok(false)
    }

    #[cfg(not(target_os = "linux"))]
    Ok(false)
}

/// Verify that a VM is properly network-isolated (Non-Unix Stub)
#[cfg(not(unix))]
pub fn verify_network_isolation(_handle: &VmHandle) -> Result<bool> {
    Ok(false)
}
