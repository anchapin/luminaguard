// Firecracker Jailer Integration
//
// This module provides integration with the Firecracker Jailer for enhanced
// process sandboxing and security isolation.
//
// The Jailer creates a chroot jail for each Firecracker process, providing:
// - chroot filesystem isolation
// - cgroup resource limits (CPU, memory)
// - Network namespace isolation
// - UID/GID privilege separation
// - Mount namespace isolation via pivot_root

pub mod config;
pub mod process;

#[cfg(test)]
mod tests;

pub use config::JailerConfig;
pub use process::{start_jailed_firecracker, stop_jailed_firecracker, JailerProcess};

use anyhow::Result;
use std::path::Path;

/// Verify that jailer binary is available
pub fn verify_jailer_installed() -> Result<()> {
    let jailer_path = std::path::PathBuf::from("/usr/local/bin/jailer");

    if !jailer_path.exists() {
        // Try alternative paths
        let alternative_paths = vec![
            "/usr/bin/jailer",
            "/opt/firecracker/bin/jailer",
            "./jailer",
        ];

        for path in alternative_paths {
            if Path::new(path).exists() {
                return Ok(());
            }
        }

        anyhow::bail!(
            "Jailer binary not found. Searched in: {:?}. \
            Install Firecracker from: https://github.com/firecracker-microvm/firecracker",
            vec![jailer_path]
        );
    }

    Ok(())
}

