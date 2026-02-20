//! Tool Executor
//!
//! This module provides secure subprocess execution for MCP tools.
//! It implements timeout handling, output capture, and proper error handling.

use super::validator::{CommandValidator, ValidationResult};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tracing::{debug, info, warn};

/// Default timeout for tool execution in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum output size in bytes (1MB)
const MAX_OUTPUT_SIZE: usize = 1024 * 1024;

/// Result of tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,

    /// Standard output (truncated if too large)
    pub stdout: String,

    /// Standard error (truncated if too large)
    pub stderr: String,

    /// Exit code (None if process was terminated)
    pub exit_code: Option<i32>,

    /// Execution duration in milliseconds
    pub duration_ms: f64,

    /// Whether execution timed out
    pub timed_out: bool,
}

impl ExecutionResult {
    /// Create a success result
    fn success(stdout: String, duration_ms: f64, exit_code: i32) -> Self {
        Self {
            success: true,
            stdout,
            stderr: String::new(),
            exit_code: Some(exit_code),
            duration_ms,
            timed_out: false,
        }
    }

    /// Create a failure result
    fn failure(
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
        duration_ms: f64,
    ) -> Self {
        Self {
            success: false,
            stdout,
            stderr,
            exit_code,
            duration_ms,
            timed_out: false,
        }
    }

    /// Create a timeout result
    fn timeout(duration_ms: f64) -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            duration_ms,
            timed_out: true,
        }
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        if self.timed_out {
            format!("Timeout after {:.0}ms", self.duration_ms)
        } else if self.success {
            format!(
                "Success (exit code: {:?}, {:.0}ms, {} bytes output)",
                self.exit_code,
                self.duration_ms,
                self.stdout.len()
            )
        } else {
            format!(
                "Failed (exit code: {:?}, {:.0}ms, {} bytes output)",
                self.exit_code,
                self.duration_ms,
                self.stdout.len() + self.stderr.len()
            )
        }
    }
}

/// Configuration for tool execution
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Timeout for execution (default: 30 seconds)
    pub timeout: Duration,

    /// Maximum output size in bytes (default: 1MB)
    pub max_output_size: usize,

    /// Whether to validate commands before execution (default: true)
    pub validate_commands: bool,

    /// Working directory for command execution (default: current directory)
    pub working_dir: Option<String>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            max_output_size: MAX_OUTPUT_SIZE,
            validate_commands: true,
            working_dir: None,
        }
    }
}

impl ExecutorConfig {
    /// Create a new executor config with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_secs),
            ..Default::default()
        }
    }

    /// Create a new executor config with custom output limit
    pub fn with_max_output_size(size: usize) -> Self {
        Self {
            max_output_size: size,
            ..Default::default()
        }
    }

    /// Create a new executor config with custom working directory
    pub fn with_working_dir(dir: impl Into<String>) -> Self {
        Self {
            working_dir: Some(dir.into()),
            ..Default::default()
        }
    }

    /// Disable command validation
    pub fn skip_validation(mut self) -> Self {
        self.validate_commands = false;
        self
    }
}

/// Tool executor for secure subprocess execution
///
/// # Security
///
/// This executor implements secure subprocess execution by:
/// 1. Using `tokio::process::Command` without `shell()`
/// 2. Validating commands against a whitelist
/// 3. Enforcing timeout limits
/// 4. Limiting output size to prevent memory exhaustion
///
/// # Example
///
/// ```ignore
/// use luminaguard_orchestrator::tools::executor::ToolExecutor;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let executor = ToolExecutor::new();
///
///     // Execute a simple command
///     let result = executor.execute(&["echo", "hello world"]).await?;
///     assert!(result.success);
///     assert_eq!(result.stdout, "hello world\n");
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ToolExecutor {
    /// Command validator
    validator: CommandValidator,

    /// Executor configuration
    config: ExecutorConfig,
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolExecutor {
    /// Create a new tool executor with default configuration
    pub fn new() -> Self {
        Self {
            validator: CommandValidator::new(),
            config: ExecutorConfig::default(),
        }
    }

    /// Create a new tool executor with custom configuration
    pub fn with_config(config: ExecutorConfig) -> Self {
        Self {
            validator: CommandValidator::new(),
            config,
        }
    }

    /// Create a new tool executor with custom validator
    pub fn with_validator(validator: CommandValidator) -> Self {
        Self {
            validator,
            config: ExecutorConfig::default(),
        }
    }

    /// Execute a command and return the result
    ///
    /// # Arguments
    ///
    /// * `command` - Command and arguments to execute (e.g., `["echo", "hello"]`)
    ///
    /// # Returns
    ///
    /// Returns `ExecutionResult` with output and status
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Command validation fails
    /// - Process fails to spawn
    /// - IO error occurs during execution
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = executor.execute(&["echo", "hello"]).await?;
    /// assert!(result.success);
    /// ```
    pub async fn execute(&self, command: &[&str]) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

        // Log command (truncated for safety)
        let cmd_str = if command.len() > 3 {
            format!("{:?} ... ({} args)", command[0], command.len())
        } else {
            format!("{:?}", command)
        };
        info!("Executing: {}", cmd_str);

        // Validate command if enabled
        if self.config.validate_commands {
            match self.validator.validate(command) {
                ValidationResult::Safe => {
                    debug!("Command validated successfully");
                }
                ValidationResult::Unsafe(msg) => {
                    warn!("Command validation failed: {}", msg);
                    anyhow::bail!("Command validation failed: {}", msg);
                }
            }
        }

        // Convert to Vec<String>
        let cmd_vec: Vec<String> = command.iter().map(|s| s.to_string()).collect();

        // Create tokio process command
        let mut process = TokioCommand::new(&cmd_vec[0]);
        process.args(&cmd_vec[1..]);

        // Set working directory if specified
        if let Some(ref dir) = self.config.working_dir {
            process.current_dir(dir);
        }

        // Capture stdout and stderr
        process.stdout(std::process::Stdio::piped());
        process.stderr(std::process::Stdio::piped());

        // Spawn process
        let child = process.spawn().with_context(|| {
            format!("Failed to spawn process: {}", cmd_vec[0])
        })?;

        // Wait for completion with timeout
        let output = match tokio::time::timeout(self.config.timeout, child.wait_with_output()).await {
            Ok(result) => result?,
            Err(_) => {
                // Timeout - Note: child was consumed by wait_with_output, so we can't kill it here
                // In a timeout scenario, the child process will be killed when the handle is dropped
                let duration = start.elapsed();
                warn!("Command timed out after {:?}", self.config.timeout);
                return Ok(ExecutionResult::timeout(duration.as_millis() as f64));
            }
        };

        let duration = start.elapsed();

        // Truncate output if too large
        let stdout = truncate_string(
            String::from_utf8_lossy(&output.stdout).to_string(),
            self.config.max_output_size,
        );
        let stderr = truncate_string(
            String::from_utf8_lossy(&output.stderr).to_string(),
            self.config.max_output_size,
        );

        // Determine success based on exit code
        let exit_code = output.status.code();

        if output.status.success() {
            info!("Command succeeded: {}", cmd_str);
            Ok(ExecutionResult::success(
                stdout,
                duration.as_millis() as f64,
                exit_code.unwrap_or(0),
            ))
        } else {
            warn!("Command failed: {} (exit code: {:?})", cmd_str, exit_code);
            Ok(ExecutionResult::failure(stdout, stderr, exit_code, duration.as_millis() as f64))
        }
    }

    /// Execute a command and validate against expected output
    ///
    /// # Arguments
    ///
    /// * `command` - Command and arguments to execute
    /// * `expected_stdout` - Expected stdout content (partial match)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if command succeeds and output matches, error otherwise
    pub async fn execute_and_validate(
        &self,
        command: &[&str],
        expected_stdout: &str,
    ) -> Result<()> {
        let result = self.execute(command).await?;

        if !result.success {
            anyhow::bail!(
                "Command failed with exit code {:?}: {}",
                result.exit_code,
                result.stderr
            );
        }

        if !result.stdout.contains(expected_stdout) {
            anyhow::bail!(
                "Command succeeded but output does not contain '{}'. Got: {}",
                expected_stdout,
                result.stdout
            );
        }

        Ok(())
    }

    /// Get a reference to the validator
    pub fn validator(&self) -> &CommandValidator {
        &self.validator
    }

    /// Get a mutable reference to the validator
    pub fn validator_mut(&mut self) -> &mut CommandValidator {
        &mut self.validator
    }

    /// Get a reference to the config
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Get a mutable reference to the config
    pub fn config_mut(&mut self) -> &mut ExecutorConfig {
        &mut self.config
    }
}

/// Truncate a string to a maximum length, adding ellipsis if truncated
fn truncate_string(mut s: String, max_len: usize) -> String {
    if s.len() > max_len {
        s.truncate(max_len.saturating_sub(3));
        s.push_str("...");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::time::Duration;

    /// Test executing a simple echo command
    #[tokio::test]
    async fn test_execute_echo() {
        let executor = ToolExecutor::new();
        let result = executor.execute(&["echo", "hello world"]).await.unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("hello world"));
        assert!(!result.timed_out);
        assert!(result.duration_ms >= 0.0);
    }

    /// Test executing a command that fails
    #[tokio::test]
    async fn test_execute_failing_command() {
        let executor = ToolExecutor::new();
        let result = executor.execute(&["false"]).await.unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
        assert!(!result.timed_out);
    }

    /// Test timeout handling
    #[tokio::test]
    async fn test_timeout() {
        let config = ExecutorConfig::with_timeout(1);
        let executor = ToolExecutor::with_config(config);

        // Sleep command should timeout
        let result = executor.execute(&["sleep", "10"]).await.unwrap();

        assert!(!result.success);
        assert!(result.timed_out);
        assert!(result.exit_code.is_none());
        assert!(result.duration_ms > 0.0);
    }

    /// Test output truncation
    #[tokio::test]
    async fn test_output_truncation() {
        let config = ExecutorConfig::with_max_output_size(100);
        let executor = ToolExecutor::with_config(config);

        // Generate large output
        let result = executor.execute(&["seq", "1000"]).await.unwrap();

        assert!(result.success);
        assert!(result.stdout.len() <= 100 + 3); // max + "..."
    }

    /// Test command validation rejection
    #[tokio::test]
    async fn test_command_validation_rejection() {
        let executor = ToolExecutor::new();

        // Command with shell metacharacter should fail
        let result = executor.execute(&["echo", "test; hack"]).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("validation failed"));
    }

    /// Test skipping validation
    #[tokio::test]
    async fn test_skip_validation() {
        let config = ExecutorConfig::default().skip_validation();
        let executor = ToolExecutor::with_config(config);

        // This would normally fail validation
        let result = executor.execute(&["echo", "test; hack"]).await;

        // Should succeed (though the semicolon is part of the argument)
        assert!(result.is_ok());
        assert!(result.unwrap().stdout.contains("test; hack"));
    }

    /// Test execute_and_validate
    #[tokio::test]
    async fn test_execute_and_validate() {
        let executor = ToolExecutor::new();

        // Should succeed
        executor
            .execute_and_validate(&["echo", "hello"], "hello")
            .await
            .unwrap();

        // Should fail - wrong output
        let result = executor.execute_and_validate(&["echo", "hello"], "goodbye").await;
        assert!(result.is_err());

        // Should fail - command fails
        let result = executor.execute_and_validate(&["false"], "").await;
        assert!(result.is_err());
    }

    /// Test working directory
    #[tokio::test]
    async fn test_working_directory() {
        let config = ExecutorConfig::with_working_dir("/tmp");
        let executor = ToolExecutor::with_config(config);

        let result = executor.execute(&["pwd"]).await.unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("/tmp"));
    }

    /// Test multiple commands
    #[tokio::test]
    async fn test_multiple_commands() {
        let executor = ToolExecutor::new();

        let commands = vec![
            vec!["echo", "test"],
            vec!["true"],
            vec!["printf", "hello"],
        ];

        for cmd in commands {
            let result = executor.execute(&cmd).await.unwrap();
            assert!(result.success, "Command should succeed: {:?}", cmd);
        }
    }

    /// Test ExecutionResult summary
    #[test]
    fn test_execution_result_summary() {
        let success = ExecutionResult::success("output".to_string(), 100.0, 0);
        assert!(success.summary().contains("Success"));

        let failure = ExecutionResult::failure("out".to_string(), "err".to_string(), Some(1), 100.0);
        assert!(failure.summary().contains("Failed"));

        let timeout = ExecutionResult::timeout(5000.0);
        assert!(timeout.summary().contains("Timeout"));
    }

    /// Test truncate_string
    #[test]
    fn test_truncate_string() {
        // String within limit
        assert_eq!(truncate_string("hello".to_string(), 10), "hello");

        // String at limit
        assert_eq!(truncate_string("hello".to_string(), 5), "hello");

        // String over limit
        assert_eq!(
            truncate_string("hello world".to_string(), 5),
            "he..."
        );

        // Empty string
        assert_eq!(truncate_string("".to_string(), 10), "");
    }

    proptest! {
        #[test]
        fn prop_echo_succeeds(
            arg in "[a-zA-Z0-9 ]+"
        ) {
            // Note: This test would need async, but proptest doesn't support async well
            // So we'll just test validation here
            let validator = CommandValidator::new();
            let command = vec!["echo", arg.as_str()];
            let result = validator.validate(&command);
            assert!(matches!(result, ValidationResult::Safe));
        }
    }

    proptest! {
        #[test]
        fn prop_safe_commands_validate(
            cmd in "[a-z0-9_-]+",
            args in prop::collection::vec("[a-zA-Z0-9_./-]+", 0..5)
        ) {
            let validator = CommandValidator::new();
            let mut command = vec![cmd.as_str()];
            command.extend(args.iter().map(|s| s.as_str()));
            let result = validator.validate(&command);
            // Should be safe (no metacharacters)
            assert!(matches!(result, ValidationResult::Safe));
        }
    }

    /// Test that non-existent commands fail appropriately
    #[tokio::test]
    async fn test_nonexistent_command() {
        let executor = ToolExecutor::new();

        let result = executor.execute(&["this-command-does-not-exist-12345"]).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to spawn"));
    }

    /// Test that cat with piped input works
    #[tokio::test]
    async fn test_cat_command() {
        let executor = ToolExecutor::new();

        // Create a temporary file
        let temp_file = "/tmp/luminaguard_test_cat.txt";
        let _ = std::fs::write(temp_file, "test content");

        let result = executor.execute(&["cat", temp_file]).await.unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("test content"));

        // Cleanup
        let _ = std::fs::remove_file(temp_file);
    }

    /// Test ExecutorConfig builder pattern
    #[test]
    fn test_executor_config_builder() {
        let config = ExecutorConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));

        let config = ExecutorConfig::with_timeout(60);
        assert_eq!(config.timeout, Duration::from_secs(60));

        let config = ExecutorConfig::with_max_output_size(2048);
        assert_eq!(config.max_output_size, 2048);

        let config = ExecutorConfig::with_working_dir("/tmp");
        assert_eq!(config.working_dir, Some("/tmp".to_string()));

        let config = ExecutorConfig::default().skip_validation();
        assert!(!config.validate_commands);
    }

    /// Test ToolExecutor getters and setters
    #[test]
    fn test_executor_getters_setters() {
        let mut executor = ToolExecutor::new();

        // Get validator
        let _validator = executor.validator();

        // Get config
        let _config = executor.config();

        // Modify validator
        executor.validator_mut().allow_command("my-tool");

        // Modify config
        executor.config_mut().timeout = Duration::from_secs(60);

        assert_eq!(executor.config().timeout, Duration::from_secs(60));
    }
}
