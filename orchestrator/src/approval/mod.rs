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
    FileWrite {
        path: String,
        old_content: String,
        new_content: String,
    },

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
}
