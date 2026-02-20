//! Tool Execution Subsystem
//!
//! This module provides secure subprocess execution for MCP tools.
//!
//! # Components
//!
//! - **validator**: Command validation to prevent shell injection
//! - **executor**: Secure subprocess execution with timeout handling
//!
//! # Security
//!
//! This subsystem implements defense-in-depth:
//! 1. Command validation against a whitelist
//! 2. Subprocess execution without shell (no `shell=True`)
//! 3. Timeout handling to prevent hanging
//! 4. Output size limits to prevent memory exhaustion
//!
//! # Example
//!
//! ```ignore
//! use luminaguard_orchestrator::tools::{ToolExecutor, ExecutorConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let executor = ToolExecutor::new();
//!
//!     // Execute a simple command
//!     let result = executor.execute(&["echo", "hello world"]).await?;
//!     assert!(result.success);
//!
//!     // Execute with timeout
//!     let config = ExecutorConfig::with_timeout(5);
//!     let executor = ToolExecutor::with_config(config);
//!     let result = executor.execute(&["sleep", "1"]).await?;
//!     assert!(result.success);
//!
//!     Ok(())
//! }
//! ```

pub mod executor;
pub mod validator;

// Re-export commonly used types
pub use executor::{ExecutorConfig, ExecutionResult, ToolExecutor};
pub use validator::{CommandValidator, ValidationResult};
