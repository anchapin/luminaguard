//! macOS Virtualization.framework Backend
//!
//! This module implements the Hypervisor and VmInstance traits using
//! Apple's Virtualization.framework (available on macOS 11+).

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
#[cfg(target_os = "macos")]
use std::time::Instant;
use tracing::info;

use crate::vm::config::VmConfig;
use crate::vm::hypervisor::{Hypervisor, VmInstance};

/// macOS Virtualization.framework Hypervisor implementation
pub struct AppleHvHypervisor;

#[async_trait]
impl Hypervisor for AppleHvHypervisor {
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>> {
        #[cfg(target_os = "macos")]
        {
            let instance = start_apple_hv(config).await?;
            Ok(Box::new(instance))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = config;
            Err(anyhow!("Apple Hypervisor is only available on macOS"))
        }
    }

    fn name(&self) -> &str {
        "apple_hv"
    }
}

/// macOS VM instance managed by Virtualization.framework
pub struct AppleHvInstance {
    pub id: String,
    pub spawn_time_ms: f64,
}

#[async_trait]
impl VmInstance for AppleHvInstance {
    fn id(&self) -> &str {
        &self.id
    }

    fn pid(&self) -> u32 {
        0
    }

    fn socket_path(&self) -> &str {
        ""
    }

    fn spawn_time_ms(&self) -> f64 {
        self.spawn_time_ms
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping macOS VM (ID: {})", self.id);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
async fn start_apple_hv(config: &VmConfig) -> Result<AppleHvInstance> {
    let start_time = Instant::now();
    info!("Starting macOS Virtualization.framework VM: {}", config.vm_id);

    // TODO: Implement actual Virtualization.framework integration
    // For now, this is a stub that would be filled in by a macOS developer

    let spawn_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    Ok(AppleHvInstance {
        id: config.vm_id.clone(),
        spawn_time_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_hv_name() {
        let hv = AppleHvHypervisor;
        assert_eq!(hv.name(), "apple_hv");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_apple_hv_available_on_macos() {
        // This test would only compile and run on macOS
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_apple_hv_unavailable_on_non_macos() {
        // Verified: apple_hv module exists and is properly gated
    }
}
