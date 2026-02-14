use anyhow::{anyhow, Result};
use async_trait::async_trait;
use libwhp::{
    Partition,
    VirtualProcessor,
    WHV_PARTITION_PROPERTY,
    WHV_PARTITION_PROPERTY_CODE,
};
use std::time::Instant;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::vm::config::VmConfig;
use crate::vm::hypervisor::{Hypervisor, VmInstance};

/// Hyper-V (WHPX) Hypervisor implementation
pub struct HypervHypervisor;

#[async_trait]
impl Hypervisor for HypervHypervisor {
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>> {
        #[cfg(windows)]
        {
            let instance = HypervInstance::new(config)?;
            Ok(Box::new(instance))
        }
        #[cfg(not(windows))]{
            let _ = config;
            Err(anyhow!("Hyper-V backend is only available on Windows"))
        }
    }

    fn name(&self) -> &str {
        "hyperv"
    }
}

/// Hyper-V (WHPX) VM instance
pub struct HypervInstance {
    pub id: String,
    pub spawn_time_ms: f64,
    #[cfg(windows)]
    partition: Arc<Mutex<Partition>>,
}

#[cfg(windows)]
impl HypervInstance {
    pub fn new(config: &VmConfig) -> Result<Self> {
        let start_time = Instant::now();
        info!("Starting Hyper-V (WHPX) VM: {}", config.vm_id);

        // 1. Create WHPX partition
        let mut partition = Partition::new().map_err(|e| anyhow!("Failed to create WHPX partition: {:?}", e))?;

        // 2. Configure partition
        let vcpu_count_u32 = config.vcpu_count as u32;
        let partition_property = WHV_PARTITION_PROPERTY {
            ProcessorCount: vcpu_count_u32,
        };
        partition
            .set_property(
                WHV_PARTITION_PROPERTY_CODE::WHV_PARTITION_PROPERTY_CODE_PROCESSOR_COUNT,
                &partition_property,
            )
            .map_err(|e| anyhow!("Failed to set vCPU count: {:?}", e))?;

        // TODO: Map memory, setup vCPUs, load kernel/rootfs
        // This is a complex process in WHPX and requires a full VMM implementation.
        // For now, we provide the skeletal implementation of the traits.

        partition.setup().map_err(|e| anyhow!("Failed to setup WHPX partition: {:?}", e))?;

        let elapsed = start_time.elapsed();
        let spawn_time_ms = elapsed.as_secs_f64() * 1000.0;

        Ok(Self {
            id: config.vm_id.clone(),
            spawn_time_ms,
            #[cfg(windows)]
            partition: Arc::new(Mutex::new(partition)),
        })
    }
}


#[async_trait]
impl VmInstance for HypervInstance {
    fn id(&self) -> &str {
        &self.id
    }

    fn pid(&self) -> u32 {
        // In WHPX, the "PID" is the orchestrator process itself as it's a library-based hypervisor,
        // but we could return a placeholder or an actual OS-level handle if applicable.
        std::process::id()
    }

    fn socket_path(&self) -> &str {
        // Hyper-V backend might use named pipes or vsock on Windows
        ""
    }

    fn spawn_time_ms(&self) -> f64 {
        self.spawn_time_ms
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Hyper-V VM (ID: {})", self.id);
        // The partition is automatically terminated when the Arc<Mutex<Partition>> is dropped.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::config::VmConfig;

    #[tokio::test]
    async fn test_hyperv_hypervisor_name() {
        let hv = HypervHypervisor;
        assert_eq!(hv.name(), "hyperv");
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn test_hyperv_spawn_fails_on_linux() {
        let hv = HypervHypervisor;
        let config = VmConfig::new("test".to_string());
        let result = hv.spawn(&config).await;
        assert!(result.is_err());
        match result {
            Err(e) => assert_eq!(e.to_string(), "Hyper-V backend is only available on Windows"),
            Ok(_) => panic!("Should have failed"),
        }
    }
}

