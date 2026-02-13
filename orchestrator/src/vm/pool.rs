// VM Snapshot Pool
//
// This module manages a pool of pre-created VM snapshots for fast VM spawning.
// The pool maintains a set of ready-to-use VM snapshots that can be loaded
// in 10-50ms instead of 110ms for cold boot.
//
// Architecture:
// - Pool size: 5 VMs (configurable via IRONCLAW_POOL_SIZE env var)
// - Refresh interval: 1 hour (configurable via IRONCLAW_SNAPSHOT_REFRESH_SECS)
// - Location: /var/lib/ironclaw/snapshots
// - Allocation: Round-robin with automatic refresh

use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;

use crate::vm::snapshot::{create_snapshot, load_snapshot, SnapshotMetadata};

/// Default pool size
const DEFAULT_POOL_SIZE: usize = 5;

/// Default snapshot refresh interval (1 hour)
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 3600;

/// Snapshot age threshold (refresh snapshots older than this)
const SNAPSHOT_MAX_AGE_SECS: u64 = 3600;

/// Snapshot pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Number of snapshots to maintain in pool
    pub pool_size: usize,

    /// Snapshot storage location
    pub snapshot_path: PathBuf,

    /// Snapshot refresh interval in seconds
    pub refresh_interval_secs: u64,

    /// Maximum snapshot age before refresh
    pub max_snapshot_age_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            pool_size: DEFAULT_POOL_SIZE,
            snapshot_path: PathBuf::from("/var/lib/ironclaw/snapshots"),
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
            max_snapshot_age_secs: SNAPSHOT_MAX_AGE_SECS,
        }
    }
}

impl PoolConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(size_str) = std::env::var("IRONCLAW_POOL_SIZE") {
            if let Ok(size) = size_str.parse::<usize>() {
                if size > 0 && size <= 20 {
                    config.pool_size = size;
                }
            }
        }

        if let Ok(refresh_str) = std::env::var("IRONCLAW_SNAPSHOT_REFRESH_SECS") {
            if let Ok(refresh) = refresh_str.parse::<u64>() {
                if refresh >= 60 {
                    config.refresh_interval_secs = refresh;
                }
            }
        }

        if let Ok(path_str) = std::env::var("IRONCLAW_SNAPSHOT_PATH") {
            config.snapshot_path = PathBuf::from(path_str);
        }

        config
    }
}

/// VM Snapshot Pool
///
/// Manages a pool of pre-created VM snapshots for fast spawning.
/// Thread-safe: Can be shared across async tasks.
#[derive(Debug, Clone)]
pub struct SnapshotPool {
    /// Pool configuration
    config: PoolConfig,

    /// Available snapshots (round-robin queue)
    snapshots: Arc<Mutex<VecDeque<SnapshotMetadata>>>,

    /// Last refresh timestamp
    last_refresh: Arc<Mutex<SystemTime>>,
}

impl SnapshotPool {
    /// Create a new snapshot pool
    ///
    /// # Arguments
    ///
    /// * `config` - Pool configuration
    pub async fn new(config: PoolConfig) -> Result<Self> {
        tracing::info!("Creating snapshot pool with size {}", config.pool_size);

        // Create snapshot directory if it doesn't exist
        tokio::fs::create_dir_all(&config.snapshot_path)
            .await
            .context("Failed to create snapshot directory")?;

        let pool = Self {
            config: config.clone(),
            snapshots: Arc::new(Mutex::new(VecDeque::with_capacity(config.pool_size))),
            last_refresh: Arc::new(Mutex::new(SystemTime::UNIX_EPOCH)),
        };

        // Initialize pool with snapshots
        pool.initialize().await?;

        Ok(pool)
    }

    /// Initialize pool with fresh snapshots
    async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing snapshot pool");

        let mut snapshots = self.snapshots.lock().await;

        // Try to load existing snapshots from disk
        let existing_snapshots = crate::vm::snapshot::list_snapshots().await?;

        for metadata in existing_snapshots {
            if snapshots.len() < self.config.pool_size {
                snapshots.push_back(metadata);
            }
        }

        // Fill remaining slots with new snapshots
        while snapshots.len() < self.config.pool_size {
            match self.create_snapshot().await {
                Ok(metadata) => {
                    snapshots.push_back(metadata);
                }
                Err(e) => {
                    tracing::warn!("Failed to create snapshot: {}", e);
                    break;
                }
            }
        }

        *self.last_refresh.lock().await = SystemTime::now();

        tracing::info!("Pool initialized with {} snapshots", snapshots.len());

        Ok(())
    }

    /// Create a new snapshot
    async fn create_snapshot(&self) -> Result<SnapshotMetadata> {
        let snapshot_id = format!("pool-snapshot-{}", uuid::Uuid::new_v4());

        // TODO: Phase 2 - Create actual VM from snapshot
        // For now, we create a snapshot without a real VM
        let snapshot = create_snapshot("base-vm", &snapshot_id).await?;

        Ok(snapshot.metadata)
    }

    /// Acquire a VM from the pool
    ///
    /// # Returns
    ///
    /// * `String` - ID of the acquired VM
    ///
    /// # Performance
    ///
    /// Target: 10-50ms (snapshot load time)
    pub async fn acquire_vm(&self) -> Result<String> {
        let start = std::time::Instant::now();

        let mut snapshots = self.snapshots.lock().await;

        if snapshots.is_empty() {
            drop(snapshots);
            tracing::warn!("Pool exhausted, creating new snapshot");

            // Pool exhausted, create new snapshot
            let metadata = self.create_snapshot().await?;
            let snapshot_id = metadata.id.clone();
            let vm_id = load_snapshot(&snapshot_id).await?;

            let elapsed = start.elapsed();
            tracing::debug!("VM acquired (cold start) in {:?}", elapsed);

            return Ok(vm_id);
        }

        // Round-robin: take from front
        let metadata = snapshots.pop_front().unwrap();
        let snapshot_id = metadata.id.clone();

        // Check if snapshot is stale
        let age_secs = metadata.created_at.elapsed().unwrap_or_default().as_secs();

        if age_secs > self.config.max_snapshot_age_secs {
            tracing::info!(
                "Snapshot {} is stale ({}s old), refreshing",
                snapshot_id,
                age_secs
            );

            // Delete stale snapshot
            drop(snapshots);
            crate::vm::snapshot::delete_snapshot(&snapshot_id).await?;

            // Create fresh snapshot
            let new_metadata = self.create_snapshot().await?;
            let vm_id = load_snapshot(&new_metadata.id).await?;

            // Add to pool
            let mut snapshots = self.snapshots.lock().await;
            snapshots.push_back(new_metadata);

            let elapsed = start.elapsed();
            tracing::debug!("VM acquired (stale refresh) in {:?}", elapsed);

            return Ok(vm_id);
        }

        // Re-add to back of queue (round-robin)
        snapshots.push_back(metadata.clone());
        drop(snapshots);

        // Load snapshot
        let vm_id = load_snapshot(&snapshot_id).await?;

        let elapsed = start.elapsed();
        tracing::debug!("VM acquired from pool in {:?}", elapsed);

        Ok(vm_id)
    }

    /// Release a VM back to the pool
    ///
    /// Note: VMs are ephemeral and are destroyed after use.
    /// This method is a no-op but kept for API compatibility.
    pub async fn release_vm(&self, _vm_id: &str) -> Result<()> {
        // VMs are ephemeral - they are destroyed after use
        // The pool maintains pre-created snapshots, not running VMs
        tracing::debug!("VM {} released (ephemeral, not returned to pool)", _vm_id);
        Ok(())
    }

    /// Refresh the pool with new snapshots
    ///
    /// Called periodically to keep snapshots fresh
    pub async fn refresh_pool(&self) -> Result<()> {
        tracing::info!("Refreshing snapshot pool");

        let snapshots = self.snapshots.lock().await;

        // Check if refresh is needed
        let last_refresh = *self.last_refresh.lock().await;
        let elapsed = last_refresh.elapsed().unwrap_or_default().as_secs();

        drop(snapshots);

        if elapsed < self.config.refresh_interval_secs {
            tracing::debug!("Pool refresh not needed ({}s old)", elapsed);
            return Ok(());
        }

        // Re-initialize pool
        self.initialize().await?;

        Ok(())
    }

    /// Get current pool size
    pub async fn pool_size(&self) -> usize {
        self.snapshots.lock().await.len()
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let snapshots = self.snapshots.lock().await;
        let oldest_snapshot = snapshots.front().and_then(|m| m.created_at.elapsed().ok());
        let newest_snapshot = snapshots.back().and_then(|m| m.created_at.elapsed().ok());

        PoolStats {
            current_size: snapshots.len(),
            max_size: self.config.pool_size,
            oldest_snapshot_age_secs: oldest_snapshot.map(|d| d.as_secs()),
            newest_snapshot_age_secs: newest_snapshot.map(|d| d.as_secs()),
        }
    }

    /// Start background refresh task
    ///
    /// Spawns a task that periodically refreshes the pool
    pub fn spawn_refresh_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(self.config.refresh_interval_secs));

            loop {
                interval.tick().await;

                if let Err(e) = self.refresh_pool().await {
                    tracing::error!("Pool refresh failed: {}", e);
                }
            }
        })
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Current number of snapshots in pool
    pub current_size: usize,

    /// Maximum pool size
    pub max_size: usize,

    /// Age of oldest snapshot in seconds
    pub oldest_snapshot_age_secs: Option<u64>,

    /// Age of newest snapshot in seconds
    pub newest_snapshot_age_secs: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.pool_size, 5);
        assert_eq!(config.refresh_interval_secs, 3600);
    }

    #[tokio::test]
    async fn test_pool_config_from_env() {
        std::env::set_var("IRONCLAW_POOL_SIZE", "10");
        std::env::set_var("IRONCLAW_SNAPSHOT_REFRESH_SECS", "1800");

        let config = PoolConfig::from_env();

        assert_eq!(config.pool_size, 10);
        assert_eq!(config.refresh_interval_secs, 1800);

        std::env::remove_var("IRONCLAW_POOL_SIZE");
        std::env::remove_var("IRONCLAW_SNAPSHOT_REFRESH_SECS");
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let stats = PoolStats {
            current_size: 3,
            max_size: 5,
            oldest_snapshot_age_secs: Some(100),
            newest_snapshot_age_secs: Some(50),
        };

        assert_eq!(stats.current_size, 3);
        assert_eq!(stats.max_size, 5);
    }

    #[tokio::test]
    async fn test_pool_with_temp_dir() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshots");

        let mut config = PoolConfig::default();
        config.snapshot_path = snapshot_path.clone();
        config.pool_size = 2;

        // Set env var for the pool to pick up
        std::env::set_var("IRONCLAW_SNAPSHOT_PATH", snapshot_path.to_str().unwrap());
        std::env::set_var("IRONCLAW_POOL_SIZE", "2");

        // Note: This test verifies the config is properly set
        // Actual pool initialization would require more setup
        assert_eq!(config.snapshot_path, snapshot_path);
        assert_eq!(config.pool_size, 2);

        std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
        std::env::remove_var("IRONCLAW_POOL_SIZE");
    }

    // Property-based test: pool size is always within bounds
    #[test]
    fn test_pool_size_bounds() {
        let config = PoolConfig::default();
        assert!(config.pool_size > 0);
        assert!(config.pool_size <= 20);
    }

    // Property-based test: refresh interval is reasonable
    #[test]
    fn test_refresh_interval_bounds() {
        let config = PoolConfig::default();
        assert!(config.refresh_interval_secs >= 60);
        assert!(config.refresh_interval_secs <= 86400); // Max 1 day
    }
}
