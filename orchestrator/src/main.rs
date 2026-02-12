// IronClaw Orchestrator - Main Entry Point
//
// This is the Rust Orchestrator that manages:
// - CLI interface
// - JIT Micro-VM spawning
// - MCP client connections
// - Memory management
//
// Startup target: <500ms for new sessions

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

// Use the VM module from the library
use ironclaw_orchestrator::vm::{destroy_vm, spawn_vm};

/// IronClaw: Local-first Agentic AI Runtime
#[derive(Parser, Debug)]
#[command(name = "ironclaw")]
#[command(author = "IronClaw Contributors")]
#[command(version = "0.1.0")]
#[command(about = "Secure agentic AI runtime with JIT Micro-VMs", long_about = None)]
struct Args {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Agent command to run
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the agent interactively
    Run {
        /// Task description for the agent
        task: String,
    },
    /// Spawn a new JIT Micro-VM
    SpawnVm,
    /// Test MCP connection
    TestMcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize tracing
    let filter = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    tracing_subscriber::fmt()
        .with_max_level(filter)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(filter.into())
                .from_env_lossy(),
        )
        .init();

    info!("🦊 IronClaw Orchestrator v0.1.0 starting...");

    // Match commands
    match args.command {
        Some(Commands::Run { task }) => {
            info!("Running agent task: {}", task);
            run_agent(task).await?;
        }
        Some(Commands::SpawnVm) => {
            info!("Spawning JIT Micro-VM...");
            let handle = spawn_vm("cli-spawned-vm").await?;
            info!(
                "VM Spawned: ID={}, Time={:.2}ms",
                handle.id, handle.spawn_time_ms
            );

            // For CLI utility, we demonstrate lifecycle
            info!("Destroying VM...");
            destroy_vm(handle).await?;
        }
        Some(Commands::TestMcp) => {
            info!("Testing MCP connection...");
            test_mcp().await?;
        }
        None => {
            info!("No command specified. Use 'ironclaw --help' for usage.");
        }
    }

    Ok(())
}

/// Run the agent with the specified task
async fn run_agent(task: String) -> Result<()> {
    info!("🎯 Task: {}", task);
    // TODO: Implement agent execution
    // 1. Spawn JIT Micro-VM
    // 2. Launch Python reasoning loop inside VM
    // 3. Monitor execution
    // 4. Collect results
    println!("Agent execution placeholder for task: {}", task);
    Ok(())
}

/// Test MCP (Model Context Protocol) connection
async fn test_mcp() -> Result<()> {
    info!("🔌 Testing MCP connection...");
    // TODO: Implement MCP client
    // 1. Connect to MCP server
    // 2. List available tools
    // 3. Test tool execution
    println!("MCP connection test placeholder");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(["ironclaw", "run", "test task"]);
        assert!(matches!(args.command, Some(Commands::Run { .. })));
    }
}
