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
use luminaguard_orchestrator::approval::action::ActionType;
use luminaguard_orchestrator::approval::action::RiskLevel;
use luminaguard_orchestrator::approval::diff::{Change, DiffCard};
use luminaguard_orchestrator::approval::tui::TuiResult;
use luminaguard_orchestrator::mcp::{McpClient, StdioTransport};
use luminaguard_orchestrator::metrics_server;
use luminaguard_orchestrator::vm::{self, destroy_vm};
use serde_json::json;
use std::fs;
use tracing::{error, info, warn, Level};
use tracing_subscriber::{fmt, EnvFilter, Layer};

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
    /// Run the agent with a single task
    Run {
        /// Task description for the agent
        task: String,
    },
    /// Start interactive chat mode
    Chat,
    /// Spawn a new JIT Micro-VM
    SpawnVm,
    /// VM management commands
    Vm {
        #[command(subcommand)]
        command: VmCommands,
    },
    /// Start daemon mode with metrics server
    Daemon {
        /// Port for metrics server (default: 9090)
        #[arg(short, long, default_value = "9090")]
        metrics_port: u16,
    },
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

/// VM management subcommands
#[derive(Subcommand, Debug)]
enum VmCommands {
    /// List active VMs
    List,
    /// Show VM status
    Status {
        /// VM ID to check
        vm_id: String,
    },
    /// Kill a VM
    Kill {
        /// VM ID to kill
        vm_id: String,
    },
    /// Show VM pool statistics
    Pool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize structured tracing
    // Environment variable: RUST_LOG (e.g., RUST_LOG=debug,luminaguard_orchestrator=info)
    // Environment variable: LUMINAGUARD_LOG_FORMAT (json or text, default: text)
    // Environment variable: LUMINAGUARD_LOG_LEVEL (trace, debug, info, warn, error, default: info)
    let log_format = std::env::var("LUMINAGUARD_LOG_FORMAT")
        .unwrap_or_else(|_| "text".to_string())
        .to_lowercase();

    let default_level = if args.verbose {
        Level::DEBUG
    } else {
        std::env::var("LUMINAGUARD_LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string())
            .parse::<Level>()
            .unwrap_or(Level::INFO)
    };

    // Build env filter with defaults
    let filter = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    match log_format.as_str() {
        "json" => {
            // JSON format for production/machine parsing
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_span_events(fmt::format::FmtSpan::CLOSE)
                )
                .init();
            info!(
                format = "json",
                level = %default_level,
                "LuminaGuard Orchestrator starting with JSON logging"
            );
        }
        _ => {
            // Human-readable text format for development
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_file(false)
                        .with_line_number(false)
                )
                .init();
            info!("🦊 LuminaGuard Orchestrator v0.1.0 starting...");
            if args.verbose {
                info!("Debug logging enabled");
            }
        }
    }

    // Log startup information
    info!(
        version = env!("CARGO_PKG_VERSION"),
        git_commit = option_env!("GIT_COMMIT").unwrap_or("unknown"),
        "LuminaGuard Orchestrator initialized"
    );

    // Match commands
    match args.command {
        Some(Commands::Run { task }) => {
            info!("Running agent task: {}", task);
            run_agent(task).await?;
        }
        Some(Commands::Chat) => {
            info!("Starting interactive chat mode...");
            chat_mode().await?;
        }
        Some(Commands::SpawnVm) => {
            info!("Spawning JIT Micro-VM...");
            spawn_vm().await?;
        }
        Some(Commands::Vm { command }) => {
            handle_vm_command(command).await?;
        }
        Some(Commands::Daemon { metrics_port }) => {
            info!(
                "Starting daemon mode with metrics server on port {}",
                metrics_port
            );
            metrics_server::start_metrics_server(metrics_port).await?;
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
///
/// This function attempts to execute the agent inside a VM for true isolation.
/// If VM execution is not available (missing rootfs, etc.), it falls back to
/// host execution.
async fn run_agent(task: String) -> Result<()> {
    info!("🎯 Task: {}", task);

    // Generate unique task ID for this agent execution
    let task_id = format!("agent-{}", uuid::Uuid::new_v4());

    // Try VM-isolated execution first (issue #507)
    // This provides true isolation by running the agent inside a VM
    info!("🚀 Attempting VM-isolated agent execution...");

    use luminaguard_orchestrator::vm::agent_executor::execute_in_vm_or_fallback;

    let result = execute_in_vm_or_fallback(task.clone(), task_id.clone()).await;

    match result {
        Ok(output) => {
            // Print agent output
            if !output.is_empty() {
                println!("{}", output);
            }

            // Print summary
            println!("\n==========================================");
            println!("🤖 Agent Execution Summary");
            println!("==========================================");
            println!("Task ID: {}", task_id);
            println!("Task: {}", task);
            println!("Execution Mode: VM-isolated (with host fallback)");
            println!("==========================================\n");

            info!("✅ Agent execution complete!");
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Agent execution failed: {}", e);
            Err(e)
        }
    }
}

/// Interactive chat mode - allows multiple exchanges with the agent
async fn chat_mode() -> Result<()> {
    use std::io::{self, Write};

    println!("\n==========================================");
    println!("🦊 LuminaGuard Interactive Chat");
    println!("==========================================");
    println!("Type your prompts and press Enter to send.");
    println!("Type 'quit', 'exit', or 'bye' to end the session.");
    println!("Type 'help' for available commands.");
    println!("==========================================\n");

    // Session state
    let session_id = uuid::Uuid::new_v4();
    let mut message_count = 0;

    // Spawn VM once for the session (for efficiency)
    info!("⚡ Spawning JIT Micro-VM for chat session...");
    let task_id = format!("chat-{}", session_id);
    let mut handle = match vm::spawn_vm(&task_id).await {
        Ok(h) => {
            println!("✅ VM ready (spawn time: {:.2}ms)", h.spawn_time_ms);
            Some(h)
        }
        Err(e) => {
            eprintln!("⚠️  Failed to spawn VM: {}. Running in host mode.", e);
            None
        }
    };

    // Print prompt
    print!("\n> ");
    io::stdout().flush()?;

    // Main chat loop
    loop {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF
                println!("\n👋 Goodbye!");
                break;
            }
            Ok(_) => {
                let input = input.trim();

                // Handle commands
                match input.to_lowercase().as_str() {
                    "quit" | "exit" | "bye" => {
                        println!("👋 Goodbye!");
                        break;
                    }
                    "help" => {
                        println!("\n📚 Available commands:");
                        println!("  help     - Show this help message");
                        println!("  quit     - Exit chat mode");
                        println!("  exit     - Exit chat mode");
                        println!("  bye      - Exit chat mode");
                        println!("  clear    - Clear the screen");
                        println!("  status   - Show session status");
                        println!("  spawn    - (Re)spawn the VM");
                        println!("  kill     - Destroy the VM");
                        println!("  tools    - List available MCP tools");
                        println!();
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    "clear" => {
                        // Simple clear - print newlines
                        print!("\x1B[2J\x1B[H");
                        println!("🦊 LuminaGuard Interactive Chat - Session: {}", session_id);
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    "status" => {
                        println!("\n📊 Session Status:");
                        println!("  Session ID: {}", session_id);
                        println!("  Messages: {}", message_count);
                        if let Some(ref h) = handle {
                            println!("  VM ID: {}", h.id);
                            println!("  VM Spawn Time: {:.2}ms", h.spawn_time_ms);
                            println!("  VM Status: Active");
                        } else {
                            println!("  VM Status: Not available (running on host)");
                        }
                        println!();
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    "spawn" => {
                        if handle.is_some() {
                            println!("ℹ️  VM already spawned. Use 'kill' first to respawn.");
                        } else {
                            info!("⚡ Spawning JIT Micro-VM...");
                            match vm::spawn_vm(&task_id).await {
                                Ok(h) => {
                                    println!("✅ VM ready (spawn time: {:.2}ms)", h.spawn_time_ms);
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to spawn VM: {}", e);
                                }
                            }
                        }
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    "kill" => {
                        if let Some(h) = handle.take() {
                            match vm::destroy_vm(h).await {
                                Ok(_) => println!("✅ VM destroyed"),
                                Err(e) => eprintln!("❌ Failed to destroy VM: {}", e),
                            }
                        } else {
                            println!("ℹ️  No VM to kill");
                        }
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    "tools" => {
                        // Quick MCP tool listing
                        println!("\n🔌 To list MCP tools, use: luminaguard test-mcp --list-tools");
                        println!();
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    "" => {
                        // Empty input
                        print!("> ");
                        io::stdout().flush()?;
                        continue;
                    }
                    _ => {
                        // Regular message - send to agent
                        message_count += 1;
                        println!("\n🤖 Processing...");
                    }
                }

                // Send to agent (run_agent handles the actual execution)
                match run_agent(input.to_string()).await {
                    Ok(_) => {
                        // Agent completed successfully
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                    }
                }

                // Print prompt for next message
                print!("\n> ");
                io::stdout().flush()?;
            }
            Err(e) => {
                eprintln!("\n❌ Error reading input: {}", e);
                break;
            }
        }
    }

    // Cleanup - destroy VM if still active
    if let Some(h) = handle {
        info!("🧹 Cleaning up VM...");
        if let Err(e) = vm::destroy_vm(h).await {
            eprintln!("⚠️  Failed to cleanup VM: {}", e);
        }
    }

    println!("\n📊 Session Summary:");
    println!("  Session ID: {}", session_id);
    println!("  Total Messages: {}", message_count);
    println!("\n✅ Chat session ended.\n");

    Ok(())
}

/// Handle VM management commands
async fn handle_vm_command(command: VmCommands) -> Result<()> {
    match command {
        VmCommands::List => {
            println!("\n==========================================");
            println!("📋 Active VMs");
            println!("==========================================");

            // Get pool stats
            match vm::pool_stats().await {
                Ok(stats) => {
                    println!("  Pool size: {}/{}", stats.current_size, stats.max_size);
                    println!("  Active VMs: {}", stats.active_vms);
                }
                Err(e) => {
                    println!("  Pool stats unavailable: {}", e);
                }
            }

            // Note: In a full implementation, we would track active VMs
            // For now, show metrics
            println!("\n  Note: Active VM tracking requires daemon mode.");
            println!("  Use 'luminaguard daemon' to start the daemon.\n");
        }
        VmCommands::Status { vm_id } => {
            println!("\n==========================================");
            println!("📊 VM Status: {}", vm_id);
            println!("==========================================");
            println!("  Status: Unknown (daemon mode required)");
            println!("  Use 'luminaguard daemon' to track VM status.\n");
        }
        VmCommands::Kill { vm_id } => {
            println!("\n==========================================");
            println!("🔪 Kill VM: {}", vm_id);
            println!("==========================================");
            println!("  Note: VM killing requires daemon mode.");
            println!("  Use 'luminaguard daemon' to manage VMs.\n");
        }
        VmCommands::Pool => {
            println!("\n==========================================");
            println!("📊 VM Pool Statistics");
            println!("==========================================");

            match vm::pool_stats().await {
                Ok(stats) => {
                    println!("  Current size: {}", stats.current_size);
                    println!("  Max size: {}", stats.max_size);
                    println!("  Active VMs: {}", stats.active_vms);
                    println!("  Queued tasks: {}", stats.queued_tasks);
                }
                Err(e) => {
                    println!("  Failed to get pool stats: {}", e);
                }
            }
            println!();
        }
    }
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
        action_type: ActionType::Unknown, // Simplified: use Unknown as placeholder
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
