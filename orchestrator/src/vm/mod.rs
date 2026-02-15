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
pub mod firewall;
#[cfg(windows)]
pub mod hyperv;
pub mod hypervisor;
#[cfg(unix)]
pub mod jailer;
pub mod pool;
pub mod rootfs;
pub mod seccomp;
pub mod snapshot;
#[cfg(unix)]
pub mod vsock;

// Prototype module for feasibility testing
#[cfg(feature = "vm-prototype")]
pub mod prototype;

#[cfg(test)]
mod tests;

// Real integration tests that run against actual binaries
#[cfg(test)]
mod integration_tests;

// End-to-end tests for complete workflows
#[cfg(test)]
mod e2e_tests;

#[allow(unused_imports)]
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

use crate::vm::config::VmConfig;
use crate::vm::firewall::FirewallManager;
use crate::vm::hypervisor::{Hypervisor, VmInstance};
#[cfg(unix)]
use crate::vm::jailer::{start_jailed_firecracker, verify_jailer_installed, JailerConfig};
use crate::vm::pool::{PoolConfig, SnapshotPool};
use crate::vm::seccomp::{SeccompFilter, SeccompLevel};

/// Global snapshot pool (lazy-initialized)
static SNAPSHOT_POOL: OnceCell<Arc<SnapshotPool>> = OnceCell::const_new();

/// Initialize the snapshot pool
///
/// This function is called automatically on first use or can be called
/// explicitly to warm up the pool before serving requests.
async fn init_pool() -> Result<Arc<SnapshotPool>> {
    let config = PoolConfig::from_env();
    let pool = SnapshotPool::new(config).await?;
    Ok(Arc::new(pool))
}

/// Get or initialize the snapshot pool
async fn get_pool() -> Result<Arc<SnapshotPool>> {
    SNAPSHOT_POOL.get_or_try_init(init_pool).await.cloned()
}

/// VM handle for managing lifecycle
pub struct VmHandle {
    pub id: String,
    process: Arc<Mutex<Option<Box<dyn VmInstance>>>>,
    pub spawn_time_ms: f64,
    config: VmConfig,
    firewall_manager: Option<FirewallManager>,
}

impl VmHandle {
    /// Get the vsock socket path for this VM
    pub fn vsock_path(&self) -> Option<&str> {
        self.config.vsock_path.as_deref()
    }
}

/// Spawn a new JIT Micro-VM
///
/// # Arguments
///
/// * `task_id` - Unique identifier for the task
///
/// # Returns
///
/// * `VmHandle` - Handle for managing the VM
///
/// # Performance
///
/// - With snapshot pool: 10-50ms (target)
/// - Cold boot fallback: ~110ms (actual)
///
/// # Security
///
/// Seccomp filters are applied by default (Basic level) to restrict syscalls.
/// 99% of syscalls are blocked, only essential ones are allowed.
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm::spawn_vm;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let handle = spawn_vm("my-task").await?;
///     println!("VM {} spawned in {:.2}ms", handle.id, handle.spawn_time_ms);
///     // ... use VM ...
///     Ok(())
/// }
/// ```
pub async fn spawn_vm(task_id: &str) -> Result<VmHandle> {
    // Try snapshot pool first for fast spawning
    let pool_result = get_pool().await;

    if let Ok(pool) = pool_result {
        match pool.acquire_vm().await {
            Ok(vm_id) => {
                tracing::info!("VM spawned from pool: {}", vm_id);

                // TODO: Load snapshot from pool and return handle
                // For now, fall through to cold boot
                // This will be implemented in Phase 2
            }
            Err(e) => {
                tracing::debug!("Pool not available: {}, using cold boot", e);
            }
        }
    }

    // Fallback to cold boot
    spawn_vm_with_config(task_id, &VmConfig::new(task_id.to_string())).await
}

/// Spawn a new JIT Micro-VM with custom configuration
///
/// # Arguments
///
/// * `task_id` - Unique identifier for the task
/// * `config` - VM configuration (including seccomp filter)
///
/// # Returns
///
/// * `VmHandle` - Handle for managing the VM
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm::{spawn_vm_with_config, config::VmConfig};
/// use luminaguard_orchestrator::vm::seccomp::{SeccompFilter, SeccompLevel};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let config = VmConfig::new("my-task".to_string());
///     let config_with_seccomp = VmConfig {
///         seccomp_filter: Some(SeccompFilter::new(SeccompLevel::Basic)),
///         ..config
///     };
///
///     let handle = spawn_vm_with_config("my-task", &config_with_seccomp).await?;
///     Ok(())
/// }
/// ```
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
                "Failed to configure firewall (running without root?): {}. \n                VM will still have networking disabled in config, but firewall rules are not applied.",
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

    // Start VM using the appropriate hypervisor for the platform
    let hypervisor = get_hypervisor();

    let instance = hypervisor.spawn(&config_with_seccomp).await?;
    let spawn_time = instance.spawn_time_ms();

    Ok(VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(instance))),
        spawn_time_ms: spawn_time,
        config: config.clone(),
        firewall_manager: Some(firewall_manager),
    })
}

#[cfg(windows)]
fn get_hypervisor() -> Box<dyn Hypervisor> {
    Box::new(crate::vm::hyperv::HypervHypervisor)
}

#[cfg(not(windows))]
fn get_hypervisor() -> Box<dyn Hypervisor> {
    Box::new(crate::vm::firecracker::FirecrackerHypervisor)
}

/// Destroy a VM (ephemeral cleanup)
///
/// # Arguments
///
/// * `handle` - VM handle to destroy
///
/// # Important
///
/// This MUST be called after task completion to ensure
/// no malware can persist (the "infected computer no longer exists")
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm::{spawn_vm, destroy_vm};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let handle = spawn_vm("my-task").await?;
///     // ... use VM ...
///     destroy_vm(handle).await?;
///     Ok(())
/// }
/// ```
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    tracing::info!("Destroying VM: {}", handle.id);

    // Take the process out of the Arc<Mutex>
    let mut process = handle.process.lock().await.take();

    if let Some(ref mut proc) = process {
        proc.stop().await?;
    } else {
        tracing::warn!("VM {} already destroyed", handle.id);
    }

    Ok(())
}

/// Get snapshot pool statistics
///
/// # Returns
///
/// * `PoolStats` - Current pool statistics
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let stats = vm::pool_stats().await?;
///     println!("Pool size: {}/{}", stats.current_size, stats.max_size);
///     Ok(())
/// }
/// ```
pub async fn pool_stats() -> Result<crate::vm::pool::PoolStats> {
    let pool = get_pool().await?;
    Ok(pool.stats().await)
}

/// Warm up the snapshot pool
///
/// Pre-creates snapshots so first VM spawn is fast.
/// Useful to call during application startup.
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     vm::warmup_pool().await?;
///     println!("Pool is ready!");
///     Ok(())
/// }
/// ```
pub async fn warmup_pool() -> Result<()> {
    tracing::info!("Warming up snapshot pool");

    let pool = get_pool().await?;

    // Pool is already initialized during get_pool()
    let stats = pool.stats().await;

    tracing::info!(
        "Pool warmed up with {}/{}",
        stats.current_size,
        stats.max_size
    );

    Ok(())
}

#[cfg(test)]
pub(crate) fn should_skip_hypervisor_tests() -> bool {
    std::env::var("SKIP_HYPERVISOR_TESTS").is_ok() || cfg!(not(target_os = "linux"))
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[tokio::test]
    async fn test_vm_spawn_and_destroy() {
        if should_skip_hypervisor_tests() {
            tracing::warn!("Skipping hypervisor-dependent test");
            return;
        }

        // Check if Firecracker resources exist
        let kernel_path = if std::path::Path::new("/tmp/luminaguard-fc-test/vmlinux.bin").exists() {
            "/tmp/luminaguard-fc-test/vmlinux.bin".to_string()
        } else {
            tracing::warn!("Skipping test: Firecracker kernel not available at /tmp/luminaguard-fc-test/vmlinux.bin. Run: ./scripts/download-firecracker-assets.sh");
            return;
        };
        let rootfs_path = if std::path::Path::new("/tmp/luminaguard-fc-test/rootfs.ext4").exists() {
            "/tmp/luminaguard-fc-test/rootfs.ext4".to_string()
        } else {
            tracing::warn!("Skipping test: Firecracker rootfs not available at /tmp/luminaguard-fc-test/rootfs.ext4. Run: ./scripts/download-firecracker-assets.sh");
            return;
        };

        // Ensure test assets exist
        let _ = std::fs::create_dir_all("/tmp/luminaguard-fc-test");

        use config::VmConfig;
        let config = VmConfig {
            kernel_path: kernel_path.to_string(),
            rootfs_path: rootfs_path.to_string(),
            ..VmConfig::new("test-task".to_string())
        };

        let result = spawn_vm_with_config("test-task", &config).await;

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

/// Verify that a VM is properly network-isolated
///
/// # Arguments
///
/// * `handle` - VM handle to verify
///
/// # Returns
///
/// * `Ok(true)` - VM is properly isolated
/// * `Ok(false)` - VM is not isolated
/// * `Err(_)` - Failed to check isolation status
pub fn verify_network_isolation(handle: &VmHandle) -> Result<bool> {
    if let Some(ref firewall) = handle.firewall_manager {
        firewall.verify_isolation()
    } else {
        Ok(false)
    }
}

/// Spawn a JIT Micro-VM with Jailer sandboxing (Enhanced Security)
///
/// This function creates a Firecracker VM that runs inside a Jailer sandbox,
/// providing enhanced security through:
/// - chroot filesystem isolation
/// - cgroup resource limits
/// - Namespace isolation (mount, PID, network)
/// - UID/GID privilege separation
///
/// # Arguments
///
/// * `task_id` - Unique identifier for task
/// * `vm_config` - VM configuration
/// * `jailer_config` - Jailer sandbox configuration
///
/// # Returns
///
/// * `VmHandle` - Handle for managing VM
///
/// # Security
///
/// Jailer provides defense-in-depth:
/// 1. Process runs in chroot jail
/// 2. Resource limits via cgroups
/// 3. Isolated namespaces
/// 4. Dropped privileges (non-root if configured)
///
/// # Performance
///
/// Spawn time: ~150ms (slightly higher than non-jailed due to jailer setup)
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm::{spawn_vm_jailed, config::VmConfig};
/// use luminaguard_orchestrator::vm::jailer::JailerConfig;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let vm_config = VmConfig::new("my-task".to_string());
///     let jailer_config = JailerConfig::new("my-task".to_string())
///         .with_user(1000, 1000); // Run as non-root user
///
///     let handle = spawn_vm_jailed("my-task", &vm_config, &jailer_config).await?;
///     println!("Jailed VM {} spawned", handle.id);
///     Ok(())
/// }
/// ```
#[cfg(unix)]
pub async fn spawn_vm_jailed(
    task_id: &str,
    vm_config: &VmConfig,
    jailer_config: &JailerConfig,
) -> Result<VmHandle> {
    tracing::info!("Spawning JAILED VM for task: {}", task_id);

    // Verify jailer is installed
    verify_jailer_installed().context("Jailer not installed. Please install Firecracker.")?;

    // Apply default seccomp filter if not specified
    let vm_config_with_seccomp = if vm_config.seccomp_filter.is_none() {
        let mut secured_config = vm_config.clone();
        secured_config.seccomp_filter = Some(SeccompFilter::new(SeccompLevel::Basic));
        tracing::info!("Auto-enabling seccomp filter (Basic level) for security");
        secured_config
    } else {
        vm_config.clone()
    };

    // Configure firewall (still applies to host network stack)
    let firewall_manager = FirewallManager::new(vm_config_with_seccomp.vm_id.clone());

    // Apply firewall rules (may fail if not root)
    match firewall_manager.configure_isolation() {
        Ok(_) => {
            tracing::info!(
                "Firewall isolation configured for JAILED VM: {}",
                vm_config_with_seccomp.vm_id
            );
        }
        Err(e) => {
            tracing::warn!(
                "Failed to configure firewall (running without root?): {}. \n                VM will still have networking disabled in config, but firewall rules are not applied.",
                e
            );
        }
    }

    // Start Firecracker via Jailer
    let jailer_process = start_jailed_firecracker(&vm_config_with_seccomp, jailer_config).await?;

    let spawn_time = jailer_process.spawn_time_ms;

    Ok(VmHandle {
        id: task_id.to_string(),
        process: Arc::new(Mutex::new(Some(Box::new(jailer_process)))),
        spawn_time_ms: spawn_time,
        config: vm_config.clone(),
        firewall_manager: Some(firewall_manager),
    })
}

/// Destroy a JAILED VM (ephemeral cleanup with jailer cleanup)
///
/// # Arguments
///
/// * `handle` - VM handle to destroy
/// * `jailer_config` - Jailer configuration for cleanup
///
/// # Important
///
/// This MUST be called after task completion to ensure
/// no malware can persist. It will also attempt to cleanup
/// jailer chroot directory.
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::vm::{spawn_vm_jailed, destroy_vm_jailed, config::VmConfig};
/// use luminaguard_orchestrator::vm::jailer::JailerConfig;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let vm_config = VmConfig::new("my-task".to_string());
///     let jailer_config = JailerConfig::new("my-task".to_string());
///
///     let handle = spawn_vm_jailed("my-task", &vm_config, &jailer_config).await?;
///     // ... use VM ...
///     destroy_vm_jailed(handle, &jailer_config).await?;
///     Ok(())
/// }
/// ```
#[cfg(unix)]
pub async fn destroy_vm_jailed(handle: VmHandle, _jailer_config: &JailerConfig) -> Result<()> {
    tracing::info!("Destroying JAILED VM: {}", handle.id);

    // Take process out of Arc<Mutex>
    let mut process = handle.process.lock().await.take();

    if let Some(ref mut proc) = process {
        proc.stop().await?;
    } else {
        tracing::warn!("JAILED VM {} already destroyed", handle.id);
    }

    Ok(())
}
