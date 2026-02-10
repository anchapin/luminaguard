// IronClaw Orchestrator - Main Entry Point
//
// This is the Rust Orchestrator that manages:
// - CLI interface
// - JIT Micro-VM spawning
// - MCP client connections
// - Memory management
//
// Startup target: <500ms for new sessions

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ironclaw_orchestrator::mcp::{McpClient, StdioTransport};
use serde_json::json;
use tracing::{error, info, Level};
use tracing_subscriber::EnvFilter;

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
    TestMcp {
        /// Command to spawn the MCP server (default: "npx" with filesystem server)
        #[arg(long)]
        command: Option<String>,

        /// Arguments for the MCP server
        #[arg(long, num_args = 0.., value_delimiter = ' ', allow_hyphen_values = true)]
        args: Vec<String>,

        /// Only list tools, do not call any
        #[arg(long)]
        list_tools: bool,
    },
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
            spawn_vm().await?;
        }
        Some(Commands::TestMcp {
            command,
            args,
            list_tools,
        }) => {
            info!("Testing MCP connection...");
            test_mcp(command, args, list_tools).await?;
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

/// Spawn a JIT Micro-VM
/// Target: <200ms spawn time
async fn spawn_vm() -> Result<()> {
    info!("⚡ Spawning JIT Micro-VM...");
    // TODO: Implement Firecracker VM spawning
    // 1. Create VM configuration
    // 2. Load kernel image
    // 3. Configure network (if needed)
    // 4. Start VM
    // 5. Verify startup time <200ms
    println!("VM spawning placeholder");
    Ok(())
}

/// Test MCP (Model Context Protocol) connection
async fn test_mcp(command: Option<String>, args: Vec<String>, list_tools_only: bool) -> Result<()> {
    // Determine command and args
    let (cmd, cmd_args) = if let Some(c) = command {
        (c, args)
    } else if !args.is_empty() {
        ("npx".to_string(), args)
    } else {
        // Default to npx filesystem server
        // Using `.` as the allowed directory so we can read Cargo.toml
        (
            "npx".to_string(),
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                ".".to_string(),
            ],
        )
    };

    // Prepare string slices for spawn
    let args_slices: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();

    info!("🔌 Connecting to MCP server: {} {:?}", cmd, args_slices);

    // 1. Connect to MCP server
    let transport = match StdioTransport::spawn(&cmd, &args_slices).await {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to spawn MCP server '{}': {}", cmd, e);
            if cmd == "npx" {
                info!("Tip: Make sure Node.js and npx are installed and available in your PATH.");
            }
            return Err(e);
        }
    };

    let mut client = McpClient::new(transport);

    info!("Initializing MCP client...");
    client
        .initialize()
        .await
        .context("Failed to initialize MCP client")?;

    info!("MCP client initialized successfully!");
    if let Some(caps) = client.server_capabilities() {
        info!(
            "Server: {} v{}",
            caps.server_info.name, caps.server_info.version
        );
    }

    // 2. List available tools
    info!("Listing available tools...");
    let tools = client.list_tools().await.context("Failed to list tools")?;

    info!("Found {} tools:", tools.len());
    for tool in &tools {
        info!("  - {}: {}", tool.name, tool.description);
    }

    if list_tools_only {
        return Ok(());
    }

    // 3. Test tool execution
    // If using the default filesystem server, try to read Cargo.toml
    if cmd == "npx" && tools.iter().any(|t| t.name == "read_file") {
        info!("Testing 'read_file' tool with Cargo.toml...");
        match client
            .call_tool("read_file", json!({"path": "Cargo.toml"}))
            .await
        {
            Ok(result) => {
                info!("Tool execution successful!");
                // The result from read_file usually contains "content"
                if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                    if let Some(first) = content.first() {
                        if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                            // Print first few lines
                            let preview: String =
                                text.lines().take(5).collect::<Vec<_>>().join("\n");
                            println!("--- Cargo.toml preview ---\n{}\n...", preview);
                        }
                    }
                } else {
                    println!("Result: {:?}", result);
                }
            }
            Err(e) => {
                error!("Tool execution failed: {}", e);
                return Err(e);
            }
        }
    } else if !tools.is_empty() {
        // Just print a message for other servers
        info!("Skipping tool execution test (no known test tool found or not using default server). Use specific arguments to test tools.");
    } else {
        info!("No tools available to test.");
    }

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

    #[tokio::test]
    async fn test_spawn_vm_placeholder() {
        let result = spawn_vm().await;
        assert!(result.is_ok());
    }
}
