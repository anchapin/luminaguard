//! Approval Prompt UI/CLI Interaction
//!
//! This module handles user interaction for approval decisions.
//! Supports both interactive (CLI) and non-interactive (mock) modes.

use super::diff::DiffCard;
use super::history::ApprovalDecision;
use std::io::{self, Write};
use tracing::{debug, warn};

/// Configuration for approval prompts
#[derive(Debug, Clone)]
pub struct ApprovalPromptConfig {
    /// Enable interactive prompts (false for testing)
    pub interactive: bool,

    /// Auto-approve Green actions (always safe)
    pub auto_approve_green: bool,

    /// Default decision for mocked prompts
    pub default_decision: ApprovalDecision,
}

impl Default for ApprovalPromptConfig {
    fn default() -> Self {
        Self {
            interactive: true,
            auto_approve_green: true,
            default_decision: ApprovalDecision::Denied,
        }
    }
}

/// Approval prompt UI manager
pub struct ApprovalPrompt {
    config: ApprovalPromptConfig,
}

impl ApprovalPrompt {
    /// Create a new approval prompt with default config
    pub fn new() -> Self {
        Self {
            config: ApprovalPromptConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: ApprovalPromptConfig) -> Self {
        Self { config }
    }

    /// Ask user for approval via CLI
    ///
    /// In interactive mode: displays diff card and waits for user input
    /// In mock mode: returns default decision
    pub async fn ask_for_approval(&self, diff_card: &DiffCard) -> anyhow::Result<ApprovalDecision> {
        // Validate risk level
        debug!(
            "Asking for approval: action={}, risk={}",
            diff_card.action_type, diff_card.risk_level
        );

        if self.config.interactive {
            self.prompt_user_interactive(diff_card).await
        } else {
            // Mock mode: return default decision
            debug!("Mock approval prompt: using default decision");
            Ok(self.config.default_decision)
        }
    }

    /// Interactive prompt (async-compatible)
    async fn prompt_user_interactive(
        &self,
        diff_card: &DiffCard,
    ) -> anyhow::Result<ApprovalDecision> {
        // Print the diff card
        println!("\n{}\n", diff_card);

        // Show approval options
        self.print_approval_options();

        // Get user input (blocking)
        let decision = self.get_user_input()?;

        println!("You chose: {}\n", decision);

        Ok(decision)
    }

    /// Print available options for user
    fn print_approval_options(&self) {
        println!("Please choose an action:");
        println!("  (a) Approve  - Allow this action to proceed");
        println!("  (d) Deny     - Block this action");
        println!("  (q) Quit     - Exit without deciding");
        println!("  (?) Help     - Show this help");
        print!("\nYour choice: ");
        let _ = io::stdout().flush();
    }

    /// Get user input from stdin
    fn get_user_input(&self) -> anyhow::Result<ApprovalDecision> {
        let mut input = String::new();

        loop {
            input.clear();

            match io::stdin().read_line(&mut input) {
                Ok(0) => {
                    // EOF reached
                    warn!("No input provided (EOF), denying by default");
                    return Ok(ApprovalDecision::Denied);
                }
                Ok(_) => {
                    let input = input.trim().to_lowercase();

                    match input.as_str() {
                        "a" | "approve" => return Ok(ApprovalDecision::Approved),
                        "d" | "deny" => return Ok(ApprovalDecision::Denied),
                        "q" | "quit" | "exit" => {
                            return Err(anyhow::anyhow!("User canceled approval prompt"))
                        }
                        "?" | "help" => {
                            self.print_approval_options();
                            continue;
                        }
                        _ => {
                            println!("Invalid choice. Please enter 'a' (approve), 'd' (deny), or '?' (help)");
                            print!("Your choice: ");
                            let _ = io::stdout().flush();
                            continue;
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to read input: {}", e));
                }
            }
        }
    }

    /// Create a mock prompt for testing
    pub fn mock(decision: ApprovalDecision) -> Self {
        Self {
            config: ApprovalPromptConfig {
                interactive: false,
                auto_approve_green: false,
                default_decision: decision,
            },
        }
    }

    /// Create a prompt that auto-approves (for testing)
    pub fn auto_approve() -> Self {
        Self {
            config: ApprovalPromptConfig {
                interactive: false,
                auto_approve_green: true,
                default_decision: ApprovalDecision::Approved,
            },
        }
    }

    /// Create a prompt that auto-rejects (for testing)
    pub fn auto_reject() -> Self {
        Self {
            config: ApprovalPromptConfig {
                interactive: false,
                auto_approve_green: true,
                default_decision: ApprovalDecision::Denied,
            },
        }
    }
}

impl Default for ApprovalPrompt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::action::ActionType;

    #[test]
    fn test_prompt_config_default() {
        let config = ApprovalPromptConfig::default();
        assert!(config.interactive);
        assert!(config.auto_approve_green);
        assert_eq!(config.default_decision, ApprovalDecision::Denied);
    }

    #[test]
    fn test_approval_prompt_new() {
        let prompt = ApprovalPrompt::new();
        assert!(prompt.config.interactive);
    }

    #[test]
    fn test_approval_prompt_with_config() {
        let config = ApprovalPromptConfig {
            interactive: false,
            auto_approve_green: false,
            default_decision: ApprovalDecision::Approved,
        };

        let prompt = ApprovalPrompt::with_config(config.clone());
        assert!(!prompt.config.interactive);
        assert_eq!(prompt.config.default_decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_mock_approval_approve() {
        let prompt = ApprovalPrompt::mock(ApprovalDecision::Approved);

        let card = crate::approval::diff::DiffCard::new(
            ActionType::DeleteFile,
            "Test deletion".to_string(),
            vec![],
        );

        let decision = prompt.ask_for_approval(&card).await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_mock_approval_reject() {
        let prompt = ApprovalPrompt::mock(ApprovalDecision::Denied);

        let card = crate::approval::diff::DiffCard::new(
            ActionType::DeleteFile,
            "Test deletion".to_string(),
            vec![],
        );

        let decision = prompt.ask_for_approval(&card).await.unwrap();
        assert_eq!(decision, ApprovalDecision::Denied);
    }

    #[tokio::test]
    async fn test_auto_approve() {
        let prompt = ApprovalPrompt::auto_approve();

        let card = crate::approval::diff::DiffCard::new(
            ActionType::CreateFile,
            "Test creation".to_string(),
            vec![],
        );

        let decision = prompt.ask_for_approval(&card).await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_auto_reject() {
        let prompt = ApprovalPrompt::auto_reject();

        let card = crate::approval::diff::DiffCard::new(
            ActionType::CreateFile,
            "Test creation".to_string(),
            vec![],
        );

        let decision = prompt.ask_for_approval(&card).await.unwrap();
        assert_eq!(decision, ApprovalDecision::Denied);
    }

    #[test]
    fn test_approval_prompt_default() {
        let prompt = ApprovalPrompt::default();
        assert!(prompt.config.interactive);
    }
}
