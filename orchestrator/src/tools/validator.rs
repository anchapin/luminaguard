//! Command Validation Module
//!
//! This module provides command validation to prevent shell injection attacks.
//! It uses a whitelist approach to ensure only known-safe commands can be executed.

use anyhow::Result;
use std::path::Path;

/// Error types for command validation
#[derive(Debug, thiserror::Error)]
pub enum CommandValidationError {
    #[error("Command '{0}' is not in the allowed whitelist")]
    NotAllowed(String),

    #[error("Command '{0}' contains invalid characters")]
    InvalidCharacters(String),

    #[error("Command argument '{0}' contains shell metacharacters")]
    ShellMetacharacter(String),

    #[error("Command path is absolute and potentially unsafe: '{0}'")]
    AbsolutePath(String),

    #[error("Command path contains directory traversal: '{0}'")]
    DirectoryTraversal(String),
}

/// Safe command wrapper that has been validated
#[derive(Debug, Clone)]
pub struct SafeCommand {
    /// The command to execute
    pub command: String,

    /// The arguments to pass to the command
    pub args: Vec<String>,
}

impl SafeCommand {
    /// Create a new safe command
    ///
    /// This should only be called after validation via CommandValidator.
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self { command, args }
    }

    /// Get the command and args as a tuple for subprocess execution
    pub fn as_tuple(&self) -> (&str, &[String]) {
        (&self.command, &self.args)
    }
}

/// Command validator that enforces security policies
///
/// # Security Principles
///
/// 1. **Whitelist Only**: Only known-safe commands are allowed
/// 2. **No Shell Injection**: Commands are never interpreted by a shell
/// 3. **Path Validation**: Prevents directory traversal and absolute paths
/// 4. **Argument Sanitization**: All arguments are checked for metacharacters
pub struct CommandValidator {
    /// Whitelist of allowed commands
    allowed_commands: Vec<String>,
}

impl Default for CommandValidator {
    fn default() -> Self {
        Self::with_default_whitelist()
    }
}

impl CommandValidator {
    /// Create a new validator with the default whitelist
    ///
    /// The default whitelist includes:
    /// - npx (for Node.js packages)
    /// - python, python3 (for Python scripts)
    /// - node (for Node.js scripts)
    /// - cargo (for Rust tools)
    pub fn with_default_whitelist() -> Self {
        Self {
            allowed_commands: vec![
                "npx".to_string(),
                "python".to_string(),
                "python3".to_string(),
                "node".to_string(),
                "cargo".to_string(),
            ],
        }
    }

    /// Create a new validator with a custom whitelist
    pub fn with_whitelist(allowed: Vec<String>) -> Self {
        Self {
            allowed_commands: allowed,
        }
    }

    /// Validate a command and its arguments
    ///
    /// # Arguments
    ///
    /// * `command` - The command to validate
    /// * `args` - The arguments to validate
    ///
    /// # Returns
    ///
    /// Returns a `SafeCommand` if validation passes, or an error if validation fails.
    ///
    /// # Example
    ///
    /// ```
    /// use luminaguard_orchestrator::tools::CommandValidator;
    ///
    /// let validator = CommandValidator::default();
    /// let safe = validator.validate("npx", &["-y", "@modelcontextprotocol/server-filesystem"]);
    /// assert!(safe.is_ok());
    /// ```
    pub fn validate(&self, command: &str, args: &[&str]) -> Result<SafeCommand> {
        // Check if command is whitelisted
        self.check_whitelist(command)?;

        // Check command path safety
        self.check_path(command)?;

        // Check for shell metacharacters in command
        self.check_shell_metacharacters(command, "command")?;

        // Validate all arguments
        for arg in args {
            self.check_shell_metacharacters(arg, "argument")?;
        }

        Ok(SafeCommand {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        })
    }

    /// Check if the command is in the whitelist
    fn check_whitelist(&self, command: &str) -> Result<()> {
        if !self.allowed_commands.contains(&command.to_string()) {
            return Err(CommandValidationError::NotAllowed(command.to_string()).into());
        }
        Ok(())
    }

    /// Check if the command path is safe
    ///
    /// Prevents:
    /// - Absolute paths (e.g., /bin/bash)
    /// - Directory traversal (e.g., ../malicious)
    fn check_path(&self, command: &str) -> Result<()> {
        let path = Path::new(command);

        // Check for absolute path
        if path.is_absolute() {
            return Err(CommandValidationError::AbsolutePath(command.to_string()).into());
        }

        // Check for directory traversal
        if command.contains("..") {
            return Err(CommandValidationError::DirectoryTraversal(command.to_string()).into());
        }

        Ok(())
    }

    /// Check for shell metacharacters that could enable injection
    ///
    /// These characters are dangerous when interpreted by a shell:
    /// - ; : Command separator
    /// - | : Pipe
    /// - & : Background execution
    /// - $ : Variable expansion
    /// - ` : Command substitution
    /// - \n : Newline (command separator)
    /// - \r : Carriage return
    /// - ( ) : Subshell
    /// - < > : Redirection
    fn check_shell_metacharacters(&self, input: &str, context: &str) -> Result<()> {
        let dangerous_chars = [';', '|', '&', '$', '`', '\n', '\r', '(', ')', '<', '>'];

        for &char in dangerous_chars.iter() {
            if input.contains(char) {
                return Err(CommandValidationError::ShellMetacharacter(format!(
                    "{} with {}",
                    context, char
                ))
                .into());
            }
        }

        Ok(())
    }

    /// Get the current whitelist
    pub fn whitelist(&self) -> &[String] {
        &self.allowed_commands
    }

    /// Check if a command is in the whitelist
    pub fn is_allowed(&self, command: &str) -> bool {
        self.allowed_commands.contains(&command.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_whitelist() {
        let validator = CommandValidator::default();

        // Check all default allowed commands
        assert!(validator.is_allowed("npx"));
        assert!(validator.is_allowed("python"));
        assert!(validator.is_allowed("python3"));
        assert!(validator.is_allowed("node"));
        assert!(validator.is_allowed("cargo"));

        // Check some disallowed commands
        assert!(!validator.is_allowed("bash"));
        assert!(!validator.is_allowed("sh"));
        assert!(!validator.is_allowed("rm"));
        assert!(!validator.is_allowed("/bin/bash"));
    }

    #[test]
    fn test_validate_allowed_command() {
        let validator = CommandValidator::default();

        // Valid npx command
        let result = validator.validate("npx", &["-y", "@modelcontextprotocol/server-filesystem"]);
        assert!(result.is_ok());
        let safe = result.unwrap();
        assert_eq!(safe.command, "npx");
        assert_eq!(safe.args.len(), 2);
    }

    #[test]
    fn test_validate_not_allowed_command() {
        let validator = CommandValidator::default();

        // Disallowed command
        let result = validator.validate("bash", &["-c", "echo test"]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().downcast::<CommandValidationError>(),
            Ok(CommandValidationError::NotAllowed(_))
        ));
    }

    #[test]
    fn test_validate_absolute_path() {
        let validator = CommandValidator::default();

        // Create a custom whitelist with an absolute path
        let whitelist = vec!["/bin/bash".to_string()];
        let validator_with_path = CommandValidator::with_whitelist(whitelist);

        // Absolute path should be rejected even if in whitelist
        let result = validator_with_path.validate("/bin/bash", &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().downcast::<CommandValidationError>(),
            Ok(CommandValidationError::AbsolutePath(_))
        ));
    }

    #[test]
    fn test_validate_directory_traversal() {
        let validator = CommandValidator::default();

        // Create a custom whitelist with directory traversal
        let whitelist = vec!["../bash".to_string()];
        let validator_with_traversal = CommandValidator::with_whitelist(whitelist);

        // Directory traversal should be rejected even if in whitelist
        let result = validator_with_traversal.validate("../bash", &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().downcast::<CommandValidationError>(),
            Ok(CommandValidationError::DirectoryTraversal(_))
        ));
    }

    #[test]
    fn test_validate_shell_metacharacters_in_command() {
        let validator = CommandValidator::default();

        let dangerous_commands = vec![
            ("npx; rm -rf /", ";"),
            ("npx| cat", "|"),
            ("npx& echo", "&"),
            ("npx$HOME", "$"),
            ("npx`whoami`", "`"),
            ("npx\ncmd", "\n"),
            ("npx\rcmd", "\r"),
            ("npx(cmd)", "("),
            ("npx<file", "<"),
        ];

        for (command, char) in dangerous_commands {
            let result = validator.validate(command, &[]);
            assert!(result.is_err(), "Should reject command with '{}'", char);
        }
    }

    #[test]
    fn test_validate_shell_metacharacters_in_args() {
        let validator = CommandValidator::default();

        let dangerous_args = vec![
            (vec!["-y", "echo test;"], ";"),
            (vec!["-y", "cat|evil"], "|"),
            (vec!["-y", "test&evil"], "&"),
            (vec!["-y", "path$HOME"], "$"),
            (vec!["-y", "cmd`whoami`"], "`"),
            (vec!["-y", "arg\ncmd"], "\n"),
            (vec!["-y", "arg\rcmd"], "\r"),
            (vec!["-y", "arg(cmd)"], "("),
            (vec!["-y", "arg<file"], "<"),
        ];

        for (args, char) in dangerous_args {
            let args_slice: &[&str] = &args;
            let result = validator.validate("npx", args_slice);
            assert!(result.is_err(), "Should reject args with '{}'", char);
        }
    }

    #[test]
    fn test_validate_safe_arguments() {
        let validator = CommandValidator::default();

        // These should all be safe
        let safe_cases = vec![
            ("npx", vec!["-y", "@modelcontextprotocol/server-filesystem"]),
            ("python", vec!["script.py", "--verbose"]),
            ("python3", vec!["-m", "module", "arg1", "arg2"]),
            ("node", vec!["server.js", "--port", "8080"]),
            ("cargo", vec!["build", "--release"]),
        ];

        for (command, args) in safe_cases {
            let args_slice: &[&str] = &args;
            let result = validator.validate(command, args_slice);
            assert!(result.is_ok(), "Should accept: {} {:?}", command, args);
        }
    }

    #[test]
    fn test_custom_whitelist() {
        let whitelist = vec!["custom-tool".to_string(), "another-tool".to_string()];
        let validator = CommandValidator::with_whitelist(whitelist);

        // Custom allowed command should work
        assert!(validator.is_allowed("custom-tool"));
        let result = validator.validate("custom-tool", &["arg1"]);
        assert!(result.is_ok());

        // Default commands should not be allowed
        assert!(!validator.is_allowed("npx"));
        let result = validator.validate("npx", &["-y", "package"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_command_as_tuple() {
        let safe = SafeCommand::new(
            "npx".to_string(),
            vec!["-y".to_string(), "package".to_string()],
        );
        let (cmd, args) = safe.as_tuple();
        assert_eq!(cmd, "npx");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn test_validator_get_whitelist() {
        let validator = CommandValidator::default();
        let whitelist = validator.whitelist();
        assert_eq!(whitelist.len(), 5);
        assert!(whitelist.contains(&"npx".to_string()));
    }

    #[test]
    fn test_empty_arguments() {
        let validator = CommandValidator::default();

        // Empty arguments should be valid
        let result = validator.validate("npx", &[]);
        assert!(result.is_ok());
        let safe = result.unwrap();
        assert!(safe.args.is_empty());
    }

    #[test]
    fn test_special_characters_in_args() {
        let validator = CommandValidator::default();

        // These special characters should be allowed (not shell metacharacters)
        let special_args = vec![
            vec!["-v", "-f", "/tmp/file.txt"],
            vec!["--config", "config.json"],
            vec!["-p", "8080", "--host", "localhost"],
            vec!["file_name_with_underscore.txt"],
            vec!["file-name-with-dashes.txt"],
            vec!["CamelCaseArg"],
            vec!["arg.with.dots"],
        ];

        for args in special_args {
            let args_slice: &[&str] = &args;
            let result = validator.validate("python", args_slice);
            assert!(result.is_ok(), "Should accept: {:?}", args);
        }
    }

    #[test]
    fn test_command_validation_error_messages() {
        let validator = CommandValidator::default();

        // Check that error messages are descriptive
        let result = validator.validate("malicious", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not in the allowed whitelist"));
    }
}
