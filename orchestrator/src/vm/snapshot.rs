// VM Snapshot Management
//
// This module handles creating and loading Firecracker VM snapshots
// to enable fast VM spawning (10-50ms vs 110ms cold boot).
//
// Architecture:
// - Snapshots stored at: /var/lib/ironclaw/snapshots/{snapshot_id}/
// - Each snapshot contains: memory state, microVM state, and metadata
// - Snapshot load time target: <20ms

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;

use crate::vm::config::VmConfig;

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Unique snapshot identifier
    pub id: String,

    /// VM configuration used to create this snapshot
    pub vm_config: VmConfig,

    /// Timestamp when snapshot was created
    pub created_at: SystemTime,

    /// Size of snapshot in bytes
    pub size_bytes: u64,

    /// Snapshot format version
    pub version: u32,
}

/// VM Snapshot handle
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Snapshot metadata
    pub metadata: SnapshotMetadata,

    /// Path to snapshot directory
    pub path: PathBuf,

    /// Path to memory snapshot file
    pub memory_path: PathBuf,

    /// Path to microVM state file
    pub state_path: PathBuf,
}

impl Snapshot {
    /// Create a new snapshot instance
    fn new(metadata: SnapshotMetadata, base_path: &Path) -> Self {
        let snapshot_dir = base_path.join(&metadata.id);
        let memory_path = snapshot_dir.join("memory.snap");
        let state_path = snapshot_dir.join("vmstate.json");

        Self {
            metadata,
            path: snapshot_dir,
            memory_path,
            state_path,
        }
    }

    /// Check if snapshot files exist on disk
    pub fn exists(&self) -> bool {
        self.memory_path.exists() && self.state_path.exists()
    }

    /// Get snapshot age in seconds
    pub fn age_seconds(&self) -> u64 {
        self.metadata
            .created_at
            .elapsed()
            .unwrap_or_default()
            .as_secs()
    }
}

/// Create a VM snapshot
///
/// # Arguments
///
/// * `vm_id` - ID of the running VM to snapshot
/// * `snapshot_id` - Unique identifier for the snapshot
///
/// # Returns
///
/// * `Snapshot` - Snapshot handle with metadata
///
/// # Performance
///
/// Target: <100ms to create snapshot
pub async fn create_snapshot(vm_id: &str, snapshot_id: &str) -> Result<Snapshot> {
    tracing::info!("Creating snapshot {} from VM {}", snapshot_id, vm_id);

    let start = std::time::Instant::now();

    // TODO: Phase 2 - Implement actual Firecracker snapshot creation
    // 1. Call Firecracker API to pause VM
    // 2. Create memory snapshot
    // 3. Create microVM state snapshot
    // 4. Resume VM (if needed)
    // 5. Save snapshot to disk

    let base_path = PathBuf::from("/var/lib/ironclaw/snapshots");
    let snapshot_dir = base_path.join(snapshot_id);

    // Create snapshot directory
    fs::create_dir_all(&snapshot_dir)
        .await
        .context("Failed to create snapshot directory")?;

    // Create metadata
    let metadata = SnapshotMetadata {
        id: snapshot_id.to_string(),
        vm_config: VmConfig::new(vm_id.to_string()),
        created_at: SystemTime::now(),
        size_bytes: 0, // Will be updated when real snapshot is created
        version: 1,
    };

    // Create snapshot placeholder files
    let memory_path = snapshot_dir.join("memory.snap");
    let state_path = snapshot_dir.join("vmstate.json");

    // Write placeholder files (Phase 2: replace with real snapshot data)
    fs::write(&memory_path, b"PLACEHOLDER_MEMORY_SNAPSHOT")
        .await
        .context("Failed to write memory snapshot")?;

    fs::write(&state_path, serde_json::to_string_pretty(&metadata)?)
        .await
        .context("Failed to write VM state")?;

    let snapshot = Snapshot::new(metadata, &base_path);

    let elapsed = start.elapsed();
    tracing::debug!("Snapshot created in {:?}", elapsed);

    Ok(snapshot)
}

/// Load a VM snapshot
///
/// # Arguments
///
/// * `snapshot_id` - ID of the snapshot to load
///
/// # Returns
///
/// * `String` - ID of the restored VM
///
/// # Performance
///
/// Target: <20ms to load snapshot
pub async fn load_snapshot(snapshot_id: &str) -> Result<String> {
    tracing::info!("Loading snapshot {}", snapshot_id);

    let start = std::time::Instant::now();

    let base_path = PathBuf::from("/var/lib/ironclaw/snapshots");
    let snapshot_dir = base_path.join(snapshot_id);
    let state_path = snapshot_dir.join("vmstate.json");

    // Verify snapshot exists
    if !snapshot_dir.exists() {
        anyhow::bail!("Snapshot {} not found at {:?}", snapshot_id, snapshot_dir);
    }

    // Load metadata
    let metadata_json = fs::read_to_string(&state_path)
        .await
        .context("Failed to read snapshot metadata")?;

    let _metadata: SnapshotMetadata =
        serde_json::from_str(&metadata_json).context("Failed to parse snapshot metadata")?;

    // TODO: Phase 2 - Implement actual Firecracker snapshot loading
    // 1. Start new Firecracker process
    // 2. Load microVM state
    // 3. Load memory snapshot
    // 4. Resume VM execution
    // 5. Verify VM is responsive

    let vm_id = format!("vm-from-snapshot-{}", snapshot_id);

    let elapsed = start.elapsed();
    tracing::debug!("Snapshot loaded in {:?}", elapsed);

    Ok(vm_id)
}

/// Delete a snapshot from disk
///
/// # Arguments
///
/// * `snapshot_id` - ID of the snapshot to delete
pub async fn delete_snapshot(snapshot_id: &str) -> Result<()> {
    tracing::info!("Deleting snapshot {}", snapshot_id);

    let base_path = PathBuf::from("/var/lib/ironclaw/snapshots");
    let snapshot_dir = base_path.join(snapshot_id);

    if snapshot_dir.exists() {
        fs::remove_dir_all(&snapshot_dir)
            .await
            .context("Failed to delete snapshot directory")?;
    }

    Ok(())
}

/// List all available snapshots
///
/// # Returns
///
/// * `Vec<SnapshotMetadata>` - List of snapshot metadata
pub async fn list_snapshots() -> Result<Vec<SnapshotMetadata>> {
    let base_path = PathBuf::from("/var/lib/ironclaw/snapshots");

    if !base_path.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();

    let mut entries = fs::read_dir(&base_path)
        .await
        .context("Failed to read snapshots directory")?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            let state_path = path.join("vmstate.json");

            if state_path.exists() {
                let metadata_json = fs::read_to_string(&state_path).await?;

                if let Ok(metadata) = serde_json::from_str::<SnapshotMetadata>(&metadata_json) {
                    snapshots.push(metadata);
                }
            }
        }
    }

    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_metadata_serialization() {
        let metadata = SnapshotMetadata {
            id: "test-snapshot".to_string(),
            vm_config: VmConfig::new("test-vm".to_string()),
            created_at: SystemTime::now(),
            size_bytes: 1024 * 1024, // 1MB
            version: 1,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: SnapshotMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.id, deserialized.id);
        assert_eq!(metadata.size_bytes, deserialized.size_bytes);
        assert_eq!(metadata.version, deserialized.version);
    }

    #[test]
    fn test_snapshot_age() {
        let metadata = SnapshotMetadata {
            id: "test-snapshot".to_string(),
            vm_config: VmConfig::new("test-vm".to_string()),
            created_at: SystemTime::now(),
            size_bytes: 1024,
            version: 1,
        };

        let base_path = PathBuf::from("/tmp/test");
        let snapshot = Snapshot::new(metadata, &base_path);

        // Age should be very small (< 10 seconds)
        assert!(snapshot.age_seconds() < 10);
    }

    #[tokio::test]
    async fn test_snapshot_exists_check() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("test-snapshot");
        let memory_path = snapshot_path.join("memory.snap");
        let state_path = snapshot_path.join("vmstate.json");

        // Create snapshot directory and files
        fs::create_dir_all(&snapshot_path).await.unwrap();
        fs::write(&memory_path, b"test").await.unwrap();
        fs::write(&state_path, b"test").await.unwrap();

        let metadata = SnapshotMetadata {
            id: "test-snapshot".to_string(),
            vm_config: VmConfig::new("test-vm".to_string()),
            created_at: SystemTime::now(),
            size_bytes: 4,
            version: 1,
        };

        let snapshot = Snapshot::new(metadata, temp_dir.path());

        assert!(snapshot.exists());
    }

    #[test]
    fn test_snapshot_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("nonexistent");

        let metadata = SnapshotMetadata {
            id: "nonexistent".to_string(),
            vm_config: VmConfig::new("test-vm".to_string()),
            created_at: SystemTime::now(),
            size_bytes: 0,
            version: 1,
        };

        let snapshot = Snapshot::new(metadata, &snapshot_path);

        assert!(!snapshot.exists());
    }

    // Property-based test: snapshot IDs are unique
    #[test]
    fn test_snapshot_id_uniqueness() {
        let id1 = format!("snapshot-{}", uuid::Uuid::new_v4());
        let id2 = format!("snapshot-{}", uuid::Uuid::new_v4());

        assert_ne!(id1, id2);
    }

    // Property-based test: metadata serialization is idempotent
    #[test]
    fn test_metadata_serialization_roundtrip() {
        let original = SnapshotMetadata {
            id: "test-snapshot".to_string(),
            vm_config: VmConfig::new("test-vm".to_string()),
            created_at: SystemTime::UNIX_EPOCH,
            size_bytes: 2048,
            version: 1,
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: SnapshotMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id, restored.id);
        assert_eq!(original.size_bytes, restored.size_bytes);
        assert_eq!(original.created_at, restored.created_at);
    }
}
