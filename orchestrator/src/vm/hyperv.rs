use anyhow::{anyhow, Result};
use async_trait::async_trait;
#[cfg(windows)]
use libwhp::{
    Partition,
    WHV_PARTITION_PROPERTY,
    WHV_PARTITION_PROPERTY_CODE,
};
use std::time::Instant;
#[cfg(windows)]
use std::sync::mpsc;
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

/// Commands for the background Hyper-V thread
#[cfg(windows)]
enum HypervCommand {
    Stop,
}

/// Hyper-V (WHPX) VM instance
pub struct HypervInstance {
    pub id: String,
    pub spawn_time_ms: f64,
    // We use a sender to communicate with the background thread that owns the Partition.
    // This decouples the !Send Partition from the VmInstance, making VmInstance Send + Sync.
    #[cfg(windows)]
    sender: mpsc::Sender<HypervCommand>,
}

#[cfg(windows)]
impl HypervInstance {
    pub fn new(config: &VmConfig) -> Result<Self> {
        let start_time = Instant::now();
        info!("Starting Hyper-V (WHPX) VM: {}", config.vm_id);

        let vm_id = config.vm_id.clone();
        let vcpu_count = config.vcpu_count;

        // Create channels for communication
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (init_tx, init_rx) = mpsc::channel();

        // Spawn a background thread to own the Partition
        // This thread will handle initialization and the message loop.
        std::thread::spawn(move || {
            // 1. Create WHPX partition
            let mut partition = match Partition::new() {
                Ok(p) => p,
                Err(e) => {
                    let _ = init_tx.send(Err(anyhow!("Failed to create WHPX partition: {:?}", e)));
                    return;
                }
            };

            // 2. Configure partition
            let vcpu_count_u32 = vcpu_count as u32;
            let partition_property = WHV_PARTITION_PROPERTY {
                ProcessorCount: vcpu_count_u32,
            };

            if let Err(e) = partition.set_property(
                WHV_PARTITION_PROPERTY_CODE::WHvPartitionPropertyCodeProcessorCount,
                &partition_property,
            ) {
                let _ = init_tx.send(Err(anyhow!("Failed to set vCPU count: {:?}", e)));
                return;
            }

            // TODO: Map memory, setup vCPUs, load kernel/rootfs
            // This is a complex process in WHPX and requires a full VMM implementation.

            if let Err(e) = partition.setup() {
                 let _ = init_tx.send(Err(anyhow!("Failed to setup WHPX partition: {:?}", e)));
                 return;
            }

            // Initialization successful
            if init_tx.send(Ok(())).is_err() {
                // Main thread died?
                return;
            }

            // 3. Message Loop
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    HypervCommand::Stop => {
                        info!("Stopping Hyper-V partition thread for {}", vm_id);
                        break; // Breaking the loop drops the partition
                    }
                }
            }
        });

        // Wait for initialization result from the thread
        match init_rx.recv() {
            Ok(Ok(())) => {
                let elapsed = start_time.elapsed();
                let spawn_time_ms = elapsed.as_secs_f64() * 1000.0;

                Ok(Self {
                    id: config.vm_id.clone(),
                    spawn_time_ms,
                    sender: cmd_tx,
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("Hyper-V background thread panicked or exited early")),
        }
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
        #[cfg(windows)]
        {
            // Send stop command to background thread
            // We use standard mpsc, so send is synchronous but non-blocking for unbounded channels
            self.sender.send(HypervCommand::Stop)
                .map_err(|_| anyhow!("Failed to send stop command to Hyper-V thread"))?;
        }
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
