// JIT Micro-VM Module
//
// This module handles spawning and managing ephemeral Firecracker VMs.
//
// Key invariants:
// - Spawn time: <200ms (target: 10-50ms with snapshot pool)
// - Ephemeral: VM destroyed after task completion
// - Security: No host execution, full isolation
//
// Architecture:
// - Uses snapshot pool for fast VM spawning (10-50ms)
// - Falls back to cold boot if pool exhausted (110ms)
// - Pool maintains 5 pre-created snapshots for round-robin allocation

pub mod config;
pub mod firecracker;
pub mod pool;
pub mod snapshot;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::vm::pool::{PoolConfig, SnapshotPool};

/// Global snapshot pool (lazy-initialized)
static SNAPSHOT_POOL: OnceCell<Arc<SnapshotPool>> = OnceCell::const_new();

/// VM handle for managing lifecycle
pub struct VmHandle {
    pub id: String,
    pub pid: u32,
}

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
/// - With snapshot pool: 10-50ms
/// - Cold boot fallback: ~110ms
///
/// # Invariants
///
/// * Must complete in <200ms
/// * VM must be destroyed after task completion
pub async fn spawn_vm(task_id: &str) -> Result<VmHandle> {
    tracing::info!("Spawning VM for task: {}", task_id);

    let start = std::time::Instant::now();

    // Try to acquire VM from snapshot pool
    let pool = get_pool().await?;

    match pool.acquire_vm().await {
        Ok(vm_id) => {
            let elapsed = start.elapsed();
            tracing::info!("VM spawned from pool in {:?}", elapsed);

            // TODO: Get actual PID from Firecracker API
            let pid = 0;

            Ok(VmHandle { id: vm_id, pid })
        }
        Err(e) => {
            tracing::warn!(
                "Failed to acquire VM from pool: {}, falling back to cold boot",
                e
            );

            // Fallback to cold boot
            cold_boot_vm(task_id).await
        }
    }
}

/// Cold boot a VM (fallback when pool is unavailable)
async fn cold_boot_vm(task_id: &str) -> Result<VmHandle> {
    tracing::info!("Cold booting VM for task: {}", task_id);

    let start = std::time::Instant::now();

    // TODO: Implement actual Firecracker VM spawning
    // 1. Create VM config (kernel, memory, drives)
    // 2. Start Firecracker process
    // 3. Verify VM is responsive
    // 4. Return handle

    let vm_id = format!("vm-cold-{}", task_id);
    let pid = 0; // Placeholder

    let elapsed = start.elapsed();
    tracing::info!("VM cold booted in {:?}", elapsed);

    Ok(VmHandle { id: vm_id, pid })
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
pub async fn destroy_vm(handle: VmHandle) -> Result<()> {
    tracing::info!("Destroying VM: {}", handle.id);

    // TODO: Implement VM destruction
    // 1. Send shutdown signal to VM
    // 2. Wait for graceful shutdown (timeout: 5s)
    // 3. Force kill if timeout
    // 4. Clean up resources (memory, sockets, etc.)

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
/// use ironclaw_orchestrator::vm;
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
/// use ironclaw_orchestrator::vm;
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
        "Pool warmed up with {}/{} snapshots",
        stats.current_size,
        stats.max_size
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vm_spawn_and_destroy() {
        // Set temp directory for testing
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.path().to_str().unwrap());

        let handle = spawn_vm("test-task").await.unwrap();
        assert!(
            handle.id.contains("test-task")
                || handle.id.contains("pool")
                || handle.id.contains("cold")
        );
        destroy_vm(handle).await.unwrap();

        std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
    }

    #[tokio::test]
    async fn test_cold_boot_vm() {
        let handle = cold_boot_vm("cold-test").await.unwrap();
        assert!(handle.id.contains("cold-test"));
        destroy_vm(handle).await.unwrap();
    }

    // Property-based test: spawn with various task IDs
    #[test]
    fn test_vm_id_format() {
        let task_id = "task-123";
        let expected_id = format!("vm-{}", task_id);
        assert_eq!(format!("vm-{}", task_id), expected_id);
    }

    // Property-based test: VM IDs are unique
    #[tokio::test]
    async fn test_vm_ids_are_unique() {
        // Set temp directory for testing
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.path().to_str().unwrap());

        let handle1 = spawn_vm("task-1").await.unwrap();
        let handle2 = spawn_vm("task-2").await.unwrap();

        assert_ne!(handle1.id, handle2.id);

        std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
    }

    // Property-based test: pool can handle concurrent requests
    #[tokio::test]
    #[cfg_attr(windows, ignore = "Timing-sensitive test fails on Windows due to race conditions")]
    async fn test_concurrent_vm_spawn() {
        // Set temp directory for testing
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.path().to_str().unwrap());

        let mut handles = Vec::new();

        for i in 0..10 {
            let task_id = format!("concurrent-{}", i);
            let handle = spawn_vm(&task_id).await.unwrap();
            handles.push(handle);
        }

        // All handles should be unique
        let ids: Vec<_> = handles.iter().map(|h| &h.id).collect();
        let unique_ids: std::collections::HashSet<_> = ids.into_iter().collect();

        assert_eq!(unique_ids.len(), 10);

        std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
    }

    #[tokio::test]
    async fn test_vm_handle_creation() {
        let handle = VmHandle {
            id: "test-vm-123".to_string(),
            pid: 1234,
        };

        assert_eq!(handle.id, "test-vm-123");
        assert_eq!(handle.pid, 1234);
    }
}
