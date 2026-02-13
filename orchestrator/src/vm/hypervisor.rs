use anyhow::Result;
use async_trait::async_trait;
use crate::vm::config::VmConfig;

/// Unified VM abstraction layer
/// 
/// This trait defines the interface for different hypervisors (Firecracker, Jailer, etc.)
#[async_trait]
pub trait Hypervisor: Send + Sync {
    /// Spawn a new VM instance with the given configuration
    async fn spawn(&self, config: &VmConfig) -> Result<Box<dyn VmInstance>>;
    
    /// Returns the name of the hypervisor (e.g., "firecracker", "jailer")
    fn name(&self) -> &str;
}

/// A running VM instance
#[async_trait]
pub trait VmInstance: Send + Sync {
    /// Get the unique ID of the VM instance
    fn id(&self) -> &str;
    
    /// Get the PID of the VM process
    fn pid(&self) -> u32;
    
    /// Get the path to the API socket
    fn socket_path(&self) -> &str;
    
    /// Get the spawn time in milliseconds
    fn spawn_time_ms(&self) -> f64;
    
    /// Stop the VM instance
    async fn stop(&mut self) -> Result<()>;
}
