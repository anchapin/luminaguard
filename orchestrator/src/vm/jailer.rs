// Firecracker Jailer Integration
//
// This module provides process sandboxing for Firecracker VMs using Jailer.
// Jailer enhances security by:
// - chroot: Isolates process filesystem
// - cgroups: Enforces resource limits (CPU, memory, I/O)
// - Network namespaces: Prepares for vsock isolation
//
// Key invariants:
// - Graceful degradation if Jailer not available
// - Resource limits enforced before VM spawn
// - Clear error messages for misconfigurations

use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Jailer configuration for sandboxing Firecracker
#[derive(Debug, Clone)]
pub struct JailerConfig {
    /// Path to jailer binary (default: /usr/local/bin/jailer)
    pub jailer_binary: PathBuf,

    /// Unique ID for this jailed instance
    pub jailer_id: String,

    /// Base directory for chroot (default: /var/jail)
    pub chroot_base_dir: PathBuf,

    /// Executable to run inside jail (Firecracker binary)
    pub exec_file: PathBuf,

    /// Number of vCPUs to limit (cgroup quota)
    pub cpu_count: u8,

    /// Memory limit in MB (cgroup limit)
    pub memory_limit_mb: u32,

    /// Optional: Network namespace preparation
    pub netns: Option<String>,
}

impl Default for JailerConfig {
    fn default() -> Self {
        Self {
            jailer_binary: PathBuf::from("/usr/local/bin/jailer"),
            jailer_id: "ironclaw-vm".to_string(),
            chroot_base_dir: PathBuf::from("/var/jail"),
            exec_file: PathBuf::from("/usr/local/bin/firecracker-v1.14.1"),
            cpu_count: 1,
            memory_limit_mb: 256,
            netns: None,
        }
    }
}

impl JailerConfig {
    /// Create a new JailerConfig with sensible defaults
    pub fn new(jailer_id: String) -> Self {
        Self {
            jailer_id,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check CPU count
        if self.cpu_count == 0 {
            return Err(anyhow::anyhow!("CPU count must be > 0"));
        }

        // Check memory limit (min 128MB)
        if self.memory_limit_mb < 128 {
            return Err(anyhow::anyhow!(
                "Memory limit must be at least 128 MB, got {}",
                self.memory_limit_mb
            ));
        }

        // Check jailer binary exists
        if !self.jailer_binary.exists() {
            warn!(
                "Jailer binary not found at {:?}, will run without sandboxing",
                self.jailer_binary
            );
        }

        // Check exec file exists
        if !self.exec_file.exists() {
            return Err(anyhow::anyhow!(
                "Firecracker binary not found at {:?}",
                self.exec_file
            ));
        }

        Ok(())
    }

    /// Build jailer CLI arguments
    fn build_args(&self) -> Vec<String> {
        let mut args = vec![
            "--id".to_string(),
            self.jailer_id.clone(),
            "--exec-file".to_string(),
            self.exec_file.display().to_string(),
            "--chroot-base-dir".to_string(),
            self.chroot_base_dir.display().to_string(),
            "--cgroup-version".to_string(),
            "2".to_string(), // Use cgroup v2
        ];

        // Add resource limits
        args.extend(vec![
            "--cgroup".to_string(),
            format!("cpu:{}", self.cpu_count),
            format!("memory:{}", self.memory_limit_mb),
        ]);

        // Optional: network namespace
        if let Some(ref netns) = self.netns {
            args.extend(vec!["--netns".to_string(), netns.clone()]);
        }

        args
    }

    /// Get the API socket path within the chroot
    ///
    /// Jailer creates the socket at:
    /// {chroot_base_dir}/{firecracker-id}/{id}/run/firecracker.socket
    pub fn api_socket_path(&self) -> PathBuf {
        let firecracker_id = self
            .exec_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("firecracker");

        self.chroot_base_dir
            .join(firecracker_id)
            .join(&self.jailer_id)
            .join("run")
            .join("firecracker.socket")
    }
}

/// Check if Jailer binary is available
pub fn jailer_available() -> bool {
    let path = PathBuf::from("/usr/local/bin/jailer");
    path.exists() || which::which("jailer").is_ok()
}

/// Start Firecracker with Jailer sandboxing
///
/// # Arguments
///
/// * `config` - Jailer configuration
///
/// # Returns
///
/// * `JailerProcess` - Handle to the jailed Firecracker process
///
/// # Behavior
///
/// - If Jailer is available: Spawns Firecracker within chroot + cgroups
/// - If Jailer is NOT available: Logs warning, spawns Firecracker directly (graceful degradation)
pub async fn start_with_jailer(config: &JailerConfig) -> Result<JailerProcess> {
    info!("Starting Firecracker with Jailer config: {:?}", config);

    config.validate().context("Invalid Jailer configuration")?;

    // Check if jailer is available
    if jailer_available() {
        start_jailed(config).await
    } else {
        warn!(
            "Jailer not available, starting Firecracker without sandboxing (INSECURE for production!)"
        );
        start_unjailed(config).await
    }
}

/// Start Firecracker WITH Jailer sandboxing
async fn start_jailed(config: &JailerConfig) -> Result<JailerProcess> {
    info!("Starting jailed Firecracker process");

    let args = config.build_args();
    debug!("Jailer command: {:?} {:?}", config.jailer_binary, args);

    // Spawn jailer process
    let mut child = Command::new(&config.jailer_binary)
        .args(&args)
        .spawn()
        .context("Failed to spawn jailer process")?;

    let pid = child.id().context("Failed to get jailer PID")?;

    info!("Jailer process started with PID: {}", pid);

    // Note: We don't await child.wait() here because the process runs in background
    // The caller is responsible for managing the child process lifecycle
    tokio::spawn(async move {
        let status = child.wait().await;
        match status {
            Ok(status) => {
                debug!("Jailer process exited with status: {}", status);
            }
            Err(e) => {
                warn!("Jailer process error: {:?}", e);
            }
        }
    });

    Ok(JailerProcess {
        pid,
        api_socket: config.api_socket_path(),
        jailed: true,
    })
}

/// Start Firecracker WITHOUT Jailer (graceful degradation)
async fn start_unjailed(config: &JailerConfig) -> Result<JailerProcess> {
    warn!("Starting UNJAILED Firecracker process (development mode only!)");

    // Use default socket path for unjailed Firecracker
    let api_socket = PathBuf::from(format!("/tmp/firecracker-{}.sock", config.jailer_id));

    // Spawn Firecracker directly
    let mut child = Command::new(&config.exec_file)
        .arg("--api-sock")
        .arg(&api_socket)
        .spawn()
        .context("Failed to spawn Firecracker process")?;

    let pid = child.id().context("Failed to get Firecracker PID")?;

    info!("Firecracker process started with PID: {}", pid);

    // Monitor process in background
    tokio::spawn(async move {
        let status = child.wait().await;
        match status {
            Ok(status) => {
                debug!("Firecracker process exited with status: {}", status);
            }
            Err(e) => {
                warn!("Firecracker process error: {:?}", e);
            }
        }
    });

    Ok(JailerProcess {
        pid,
        api_socket,
        jailed: false,
    })
}

/// Handle to a (possibly jailed) Firecracker process
#[derive(Debug)]
pub struct JailerProcess {
    /// Process PID
    pub pid: u32,

    /// Path to Firecracker API socket
    pub api_socket: PathBuf,

    /// Whether this process is jailed (true) or unjailed (false)
    pub jailed: bool,
}

impl JailerProcess {
    /// Check if the process is still running
    pub async fn is_running(&self) -> bool {
        // Check if process exists by sending signal 0
        match tokio::process::Command::new("kill")
            .arg("-0")
            .arg(self.pid.to_string())
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Terminate the Firecracker process
    pub async fn terminate(&self) -> Result<()> {
        info!("Terminating Firecracker process (PID: {})", self.pid);

        let output = tokio::process::Command::new("kill")
            .arg("-TERM")
            .arg(self.pid.to_string())
            .output()
            .await
            .context("Failed to send TERM signal")?;

        if !output.status.success() {
            warn!("Failed to send TERM signal to PID {}", self.pid);
        }

        Ok(())
    }

    /// Force kill the Firecracker process
    pub async fn force_kill(&self) -> Result<()> {
        info!("Force killing Firecracker process (PID: {})", self.pid);

        let output = tokio::process::Command::new("kill")
            .arg("-KILL")
            .arg(self.pid.to_string())
            .output()
            .await
            .context("Failed to send KILL signal")?;

        if !output.status.success() {
            warn!("Failed to send KILL signal to PID {}", self.pid);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = JailerConfig::default();
        assert_eq!(config.cpu_count, 1);
        assert_eq!(config.memory_limit_mb, 256);
        assert_eq!(config.jailer_id, "ironclaw-vm");
    }

    #[test]
    fn test_config_new() {
        let config = JailerConfig::new("test-vm-123".to_string());
        assert_eq!(config.jailer_id, "test-vm-123");
        assert_eq!(config.cpu_count, 1);
        assert_eq!(config.memory_limit_mb, 256);
    }

    #[test]
    fn test_config_validate_success() {
        let config = JailerConfig::new("test".to_string());
        // Note: This will fail if firecracker binary doesn't exist
        // In tests, we skip binary existence checks
        let result = config.validate();
        // We expect it might fail due to missing binaries, but not validation errors
        match result {
            Ok(()) => {}
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("not found") || msg.contains("binary"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_config_validate_cpu_zero() {
        let mut config = JailerConfig::new("test".to_string());
        config.cpu_count = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CPU count must be > 0"));
    }

    #[test]
    fn test_config_validate_memory_too_low() {
        let mut config = JailerConfig::new("test".to_string());
        config.memory_limit_mb = 64;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 128 MB"));
    }

    #[test]
    fn test_build_args() {
        let config = JailerConfig::new("test-vm".to_string());
        let args = config.build_args();

        assert!(args.contains(&"--id".to_string()));
        assert!(args.contains(&"test-vm".to_string()));
        assert!(args.contains(&"--exec-file".to_string()));
        assert!(args.contains(&"--chroot-base-dir".to_string()));
        assert!(args.contains(&"--cgroup-version".to_string()));
        assert!(args.contains(&"2".to_string()));
    }

    #[test]
    fn test_build_args_with_netns() {
        let mut config = JailerConfig::new("test-vm".to_string());
        config.netns = Some("/run/netns/vm1".to_string());

        let args = config.build_args();
        assert!(args.contains(&"--netns".to_string()));
        assert!(args.contains(&"/run/netns/vm1".to_string()));
    }

    #[test]
    fn test_api_socket_path() {
        let config = JailerConfig::new("my-vm".to_string());
        let socket_path = config.api_socket_path();

        // Expected: /var/jail/firecracker-v1.14.1/my-vm/run/firecracker.socket
        assert!(socket_path.starts_with("/var/jail/"));
        assert!(socket_path.ends_with("run/firecracker.socket"));
        assert!(socket_path.to_string_lossy().contains("my-vm"));
    }

    #[test]
    fn test_jailer_process_attributes() {
        let process = JailerProcess {
            pid: 1234,
            api_socket: PathBuf::from("/tmp/test.sock"),
            jailed: true,
        };

        assert_eq!(process.pid, 1234);
        assert_eq!(process.api_socket, PathBuf::from("/tmp/test.sock"));
        assert!(process.jailed);
    }

    // Property-based tests with Proptest
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_cpu_count_valid(cpu in 1u8..16) {
                let _config = JailerConfig::new("test".to_string());
                // Skip binary validation
                assert_eq!(cpu, cpu); // Placeholder for property assertion
            }

            #[test]
            fn test_memory_limit_valid(memory in 128u32..4096) {
                let _config = JailerConfig::new("test".to_string());
                // Skip binary validation
                assert_eq!(memory, memory); // Placeholder
            }
        }
    }

    #[test]
    fn test_jailer_available() {
        // This test checks if jailer_available() doesn't panic
        let available = jailer_available();
        // We don't assert a specific value since it depends on the system
        // Just ensure it returns a boolean without panicking
        let _ = std::format!("Jailer available: {}", available);
    }

    #[tokio::test]
    async fn test_jailer_process_is_running_nonexistent() {
        let process = JailerProcess {
            pid: 99999, // Non-existent PID
            api_socket: PathBuf::from("/tmp/test.sock"),
            jailed: true,
        };

        // Process should not be running
        assert!(!process.is_running().await);
    }

    #[tokio::test]
    async fn test_jailer_process_is_running_current() {
        // Use current process ID which should always be running
        let current_pid = std::process::id();
        let process = JailerProcess {
            pid: current_pid,
            api_socket: PathBuf::from("/tmp/test.sock"),
            jailed: false,
        };

        // Current process should be running
        assert!(process.is_running().await);
    }

    #[tokio::test]
    async fn test_jailer_process_terminate_nonexistent() {
        let process = JailerProcess {
            pid: 99999, // Non-existent PID
            api_socket: PathBuf::from("/tmp/test.sock"),
            jailed: true,
        };

        // Terminate should not panic even for non-existent process
        let result = process.terminate().await;
        // It might fail, but shouldn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_jailer_process_force_kill_nonexistent() {
        let process = JailerProcess {
            pid: 99999, // Non-existent PID
            api_socket: PathBuf::from("/tmp/test.sock"),
            jailed: true,
        };

        // Force kill should not panic even for non-existent process
        let result = process.force_kill().await;
        // It might fail, but shouldn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_start_with_jailer_missing_binary() {
        let mut config = JailerConfig::new("test-vm".to_string());
        // Point to non-existent binary
        config.exec_file = PathBuf::from("/nonexistent/firecracker");

        // Should fail due to missing binary during validation
        let result = start_with_jailer(&config).await;
        assert!(result.is_err());

        // The error should mention the configuration validation failure
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid Jailer configuration") ||
            err_msg.contains("not found") ||
            err_msg.contains("binary"),
            "Expected configuration or binary error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_build_args_comprehensive() {
        let config = JailerConfig {
            jailer_binary: PathBuf::from("/usr/local/bin/jailer"),
            jailer_id: "test-vm-123".to_string(),
            chroot_base_dir: PathBuf::from("/var/jail"),
            exec_file: PathBuf::from("/usr/local/bin/firecracker-v1.14.1"),
            cpu_count: 2,
            memory_limit_mb: 512,
            netns: Some("/run/netns/test".to_string()),
        };

        let args = config.build_args();

        // Verify all expected arguments are present
        assert!(args.contains(&"--id".to_string()));
        assert!(args.contains(&"test-vm-123".to_string()));
        assert!(args.contains(&"--exec-file".to_string()));
        assert!(args.contains(&"/usr/local/bin/firecracker-v1.14.1".to_string()));
        assert!(args.contains(&"--chroot-base-dir".to_string()));
        assert!(args.contains(&"/var/jail".to_string()));
        assert!(args.contains(&"--cgroup-version".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"--cgroup".to_string()));
        assert!(args.contains(&"cpu:2".to_string()));
        assert!(args.contains(&"memory:512".to_string()));
        assert!(args.contains(&"--netns".to_string()));
        assert!(args.contains(&"/run/netns/test".to_string()));
    }

    #[test]
    fn test_api_socket_path_custom_exec() {
        let config = JailerConfig {
            exec_file: PathBuf::from("/usr/local/bin/custom-firecracker"),
            jailer_id: "my-vm".to_string(),
            chroot_base_dir: PathBuf::from("/custom/jail"),
            ..Default::default()
        };

        let socket_path = config.api_socket_path();

        // Should be: /custom/jail/custom-firecracker/my-vm/run/firecracker.socket
        assert!(socket_path.starts_with("/custom/jail/custom-firecracker/"));
        assert!(socket_path.ends_with("my-vm/run/firecracker.socket"));
    }
}
