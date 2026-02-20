//! Command Validator
//!
//! This module provides secure command validation to prevent shell injection attacks.
//! It implements defense-in-depth by validating commands before subprocess execution.

use anyhow::Result;
use std::collections::HashSet;

/// Shell metacharacters that could enable command injection
///
/// Note: We exclude backslash (`\`) from this list because it's needed for
/// Windows paths. Since we use subprocess without `shell=True`, backslash
/// doesn't enable injection anyway.
const SHELL_METACHARS: &[char] = &[
    ';', '&', '|', '$', '`', '(', ')', '<', '>', '\n', '\r', '\t', '*', '?', '[', ']',
];

/// Known-safe base commands
///
/// This whitelist documents expected commands and prevents accidental use
/// of dangerous commands. This is not a complete security boundary (the
/// subprocess runs with the same permissions as the user), but it helps
/// catch mistakes and documents the expected command surface.
const SAFE_COMMANDS: &[&str] = &[
    "npx",       // Node.js package runner
    "python",    // Python interpreter
    "python3",   // Python 3 interpreter
    "node",      // Node.js runtime
    "cargo",     // Rust toolchain
    "rustc",     // Rust compiler
    "npm",       // Node.js package manager
    "pip",       // Python package manager
    "pip3",      // Python 3 package manager
    "git",       // Version control
    "echo",      // Testing (benign)
    "true",      // Testing (benign)
    "false",     // Testing (benign)
    "cat",       // File operations (for trusted input)
    "ls",        // Directory listing
    "pwd",       // Print working directory
    "date",      // System time
];

/// Command validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Command is safe to execute
    Safe,
    /// Command contains potentially dangerous content
    Unsafe(String),
}

/// Command validator for secure subprocess execution
///
/// # Security
///
/// This validator implements defense-in-depth by:
/// 1. Checking for shell metacharacters
/// 2. Validating argument types
/// 3. Checking against a whitelist of known-safe commands
///
/// Note: This validation is complementary to using `tokio::process::Command`
/// without `shell()`, which is the primary security measure. This validator
/// catches obvious mistakes and documents expected usage.
///
/// # Example
///
/// ```ignore
/// use luminaguard_orchestrator::tools::validator::CommandValidator;
///
/// let validator = CommandValidator::new();
///
/// // Validate safe command
/// let result = validator.validate(&["npx", "-y", "some-package"]);
/// assert!(matches!(result, ValidationResult::Safe));
///
/// // Reject command with metacharacters
/// let result = validator.validate(&["rm", "-rf", "/; echo pwned"]);
/// assert!(matches!(result, ValidationResult::Unsafe(_)));
/// ```
#[derive(Debug, Clone)]
pub struct CommandValidator {
    /// Whitelist of allowed commands
    whitelist: HashSet<String>,
}

impl Default for CommandValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandValidator {
    /// Create a new command validator with default safe commands whitelist
    pub fn new() -> Self {
        let whitelist = SAFE_COMMANDS.iter().map(|s| s.to_string()).collect();

        Self { whitelist }
    }

    /// Create a validator with a custom whitelist
    ///
    /// # Arguments
    ///
    /// * `whitelist` - Set of allowed base command names
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::collections::HashSet;
    /// use luminaguard_orchestrator::tools::validator::CommandValidator;
    ///
    /// let whitelist = HashSet::from(["echo".to_string(), "cat".to_string()]);
    /// let validator = CommandValidator::with_whitelist(whitelist);
    /// ```
    pub fn with_whitelist(whitelist: HashSet<String>) -> Self {
        Self { whitelist }
    }

    /// Validate a command list
    ///
    /// # Arguments
    ///
    /// * `command` - Command list to validate (e.g., `["npx", "-y", "package"]`)
    ///
    /// # Returns
    ///
    /// Returns `ValidationResult::Safe` if the command is valid, or
    /// `ValidationResult::Unsafe` with an error message otherwise.
    ///
    /// # Security Checks
    ///
    /// 1. Command must be a non-empty list
    /// 2. All arguments must be strings
    /// 3. No shell metacharacters in any argument
    /// 4. Base command must be in whitelist (with warning if not)
    pub fn validate(&self, command: &[&str]) -> ValidationResult {
        // Check 1: Non-empty list
        if command.is_empty() {
            return ValidationResult::Unsafe("Command list is empty".to_string());
        }

        // Check 2: All arguments are strings (already enforced by &[&str] type)
        // This is a no-op in Rust but documented for clarity

        // Check 3: No shell metacharacters
        for (i, arg) in command.iter().enumerate() {
            if let Some(char) = arg.chars().find(|c| SHELL_METACHARS.contains(c)) {
                return ValidationResult::Unsafe(format!(
                    "Argument {} contains shell metacharacter '{}': {}",
                    i, char, arg
                ));
            }
        }

        // Check 4: Base command in whitelist (with path support)
        let base_cmd = command[0];
        let base_name = extract_base_name(base_cmd);

        if !self.whitelist.contains(base_name) {
            // Note: We don't fail here, just warn. This allows custom setups
            // while still catching mistakes. In production, you might want to
            // make this strict.
            tracing::warn!(
                "Command '{}' not in known-safe list. Ensure this command is trusted.",
                base_name
            );
        }

        ValidationResult::Safe
    }

    /// Validate and return a Vec<String> for use with tokio::process::Command
    ///
    /// This is a convenience method that validates and converts to Vec<String>.
    ///
    /// # Arguments
    ///
    /// * `command` - Command list to validate
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<String>)` if valid, or `Err` with validation error.
    pub fn validate_to_vec(&self, command: &[&str]) -> Result<Vec<String>> {
        match self.validate(command) {
            ValidationResult::Safe => Ok(command.iter().map(|s| s.to_string()).collect()),
            ValidationResult::Unsafe(msg) => anyhow::bail!("Command validation failed: {}", msg),
        }
    }

    /// Add a command to the whitelist
    ///
    /// # Arguments
    ///
    /// * `command` - Command name to add (e.g., "my-script")
    pub fn allow_command(&mut self, command: &str) {
        self.whitelist.insert(command.to_string());
    }

    /// Remove a command from the whitelist
    ///
    /// # Arguments
    ///
    /// * `command` - Command name to remove
    pub fn disallow_command(&mut self, command: &str) {
        self.whitelist.remove(command);
    }
}

/// Extract base name from a command path
///
/// Handles both Unix and Windows paths:
/// - "./node_modules/.bin/npx" -> "npx"
/// - ".\\node_modules\\.bin\\npx.exe" -> "npx.exe"
///
/// # Arguments
///
/// * `command` - Command path or name
///
/// # Returns
///
/// Base name of the command
fn extract_base_name(command: &str) -> &str {
    // Split by both Unix and Windows path separators
    command
        .rsplit('/')
        .next()
        .and_then(|s| s.rsplit('\\').next())
        .unwrap_or(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Test that safe commands pass validation
    #[test]
    fn test_validate_safe_commands() {
        let validator = CommandValidator::new();

        let safe_commands = vec![
            vec!["npx", "-y", "@modelcontextprotocol/server-filesystem"],
            vec!["python", "-m", "pip", "list"],
            vec!["python3", "script.py"],
            vec!["node", "app.js"],
            vec!["cargo", "build"],
            vec!["echo", "hello"],
            vec!["cat", "file.txt"],
        ];

        for cmd in safe_commands {
            assert_eq!(
                validator.validate(&cmd),
                ValidationResult::Safe,
                "Command should be safe: {:?}",
                cmd
            );
        }
    }

    /// Test that commands with shell metacharacters are rejected
    #[test]
    fn test_reject_shell_metacharacters() {
        let validator = CommandValidator::new();

        let unsafe_commands = vec![
            vec!["rm", "-rf", "/; echo pwned"],
            vec!["cat", "file.txt; echo hack"],
            vec!["echo", "test & background"],
            vec!["echo", "$(whoami)"],
            vec!["echo", "`date`"],
            vec!["cmd1", "|", "cmd2"],
            vec!["cmd", ">", "output.txt"],
            vec!["cmd", "<", "input.txt"],
            vec!["cmd", "\n", "second-cmd"],
            vec!["cmd", "\r", "second-cmd"],
            vec!["cmd", "\t", "arg"],
        ];

        for cmd in unsafe_commands {
            let result = validator.validate(&cmd);
            assert!(
                matches!(result, ValidationResult::Unsafe(_)),
                "Command should be unsafe: {:?}",
                cmd
            );
        }
    }

    /// Test that empty commands are rejected
    #[test]
    fn test_reject_empty_command() {
        let validator = CommandValidator::new();

        let result = validator.validate(&[]);
        assert!(matches!(result, ValidationResult::Unsafe(_)));
    }

    /// Test path-based commands
    #[test]
    fn test_path_based_commands() {
        let validator = CommandValidator::new();

        // Unix paths
        let result = validator.validate(&["./node_modules/.bin/npx", "-y", "package"]);
        assert_eq!(result, ValidationResult::Safe);

        // Windows paths
        let result = validator.validate(&[".\\node_modules\\.bin\\npx", "-y", "package"]);
        assert_eq!(result, ValidationResult::Safe);

        // Absolute paths
        let result = validator.validate(&["/usr/bin/python3", "script.py"]);
        assert_eq!(result, ValidationResult::Safe);
    }

    /// Test validate_to_vec convenience method
    #[test]
    fn test_validate_to_vec() {
        let validator = CommandValidator::new();

        let command = vec!["echo", "hello"];
        let result = validator.validate_to_vec(&command).unwrap();

        assert_eq!(result, vec!["echo", "hello"]);
    }

    /// Test validate_to_vec with unsafe command
    #[test]
    fn test_validate_to_vec_unsafe() {
        let validator = CommandValidator::new();

        let command = vec!["echo", "test; hack"];
        let result = validator.validate_to_vec(&command);

        assert!(result.is_err());
    }

    /// Test custom whitelist
    #[test]
    fn test_custom_whitelist() {
        let whitelist = HashSet::from(["my-tool".to_string(), "custom-cmd".to_string()]);
        let validator = CommandValidator::with_whitelist(whitelist);

        // Whitelisted command should be safe
        let result = validator.validate(&["my-tool", "arg"]);
        assert_eq!(result, ValidationResult::Safe);

        // Non-whitelisted command should warn but not fail
        let result = validator.validate(&["random-command"]);
        assert_eq!(result, ValidationResult::Safe);
    }

    /// Test allow_command and disallow_command
    #[test]
    fn test_allow_disallow_command() {
        let mut validator = CommandValidator::new();

        // Remove a default command
        validator.disallow_command("echo");
        let result = validator.validate(&["echo", "test"]);
        assert_eq!(result, ValidationResult::Safe); // Still safe, just warns

        // Add a custom command
        validator.allow_command("my-tool");
        let result = validator.validate(&["my-tool", "arg"]);
        assert_eq!(result, ValidationResult::Safe);
    }

    proptest! {
        #[test]
        fn prop_valid_commands_no_metachars(
            cmd in "[a-z0-9_-]+",
            arg1 in "[a-zA-Z0-9_./-]+",
            arg2 in "[a-zA-Z0-9_./-]+"
        ) {
            let validator = CommandValidator::new();
            let command = vec![cmd.as_str(), arg1.as_str(), arg2.as_str()];
            let result = validator.validate(&command);
            assert!(matches!(result, ValidationResult::Safe));
        }
    }

    proptest! {
        #[test]
        fn prop_commands_with_metachars_fail(
            cmd in "[a-z0-9_-]+",
            arg in r"[a-zA-Z0-9_./\-]*[;|&$`()<>\n\r][a-zA-Z0-9_./\-]*"
        ) {
            let validator = CommandValidator::new();
            let command = vec![cmd.as_str(), arg.as_str()];
            let result = validator.validate(&command);
            assert!(matches!(result, ValidationResult::Unsafe(_)));
        }
    }

    /// Test extract_base_name
    #[test]
    fn test_extract_base_name() {
        assert_eq!(extract_base_name("npx"), "npx");
        assert_eq!(extract_base_name("./npx"), "npx");
        assert_eq!(extract_base_name("./node_modules/.bin/npx"), "npx");
        assert_eq!(extract_base_name(".\\node_modules\\.bin\\npx"), "npx");
        assert_eq!(extract_base_name("/usr/bin/python3"), "python3");
        assert_eq!(extract_base_name("C:\\Python39\\python.exe"), "python.exe");
    }

    /// Test all default safe commands
    #[test]
    fn test_all_default_safe_commands() {
        let validator = CommandValidator::new();

        for cmd in SAFE_COMMANDS {
            let result = validator.validate(&[cmd, "--version"]);
            assert_eq!(result, ValidationResult::Safe, "{} should be safe", cmd);
        }
    }
}
