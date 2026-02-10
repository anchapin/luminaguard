//! IronClaw Orchestrator Library
//!
//! This library provides the core functionality for the IronClaw Orchestrator,
//! including MCP client implementation, VM spawning, and memory management.

pub mod mcp;
#[cfg(unix)]
pub mod vm;

#[cfg(not(unix))]
pub mod vm {
    use anyhow::{anyhow, Result};

    pub struct VmHandle {
        pub id: String,
        pub spawn_time_ms: f64,
    }

    pub async fn spawn_vm(task_id: &str) -> Result<VmHandle> {
        Err(anyhow!("VM spawning is only supported on Unix-like systems"))
    }

    pub async fn destroy_vm(_handle: VmHandle) -> Result<()> {
        Ok(())
    }
}
