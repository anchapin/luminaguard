//! Tool Execution Module
//!
//! This module provides secure subprocess execution for MCP tools and other commands.
//! It integrates command validation with timeout handling to ensure safe execution.

use super::{timeout::ExecutionTimeout, validator::{CommandValidator, SafeCommand}};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

/// Output from a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionOutput {
    /// The exit code (None if process was terminated by signal)
    pub exit_code: Option<i32>,

    /// The stdout output
    pub stdout: String,

    /// The stderr output
    pub stderr: String,

    /// Whether execution timed out
    pub timed_out: bool,

    /// Execution duration in milliseconds
    pub duration_ms: u128,
}

/// Result type for tool execution
pub type ToolExecutionResult = Result<ToolExecutionOutput>;

/// Tool command wrapper
///
/// This represents a command to be executed, including validation metadata.
#[derive(Debug, Clone)]
pub struct ToolCommand {
    /// The validated safe command
    safe: SafeCommand,

    /// The working directory for execution
    working_dir: Option<String>,

    /// Whether to capture stdout
    capture_stdout: bool,

    /// Whether to capture stderr
    capture_stderr: bool,
}

impl ToolCommand {
    /// Create a new tool command (with default whitelist validation)
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute (must be in whitelist)
    /// * `args` - The arguments to pass to the command
    ///
    /// # Returns
    ///
    /// Returns a `ToolCommand` if validation passes, or an error if validation fails.
    ///
    /// # Example
    ///
    /// ```
    /// use luminaguard_orchestrator::tools::ToolCommand;
    ///
    /// let command = ToolCommand::new("npx", &["-y", "@modelcontextprotocol/server-filesystem"]);
    /// assert!(command.is_ok());
    /// ```
    pub fn new(command: &str, args: &[&str]) -> Result<Self> {
        let validator = CommandValidator::default();
        let safe = validator
            .validate(command, args)
            .context("Command validation failed")?;

        Ok(Self {
            safe,
            working_dir: None,
            capture_stdout: true,
            capture_stderr: true,
        })
    }

    /// Create a new tool command with a custom validator
    ///
    /// # Arguments
    ///
    /// * `validator` - The command validator to use
    /// * `command` - The command to execute
    /// * `args` - The arguments to pass to the command
    pub fn new_with_validator(
        validator: &CommandValidator,
        command: &str,
        args: &[&str],
    ) -> Result<Self> {
        let safe = validator
            .validate(command, args)
            .context("Command validation failed")?;

        Ok(Self {
            safe,
            working_dir: None,
            capture_stdout: true,
            capture_stderr: true,
        })
    }

    /// Set the working directory for execution
    pub fn with_working_dir(mut self, dir: &str) -> Self {
        self.working_dir = Some(dir.to_string());
        self
    }

    /// Set whether to capture stdout
    pub fn with_stdout(mut self, capture: bool) -> Self {
        self.capture_stdout = capture;
        self
    }

    /// Set whether to capture stderr
    pub fn with_stderr(mut self, capture: bool) -> Self {
        self.capture_stderr = capture;
        self
    }

    /// Get the safe command
    pub fn safe(&self) -> &SafeCommand {
        &self.safe
    }
}

/// Tool executor for secure subprocess execution
///
/// # Security
///
/// - All commands are validated against a whitelist
/// - Commands are executed as lists (no shell interpretation)
/// - Timeouts are enforced to prevent hanging
/// - Output is captured and sanitized
///
/// # Example
///
/// ```no_run
/// use luminaguard_orchestrator::tools::{ToolExecutor, ToolCommand};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let executor = ToolExecutor::new();
///
///     let command = ToolCommand::new("npx", &["-y", "@modelcontextprotocol/server-filesystem"])?;
///     let output = executor.execute(command).await?;
///
///     println!("Exit code: {:?}", output.exit_code);
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ToolExecutor {
    /// Default timeout for executions
    default_timeout: ExecutionTimeout,
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolExecutor {
    /// Create a new tool executor with default settings
    pub fn new() -> Self {
        Self {
            default_timeout: ExecutionTimeout::medium(),
        }
    }

    /// Create a new tool executor with a custom default timeout
    pub fn with_timeout(timeout: ExecutionTimeout) -> Self {
        Self {
            default_timeout: timeout,
        }
    }

    /// Execute a tool command
    ///
    /// # Arguments
    ///
    /// * `command` - The validated tool command to execute
    ///
    /// # Returns
    ///
    /// Returns the execution output, or an error if execution fails.
    pub async fn execute(&self, command: ToolCommand) -> ToolExecutionResult {
        self.execute_with_timeout(command, self.default_timeout.clone())
            .await
    }

    /// Execute a tool command with a custom timeout
    ///
    /// # Arguments
    ///
    /// * `command` - The validated tool command to execute
    /// * `timeout` - The timeout for this execution
    ///
    /// # Returns
    ///
    /// Returns the execution output, or an error if execution fails.
    pub async fn execute_with_timeout(
        &self,
        command: ToolCommand,
        timeout: ExecutionTimeout,
    ) -> ToolExecutionResult {
        let start = std::time::Instant::now();

        // Execute with timeout
        let (exit_code, stdout, stderr) = timeout
            .run(async move {
                self.execute_internal(command).await
            })
            .await?;

        let duration = start.elapsed();

        Ok(ToolExecutionOutput {
            exit_code: Some(exit_code),
            stdout,
            stderr,
            timed_out: false,
            duration_ms: duration.as_millis(),
        })
    }

    /// Internal execution logic
    async fn execute_internal(&self, command: ToolCommand) -> Result<(i32, String, String)> {
        let (cmd, args) = command.safe.as_tuple();

        tracing::info!("Executing tool: {}", cmd);
        tracing::debug!("Arguments: {:?}", args);

        // Build the command
        let mut process = Command::new(cmd);
        process.args(args);

        // Set working directory if specified
        if let Some(dir) = command.working_dir {
            process.current_dir(&dir);
            tracing::debug!("Working directory: {}", dir);
        }

        // Configure output capture
        if command.capture_stdout {
            process.stdout(Stdio::piped());
        } else {
            process.stdout(Stdio::inherit());
        }

        if command.capture_stderr {
            process.stderr(Stdio::piped());
        } else {
            process.stderr(Stdio::inherit());
        }

        // Spawn the process
        let mut child = process
            .spawn()
            .context(format!("Failed to spawn command: {}", cmd))?;

        // Wait for completion and capture output
        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for command completion")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        tracing::debug!("Command exited with status: {:?}", output.status);

        Ok((output.status.code().unwrap_or(-1), stdout, stderr))
    }

    /// Execute multiple commands in parallel
    ///
    /// # Arguments
    ///
    /// * `commands` - A slice of tool commands to execute
    ///
    /// # Returns
    ///
    /// Returns a vector of execution results in the same order as the commands.
    pub async fn execute_parallel(&self, commands: Vec<ToolCommand>) -> Vec<ToolExecutionResult> {
        let futures: Vec<_> = commands
            .into_iter()
            .map(|cmd| self.execute(cmd))
            .collect();

        futures::future::join_all(futures).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_executor_default() {
        use std::time::Duration;

        let executor = ToolExecutor::new();
        // Just verify it can be created
        assert_eq!(executor.default_timeout.duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_tool_executor_with_timeout() {
        use std::time::Duration;

        let timeout = ExecutionTimeout::short();
        let executor = ToolExecutor::with_timeout(timeout);
        assert_eq!(executor.default_timeout.duration(), Duration::from_secs(10));
    }

    #[test]
    fn test_tool_command_new_valid() {
        let command = ToolCommand::new("npx", &["-y", "package"]);
        assert!(command.is_ok());
        let cmd = command.unwrap();
        assert_eq!(cmd.safe().command, "npx");
        assert_eq!(cmd.safe().args.len(), 2);
    }

    #[test]
    fn test_tool_command_new_invalid() {
        let command = ToolCommand::new("bash", &["-c", "echo"]);
        assert!(command.is_err());
    }

    #[test]
    fn test_tool_command_with_working_dir() {
        let command = ToolCommand::new("python", &["--version"])
            .unwrap()
            .with_working_dir("/tmp");
        assert_eq!(command.working_dir, Some("/tmp".to_string()));
    }

    #[test]
    fn test_tool_command_with_stdout() {
        let command = ToolCommand::new("python", &["--version"])
            .unwrap()
            .with_stdout(false);
        assert!(!command.capture_stdout);
    }

    #[test]
    fn test_tool_command_with_stderr() {
        let command = ToolCommand::new("python", &["--version"])
            .unwrap()
            .with_stderr(false);
        assert!(!command.capture_stderr);
    }

    #[test]
    fn test_tool_command_with_custom_validator() {
        let whitelist = vec!["my-tool".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);
        let command = ToolCommand::new_with_validator(&validator, "my-tool", &[]);
        assert!(command.is_ok());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_echo_command() {
        let executor = ToolExecutor::new();
        let command = ToolCommand::new("echo", &["hello", "world"]);
        // echo is not in the whitelist, so this should fail
        assert!(command.is_err());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_timeout() {
        let executor = ToolExecutor::new();

        // Create a custom validator for sleep
        let whitelist = vec!["sleep".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);

        let command = ToolCommand::new_with_validator(&validator, "sleep", &["10"])
            .unwrap();

        // Execute with a short timeout
        let timeout = ExecutionTimeout::from_secs(1);
        let result = executor.execute_with_timeout(command, timeout).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("timed out"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_timeout_not_hit() {
        let executor = ToolExecutor::new();

        // Create a custom validator for sleep
        let whitelist = vec!["sleep".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);

        let command = ToolCommand::new_with_validator(&validator, "sleep", &["0"])
            .unwrap();

        // Execute with a long timeout
        let timeout = ExecutionTimeout::from_secs(10);
        let result = executor.execute_with_timeout(command, timeout).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, Some(0));
        assert!(!output.timed_out);
    }

    #[tokio::test]
    async fn test_tool_execution_output_structure() {
        let output = ToolExecutionOutput {
            exit_code: Some(0),
            stdout: "test output".to_string(),
            stderr: "".to_string(),
            timed_out: false,
            duration_ms: 100,
        };

        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "test output");
        assert!(!output.timed_out);
        assert_eq!(output.duration_ms, 100);
    }

    #[test]
    fn test_execution_output_serialization() {
        let output = ToolExecutionOutput {
            exit_code: Some(0),
            stdout: "test".to_string(),
            stderr: "".to_string(),
            timed_out: false,
            duration_ms: 100,
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("exit_code"));
        assert!(json.contains("stdout"));
    }

    #[test]
    fn test_execution_output_deserialization() {
        let json = r#"{
            "exit_code": 0,
            "stdout": "test",
            "stderr": "",
            "timed_out": false,
            "duration_ms": 100
        }"#;

        let output: ToolExecutionOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "test");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_with_working_dir() {
        use tempfile::tempdir;

        let executor = ToolExecutor::new();

        // Create a custom validator for pwd
        let whitelist = vec!["pwd".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);

        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap();

        let command = ToolCommand::new_with_validator(&validator, "pwd", &[])
            .unwrap()
            .with_working_dir(dir_path);

        let result = executor.execute(command).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let stdout = output.stdout.trim();
        assert!(stdout.contains(temp_dir.path().file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_parallel() {
        let executor = ToolExecutor::new();

        // Create a custom validator for true/false
        let whitelist = vec!["true".to_string(), "false".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);

        let command1 = ToolCommand::new_with_validator(&validator, "true", &[]).unwrap();
        let command2 = ToolCommand::new_with_validator(&validator, "false", &[]).unwrap();

        let results = executor.execute_parallel(vec![command1, command2]).await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());

        let output1 = results[0].as_ref().unwrap();
        let output2 = results[1].as_ref().unwrap();

        assert_eq!(output1.exit_code, Some(0));
        assert_ne!(output2.exit_code, Some(0));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_with_output_capture() {
        let executor = ToolExecutor::new();

        // Create a custom validator for echo
        let whitelist = vec!["echo".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);

        let command = ToolCommand::new_with_validator(&validator, "echo", &["-n", "test"])
            .unwrap()
            .with_stdout(true)
            .with_stderr(true);

        let result = executor.execute(command).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.stdout.trim(), "test");
    }

    #[test]
    fn test_safe_command_clone() {
        let safe = SafeCommand::new("npx".to_string(), vec!["-y".to_string()]);
        let safe2 = safe.clone();
        assert_eq!(safe.command, safe2.command);
    }
}
