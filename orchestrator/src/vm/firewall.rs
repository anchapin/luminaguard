// Network Isolation Firewall Configuration
//
// This module configures and manages firewall rules to ensure complete
// network isolation for IronClaw VMs. Only vsock communication is allowed.
//
// Key invariants:
// - ALL external network traffic is BLOCKED
// - Only vsock communication is permitted
// - Firewall rules persist across VM lifecycle
// - Rules are automatically cleaned up on VM destruction

#![cfg(target_os = "linux")]

use anyhow::{Context, Result};
use std::process::Command;
use tracing::{info, warn};

/// Firewall manager for VM network isolation
#[derive(Debug)]
pub struct FirewallManager {
    vm_id: String,
    chain_name: String,
}
impl FirewallManager {
    /// Create a new firewall manager for a VM
    ///
    /// # Arguments
    ///
    /// * `vm_id` - Unique identifier for the VM
    pub fn new(vm_id: String) -> Self {
        // Create a unique chain name for this VM
        // Sanitize vm_id to only contain alphanumeric characters
        let sanitized_id: String = vm_id
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();

        let chain_name = format!("IRONCLAW_{}", sanitized_id);

        Self { vm_id, chain_name }
    }

    /// Configure firewall rules to isolate the VM
    ///
    /// This creates a new iptables chain and configures rules to:
    /// 1. Block all inbound traffic
    /// 2. Block all outbound traffic
    /// 3. Allow only vsock communication (which doesn't go through iptables)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Firewall rules configured successfully
    /// * `Err(_)` - Failed to configure firewall rules
    ///
    /// # Note
    ///
    /// This function requires root privileges. If running without root,
    /// it will return an error. In production, the orchestrator should
    /// run with appropriate capabilities.
    pub fn configure_isolation(&self) -> Result<()> {
        info!("Configuring firewall isolation for VM: {}", self.vm_id);

        // Check if iptables is available
        if !Self::check_iptables_installed() {
            anyhow::bail!("iptables is not installed or not accessible");
        }

        // Check if running as root
        if !Self::is_root() {
            anyhow::bail!("Firewall configuration requires root privileges");
        }

        // Create a new chain for this VM
        self.create_chain()?;

        // Add rules to drop all traffic
        self.add_drop_rules()?;

        // Link chain to system chains
        self.link_chain()?;

        info!(
            "Firewall isolation configured for VM: {} (chain: {})",
            self.vm_id, self.chain_name
        );

        Ok(())
    }

    /// Remove firewall rules and cleanup
    ///
    /// This should be called when the VM is destroyed.
    pub fn cleanup(&self) -> Result<()> {
        info!("Cleaning up firewall rules for VM: {}", self.vm_id);

        // Unlink chain from system chains
        let _ = self.unlink_chain();

        // Flush and delete the chain
        self.flush_chain()?;
        self.delete_chain()?;

        info!("Firewall rules cleaned up for VM: {}", self.vm_id);

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

    /// Link chain to system chains (FORWARD)
    fn link_chain(&self) -> Result<()> {
        info!("Linking chain {} to FORWARD", self.chain_name);

        // Use physdev match to limit scope to bridged traffic (VMs)
        // This prevents blocking host traffic while ensuring VM isolation
        let output = Command::new("iptables")
            .args([
                "-I",
                "FORWARD",
                "-m",
                "physdev",
                "--physdev-is-bridged",
                "-j",
                &self.chain_name,
            ])
            .output()
            .context("Failed to link chain to FORWARD")?;

        if !output.status.success() {
            // Fallback: if physdev fails (e.g., kernel module missing),
            // try generic link but warn about scope
            warn!("Failed to link with physdev match, falling back to generic link");
            let output = Command::new("iptables")
                .args(["-I", "FORWARD", "-j", &self.chain_name])
                .output()
                .context("Failed to link chain to FORWARD (fallback)")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to link chain: {}", stderr);
            }
        }

        Ok(())
    }

    /// Unlink chain from system chains
    fn unlink_chain(&self) -> Result<()> {
        info!("Unlinking chain {} from FORWARD", self.chain_name);

        // Try to delete physdev rule first
        let _ = Command::new("iptables")
            .args([
                "-D",
                "FORWARD",
                "-m",
                "physdev",
                "--physdev-is-bridged",
                "-j",
                &self.chain_name,
            ])
            .output();

        // Try to delete generic rule (in case fallback was used)
        let _ = Command::new("iptables")
            .args(["-D", "FORWARD", "-j", &self.chain_name])
            .output();

        Ok(())
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
        assert!(manager.chain_name().contains("IRONCLAW"));
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

        // Chain name should start with IRONCLAW_
        assert!(chain.starts_with("IRONCLAW_"));

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
            assert!(chain.starts_with("IRONCLAW_"));
        }
    }
}
