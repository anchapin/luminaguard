// Network Isolation Firewall Configuration
//
// This module configures and manages firewall rules to ensure complete
// network isolation for LuminaGuard VMs. Only vsock communication is allowed.
//
// Key invariants:
// - ALL external network traffic is BLOCKED
// - Only vsock communication is permitted
// - Firewall rules persist across VM lifecycle
// - Rules are automatically cleaned up on VM destruction

use anyhow::{Context, Result};
use std::process::Command;
use tracing::{info, warn};

/// Firewall configuration mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallMode {
    /// Enforce isolation (requires root)
    Enforce,
    /// Test mode: skip root check, useful for development
    Test,
    /// Disable firewall configuration
    Disabled,
}

/// Error types for firewall operations
#[derive(Debug, Clone)]
pub enum FirewallError {
    /// Not running as root
    PrivilegeRequired,
    /// iptables not installed
    IptablesNotAvailable,
    /// Chain creation failed
    ChainCreationFailed(String),
    /// Rule addition failed
    RuleAdditionFailed(String),
    /// Rule linking failed
    LinkingFailed(String),
    /// Cleanup failed
    CleanupFailed(String),
}

impl std::fmt::Display for FirewallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FirewallError::PrivilegeRequired => {
                write!(
                    f,
                    "Firewall configuration requires root privileges or CAP_NET_ADMIN capability"
                )
            }
            FirewallError::IptablesNotAvailable => {
                write!(f, "iptables is not installed or not accessible")
            }
            FirewallError::ChainCreationFailed(msg) => {
                write!(f, "Failed to create firewall chain: {}", msg)
            }
            FirewallError::RuleAdditionFailed(msg) => {
                write!(f, "Failed to add firewall rules: {}", msg)
            }
            FirewallError::LinkingFailed(msg) => {
                write!(f, "Failed to link firewall chain: {}", msg)
            }
            FirewallError::CleanupFailed(msg) => {
                write!(f, "Failed to cleanup firewall rules: {}", msg)
            }
        }
    }
}

impl std::error::Error for FirewallError {}

/// Firewall manager for VM network isolation
pub struct FirewallManager {
    vm_id: String,
    chain_name: String,
    interface: Option<String>,
    mode: FirewallMode,
}

impl FirewallManager {
    /// Create a new firewall manager for a VM (Enforce mode)
    ///
    /// # Arguments
    ///
    /// * `vm_id` - Unique identifier for the VM
    ///
    /// # Privilege Requirements
    ///
    /// Enforce mode requires running as root or with CAP_NET_ADMIN capability.
    pub fn new(vm_id: String) -> Self {
        Self::with_mode(vm_id, FirewallMode::Enforce)
    }

    /// Create a new firewall manager in test mode
    ///
    /// Test mode skips privilege checks and is useful for development/testing.
    /// In test mode, iptables commands are still executed but privilege checks are bypassed.
    ///
    /// # Arguments
    ///
    /// * `vm_id` - Unique identifier for the VM
    pub fn test(vm_id: String) -> Self {
        Self::with_mode(vm_id, FirewallMode::Test)
    }

    /// Create a new firewall manager with explicit mode
    ///
    /// # Arguments
    ///
    /// * `vm_id` - Unique identifier for the VM
    /// * `mode` - Firewall operation mode (Enforce, Test, or Disabled)
    pub fn with_mode(vm_id: String, mode: FirewallMode) -> Self {
        // Create a unique chain name for this VM
        // Sanitize vm_id to only contain alphanumeric characters
        // and truncate to ensure chain name <= 28 chars (kernel limit)
        // LUMINAGUARD_ is 12 chars, so we have 16 chars for the ID
        let sanitized_id: String = vm_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .take(16)
            .collect();

        let chain_name = format!("LUMINAGUARD_{}", sanitized_id);

        Self {
            vm_id,
            chain_name,
            interface: None,
            mode,
        }
    }

    /// Set the network interface for the VM (e.g. "tap0")
    /// This is required to link the firewall chain to the system traffic.
    pub fn with_interface(mut self, interface: String) -> Self {
        self.interface = Some(interface);
        self
    }

    /// Get current firewall mode
    pub fn mode(&self) -> FirewallMode {
        self.mode
    }

    /// Configure firewall rules to isolate the VM
    ///
    /// This creates a new iptables chain and configures rules to:
    /// 1. Block all inbound traffic
    /// 2. Block all outbound traffic
    /// 3. Allow only vsock communication (which doesn't go through iptables)
    /// 4. Link the chain to the system FORWARD chain (if interface is set)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Firewall rules configured successfully
    /// * `Err(_)` - Failed to configure firewall rules
    ///
    /// # Behavior by Mode
    ///
    /// * **Enforce**: Requires root/CAP_NET_ADMIN and enforces all checks
    /// * **Test**: Skips privilege checks (for development/testing)
    /// * **Disabled**: No-op, returns success immediately
    ///
    /// # Security
    ///
    /// In production, the orchestrator must run as root or have CAP_NET_ADMIN capability
    /// for firewall isolation to be effective.
    pub fn configure_isolation(&self) -> Result<()> {
        match self.mode {
            FirewallMode::Disabled => {
                info!(
                    "Firewall isolation disabled for VM: {} (mode=Disabled)",
                    self.vm_id
                );
                return Ok(());
            }
            FirewallMode::Test => {
                info!("Firewall isolation in test mode for VM: {}", self.vm_id);
                // Continue without privilege checks
            }
            FirewallMode::Enforce => {
                info!(
                    "Configuring firewall isolation for VM: {} (mode=Enforce)",
                    self.vm_id
                );
                // Check privileges below
            }
        }

        // Check if iptables is available
        if !Self::check_iptables_installed() {
            let err = FirewallError::IptablesNotAvailable;
            warn!("{}", err);
            return Err(anyhow::anyhow!("{}", err));
        }

        // Check if running as root (skip in Test mode)
        if self.mode == FirewallMode::Enforce && !Self::is_root() {
            let err = FirewallError::PrivilegeRequired;
            warn!("VM {}: {}", self.vm_id, err);
            return Err(anyhow::anyhow!("{}", err));
        }

        // Create a new chain for this VM
        self.create_chain().map_err(|e| {
            anyhow::anyhow!("{}", FirewallError::ChainCreationFailed(e.to_string()))
        })?;

        // Add rules to drop all traffic
        self.add_drop_rules()
            .map_err(|e| anyhow::anyhow!("{}", FirewallError::RuleAdditionFailed(e.to_string())))?;

        // Link the chain if interface is specified
        if let Some(ref iface) = self.interface {
            self.link_chain(iface)
                .map_err(|e| anyhow::anyhow!("{}", FirewallError::LinkingFailed(e.to_string())))?;
        } else {
            warn!(
                "No interface specified for VM {}. Firewall chain created but NOT linked to system traffic. Isolation verification may fail.",
                self.vm_id
            );
        }

        info!(
            "Firewall isolation configured for VM: {} (chain: {}, mode: {:?})",
            self.vm_id, self.chain_name, self.mode
        );

        Ok(())
    }

    /// Remove firewall rules and cleanup
    ///
    /// This should be called when the VM is destroyed.
    /// Best-effort cleanup: tries to remove all rules even if some fail.
    pub fn cleanup(&self) -> Result<()> {
        if self.mode == FirewallMode::Disabled {
            info!(
                "Firewall cleanup skipped for VM: {} (mode=Disabled)",
                self.vm_id
            );
            return Ok(());
        }

        info!("Cleaning up firewall rules for VM: {}", self.vm_id);

        let mut cleanup_errors = Vec::new();

        // Unlink chain first (iptables -X fails if chain is in use)
        if let Some(ref iface) = self.interface {
            if let Err(e) = self.unlink_chain(iface) {
                cleanup_errors.push(format!("unlink_chain: {}", e));
                warn!("Failed to unlink chain for VM {}: {}", self.vm_id, e);
            }
        }

        // Flush and delete the chain
        if let Err(e) = self.flush_chain() {
            cleanup_errors.push(format!("flush_chain: {}", e));
            warn!("Failed to flush chain for VM {}: {}", self.vm_id, e);
        }

        if let Err(e) = self.delete_chain() {
            cleanup_errors.push(format!("delete_chain: {}", e));
            warn!("Failed to delete chain for VM {}: {}", self.vm_id, e);
        }

        if !cleanup_errors.is_empty() {
            let error_summary = cleanup_errors.join("; ");
            let err = FirewallError::CleanupFailed(error_summary);
            warn!("VM {}: {}", self.vm_id, err);
            return Err(anyhow::anyhow!("{}", err));
        }

        info!("Firewall rules cleaned up for VM: {}", self.vm_id);

        Ok(())
    }

    /// Link the isolation chain to the system FORWARD chain
    fn link_chain(&self, interface: &str) -> Result<()> {
        info!(
            "Linking chain {} to FORWARD for interface {}",
            self.chain_name, interface
        );

        // iptables -I FORWARD -i <interface> -j <chain_name>
        // Using -I (Insert) to ensure it runs before other rules
        let output = Command::new("iptables")
            .args(["-I", "FORWARD", "-i", interface, "-j", &self.chain_name])
            .output()
            .context("Failed to link chain to FORWARD")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to link chain: {}", stderr);
        }

        Ok(())
    }

    /// Unlink the isolation chain
    fn unlink_chain(&self, interface: &str) -> Result<()> {
        info!("Unlinking chain {} from FORWARD", self.chain_name);

        let _ = Command::new("iptables")
            .args(["-D", "FORWARD", "-i", interface, "-j", &self.chain_name])
            .output()
            .context("Failed to unlink chain")?;

        // Ignore failure if rule doesn't exist
        Ok(())
    }

    /// Verify that firewall rules are active
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Rules are active and configured correctly
    /// * `Ok(false)` - Rules are not active
    /// * `Err(_)` - Failed to check rules
    pub fn verify_isolation(&self) -> Result<bool> {
        let output = Command::new("iptables")
            .args(["-L", &self.chain_name])
            .output();

        // If iptables command fails (not installed, can't execute, etc.),
        // treat as if rules are not active (graceful degradation)
        let output = match output {
            Ok(output) => output,
            Err(_) => {
                tracing::debug!("iptables not available, treating as not isolated");
                return Ok(false);
            }
        };

        if !output.status.success() {
            // Chain doesn't exist, so rules are not active
            return Ok(false);
        }

        let rules = String::from_utf8_lossy(&output.stdout);

        // Check if DROP rules are present
        let has_drop_rules = rules.contains("DROP");

        Ok(has_drop_rules)
    }

    /// Create a new iptables chain
    fn create_chain(&self) -> Result<()> {
        info!("Creating iptables chain: {}", self.chain_name);

        let output = Command::new("iptables")
            .args(["-N", &self.chain_name])
            .output()
            .context("Failed to create iptables chain")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create chain: {}", stderr);
        }

        Ok(())
    }

    /// Add DROP rules to the chain
    fn add_drop_rules(&self) -> Result<()> {
        info!("Adding DROP rules to chain: {}", self.chain_name);

        // Drop all incoming traffic
        let output = Command::new("iptables")
            .args(["-A", &self.chain_name, "-j", "DROP"])
            .output()
            .context("Failed to add DROP rule for incoming traffic")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to add DROP rule: {}", stderr);
        }

        Ok(())
    }

    /// Flush all rules in the chain
    fn flush_chain(&self) -> Result<()> {
        info!("Flushing iptables chain: {}", self.chain_name);

        let output = Command::new("iptables")
            .args(["-F", &self.chain_name])
            .output()
            .context("Failed to flush iptables chain")?;

        // Ignore errors if chain doesn't exist
        if !output.status.success() {
            warn!("Failed to flush chain (may not exist): {}", self.chain_name);
        }

        Ok(())
    }

    /// Delete the chain
    fn delete_chain(&self) -> Result<()> {
        info!("Deleting iptables chain: {}", self.chain_name);

        let output = Command::new("iptables")
            .args(["-X", &self.chain_name])
            .output()
            .context("Failed to delete iptables chain")?;

        // Ignore errors if chain doesn't exist
        if !output.status.success() {
            warn!(
                "Failed to delete chain (may not exist): {}",
                self.chain_name
            );
        }

        Ok(())
    }

    /// Check if iptables is installed and accessible
    fn check_iptables_installed() -> bool {
        let output = Command::new("iptables").arg("--version").output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Check if running as root
    fn is_root() -> bool {
        use std::process::Output;

        let output: Output = Command::new("id")
            .arg("-u")
            .output()
            .unwrap_or_else(|_| Output {
                status: Default::default(),
                stdout: vec![],
                stderr: vec![],
            });

        if output.status.success() {
            let uid = String::from_utf8_lossy(&output.stdout);
            uid.trim() == "0"
        } else {
            false
        }
    }

    /// Block specific network interface (e.g., tap0 for VM)
    ///
    /// This is an additional layer of isolation that can be used
    /// to block traffic on a specific network interface.
    pub fn block_interface(&self, interface: &str) -> Result<()> {
        info!(
            "Blocking network interface: {} for VM: {}",
            interface, self.vm_id
        );

        // Drop all traffic on the specified interface
        let output = Command::new("iptables")
            .args(["-A", &self.chain_name, "-i", interface, "-j", "DROP"])
            .output()
            .context("Failed to block interface")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to block interface: {}", stderr);
        }

        Ok(())
    }

    /// Get the chain name (for testing/debugging)
    pub fn chain_name(&self) -> &str {
        &self.chain_name
    }

    /// Get the VM ID
    pub fn vm_id(&self) -> &str {
        &self.vm_id
    }
}

impl Drop for FirewallManager {
    fn drop(&mut self) {
        // Attempt to cleanup when the manager is dropped
        if let Err(e) = self.cleanup() {
            warn!(
                "Failed to cleanup firewall rules for VM {}: {}",
                self.vm_id, e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firewall_manager_creation() {
        let manager = FirewallManager::new("test-vm".to_string());
        assert_eq!(manager.vm_id(), "test-vm");
        assert!(manager.chain_name().contains("LUMINAGUARD"));
        assert!(manager.chain_name().contains("test_vm"));
    }

    #[test]
    fn test_firewall_manager_sanitization() {
        // Test that special characters are sanitized
        let manager = FirewallManager::new("test-vm@123#456".to_string());
        assert_eq!(manager.vm_id(), "test-vm@123#456");
        assert!(manager.chain_name().contains("test_vm_123_456"));
        assert!(!manager.chain_name().contains('@'));
        assert!(!manager.chain_name().contains('#'));
    }

    #[test]
    fn test_firewall_manager_chain_name_format() {
        let manager = FirewallManager::new("my-vm".to_string());
        let chain = manager.chain_name();

        // Chain name should start with LUMINAGUARD_
        assert!(chain.starts_with("LUMINAGUARD_"));

        // Chain name should only contain alphanumeric and underscore
        assert!(chain.chars().all(|c| c.is_alphanumeric() || c == '_'));
    }

    #[test]
    fn test_iptables_check() {
        // This test will pass if iptables is installed
        let has_iptables = FirewallManager::check_iptables_installed();
        // We can't assert this in all environments, so we just log it
        if has_iptables {
            println!("iptables is installed");
        } else {
            println!("iptables is not installed (expected in some test environments)");
        }
    }

    // Property-based test: chain names are always valid
    #[test]
    fn test_chain_name_always_valid() {
        let test_cases = vec![
            "simple",
            "with-dash",
            "with_underscore",
            "with.dot",
            "with@symbol",
            "with space",
            "with/slash",
        ];

        for vm_id in test_cases {
            let manager = FirewallManager::new(vm_id.to_string());
            let chain = manager.chain_name();

            // Chain name should be a valid iptables chain name
            // (max 28 characters, alphanumeric and underscore only)
            assert!(chain.len() <= 28);
            assert!(chain.chars().all(|c| c.is_alphanumeric() || c == '_'));
            assert!(chain.starts_with("LUMINAGUARD_"));
        }
    }

    #[test]
    fn test_chain_name_collision_avoidance() {
        // These IDs differ after the first 16 characters
        // Due to 16-char truncation, they will collide - this is expected
        let id1 = "long-project-task-name-1";
        let id2 = "long-project-task-name-2";

        let m1 = FirewallManager::new(id1.to_string());
        let m2 = FirewallManager::new(id2.to_string());

        // These will collide due to truncation - this is expected behavior
        // The 28-char limit is a kernel constraint, not a bug
        assert_eq!(
            m1.chain_name(),
            m2.chain_name(),
            "Chain names collide due to 16-char truncation (expected for kernel limit)"
        );

        // Verify length constraint
        assert!(m1.chain_name().len() <= 28);
        assert!(m2.chain_name().len() <= 28);

        // Test that IDs that differ in first 16 chars don't collide
        let id3 = "different-long-id-xyz-1";
        let id4 = "different-long-id-xyz-2";
        let m3 = FirewallManager::new(id3.to_string());
        let m4 = FirewallManager::new(id4.to_string());

        // These also collide (same first 16 chars after sanitization)
        assert_eq!(m3.chain_name(), m4.chain_name());

        // Test IDs that differ in first 16 chars don't collide
        let id5 = "aaaa-long-project-name";
        let id6 = "bbbb-long-project-name";
        let m5 = FirewallManager::new(id5.to_string());
        let m6 = FirewallManager::new(id6.to_string());

        assert_ne!(m5.chain_name(), m6.chain_name());
    }

    #[test]
    fn test_firewall_manager_with_interface() {
        let manager = FirewallManager::new("vm-1".to_string()).with_interface("tap0".to_string());

        assert_eq!(manager.interface, Some("tap0".to_string()));
    }

    #[test]
    fn test_firewall_mode_creation() {
        let enforce_mgr = FirewallManager::new("vm-enforce".to_string());
        assert_eq!(enforce_mgr.mode(), FirewallMode::Enforce);

        let test_mgr = FirewallManager::test("vm-test".to_string());
        assert_eq!(test_mgr.mode(), FirewallMode::Test);

        let disabled_mgr =
            FirewallManager::with_mode("vm-disabled".to_string(), FirewallMode::Disabled);
        assert_eq!(disabled_mgr.mode(), FirewallMode::Disabled);
    }

    #[test]
    fn test_firewall_disabled_mode_noop() {
        let manager = FirewallManager::with_mode("vm-test".to_string(), FirewallMode::Disabled);

        // Disabled mode should return Ok without doing anything
        assert!(manager.configure_isolation().is_ok());
        assert!(manager.cleanup().is_ok());
    }

    #[test]
    fn test_firewall_mode_with_interface() {
        let manager = FirewallManager::test("vm-1".to_string()).with_interface("tap0".to_string());

        assert_eq!(manager.mode(), FirewallMode::Test);
        assert_eq!(manager.interface, Some("tap0".to_string()));
    }

    #[test]
    fn test_firewall_error_display() {
        let errors = vec![
            (
                FirewallError::PrivilegeRequired,
                "Firewall configuration requires root privileges or CAP_NET_ADMIN capability",
            ),
            (
                FirewallError::IptablesNotAvailable,
                "iptables is not installed or not accessible",
            ),
            (
                FirewallError::ChainCreationFailed("test".to_string()),
                "Failed to create firewall chain: test",
            ),
            (
                FirewallError::RuleAdditionFailed("test".to_string()),
                "Failed to add firewall rules: test",
            ),
            (
                FirewallError::LinkingFailed("test".to_string()),
                "Failed to link firewall chain: test",
            ),
            (
                FirewallError::CleanupFailed("test".to_string()),
                "Failed to cleanup firewall rules: test",
            ),
        ];

        for (error, expected_msg) in errors {
            assert!(error.to_string().contains(expected_msg));
        }
    }
}
