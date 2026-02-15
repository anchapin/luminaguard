// LuminaGuard Orchestrator - Main Entry Point
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
use luminaguard_orchestrator::approval::diff::{Change, DiffCard};
use luminaguard_orchestrator::approval::tui::TuiResult;
use luminaguard_orchestrator::mcp::{McpClient, StdioTransport};
use luminaguard_orchestrator::approval::action::ActionType;
use luminaguard_orchestrator::approval::action::RiskLevel;
use luminaguard_orchestrator::vm::{self, destroy_vm};
use serde_json::json;
use std::fs;
use tracing::{error, info, Level};
use tracing_subscriber::EnvFilter;

/// LuminaGuard: Local-first Agentic AI Runtime
#[derive(Parser, Debug)]
#[command(name = "luminaguard")]
#[command(author = "LuminaGuard Contributors")]
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
    /// Present TUI approval UI for an action
    Approve {
        /// Path to JSON file containing Diff Card
        #[arg(long)]
        diff_card: String,
    },
    /// Test Firecracker feasibility prototype (requires --features vm-prototype)
    #[cfg(feature = "vm-prototype")]
    TestVmPrototype,
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

    info!("🦊 LuminaGuard Orchestrator v0.1.0 starting...");

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
        Some(Commands::Approve { diff_card }) => {
            info!("Presenting approval TUI...");
            present_approval(&diff_card).await?;
        }
        #[cfg(feature = "vm-prototype")]
        Some(Commands::TestVmPrototype) => {
            info!("Testing Firecracker feasibility...");
            test_vm_prototype().await?;
        }
        None => {
            info!("No command specified. Use \"luminaguard --help\" for usage.");
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

    // Use the vm module to spawn a VM
    // We use a random ID or a fixed CLI one for testing
    let task_id = format!("cli-{}", uuid::Uuid::new_v4());

    let handle = vm::spawn_vm(&task_id).await?;

    info!("VM spawned successfully!");
    info!("  ID: {}", handle.id);
    info!("  Spawn time: {:.2}ms", handle.spawn_time_ms);

    // Verify target constraint
    if handle.spawn_time_ms > 200.0 {
        tracing::warn!("Spawn time exceeded target of 200ms!");
    }

    // Cleanup for now since this is just a test command
    info!("Destroying VM for cleanup...");
    destroy_vm(handle).await?;
    info!("VM destroyed.");

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

/// Test Firecracker feasibility prototype
#[cfg(feature = "vm-prototype")]
async fn test_vm_prototype() -> Result<()> {
    use luminaguard_orchestrator::vm::prototype::{self, Recommendation};

    // Run the feasibility test
    let result = prototype::run_feasibility_test().await;

    // Print the report
    prototype::print_report(&result);

    // Return error if test failed
    match result.recommendation {
        Recommendation::Proceed => {
            info!("✅ Feasibility test passed!");
            Ok(())
        }
        Recommendation::Abandon => {
            error!("❌ Feasibility test failed - Firecracker not viable");
            std::process::exit(1);
        }
        Recommendation::Investigate => {
            error!("⚠️  Feasibility test inconclusive - needs investigation");
            std::process::exit(1);
        }
    }
}

/// Present approval TUI for a Diff Card
async fn present_approval(diff_card_path: &str) -> Result<()> {
    // Read Diff Card from JSON file
    let diff_card_json = fs::read_to_string(diff_card_path)
        .with_context(|| format!("Failed to read Diff Card from {}", diff_card_path))?;

    let diff_card_data: serde_json::Value =
        serde_json::from_str(&diff_card_json).context("Failed to parse Diff Card JSON")?;

    // Convert to DiffCard structure
    let changes: Vec<Change> = if let Some(changes_array) = diff_card_data["changes"].as_array() {
        changes_array
            .iter()
            .map(|c| {
                let change_type = c["change_type"].as_str().unwrap_or("Custom");
                let summary = c["summary"].as_str().unwrap_or("");

                match change_type {
                    "FileCreate" => Change::FileCreate {
                        path: c["details"]["path"].as_str().unwrap_or("").to_string(),
                        content_preview: c["details"]["before"].as_str().unwrap_or("").to_string(),
                    },
                    "FileEdit" => Change::FileEdit {
                        path: c["details"]["path"].as_str().unwrap_or("").to_string(),
                        before: c["details"]["before"].as_str().unwrap_or("").to_string(),
                        after: c["details"]["after"].as_str().unwrap_or("").to_string(),
                    },
                    "FileDelete" => Change::FileDelete {
                        path: c["details"]["path"].as_str().unwrap_or("").to_string(),
                        size_bytes: c["details"]["size_bytes"].as_u64().unwrap_or(0),
                    },
                    "CommandExec" => {
                        let args = if let Some(arr) = c["details"]["args"].as_array() {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        } else {
                            vec![]
                        };

                        Change::CommandExec {
                            command: summary.replace("Execute: ", ""),
                            args,
                            env_vars: None,
                        }
                    }
                    _ => Change::Custom {
                        description: summary.to_string(),
                    },
                }
            })
            .collect()
    } else {
        vec![]
    };

    let diff_card = DiffCard {
        action_type: ActionType::Unknown,  // Simplified: use Unknown as placeholder
        description: diff_card_data["description"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        risk_level: match diff_card_data["risk_level"].as_str().unwrap_or("medium") {
            "none" => RiskLevel::None,
            "low" => RiskLevel::Low,
            "medium" => RiskLevel::Medium,
            "high" => RiskLevel::High,
            "critical" => RiskLevel::Critical,
            _ => RiskLevel::Medium,
        },
        changes,
        timestamp: chrono::Utc::now(),
    };

    // Present TUI
    let result = luminaguard_orchestrator::approval::tui::present_tui_approval(&diff_card).await?;

    // Print result to stdout for Python client to read
    match result {
        TuiResult::Approved => {
            println!("approved");
            Ok(())
        }
        TuiResult::Rejected => {
            println!("rejected");
            Ok(())
        }
        TuiResult::Cancelled => {
            println!("cancelled");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(["luminaguard", "run", "test task"]);
        assert!(matches!(args.command, Some(Commands::Run { .. })));
    }

    #[tokio::test]
    async fn test_spawn_vm_integration() {
        // Skip if firecracker or resources are missing
        // This is a rough check; ideally we check for binary in PATH
        let has_firecracker = std::process::Command::new("which")
            .arg("firecracker")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !has_firecracker {
            println!("Skipping test: firecracker binary not found");
            return;
        }

        // We also need resources/vmlinux and resources/rootfs.ext4
        // Since we are running from orchestrator root usually
        if !Path::new("resources/vmlinux").exists() {
            println!("Skipping test: resources/vmlinux not found");
            return;
        }

        let result = spawn_vm().await;
        // If everything is present, it should succeed.
        // If it fails, it's a regression.
        assert!(result.is_ok(), "Spawn VM failed: {:?}", result.err());
    }
}
