// Firecracker Test Assets
//
// Handles preparation of kernel and rootfs for feasibility testing

use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

/// Firecracker test assets (kernel + rootfs)
pub struct FirecrackerAssets {
    pub kernel_path: PathBuf,
    pub rootfs_path: PathBuf,
    pub temp_dir: PathBuf,
}

impl FirecrackerAssets {
    /// Prepare test assets for feasibility test
    ///
    /// This will:
    /// 1. Create a temporary directory
    /// 2. Download or use cached kernel image
    /// 3. Create minimal rootfs
    ///
    /// For PROTOTYPE: We'll check if assets exist in /tmp first
    /// For PRODUCTION: Would download from official sources
    pub async fn prepare() -> Result<Self> {
        let temp_dir = PathBuf::from("/tmp/ironclaw-fc-test");

        // Create temp directory
        fs::create_dir_all(&temp_dir)
            .await
            .context("Failed to create temp directory")?;

        // Check for existing assets or create placeholder paths
        let kernel_path = temp_dir.join("vmlinux.bin");
        let rootfs_path = temp_dir.join("rootfs.ext4");

        // For prototype, check if assets exist
        // If not, we'll document what's needed
        if !kernel_path.exists() {
            tracing::warn!("Kernel image not found at: {}", kernel_path.display());
            tracing::warn!("For real testing, download from:");
            tracing::warn!("  https://s3.amazonaws.com/spec.ccfc.min/images/terraform/aws-k8s-1.23/test-1.23-x86_64/kernel.bin");
        }

        if !rootfs_path.exists() {
            tracing::warn!("Rootfs not found at: {}", rootfs_path.display());
            tracing::warn!("For real testing, create with:");
            tracing::warn!("  dd if=/dev/zero of=rootfs.ext4 bs=1M count=64");
            tracing::warn!("  mkfs.ext4 rootfs.ext4");
        }

        Ok(Self {
            kernel_path,
            rootfs_path,
            temp_dir,
        })
    }

    /// Check if assets are ready for testing
    pub fn is_ready(&self) -> bool {
        self.kernel_path.exists() && self.rootfs_path.exists()
    }

    /// Cleanup test assets
    pub async fn cleanup(&self) -> Result<()> {
        if self.temp_dir.exists() {
            fs::remove_dir_all(&self.temp_dir)
                .await
                .context("Failed to cleanup temp directory")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prepare_assets() {
        let assets = FirecrackerAssets::prepare().await.unwrap();
        assert!(assets.temp_dir.exists());
        assets.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_assets_not_ready_by_default() {
        let assets = FirecrackerAssets::prepare().await.unwrap();
        // Assets shouldn't exist without manual download
        assert!(!assets.is_ready());
        assets.cleanup().await.unwrap();
    }
}
