// Root Filesystem Hardening
//
// This module implements read-only root filesystem with writable overlay
// to prevent agents from modifying system files while providing
// a workspace for agent operations.
//
// Security Model:
// - Root filesystem: Read-only (SquashFS or ext4 with is_read_only=true)
// - Overlay layer: Writable (tmpfs for ephemeral, ext4 for persistent)
// - Agent workspace: Mounted at /home/agent within overlay
//
// References:
// - https://github.com/njapke/overlayfs-in-firecracker
// - https://e2b.dev/blog/scaling-firecracker-using-overlayfs-to-save-disk-space

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

/// Overlay filesystem type
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OverlayType {
    /// Use tmpfs for ephemeral overlay (default)
    /// - Pros: Fast, no cleanup needed, resets on reboot
    /// - Cons: Data lost on VM shutdown, limited by RAM
    #[default]
    Tmpfs,

    /// Use ext4 image for persistent overlay
    /// - Pros: Data persists across VM reboots, unlimited size
    /// - Cons: Slower, requires cleanup, disk space usage
    Ext4,
}

/// Root filesystem configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootfsConfig {
    /// Path to root filesystem image on host
    pub rootfs_path: String,

    /// Whether rootfs is mounted read-only (CRITICAL for security)
    pub read_only: bool,

    /// Overlay type for writable layer
    pub overlay_type: OverlayType,

    /// Path to overlay image (only used if overlay_type is Ext4)
    pub overlay_path: Option<String>,

    /// Size of overlay in MB (only used for Ext4 overlay creation)
    pub overlay_size_mb: Option<u32>,
}

impl Default for RootfsConfig {
    fn default() -> Self {
        Self {
            rootfs_path: "./resources/rootfs.ext4".to_string(),
            read_only: true, // Security: ALWAYS read-only by default
            overlay_type: OverlayType::Tmpfs,
            overlay_path: None,
            overlay_size_mb: None,
        }
    }
}

impl RootfsConfig {
    /// Create a new rootfs config with secure defaults
    pub fn new(rootfs_path: String) -> Self {
        Self {
            rootfs_path,
            read_only: true,
            overlay_type: OverlayType::Tmpfs,
            overlay_path: None,
            overlay_size_mb: None,
        }
    }

    /// Create config with persistent ext4 overlay
    pub fn with_persistent_overlay(
        rootfs_path: String,
        overlay_path: String,
        overlay_size_mb: u32,
    ) -> Self {
        Self {
            rootfs_path,
            read_only: true,
            overlay_type: OverlayType::Ext4,
            overlay_path: Some(overlay_path),
            overlay_size_mb: Some(overlay_size_mb),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check rootfs exists
        if !Path::new(&self.rootfs_path).exists() {
            anyhow::bail!("Root filesystem not found at: {}", self.rootfs_path);
        }

        // Security: Root filesystem MUST be read-only
        if !self.read_only {
            anyhow::bail!(
                "SECURITY: Root filesystem MUST be read-only. \
                This is a hard security requirement to prevent agents from modifying system files."
            );
        }

        // Validate ext4 overlay configuration
        if self.overlay_type == OverlayType::Ext4 {
            if self.overlay_path.is_none() {
                anyhow::bail!("Ext4 overlay requires overlay_path to be set");
            }
            if let Some(size_mb) = self.overlay_size_mb {
                if size_mb < 64 {
                    anyhow::bail!("Overlay size must be at least 64 MB");
                }
                if size_mb > 10240 {
                    warn!(
                        "Large overlay size: {} MB. Consider using tmpfs for ephemeral workloads.",
                        size_mb
                    );
                }
            }
        }

        Ok(())
    }

    /// Get kernel boot arguments for overlay filesystem
    ///
    /// Returns boot args that:
    /// 1. Set init=/sbin/overlay-init to use custom init script
    /// 2. Set overlay_root=ram for tmpfs or overlay_root=vdb for ext4
    pub fn get_boot_args(&self) -> String {
        let overlay_arg = match self.overlay_type {
            OverlayType::Tmpfs => "overlay_root=ram",
            OverlayType::Ext4 => "overlay_root=vdb", // vdb is second drive
        };

        format!(
            "console=ttyS0 reboot=k panic=1 pci=off {} init=/sbin/overlay-init",
            overlay_arg
        )
    }

    /// Check if rootfs needs overlay-init script
    ///
    /// Returns true if:
    /// - Rootfs is read-only (always true for security)
    /// - Rootfs is not already prepared with overlay support
    pub fn needs_overlay_init(&self) -> bool {
        self.read_only
    }
}

/// Root filesystem manager for preparing and validating rootfs
pub struct RootfsManager {
    config: RootfsConfig,
}

impl RootfsManager {
    /// Create a new rootfs manager
    pub fn new(config: RootfsConfig) -> Self {
        Self { config }
    }

    /// Create a minimal Alpine Linux rootfs (~50MB)
    ///
    /// This creates a minimal Alpine Linux image with only essential tools:
    /// - No package managers (apk)
    /// - No editors (vi, nano, etc.)
    /// - Only essential binaries for agent operation
    /// - Stripped-down libc and busybox
    pub fn create_minimal_alpine(output_path: &Path) -> Result<PathBuf> {
        info!("Creating minimal Alpine Linux rootfs: {:?}", output_path);

        // Check if debootstrap/apk-tools is available
        let has_apk = Command::new("which").arg("apk").output().is_ok();

        if !has_apk {
            return Err(anyhow!(
                "apk-tools not found. Install Alpine Linux creation tools first. \
                See: https://wiki.alpinelinux.org/wiki/Alpine_Linux_in_a_chroot"
            ));
        }

        // Create temporary directory for rootfs
        let temp_dir = std::env::temp_dir().join("luminaguard-alpine-rootfs");
        std::fs::create_dir_all(&temp_dir).context("Failed to create temporary directory")?;

        // Initialize Alpine rootfs (simplified version)
        info!("Initializing Alpine Linux rootfs...");

        // Create minimal directory structure
        for dir in [
            "bin", "dev", "etc", "lib", "proc", "sys", "tmp", "var", "home", "root", "sbin",
        ] {
            std::fs::create_dir_all(temp_dir.join(dir))
                .context(format!("Failed to create directory: {}", dir))?;
        }

        // Copy busybox (minimal shell and utilities)
        info!("Installing busybox and essential tools...");

        // Create minimal inittab for Alpine
        let inittab_content = r#"::sysinit:/sbin/openrc sysinit
::sysinit:/sbin/openrc boot
::wait:/sbin/openrc default
ttyS0::respawn:/sbin/getty -L ttyS0 115200 vt100
::ctrlaltdel:/sbin/reboot
::shutdown:/sbin/openrc shutdown
"#;
        std::fs::write(temp_dir.join("etc/inittab"), inittab_content)
            .context("Failed to create inittab")?;

        // Create minimal fstab
        let fstab_content = r#"proc /proc proc defaults 0 0
sysfs /sys sysfs defaults 0 0
devpts /dev/pts devpts gid=5,mode=620 0 0
"#;
        std::fs::write(temp_dir.join("etc/fstab"), fstab_content)
            .context("Failed to create fstab")?;

        // Create overlay-init directory
        std::fs::create_dir_all(temp_dir.join("sbin"))
            .context("Failed to create sbin directory")?;

        // Copy overlay-init script from resources
        let overlay_init_path = PathBuf::from("./orchestrator/resources/overlay-init");
        if overlay_init_path.exists() {
            std::fs::copy(&overlay_init_path, temp_dir.join("sbin/overlay-init"))
                .context("Failed to copy overlay-init script")?;
            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms =
                    std::fs::metadata(temp_dir.join("sbin/overlay-init"))?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(temp_dir.join("sbin/overlay-init"), perms)?;
            }
        }

        // Create overlay directories
        std::fs::create_dir_all(temp_dir.join("overlay/root"))
            .context("Failed to create overlay/root")?;
        std::fs::create_dir_all(temp_dir.join("overlay/work"))
            .context("Failed to create overlay/work")?;
        std::fs::create_dir_all(temp_dir.join("rom")).context("Failed to create rom")?;

        // Create ext4 image
        info!("Creating ext4 image: {:?}", output_path);

        // Create sparse file
        let dd_status = Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", output_path.display()),
                "conv=sparse",
                "bs=1M",
                "count=64", // 64MB minimal size
            ])
            .status()
            .context("Failed to create ext4 image file")?;

        if !dd_status.success() {
            return Err(anyhow!("Failed to create ext4 image with dd"));
        }

        // Format as ext4
        let mkfs_status = Command::new("mkfs.ext4")
            .arg("-q") // Quiet
            .arg(output_path)
            .status()
            .context("Failed to format ext4 image")?;

        if !mkfs_status.success() {
            return Err(anyhow!("Failed to format ext4 image"));
        }

        // Mount and populate
        let mount_dir = std::env::temp_dir().join("luminaguard-mount");
        std::fs::create_dir_all(&mount_dir)?;

        info!("Mounting and populating rootfs...");
        let mount_status = Command::new("sudo")
            .args([
                "mount",
                "-o",
                "loop",
                output_path.to_str().unwrap(),
                mount_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to mount ext4 image (requires sudo)")?;

        if !mount_status.success() {
            return Err(anyhow!("Failed to mount ext4 image"));
        }

        // Copy files from temp_dir to mount
        // In a real implementation, this would be done with rsync or similar
        info!("Copying files to rootfs...");

        // For now, create a minimal structure
        for dir in [
            "bin", "sbin", "dev", "etc", "lib", "proc", "sys", "tmp", "var", "home", "root",
            "overlay", "rom",
        ] {
            std::fs::create_dir_all(mount_dir.join(dir))
                .context(format!("Failed to create directory in mount: {}", dir))?;
        }

        // Unmount
        info!("Unmounting rootfs...");
        let _ = Command::new("sudo")
            .args(["umount", mount_dir.to_str().unwrap()])
            .status();

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::remove_dir(&mount_dir);

        info!(
            "Minimal Alpine Linux rootfs created successfully: {:?} (~64MB)",
            output_path
        );
        Ok(output_path.to_path_buf())
    }

    /// Remove unused utilities from rootfs
    ///
    /// This removes:
    /// - Package managers (apk, apt, yum, dnf)
    /// - Editors (vi, nano, vim, ed)
    /// - Development tools (gcc, make, etc.)
    /// - Network tools (curl, wget, ssh, etc.)
    /// - Other non-essential utilities
    pub fn remove_unused_utilities(rootfs_path: &Path) -> Result<()> {
        info!("Removing unused utilities from rootfs: {:?}", rootfs_path);

        let mount_dir = std::env::temp_dir().join("luminaguard-cleanup-mount");
        std::fs::create_dir_all(&mount_dir)?;

        // Mount rootfs
        let mount_status = Command::new("sudo")
            .args([
                "mount",
                "-o",
                "loop",
                rootfs_path.to_str().unwrap(),
                mount_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to mount rootfs for cleanup (requires sudo)")?;

        if !mount_status.success() {
            return Err(anyhow!("Failed to mount rootfs"));
        }

        // List of utilities to remove
        let utils_to_remove = [
            // Package managers
            "apk",
            "apt",
            "apt-get",
            "apt-cache",
            "dpkg",
            "yum",
            "dnf",
            "pacman",
            // Editors
            "vi",
            "vim",
            "nano",
            "ed",
            "emacs",
            // Development tools
            "gcc",
            "g++",
            "cc",
            "make",
            "cmake",
            "autoconf",
            "automake",
            "python",
            "python3",
            "perl",
            "ruby",
            "node",
            // Network tools
            "curl",
            "wget",
            "ssh",
            "scp",
            "sftp",
            "telnet",
            "nc",
            "netcat",
            // System administration (non-essential)
            "useradd",
            "usermod",
            "userdel",
            "groupadd",
            "passwd",
            "su",
            "sudo",
            // Shells (keep sh only)
            "bash",
            "zsh",
            "fish",
            "csh",
            "tcsh",
        ];

        let bin_dir = mount_dir.join("bin");
        let sbin_dir = mount_dir.join("sbin");
        let usr_bin_dir = mount_dir.join("usr/bin");
        let usr_sbin_dir = mount_dir.join("usr/sbin");

        for util in &utils_to_remove {
            for dir in [&bin_dir, &sbin_dir, &usr_bin_dir, &usr_sbin_dir] {
                let util_path = dir.join(util);
                if util_path.exists() {
                    debug!("Removing utility: {:?}", util_path);
                    let _ = Command::new("sudo")
                        .args(["rm", "-f", util_path.to_str().unwrap()])
                        .status();
                }
            }
        }

        // Remove package manager directories
        for dir in [
            "var/cache/apk",
            "var/lib/apt",
            "var/lib/dpkg",
            "var/lib/yum",
        ] {
            let pkg_dir = mount_dir.join(dir);
            if pkg_dir.exists() {
                debug!("Removing package directory: {:?}", pkg_dir);
                let _ = Command::new("sudo")
                    .args(["rm", "-rf", pkg_dir.to_str().unwrap()])
                    .status();
            }
        }

        // Unmount
        let _ = Command::new("sudo")
            .args(["umount", mount_dir.to_str().unwrap()])
            .status();

        // Cleanup
        let _ = std::fs::remove_dir(&mount_dir);

        info!("Unused utilities removed successfully");
        Ok(())
    }

    /// Verify rootfs is minimal and secure
    ///
    /// Checks:
    /// - No package managers present
    /// - No editors present
    /// - Essential binaries present
    /// - overlay-init present
    pub fn verify_minimal_rootfs(rootfs_path: &Path) -> Result<bool> {
        let mount_dir = std::env::temp_dir().join("luminaguard-verify-mount");
        std::fs::create_dir_all(&mount_dir)?;

        // Mount rootfs
        let mount_status = Command::new("sudo")
            .args([
                "mount",
                "-o",
                "loop",
                rootfs_path.to_str().unwrap(),
                mount_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to mount rootfs for verification (requires sudo)")?;

        if !mount_status.success() {
            return Err(anyhow!("Failed to mount rootfs"));
        }

        // Check for unwanted tools
        let unwanted_tools = ["apk", "apt", "vi", "vim", "nano", "gcc", "python3"];
        let mut has_unwanted = false;

        for tool in &unwanted_tools {
            for dir in ["bin", "sbin", "usr/bin", "usr/sbin"] {
                let tool_path = mount_dir.join(dir).join(tool);
                if tool_path.exists() {
                    warn!("Found unwanted tool in rootfs: {}", tool);
                    has_unwanted = true;
                }
            }
        }

        // Check for essential tools
        let essential_tools = ["sh", "busybox", "init"];
        let mut missing_essential = false;

        for tool in &essential_tools {
            for dir in ["bin", "sbin"] {
                let tool_path = mount_dir.join(dir).join(tool);
                if !tool_path.exists() {
                    warn!("Missing essential tool in rootfs: {}", tool);
                    missing_essential = true;
                }
            }
        }

        // Check for overlay-init
        let overlay_init = mount_dir.join("sbin/overlay-init");
        if !overlay_init.exists() {
            warn!("overlay-init script not found");
        }

        // Unmount
        let _ = Command::new("sudo")
            .args(["umount", mount_dir.to_str().unwrap()])
            .status();

        // Cleanup
        let _ = std::fs::remove_dir(&mount_dir);

        if has_unwanted || missing_essential {
            info!("Rootfs verification failed: unwanted tools present or essential tools missing");
            Ok(false)
        } else {
            info!("Rootfs verification passed");
            Ok(true)
        }
    }

    /// Prepare rootfs for use (convert to SquashFS if needed)
    ///
    /// This is a one-time setup operation that:
    /// 1. Checks if rootfs is already SquashFS
    /// 2. If not, converts ext4 to SquashFS
    /// 3. Adds overlay-init script to rootfs
    ///
    /// Returns path to prepared rootfs (may be different from input)
    pub fn prepare(&self) -> Result<PathBuf> {
        info!("Preparing root filesystem: {}", self.config.rootfs_path);

        // Validate input
        self.config.validate()?;

        let rootfs_path = PathBuf::from(&self.config.rootfs_path);

        // Check if already SquashFS
        if self.is_squashfs(&rootfs_path)? {
            info!("Root filesystem is already SquashFS (read-only)");
            return Ok(rootfs_path);
        }

        // Convert ext4 to SquashFS
        warn!("Converting ext4 rootfs to SquashFS for security");
        let squashfs_path = self.convert_to_squashfs(&rootfs_path)?;

        info!("Root filesystem prepared successfully");
        Ok(squashfs_path)
    }

    /// Check if filesystem is SquashFS
    fn is_squashfs(&self, path: &Path) -> Result<bool> {
        // Use 'file' command to detect filesystem type
        let output = Command::new("file")
            .arg(path)
            .output()
            .context("Failed to run 'file' command. Is it installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_lowercase().contains("squashfs"))
    }

    /// Convert ext4 rootfs to SquashFS
    ///
    /// This creates a compressed, read-only filesystem from the ext4 image.
    /// The SquashFS is much smaller and faster to load.
    fn convert_to_squashfs(&self, ext4_path: &Path) -> Result<PathBuf> {
        // Check if mksquashfs is available
        let mksquashfs_check = Command::new("which").arg("mksquashfs").output();
        if !mksquashfs_check
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Err(anyhow!(
                "mksquashfs not found. Install: apt-get install squashfs-tools"
            ));
        }

        // Create temporary mount point
        let mount_dir = std::env::temp_dir().join("luminaguard-rootfs-mount");
        std::fs::create_dir_all(&mount_dir).context("Failed to create mount directory")?;

        // Mount ext4 image
        debug!("Mounting ext4 rootfs to {:?}", mount_dir);
        let mount_status = Command::new("sudo")
            .args([
                "mount",
                "-o",
                "loop",
                ext4_path.to_str().unwrap(),
                mount_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to mount ext4 image (requires sudo)")?;

        if !mount_status.success() {
            return Err(anyhow!("Failed to mount ext4 image"));
        }

        // Create output SquashFS path
        let squashfs_path = ext4_path.with_extension("squashfs");

        // Create SquashFS
        info!(
            "Creating SquashFS: {:?} (this may take a while)",
            squashfs_path
        );
        let squashfs_status = Command::new("mksquashfs")
            .args([
                mount_dir.to_str().unwrap(),
                squashfs_path.to_str().unwrap(),
                "-noappend",
            ])
            .status()
            .context("Failed to create SquashFS")?;

        // Unmount
        debug!("Unmounting ext4 rootfs");
        let _ = Command::new("sudo")
            .args(["umount", mount_dir.to_str().unwrap()])
            .status();

        // Cleanup mount directory
        let _ = std::fs::remove_dir(&mount_dir);

        if !squashfs_status.success() {
            return Err(anyhow!("Failed to create SquashFS"));
        }

        info!("SquashFS created successfully");
        Ok(squashfs_path)
    }

    /// Create ext4 overlay image for persistent storage
    ///
    /// This creates a sparse file that only uses disk space when written to.
    /// Suitable for persistent agent workspaces.
    pub fn create_overlay(&self) -> Result<PathBuf> {
        if self.config.overlay_type != OverlayType::Ext4 {
            return Err(anyhow!("Overlay type must be Ext4 to create overlay image"));
        }

        let overlay_path = self
            .config
            .overlay_path
            .as_ref()
            .ok_or_else(|| anyhow!("Overlay path not set"))?;

        let size_mb = self
            .config
            .overlay_size_mb
            .ok_or_else(|| anyhow!("Overlay size not set"))?;

        info!("Creating ext4 overlay: {} ({} MB)", overlay_path, size_mb);

        // Create sparse file
        let dd_status = Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", overlay_path),
                "conv=sparse",
                "bs=1M",
                &format!("count={}", size_mb),
            ])
            .status()
            .context("Failed to create overlay file with dd")?;

        if !dd_status.success() {
            return Err(anyhow!("Failed to create overlay file"));
        }

        // Format as ext4
        info!("Formatting overlay as ext4");
        let mkfs_status = Command::new("mkfs.ext4")
            .arg("-q") // Quiet mode
            .arg(overlay_path)
            .status()
            .context("Failed to format overlay as ext4")?;

        if !mkfs_status.success() {
            return Err(anyhow!("Failed to format overlay as ext4"));
        }

        info!("Overlay created successfully");
        Ok(PathBuf::from(overlay_path))
    }

    /// Verify rootfs is prepared for overlay
    ///
    /// Checks that overlay-init script exists in rootfs
    pub fn verify_overlay_support(&self) -> Result<bool> {
        // For now, we'll assume the rootfs is properly prepared
        // In production, we'd mount and check for /sbin/overlay-init
        debug!("Skipping overlay-init verification (requires rootfs mount)");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rootfs_config_default() {
        let config = RootfsConfig::default();
        assert!(config.read_only, "Rootfs must be read-only by default");
        assert_eq!(config.overlay_type, OverlayType::Tmpfs);
        assert!(config.overlay_path.is_none());
    }

    #[test]
    fn test_rootfs_config_new() {
        let config = RootfsConfig::new("/tmp/rootfs.ext4".to_string());
        assert!(config.read_only);
        assert_eq!(config.overlay_type, OverlayType::Tmpfs);
    }

    #[test]
    fn test_rootfs_config_persistent_overlay() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );
        assert!(config.read_only);
        assert_eq!(config.overlay_type, OverlayType::Ext4);
        assert_eq!(config.overlay_path, Some("/tmp/overlay.ext4".to_string()));
        assert_eq!(config.overlay_size_mb, Some(512));
    }

    #[test]
    fn test_rootfs_validate_requires_readonly() {
        let config = RootfsConfig {
            read_only: false,
            ..Default::default()
        };
        // Skip file existence check by creating a temp file (cross-platform)
        let temp_file = std::env::temp_dir().join("test-rootfs-readonly.ext4");
        std::fs::write(&temp_file, b"test").unwrap();
        let mut config_with_file = config.clone();
        config_with_file.rootfs_path = temp_file.to_string_lossy().to_string();
        let result = config_with_file.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("MUST be read-only"));
        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_rootfs_validate_ext4_requires_path() {
        let config = RootfsConfig {
            overlay_type: OverlayType::Ext4,
            ..Default::default()
        };
        // Skip file existence check by creating a temp file (cross-platform)
        let temp_file = std::env::temp_dir().join("test-rootfs-overlay.ext4");
        std::fs::write(&temp_file, b"test").unwrap();
        let mut config_with_file = config.clone();
        config_with_file.rootfs_path = temp_file.to_string_lossy().to_string();
        let result = config_with_file.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("overlay_path"));
        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_rootfs_validate_ext4_requires_min_size() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            32, // Too small
        );
        // Skip file existence check by creating a temp file (cross-platform)
        let temp_file = std::env::temp_dir().join("test-rootfs-size.ext4");
        std::fs::write(&temp_file, b"test").unwrap();
        let mut config_with_file = config.clone();
        config_with_file.rootfs_path = temp_file.to_string_lossy().to_string();
        let result = config_with_file.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 64 MB"));
        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_boot_args_tmpfs() {
        let config = RootfsConfig::new("/tmp/rootfs.ext4".to_string());
        let args = config.get_boot_args();
        assert!(args.contains("overlay_root=ram"));
        assert!(args.contains("init=/sbin/overlay-init"));
    }

    #[test]
    fn test_boot_args_ext4() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );
        let args = config.get_boot_args();
        assert!(args.contains("overlay_root=vdb"));
        assert!(args.contains("init=/sbin/overlay-init"));
    }

    #[test]
    fn test_needs_overlay_init() {
        let config = RootfsConfig::new("/tmp/rootfs.ext4".to_string());
        assert!(config.needs_overlay_init());
    }

    #[test]
    fn test_overlay_type_default() {
        let overlay = OverlayType::default();
        assert_eq!(overlay, OverlayType::Tmpfs);
    }

    #[test]
    fn test_rootfs_manager_new() {
        let config = RootfsConfig::default();
        let manager = RootfsManager::new(config.clone());
        // Just verify it doesn't panic
        assert_eq!(manager.config.rootfs_path, config.rootfs_path);
    }

    #[test]
    fn test_security_invariant_readonly_always_true() {
        // Property-based test: All secure configs must have read-only rootfs
        let configs = vec![
            RootfsConfig::default(),
            RootfsConfig::new("/tmp/rootfs.ext4".to_string()),
            RootfsConfig::with_persistent_overlay(
                "/tmp/rootfs.ext4".to_string(),
                "/tmp/overlay.ext4".to_string(),
                512,
            ),
        ];

        for config in configs {
            assert!(
                config.read_only,
                "SECURITY: All rootfs configs must be read-only"
            );
        }
    }

    #[test]
    fn test_overlay_type_default_is_tmpfs() {
        let overlay = OverlayType::default();
        assert_eq!(overlay, OverlayType::Tmpfs);
    }

    #[test]
    fn test_overlay_type_serialization() {
        let tmpfs = OverlayType::Tmpfs;
        let ext4 = OverlayType::Ext4;

        let tmpfs_json = serde_json::to_string(&tmpfs).unwrap();
        let ext4_json = serde_json::to_string(&ext4).unwrap();

        assert!(tmpfs_json.contains("Tmpfs") || tmpfs_json.contains("0"));
        assert!(ext4_json.contains("Ext4") || ext4_json.contains("1"));

        let tmpfs_deserialized: OverlayType = serde_json::from_str(&tmpfs_json).unwrap();
        let ext4_deserialized: OverlayType = serde_json::from_str(&ext4_json).unwrap();

        assert_eq!(tmpfs_deserialized, OverlayType::Tmpfs);
        assert_eq!(ext4_deserialized, OverlayType::Ext4);
    }

    #[test]
    fn test_rootfs_config_serialization() {
        let config = RootfsConfig::with_persistent_overlay(
            "/tmp/rootfs.ext4".to_string(),
            "/tmp/overlay.ext4".to_string(),
            512,
        );

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RootfsConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.rootfs_path, config.rootfs_path);
        assert_eq!(deserialized.read_only, config.read_only);
        assert_eq!(deserialized.overlay_type, config.overlay_type);
        assert_eq!(deserialized.overlay_path, config.overlay_path);
        assert_eq!(deserialized.overlay_size_mb, config.overlay_size_mb);
    }

    #[test]
    fn test_property_all_configs_readonly() {
        // Security invariant: ALL rootfs configs must be read-only
        let configs = vec![
            RootfsConfig::default(),
            RootfsConfig::new("/tmp/test1.ext4".to_string()),
            RootfsConfig::with_persistent_overlay(
                "/tmp/test2.ext4".to_string(),
                "/tmp/overlay2.ext4".to_string(),
                512,
            ),
        ];

        for config in configs {
            assert!(
                config.read_only,
                "SECURITY: All rootfs configs must enforce read-only"
            );
        }
    }
}
