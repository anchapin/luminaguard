//! Approval Cliff Module
//!
//! Security boundary separating autonomous "Green Actions" from
//! "Red Actions" requiring human approval.
//!
//! Architecture:
//! - `action.rs`: Classify actions as Green (safe) or Red (requires approval)
//! - `diff.rs`: Generate human-readable Diff Cards showing exact changes
//! - `history.rs`: Record all approval decisions for audit trails
//! - `ui.rs`: CLI/interactive prompts for user approval
//! - `tui.rs`: Terminal UI (Phase 2) for rich approval interface
//! - `mod.rs`: ApprovalManager - main entry point
//!
//! Green Actions (Autonomous):
//! - Reading files
//! - Searching the web
//! - Checking logs
//! - Any read-only operations
//!
//! Red Actions (Require Approval):
//! - Editing code
//! - Deleting files
//! - Sending emails
//! - Transferring crypto/assets
//! - Any destructive or external communication

pub mod action;
pub mod diff;
pub mod history;
pub mod ui;
pub mod tui;

pub use action::{ActionType, RiskLevel};
pub use diff::{Change, DiffCard};
pub use history::{ApprovalDecision, ApprovalHistory, ApprovalRecord};
pub use ui::{ApprovalPrompt, ApprovalPromptConfig};
pub use tui::{present_tui_approval, TuiResult};

use tracing::info;

/// Main Approval Manager - entry point for approval cliff workflow
///
/// Handles the complete approval workflow:
/// 1. Classify action (Green or Red)
/// 2. Auto-approve Green actions
/// 3. Present Diff Card for Red actions
/// 4. Record decision in audit trail
/// 5. Return decision for execution
#[derive(Debug)]
pub struct ApprovalManager {
    /// Approval history (audit trail)
    history: ApprovalHistory,

    /// Enable approval cliff (can be disabled for testing)
    enable_approval_cliff: bool,

    /// UI configuration
    prompt_config: ApprovalPromptConfig,
}

impl ApprovalManager {
    /// Create a new approval manager
    pub fn new() -> Self {
        Self {
            history: ApprovalHistory::new(),
            enable_approval_cliff: true,
            prompt_config: ApprovalPromptConfig::default(),
        }
    }

    /// Create with custom prompt configuration
    pub fn with_prompt_config(config: ApprovalPromptConfig) -> Self {
        Self {
            history: ApprovalHistory::new(),
            enable_approval_cliff: true,
            prompt_config: config,
        }
    }

    /// Check if an action requires approval and get user decision
    ///
    /// Flow:
    /// 1. If approval cliff disabled, return Approved
    /// 2. If Green action, return Approved (auto-safe)
    /// 3. If Red action, present Diff Card and ask user
    /// 4. Record decision in audit trail
    /// 5. Return decision
    pub async fn check_and_approve(
        &mut self,
        action_type: ActionType,
        description: String,
        changes: Vec<Change>,
    ) -> anyhow::Result<ApprovalDecision> {
        // If approval cliff disabled, auto-approve
        if !self.enable_approval_cliff {
            info!(
                "Approval cliff disabled, auto-approving action: {}",
                description
            );
            return Ok(ApprovalDecision::Approved);
        }

        // Green actions skip approval
        if !action_type.requires_approval() {
            info!("Green action, auto-approving: {}", description);
            return Ok(ApprovalDecision::Approved);
        }

        // Generate Diff Card for Red action
        let diff_card = DiffCard::new(action_type, description.clone(), changes);

        // Ask user for approval
        let prompt = ApprovalPrompt::with_config(self.prompt_config.clone());
        let decision = prompt.ask_for_approval(&diff_card).await?;

        // Record decision in history
        let record = ApprovalRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            action_description: description,
            decision,
            approved_by: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            justification: None,
            execution_result: None,
        };

        self.history.record_decision(record)?;

        Ok(decision)
    }

    /// Check and approve using Terminal UI (Phase 2)
    ///
    /// Similar to check_and_approve but uses rich TUI rendering for diff cards.
    /// Falls back to CLI if terminal is unavailable.
    ///
    /// Flow:
    /// 1. If approval cliff disabled, return Approved
    /// 2. If Green action, return Approved (auto-safe)
    /// 3. If Red action, present TUI diff card and ask user
    /// 4. Record decision in audit trail
    /// 5. Return decision
    pub async fn check_and_approve_tui(
        &mut self,
        action_type: ActionType,
        description: String,
        changes: Vec<Change>,
    ) -> anyhow::Result<ApprovalDecision> {
        // If approval cliff disabled, auto-approve
        if !self.enable_approval_cliff {
            info!(
                "Approval cliff disabled, auto-approving action: {}",
                description
            );
            return Ok(ApprovalDecision::Approved);
        }

        // Green actions skip approval
        if !action_type.requires_approval() {
            info!("Green action, auto-approving: {}", description);
            return Ok(ApprovalDecision::Approved);
        }

        // Generate Diff Card for Red action
        let diff_card = DiffCard::new(action_type, description.clone(), changes);

        // Present TUI and get decision
        let tui_result = present_tui_approval(&diff_card).await?;
        let decision = match tui_result {
            TuiResult::Approved => ApprovalDecision::Approved,
            TuiResult::Rejected => ApprovalDecision::Denied,
        };

        // Record decision in history
        let record = ApprovalRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            action_description: description,
            decision,
            approved_by: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            justification: None,
            execution_result: None,
        };

        self.history.record_decision(record)?;

        Ok(decision)
    }

    /// Disable approval cliff (for testing only)
    pub fn disable_for_testing(&mut self) {
        self.enable_approval_cliff = false;
    }

    /// Get approval history
    pub fn get_history(&self) -> Vec<&ApprovalRecord> {
        self.history.get_history(None)
    }

    /// Get recent approval decisions (limited)
    pub fn get_recent_approvals(&self, limit: usize) -> Vec<&ApprovalRecord> {
        self.history.get_history(Some(limit))
    }

    /// Export audit log as JSON
    pub fn export_audit_log(&self) -> anyhow::Result<String> {
        self.history.export_audit_log()
    }
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_manager_new() {
        let manager = ApprovalManager::new();
        assert!(manager.enable_approval_cliff);
        assert_eq!(manager.get_history().len(), 0);
    }

    #[tokio::test]
    async fn test_green_action_auto_approved() {
        let mut manager = ApprovalManager::new();

        let decision = manager
            .check_and_approve(ActionType::ReadFile, "Read test.txt".to_string(), vec![])
            .await
            .unwrap();

        assert_eq!(decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_red_action_needs_approval() {
        let config = ApprovalPromptConfig {
            interactive: false,
            auto_approve_green: true,
            default_decision: ApprovalDecision::Approved,
        };

        let mut manager = ApprovalManager::with_prompt_config(config);

        let decision = manager
            .check_and_approve(
                ActionType::DeleteFile,
                "Delete test.txt".to_string(),
                vec![Change::FileDelete {
                    path: "test.txt".to_string(),
                    size_bytes: 100,
                }],
            )
            .await
            .unwrap();

        assert_eq!(decision, ApprovalDecision::Approved);
        assert_eq!(manager.get_history().len(), 1);
    }

    #[tokio::test]
    async fn test_disabled_approval_cliff() {
        let mut manager = ApprovalManager::new();
        manager.disable_for_testing();

        let decision = manager
            .check_and_approve(
                ActionType::DeleteFile,
                "Delete critical file".to_string(),
                vec![],
            )
            .await
            .unwrap();

        // Even dangerous actions are auto-approved when disabled
        assert_eq!(decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_audit_trail_recording() {
        let config = ApprovalPromptConfig {
            interactive: false,
            auto_approve_green: true,
            default_decision: ApprovalDecision::Denied,
        };

        let mut manager = ApprovalManager::with_prompt_config(config);

        let _ = manager
            .check_and_approve(
                ActionType::DeleteFile,
                "Delete test.txt".to_string(),
                vec![Change::FileDelete {
                    path: "test.txt".to_string(),
                    size_bytes: 100,
                }],
            )
            .await;

        let history = manager.get_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].decision, ApprovalDecision::Denied);
    }

    #[test]
    fn test_export_audit_log() {
        let manager = ApprovalManager::new();
        let log = manager.export_audit_log().unwrap();
        assert!(log.contains("[]")); // Empty array
    }

    #[tokio::test]
    async fn test_tui_approval_green_action() {
        let mut manager = ApprovalManager::new();

        let decision = manager
            .check_and_approve_tui(ActionType::ReadFile, "Read test.txt".to_string(), vec![])
            .await
            .unwrap();

        assert_eq!(decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_tui_disabled_approval_cliff() {
        let mut manager = ApprovalManager::new();
        manager.disable_for_testing();

        let decision = manager
            .check_and_approve_tui(
                ActionType::DeleteFile,
                "Delete critical file".to_string(),
                vec![],
            )
            .await
            .unwrap();

        assert_eq!(decision, ApprovalDecision::Approved);
    }
}
