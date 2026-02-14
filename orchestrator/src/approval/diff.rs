//! Diff Card Generation for Human-Readable Action Display
//!
//! This module generates "Diff Cards" that show exactly what will change
//! before the user approves an action. Diff cards are color-coded by risk level
//! and include timestamps for audit trails.

use super::action::{ActionType, RiskLevel};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A Diff Card showing the exact changes an action will make
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffCard {
    /// Action type being performed
    pub action_type: ActionType,

    /// Human-readable description of the action
    pub description: String,

    /// Risk level (None, Low, Medium, High, Critical)
    pub risk_level: RiskLevel,

    /// List of specific changes that will be made
    pub changes: Vec<Change>,

    /// Timestamp when this diff card was created (for audit trail)
    pub timestamp: DateTime<Utc>,
}

/// A single change within an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Change {
    /// File creation
    FileCreate {
        path: String,
        /// Preview of content (first 500 chars)
        content_preview: String,
    },

    /// File modification with diff
    FileEdit {
        path: String,
        /// Before content (preview, first 500 chars)
        before: String,
        /// After content (preview, first 500 chars)
        after: String,
    },

    /// File deletion (show size)
    FileDelete {
        path: String,
        /// Size in bytes
        size_bytes: u64,
    },

    /// Command execution
    CommandExec {
        command: String,
        args: Vec<String>,
        /// Optional environment variables that will be set
        env_vars: Option<Vec<(String, String)>>,
    },

    /// Email or message send
    EmailSend {
        /// Recipient (email, slack handle, phone, etc.)
        to: String,
        /// Email subject or message title
        subject: String,
        /// Message content preview (first 500 chars)
        preview: String,
    },

    /// External API call
    ExternalCall {
        /// HTTP method (GET, POST, PUT, DELETE, etc.)
        method: String,
        /// API endpoint URL
        endpoint: String,
        /// Request payload preview (first 500 chars)
        payload_preview: String,
    },

    /// Asset transfer
    AssetTransfer {
        /// Source (wallet, account, etc.)
        from: String,
        /// Destination (wallet, account, etc.)
        to: String,
        /// Amount being transferred (string to preserve precision)
        amount: String,
        /// Currency or asset type (USD, ETH, BTC, etc.)
        currency: String,
    },

    /// System configuration change
    ConfigChange {
        /// Configuration key/path
        key: String,
        /// Old value
        old_value: String,
        /// New value
        new_value: String,
    },

    /// Generic/custom change
    Custom {
        /// Description of the change
        description: String,
    },
}

impl Change {
    /// Get a human-readable name for this change type
    pub fn change_type(&self) -> &'static str {
        match self {
            Change::FileCreate { .. } => "File Creation",
            Change::FileEdit { .. } => "File Edit",
            Change::FileDelete { .. } => "File Deletion",
            Change::CommandExec { .. } => "Command Execution",
            Change::EmailSend { .. } => "Email/Message",
            Change::ExternalCall { .. } => "External API Call",
            Change::AssetTransfer { .. } => "Asset Transfer",
            Change::ConfigChange { .. } => "Configuration Change",
            Change::Custom { .. } => "Custom Change",
        }
    }

    /// Get a concise summary of the change (for list view)
    pub fn summary(&self) -> String {
        match self {
            Change::FileCreate { path, .. } => format!("Create: {}", path),
            Change::FileEdit { path, .. } => format!("Edit: {}", path),
            Change::FileDelete { path, .. } => format!("Delete: {}", path),
            Change::CommandExec { command, .. } => format!("Execute: {}", command),
            Change::EmailSend { to, subject, .. } => {
                format!("Send email to {} (subject: {})", to, subject)
            }
            Change::ExternalCall {
                method, endpoint, ..
            } => {
                format!("{} {}", method, endpoint)
            }
            Change::AssetTransfer {
                from,
                to,
                amount,
                currency,
                ..
            } => {
                format!("Transfer {} {} from {} to {}", amount, currency, from, to)
            }
            Change::ConfigChange { key, new_value, .. } => {
                format!("Change {} to {}", key, new_value)
            }
            Change::Custom { description } => description.clone(),
        }
    }
}

impl DiffCard {
    /// Create a new diff card for an action
    pub fn new(action_type: ActionType, description: String, changes: Vec<Change>) -> Self {
        let risk_level = action_type.risk_level();

        Self {
            action_type,
            description,
            risk_level,
            changes,
            timestamp: Utc::now(),
        }
    }

    /// Generate human-readable output (CLI format)
    ///
    /// Output includes:
    /// - Action type and risk level
    /// - Description
    /// - List of changes
    /// - Timestamp for audit trail
    pub fn to_human_readable(&self) -> String {
        let mut output = String::new();

        // Header with risk level
        let risk_str = match self.risk_level {
            RiskLevel::None => "🟢 [GREEN]",
            RiskLevel::Low => "🟡 [LOW]",
            RiskLevel::Medium => "🟠 [MEDIUM]",
            RiskLevel::High => "🔴 [HIGH]",
            RiskLevel::Critical => "🔴🔴 [CRITICAL]",
        };

        output.push_str(&format!("{} {} Action\n", risk_str, self.action_type));
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        // Description
        output.push_str(&format!("Description: {}\n", self.description));

        // Timestamp
        output.push_str(&format!(
            "Time: {}\n",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Changes
        if !self.changes.is_empty() {
            output.push_str("\nChanges:\n");
            for (i, change) in self.changes.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. {} - {}\n",
                    i + 1,
                    change.change_type(),
                    change.summary()
                ));

                match change {
                    Change::FileEdit { before, after, .. } => {
                        output.push_str(&format!("     Before: {}\n", truncate(before, 60)));
                        output.push_str(&format!("     After:  {}\n", truncate(after, 60)));
                    }
                    Change::FileCreate {
                        content_preview, ..
                    } => {
                        output.push_str(&format!(
                            "     Content: {}\n",
                            truncate(content_preview, 60)
                        ));
                    }
                    Change::FileDelete { size_bytes, .. } => {
                        output.push_str(&format!("     Size: {} bytes\n", size_bytes));
                    }
                    Change::CommandExec { args, .. } => {
                        if !args.is_empty() {
                            output.push_str(&format!("     Args: {}\n", args.join(" ")));
                        }
                    }
                    Change::EmailSend { .. } => {
                        // Already in summary
                    }
                    Change::ExternalCall {
                        payload_preview, ..
                    } => {
                        output.push_str(&format!(
                            "     Payload: {}\n",
                            truncate(payload_preview, 60)
                        ));
                    }
                    Change::AssetTransfer { .. } => {
                        // Already in summary
                    }
                    Change::ConfigChange {
                        old_value,
                        new_value,
                        ..
                    } => {
                        output.push_str(&format!("     Old: {}\n", old_value));
                        output.push_str(&format!("     New: {}\n", new_value));
                    }
                    Change::Custom { .. } => {
                        // Already in summary
                    }
                }
            }
        }

        output
    }

    /// Get detailed JSON representation (for logging/auditing)
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

impl fmt::Display for DiffCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_human_readable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_create_change_summary() {
        let change = Change::FileCreate {
            path: "/tmp/test.txt".to_string(),
            content_preview: "Hello, World!".to_string(),
        };
        assert_eq!(change.summary(), "Create: /tmp/test.txt");
    }

    #[test]
    fn test_file_edit_change_summary() {
        let change = Change::FileEdit {
            path: "/tmp/test.txt".to_string(),
            before: "old".to_string(),
            after: "new".to_string(),
        };
        assert_eq!(change.summary(), "Edit: /tmp/test.txt");
    }

    #[test]
    fn test_file_delete_change_summary() {
        let change = Change::FileDelete {
            path: "/tmp/test.txt".to_string(),
            size_bytes: 1024,
        };
        assert_eq!(change.summary(), "Delete: /tmp/test.txt");
    }

    #[test]
    fn test_command_exec_change_summary() {
        let change = Change::CommandExec {
            command: "rm".to_string(),
            args: vec!["-rf".to_string(), "/tmp".to_string()],
            env_vars: None,
        };
        assert_eq!(change.summary(), "Execute: rm");
    }

    #[test]
    fn test_email_send_change_summary() {
        let change = Change::EmailSend {
            to: "test@example.com".to_string(),
            subject: "Test Subject".to_string(),
            preview: "Hello there".to_string(),
        };
        assert!(change.summary().contains("Send email to test@example.com"));
    }

    #[test]
    fn test_external_call_change_summary() {
        let change = Change::ExternalCall {
            method: "POST".to_string(),
            endpoint: "https://api.example.com/endpoint".to_string(),
            payload_preview: "{}".to_string(),
        };
        assert!(change.summary().contains("POST"));
    }

    #[test]
    fn test_asset_transfer_change_summary() {
        let change = Change::AssetTransfer {
            from: "account_a".to_string(),
            to: "account_b".to_string(),
            amount: "100.50".to_string(),
            currency: "USD".to_string(),
        };
        assert!(change.summary().contains("Transfer"));
        assert!(change.summary().contains("100.50 USD"));
    }

    #[test]
    fn test_diff_card_creation() {
        let changes = vec![Change::FileCreate {
            path: "/tmp/test.txt".to_string(),
            content_preview: "Hello".to_string(),
        }];

        let card = DiffCard::new(
            ActionType::CreateFile,
            "Create a test file".to_string(),
            changes,
        );

        assert_eq!(card.action_type, ActionType::CreateFile);
        assert_eq!(card.risk_level, RiskLevel::Medium);
        assert_eq!(card.description, "Create a test file");
        assert_eq!(card.changes.len(), 1);
    }

    #[test]
    fn test_diff_card_to_human_readable() {
        let changes = vec![
            Change::FileCreate {
                path: "/tmp/test.txt".to_string(),
                content_preview: "Hello, World!".to_string(),
            },
            Change::FileDelete {
                path: "/tmp/old.txt".to_string(),
                size_bytes: 512,
            },
        ];

        let card = DiffCard::new(
            ActionType::EditFile,
            "Update test files".to_string(),
            changes,
        );

        let readable = card.to_human_readable();
        assert!(readable.contains("EditFile Action"));
        assert!(readable.contains("Update test files"));
        assert!(readable.contains("File Creation"));
        assert!(readable.contains("File Deletion"));
        assert!(readable.contains("Create: /tmp/test.txt"));
        assert!(readable.contains("Delete: /tmp/old.txt"));
    }

    #[test]
    fn test_diff_card_json_serialization() {
        let changes = vec![Change::FileCreate {
            path: "/tmp/test.txt".to_string(),
            content_preview: "Hello".to_string(),
        }];

        let card = DiffCard::new(
            ActionType::CreateFile,
            "Create a test file".to_string(),
            changes,
        );

        let json = card.to_json();
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("CreateFile"));
        assert!(json_str.contains("Create a test file"));
    }

    #[test]
    fn test_truncate_long_string() {
        let long_str = "a".repeat(100);
        let truncated = truncate(&long_str, 10);
        assert_eq!(truncated.len(), 13); // 10 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_short_string() {
        let short_str = "hello";
        let truncated = truncate(short_str, 10);
        assert_eq!(truncated, "hello");
    }

    #[test]
    fn test_change_type_names() {
        assert_eq!(
            Change::FileCreate {
                path: "".to_string(),
                content_preview: "".to_string()
            }
            .change_type(),
            "File Creation"
        );
        assert_eq!(
            Change::FileDelete {
                path: "".to_string(),
                size_bytes: 0
            }
            .change_type(),
            "File Deletion"
        );
    }
}
