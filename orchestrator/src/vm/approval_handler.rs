//! VM Approval Handler
//!
//! This module integrates the approval cliff system with vsock communication,
//! allowing guest VMs to request approval for red actions from the host.
//!
//! # Architecture
//!
//! ```text
//! Guest VM (Agent)
//!      ↓ (vsock: request_approval)
//! Host (ApprovalHandler)
//!      ↓ (ApprovalManager)
//! Human Approval (TUI/CLI)
//!      ↓ (vsock: response)
//! Guest VM (Agent)
//! ```
//!
//! # Protocol
//!
//! The guest sends a `request_approval` message with:
//! - `action_type`: The type of action (e.g., "file_edit", "file_delete")
//! - `description`: Human-readable description of the action
//! - `changes`: Array of changes being made
//!
//! The host responds with:
//! - `approved`: Boolean indicating approval status
//! - `reason`: Optional reason for denial

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::approval::{ActionType, ApprovalDecision, ApprovalManager, Change};
use crate::vm::vsock::VsockMessageHandler;

/// Approval request from guest VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Type of action being performed
    pub action_type: String,
    /// Human-readable description
    pub description: String,
    /// Changes that will be made
    pub changes: Vec<ChangeData>,
}

/// Change data from guest VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeData {
    /// Type of change
    #[serde(rename = "type")]
    pub change_type: String,
    /// Path or target of the change
    pub path: Option<String>,
    /// Old content (for edits)
    pub old_content: Option<String>,
    /// New content (for edits/creates)
    pub new_content: Option<String>,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Approval response to guest VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    /// Whether the action was approved
    pub approved: bool,
    /// Decision type
    pub decision: String,
    /// Optional reason for denial
    pub reason: Option<String>,
}

/// VM Approval Handler
///
/// Implements `VsockMessageHandler` to process approval requests from guest VMs.
/// This enables the "approval inside VM" architecture where the guest agent
/// can request approval for dangerous operations.
pub struct ApprovalHandler {
    /// Approval manager (shared with TUI)
    approval_manager: Arc<Mutex<ApprovalManager>>,
    /// Enable auto-approve for testing
    test_mode: bool,
}

impl ApprovalHandler {
    /// Create a new approval handler
    pub fn new() -> Self {
        Self {
            approval_manager: Arc::new(Mutex::new(ApprovalManager::new())),
            test_mode: false,
        }
    }

    /// Create with shared approval manager
    pub fn with_manager(manager: Arc<Mutex<ApprovalManager>>) -> Self {
        Self {
            approval_manager: manager,
            test_mode: false,
        }
    }

    /// Enable test mode (auto-approve all)
    pub fn with_test_mode(mut self, enabled: bool) -> Self {
        self.test_mode = enabled;
        self
    }

    /// Convert change data to Change enum
    fn convert_change(data: ChangeData) -> Change {
        match data.change_type.as_str() {
            "file_create" => Change::FileCreate {
                path: data.path.unwrap_or_default(),
                content_preview: data.new_content.unwrap_or_default(),
            },
            "file_edit" => Change::FileEdit {
                path: data.path.unwrap_or_default(),
                before: data.old_content.unwrap_or_default(),
                after: data.new_content.unwrap_or_default(),
            },
            "file_delete" => Change::FileDelete {
                path: data.path.unwrap_or_default(),
                size_bytes: data
                    .metadata
                    .and_then(|m| m.get("size_bytes").and_then(|v| v.as_u64()))
                    .unwrap_or(0),
            },
            "command_exec" => Change::CommandExec {
                command: data.path.unwrap_or_default(),
                args: data
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("args").and_then(|v| v.as_array()))
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                env_vars: data
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("env_vars").and_then(|v| v.as_array()))
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| {
                                let obj = v.as_object()?;
                                let key = obj.get("key")?.as_str()?.to_string();
                                let value = obj.get("value")?.as_str()?.to_string();
                                Some((key, value))
                            })
                            .collect()
                    }),
            },
            "email_send" => Change::EmailSend {
                to: data.path.unwrap_or_default(),
                subject: data
                    .metadata
                    .and_then(|m| m.get("subject").and_then(|v| v.as_str().map(String::from)))
                    .unwrap_or_default(),
                preview: data.new_content.unwrap_or_default(),
            },
            "external_call" => Change::ExternalCall {
                method: data
                    .metadata
                    .and_then(|m| m.get("method").and_then(|v| v.as_str().map(String::from)))
                    .unwrap_or_default(),
                endpoint: data.path.unwrap_or_default(),
                payload_preview: data.new_content.unwrap_or_default(),
            },
            "asset_transfer" => Change::AssetTransfer {
                from: data
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("from").and_then(|v| v.as_str().map(String::from)))
                    .unwrap_or_default(),
                to: data.path.unwrap_or_default(),
                amount: data
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("amount").and_then(|v| v.as_str().map(String::from)))
                    .unwrap_or_default(),
                currency: data
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("currency").and_then(|v| v.as_str().map(String::from)))
                    .unwrap_or_default(),
            },
            _ => Change::Custom {
                description: data.path.unwrap_or_default(),
            },
        }
    }

    /// Parse action type from string
    fn parse_action_type(s: &str) -> ActionType {
        match s.to_lowercase().as_str() {
            "read_file" | "readfile" | "read" => ActionType::ReadFile,
            "file_create" | "filecreate" | "create" => ActionType::CreateFile,
            "file_edit" | "fileedit" | "edit" => ActionType::EditFile,
            "file_delete" | "filedelete" | "delete" => ActionType::DeleteFile,
            "command_exec" | "commandexec" | "exec" => ActionType::ExecuteCommand,
            "email_send" | "emailsend" | "email" => ActionType::SendEmail,
            "external_call" | "externalcall" | "call" => ActionType::ExternalCall,
            "asset_transfer" | "assettransfer" | "transfer" => ActionType::TransferAsset,
            _ => ActionType::Unknown,
        }
    }

    /// Handle approval request
    async fn handle_approval_request(&self, params: serde_json::Value) -> Result<ApprovalResponse> {
        // Parse request
        let request: ApprovalRequest = serde_json::from_value(params)?;

        // Test mode: auto-approve
        if self.test_mode {
            return Ok(ApprovalResponse {
                approved: true,
                decision: "approved".to_string(),
                reason: None,
            });
        }

        // Convert to internal types
        let action_type = Self::parse_action_type(&request.action_type);
        let changes: Vec<Change> = request
            .changes
            .into_iter()
            .map(Self::convert_change)
            .collect();

        // Get approval decision
        let mut manager = self.approval_manager.lock().await;
        let decision = manager
            .check_and_approve_tui(action_type, request.description, changes)
            .await?;

        // Build response
        let (approved, reason) = match decision {
            ApprovalDecision::Approved => (true, None),
            ApprovalDecision::Denied => (false, Some("Action denied by user".to_string())),
            ApprovalDecision::DeferredToLater => (false, Some("Action deferred".to_string())),
        };

        Ok(ApprovalResponse {
            approved,
            decision: format!("{:?}", decision).to_lowercase(),
            reason,
        })
    }
}

impl Default for ApprovalHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VsockMessageHandler for ApprovalHandler {
    /// Handle request from guest VM
    async fn handle_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        match method {
            "request_approval" => {
                let response = self.handle_approval_request(params).await?;
                Ok(serde_json::to_value(response)?)
            }
            "get_approval_history" => {
                let manager = self.approval_manager.lock().await;
                let history = manager.get_history();
                Ok(serde_json::to_value(history)?)
            }
            _ => {
                anyhow::bail!("Unknown approval method: {}", method);
            }
        }
    }

    /// Handle notification from guest VM
    async fn handle_notification(&self, method: &str, _params: serde_json::Value) -> Result<()> {
        match method {
            "approval_log" => {
                // Log approval-related events from guest
                tracing::info!("Approval notification from guest VM");
            }
            _ => {
                tracing::warn!("Unknown approval notification: {}", method);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_type() {
        assert!(matches!(
            ApprovalHandler::parse_action_type("read_file"),
            ActionType::ReadFile
        ));
        assert!(matches!(
            ApprovalHandler::parse_action_type("file_edit"),
            ActionType::EditFile
        ));
        assert!(matches!(
            ApprovalHandler::parse_action_type("delete"),
            ActionType::DeleteFile
        ));
        assert!(matches!(
            ApprovalHandler::parse_action_type("unknown_action"),
            ActionType::Unknown
        ));
    }

    #[test]
    fn test_convert_change_file_create() {
        let data = ChangeData {
            change_type: "file_create".to_string(),
            path: Some("/tmp/test.txt".to_string()),
            old_content: None,
            new_content: Some("hello world".to_string()),
            metadata: None,
        };

        let change = ApprovalHandler::convert_change(data);
        match change {
            Change::FileCreate {
                path,
                content_preview,
            } => {
                assert_eq!(path, "/tmp/test.txt");
                assert_eq!(content_preview, "hello world");
            }
            _ => panic!("Expected FileCreate change"),
        }
    }

    #[test]
    fn test_convert_change_file_edit() {
        let data = ChangeData {
            change_type: "file_edit".to_string(),
            path: Some("/tmp/test.txt".to_string()),
            old_content: Some("old content".to_string()),
            new_content: Some("new content".to_string()),
            metadata: None,
        };

        let change = ApprovalHandler::convert_change(data);
        match change {
            Change::FileEdit {
                path,
                before,
                after,
            } => {
                assert_eq!(path, "/tmp/test.txt");
                assert_eq!(before, "old content");
                assert_eq!(after, "new content");
            }
            _ => panic!("Expected FileEdit change"),
        }
    }

    #[tokio::test]
    async fn test_approval_handler_test_mode() {
        let handler = ApprovalHandler::new().with_test_mode(true);

        let params = serde_json::json!({
            "action_type": "file_delete",
            "description": "Delete important file",
            "changes": [{
                "type": "file_delete",
                "path": "/important/file.txt"
            }]
        });

        let result = handler
            .handle_request("request_approval", params)
            .await
            .unwrap();
        let response: ApprovalResponse = serde_json::from_value(result).unwrap();

        assert!(response.approved);
        assert_eq!(response.decision, "approved");
    }

    #[tokio::test]
    async fn test_approval_handler_unknown_method() {
        let handler = ApprovalHandler::new();

        let result = handler
            .handle_request("unknown_method", serde_json::json!({}))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_approval_handler_notification() {
        let handler = ApprovalHandler::new();

        let result = handler
            .handle_notification("approval_log", serde_json::json!({}))
            .await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_approval_response_serialization() {
        let response = ApprovalResponse {
            approved: true,
            decision: "approved".to_string(),
            reason: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"approved\":true"));
        assert!(json.contains("\"decision\":\"approved\""));
    }

    #[test]
    fn test_approval_request_deserialization() {
        let json = serde_json::json!({
            "action_type": "file_edit",
            "description": "Edit config file",
            "changes": [{
                "type": "file_edit",
                "path": "/etc/config.yaml",
                "old_content": "old: value",
                "new_content": "new: value"
            }]
        });

        let request: ApprovalRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.action_type, "file_edit");
        assert_eq!(request.description, "Edit config file");
        assert_eq!(request.changes.len(), 1);
    }
}
