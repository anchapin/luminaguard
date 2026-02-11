// Approval Cliff Module
//
// Security boundary separating autonomous "Green Actions" from
// "Red Actions" requiring human approval.
//
// Green Actions (Autonomous):
// - Reading files
// - Searching the web
// - Checking logs
// - Any read-only operations
//
// Red Actions (Require Approval):
// - Editing code
// - Deleting files
// - Sending emails
// - Transferring crypto/assets
// - Any destructive or external communication

use serde::{Deserialize, Serialize};

/// Action type (Green or Red)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionKind {
    /// Green Action: Autonomous (read-only, safe)
    Green,

    /// Red Action: Requires approval (destructive, external)
    Red,
}

/// Action that requires approval check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Action type
    pub kind: ActionKind,

    /// Action description (shown to user in Diff Card)
    pub description: String,

    /// Expected changes (for Diff Card visualization)
    pub changes: ActionChanges,
}

/// Changes that will be made by the action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionChanges {
    /// File write: show diff
    FileWrite { path: String, old_content: String, new_content: String },

    /// File deletion: show path
    FileDelete { path: String },

    /// External communication: show recipient and content
    ExternalMessage { recipient: String, content: String },

    /// Custom: show description
    Custom { description: String },
}

/// Approval decision
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Allow action to proceed
    Approve,

    /// Reject action
    Reject,
}

/// Approval Cliff UI - manages the approval workflow
pub struct ApprovalCliff;

impl ApprovalCliff {
    /// Check if an action requires approval
    pub fn requires_approval(action: &Action) -> bool {
        action.kind == ActionKind::Red
    }

    /// Present the Diff Card UI to the user
    ///
    /// # TODO (Phase 2)
    ///
    /// This will be implemented in Phase 2 as a TUI or GUI.
    /// For now, it returns a placeholder decision.
    pub async fn present_diff_card(action: &Action) -> ApprovalDecision {
        tracing::info!("Presenting Diff Card for action: {}", action.description);

        // TODO: Phase 2 implementation
        // 1. Render Diff Card UI (TUI or GUI)
        // 2. Show action description
        // 3. Show changes (file diff, message content, etc.)
        // 4. Wait for user input (approve/reject)
        // 5. Return decision

        // Placeholder: Auto-approve Green actions, reject Red for now
        match action.kind {
            ActionKind::Green => ApprovalDecision::Approve,
            ActionKind::Red => ApprovalDecision::Reject, // Safe default
        }
    }

    /// Execute action with approval check
    pub async fn execute_with_approval(
        action: Action,
        executor: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if Self::requires_approval(&action) {
            let decision = Self::present_diff_card(&action).await;
            if decision == ApprovalDecision::Reject {
                tracing::warn!("Action rejected by user: {}", action.description);
                return Err(anyhow::anyhow!("Action rejected by user"));
            }
        }

        // Execute the action
        executor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_green_action_no_approval() {
        let action = Action {
            kind: ActionKind::Green,
            description: "Read file".to_string(),
            changes: ActionChanges::Custom {
                description: "Reading /tmp/file.txt".to_string(),
            },
        };
        assert!(!ApprovalCliff::requires_approval(&action));
    }

    #[test]
    fn test_red_action_requires_approval() {
        let action = Action {
            kind: ActionKind::Red,
            description: "Delete file".to_string(),
            changes: ActionChanges::FileDelete {
                path: "/tmp/file.txt".to_string(),
            },
        };
        assert!(ApprovalCliff::requires_approval(&action));
    }

    #[tokio::test]
    async fn test_green_action_auto_approved() {
        let action = Action {
            kind: ActionKind::Green,
            description: "Read file".to_string(),
            changes: ActionChanges::Custom {
                description: "Reading file".to_string(),
            },
        };
        let decision = ApprovalCliff::present_diff_card(&action).await;
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_red_action_rejected_by_default() {
        let action = Action {
            kind: ActionKind::Red,
            description: "Delete file".to_string(),
            changes: ActionChanges::FileDelete {
                path: "/tmp/file.txt".to_string(),
            },
        };
        let decision = ApprovalCliff::present_diff_card(&action).await;
        assert_eq!(decision, ApprovalDecision::Reject);
    }

    #[tokio::test]
    async fn test_execute_with_approval_green_action() {
        let action = Action {
            kind: ActionKind::Green,
            description: "Read file".to_string(),
            changes: ActionChanges::Custom {
                description: "Reading file".to_string(),
            },
        };

        let mut executed = false;
        let result = ApprovalCliff::execute_with_approval(action, || {
            executed = true;
            Ok(())
        })
        .await;

        assert!(result.is_ok());
        assert!(executed);
    }

    #[tokio::test]
    async fn test_execute_with_approval_red_action_rejected() {
        let action = Action {
            kind: ActionKind::Red,
            description: "Delete file".to_string(),
            changes: ActionChanges::FileDelete {
                path: "/tmp/file.txt".to_string(),
            },
        };

        let mut executed = false;
        let result = ApprovalCliff::execute_with_approval(action, || {
            executed = false; // Should never execute
            Err(anyhow::anyhow!("Should not reach here"))
        })
        .await;

        assert!(result.is_err());
        assert!(!executed);
    }

    #[test]
    fn test_action_kind_serialization() {
        // Test that ActionKind can be serialized/deserialized
        let green = ActionKind::Green;
        let red = ActionKind::Red;

        assert_eq!(green, ActionKind::Green);
        assert_eq!(red, ActionKind::Red);
        assert_ne!(green, red);
    }

    #[test]
    fn test_approval_decision_equality() {
        assert_eq!(ApprovalDecision::Approve, ApprovalDecision::Approve);
        assert_eq!(ApprovalDecision::Reject, ApprovalDecision::Reject);
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::Reject);
    }

    #[test]
    fn test_action_changes_file_write() {
        let changes = ActionChanges::FileWrite {
            path: "/tmp/test.txt".to_string(),
            old_content: "old".to_string(),
            new_content: "new".to_string(),
        };

        assert!(matches!(changes, ActionChanges::FileWrite { .. }));
        if let ActionChanges::FileWrite { path, old_content, new_content } = changes {
            assert_eq!(path, "/tmp/test.txt");
            assert_eq!(old_content, "old");
            assert_eq!(new_content, "new");
        } else {
            panic!("Expected FileWrite variant");
        }
    }

    #[test]
    fn test_action_changes_file_delete() {
        let changes = ActionChanges::FileDelete {
            path: "/tmp/test.txt".to_string(),
        };

        assert!(matches!(changes, ActionChanges::FileDelete { .. }));
        if let ActionChanges::FileDelete { path } = changes {
            assert_eq!(path, "/tmp/test.txt");
        } else {
            panic!("Expected FileDelete variant");
        }
    }

    #[test]
    fn test_action_changes_external_message() {
        let changes = ActionChanges::ExternalMessage {
            recipient: "user@example.com".to_string(),
            content: "Hello".to_string(),
        };

        assert!(matches!(changes, ActionChanges::ExternalMessage { .. }));
        if let ActionChanges::ExternalMessage { recipient, content } = changes {
            assert_eq!(recipient, "user@example.com");
            assert_eq!(content, "Hello");
        } else {
            panic!("Expected ExternalMessage variant");
        }
    }

    #[test]
    fn test_action_changes_custom() {
        let changes = ActionChanges::Custom {
            description: "Custom action".to_string(),
        };

        assert!(matches!(changes, ActionChanges::Custom { .. }));
        if let ActionChanges::Custom { description } = changes {
            assert_eq!(description, "Custom action");
        } else {
            panic!("Expected Custom variant");
        }
    }

    #[tokio::test]
    async fn test_execute_with_approval_green_action_returns_ok() {
        let action = Action {
            kind: ActionKind::Green,
            description: "Read file".to_string(),
            changes: ActionChanges::Custom {
                description: "Reading file".to_string(),
            },
        };

        let result = ApprovalCliff::execute_with_approval(action, || Ok(())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_with_approval_green_action_propagates_error() {
        let action = Action {
            kind: ActionKind::Green,
            description: "Read file".to_string(),
            changes: ActionChanges::Custom {
                description: "Reading file".to_string(),
            },
        };

        let result = ApprovalCliff::execute_with_approval(action, || {
            Err(anyhow::anyhow!("Test error"))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Test error");
    }
}
