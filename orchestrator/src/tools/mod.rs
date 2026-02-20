//! Tool Execution Subsystem
//!
//! This module provides secure subprocess execution for MCP tools and other commands.
//! It enforces strict security measures to prevent shell injection attacks.
//!
//! # Security Features
//!
//! - **Command Whitelisting**: Only known-safe commands are allowed (npx, python, node, cargo)
//! - **List Invocation**: Commands are executed as lists, never through a shell
//! - **Timeout Enforcement**: All executions have configurable timeouts
//! - **Input Validation**: All command arguments are validated
//! - **Resource Limits**: Process spawning is limited to prevent fork bombs
//!
//! # Architecture
//!
//! The module is organized into:
//! - `validator.rs`: Command validation and whitelisting
//! - `executor.rs`: Subprocess execution with timeout handling
//! - `timeout.rs`: Timeout management and cancellation
//!
//! # Example
//!
//! ```no_run
//! use luminaguard_orchestrator::tools::ToolExecutor;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let executor = ToolExecutor::new();
//!
//!     // Command validation and execution is handled internally
//!         "npx",
//!         &["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
//!     )?;
//!
//!     let output = executor.execute(command).await?;
//!     println!("Exit code: {:?}", output.exit_code);
//!     println!("Stdout: {}", output.stdout);
//!
//!     Ok(())
//! }
//! ```

mod executor;
mod timeout;
mod validator;

pub use executor::{ToolExecutionOutput, ToolExecutionResult, ToolExecutor};
pub use timeout::ExecutionTimeout;
pub use validator::{CommandValidationError, CommandValidator, SafeCommand};
