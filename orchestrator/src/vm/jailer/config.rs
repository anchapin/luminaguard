// Jailer Configuration
//
// Configuration for running Firecracker via the Jailer sandbox

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Jailer configuration for sandboxing Firecracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailerConfig {
    /// Unique VM identifier (alphanumeric + hyphens, max 64 chars)
    pub id: String,

    /// Path to Firecracker binary (default: /usr/local/bin/firecracker)
    pub exec_file: PathBuf,

    /// UID to run Firecracker as (default: 0 - root)
    pub uid: u32,

    /// GID to run Firecracker as (default: 0 - root)
    pub gid: u32,

    /// NUMA node to assign to (default: 0)
    pub numa_node: u32,

    /// Chroot base directory (default: /srv/jailer)
    pub chroot_base_dir: PathBuf,

    /// Cgroup settings (e.g., "cpu.shares=512")
    pub cgroups: HashMap<String, String>,

    /// Network namespace path (optional)
    pub netns: Option<PathBuf>,

    /// Daemonize the jailer process
    pub daemonize: bool,

    /// Create new PID namespace
    pub new_pid_ns: bool,

    /// Extra arguments to pass to Firecracker
    pub extra_args: Vec<String>,
}

impl Default for JailerConfig {
    fn default() -> Self {
        let mut cgroups = HashMap::new();

        // Default resource limits
        cgroups.insert("cpu.shares".to_string(), "512".to_string()); // Lower CPU priority
        cgroups.insert("memory.limit_in_bytes".to_string(), "536870912".to_string()); // 512MB

        Self {
            id: "default".to_string(),
            exec_file: PathBuf::from("/usr/local/bin/firecracker"),
            uid: 0,
            gid: 0,
            numa_node: 0,
            chroot_base_dir: PathBuf::from("/srv/jailer"),
            cgroups,
            netns: None,
            daemonize: true,
            new_pid_ns: true,
            extra_args: Vec::new(),
        }
    }
}

impl JailerConfig {
    /// Create a new Jailer config with defaults
    pub fn new(id: String) -> Self {
        let mut config = Self::default();
        config.id = id;
        config
    }

    /// Create a test config with paths that work in test environments
    /// Uses /dev/null for exec_file which always exists
    #[cfg(test)]
    pub fn test_config(id: String) -> Self {
        let mut config = Self::new(id);
        config.exec_file = PathBuf::from("/dev/null");
        config.chroot_base_dir = PathBuf::from("/tmp");
        config
    }

    /// Set custom UID/GID for privilege separation
    pub fn with_user(mut self, uid: u32, gid: u32) -> Self {
        self.uid = uid;
        self.gid = gid;
        self
    }

    /// Set NUMA node affinity
    pub fn with_numa_node(mut self, node: u32) -> Self {
        self.numa_node = node;
        self
    }

    /// Add a cgroup constraint
    pub fn with_cgroup(mut self, key: String, value: String) -> Self {
        self.cgroups.insert(key, value);
        self
    }

    /// Set network namespace
    pub fn with_netns(mut self, netns: PathBuf) -> Self {
        self.netns = Some(netns);
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate VM ID
        if self.id.is_empty() {
            anyhow::bail!("VM ID cannot be empty");
        }

        if self.id.len() > 64 {
            anyhow::bail!("VM ID too long (max 64 characters)");
        }

        // Only alphanumeric and hyphens allowed
        if !self
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            anyhow::bail!("VM ID can only contain alphanumeric characters and hyphens");
        }

        // Validate exec file exists
        if !self.exec_file.exists() {
            anyhow::bail!("Firecracker binary not found at: {:?}", self.exec_file);
        }

        // Validate chroot base dir exists or can be created
        if let Some(parent) = self.chroot_base_dir.parent() {
            if !parent.exists() {
                anyhow::bail!("Chroot base parent directory does not exist: {:?}", parent);
            }
        }

        // Validate netns if specified
        if let Some(ref netns) = self.netns {
            if !netns.exists() {
                anyhow::bail!("Network namespace does not exist: {:?}", netns);
            }
        }

        Ok(())
    }

    /// Get the chroot directory path for this VM
    pub fn chroot_dir(&self) -> PathBuf {
        let exec_name = self
            .exec_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("firecracker");

        self.chroot_base_dir
            .join(exec_name)
            .join(&self.id)
            .join("root")
    }

    /// Build jailer command line arguments
    pub fn build_args(&self) -> Vec<String> {
        let mut args = vec![
            "--id".to_string(),
            self.id.clone(),
            "--node".to_string(),
            self.numa_node.to_string(),
            "--exec-file".to_string(),
            self.exec_file.to_string_lossy().to_string(),
            "--uid".to_string(),
            self.uid.to_string(),
            "--gid".to_string(),
            self.gid.to_string(),
            "--chroot-base-dir".to_string(),
            self.chroot_base_dir.to_string_lossy().to_string(),
        ];

        // Add cgroup parameters
        for (key, value) in &self.cgroups {
            args.push("--cgroup".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Add network namespace if specified
        if let Some(ref netns) = self.netns {
            args.push("--netns".to_string());
            args.push(netns.to_string_lossy().to_string());
        }

        // Add daemonize flag
        if self.daemonize {
            args.push("--daemonize".to_string());
        }

        // Add new PID namespace flag
        if self.new_pid_ns {
            args.push("--new-pid-ns".to_string());
        }

        // Add separator for Firecracker arguments
        args.push("--".to_string());

        // Add extra arguments for Firecracker
        args.extend(self.extra_args.iter().cloned());

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = JailerConfig::default();
        assert_eq!(config.id, "default");
        assert_eq!(config.uid, 0);
        assert_eq!(config.gid, 0);
        assert!(config.cgroups.contains_key("cpu.shares"));
    }

    #[test]
    fn test_config_validation_valid_id() {
        let config = JailerConfig::test_config("valid-vm-id-123".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_id() {
        let config = JailerConfig::test_config("".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_invalid_chars_in_id() {
        let config = JailerConfig::test_config("invalid@id#with$symbols".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_id_too_long() {
        let long_id = "a".repeat(65); // 65 chars > 64 limit
        let config = JailerConfig::test_config(long_id);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_with_user() {
        let config = JailerConfig::new("test".to_string()).with_user(123, 456);
        assert_eq!(config.uid, 123);
        assert_eq!(config.gid, 456);
    }

    #[test]
    fn test_with_cgroup() {
        let config = JailerConfig::new("test".to_string())
            .with_cgroup("cpu.shares".to_string(), "1024".to_string());
        assert_eq!(config.cgroups.get("cpu.shares"), Some(&"1024".to_string()));
    }

    #[test]
    fn test_chroot_dir_path() {
        let config = JailerConfig::new("my-vm".to_string());
        let chroot_dir = config.chroot_dir();
        assert!(chroot_dir.ends_with("firecracker/my-vm/root"));
    }

    #[test]
    fn test_build_args() {
        let config = JailerConfig::new("test-vm".to_string())
            .with_numa_node(1)
            .with_user(123, 100);

        let args = config.build_args();

        assert!(args.contains(&"--id".to_string()));
        assert!(args.contains(&"test-vm".to_string()));
        assert!(args.contains(&"--node".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"--uid".to_string()));
        assert!(args.contains(&"123".to_string()));
        assert!(args.contains(&"--gid".to_string()));
        assert!(args.contains(&"100".to_string()));
        assert!(args.contains(&"--daemonize".to_string()));
        assert!(args.contains(&"--new-pid-ns".to_string()));
        assert!(args.contains(&"--".to_string()));
    }

    #[test]
    fn test_build_args_with_cgroups() {
        let mut config = JailerConfig::new("test-vm".to_string());
        config
            .cgroups
            .insert("cpu.shares".to_string(), "2048".to_string());

        let args = config.build_args();

        assert!(args.contains(&"--cgroup".to_string()));
        assert!(args.contains(&"cpu.shares=2048".to_string()));
    }
}

// Property-based tests with Proptest
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_vm_id_alphanumeric_or_dash(id in "[a-zA-Z0-9-\\-]{1,64}") {
            // Valid IDs should pass validation
            let config = JailerConfig::test_config(id.to_string());
            prop_assert!(config.validate().is_ok());
        }

        #[test]
        fn prop_vm_id_empty_or_too_long(id in ".{0,}.{65,}") {
            // Empty or too long IDs should fail validation
            let config = JailerConfig::test_config(id.to_string());
            prop_assert!(config.validate().is_err());
        }

        #[test]
        fn prop_vm_id_invalid_chars(id in "[^a-zA-Z0-9\\-]+") {
            // IDs with invalid chars should fail validation
            let config = JailerConfig::test_config(id.to_string());
            prop_assert!(config.validate().is_err());
        }
    }
}
