//! Action Classifier for MCP Tool Calls
//!
//! This module provides classification logic for MCP tool calls, determining
//! whether a tool call is "Green" (autonomous, safe) or "Red" (requires approval).
//!
//! # Classification Strategy
//!
//! Classification is based on:
//! 1. Tool name (primary classification)
//! 2. Tool parameters (parameter-specific overrides)
//! 3. Server type (e.g., filesystem, GitHub, Slack)
//!
//! # Principle: Conservative Default
//!
//! Unknown tools always default to RED (require approval) for security.
//! This follows the "fail-secure" principle.

use crate::approval::action::{ActionType, RiskLevel};
use serde_json::Value;

/// Result of tool classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationResult {
    /// Whether the tool requires approval
    pub requires_approval: bool,
    /// The action type inferred from the tool
    pub action_type: ActionType,
    /// Risk level of the tool
    pub risk_level: RiskLevel,
    /// Reason for classification
    pub reason: String,
}

/// Classifier for MCP tool calls
pub struct ToolClassifier;

impl ToolClassifier {
    /// Classify an MCP tool call based on tool name and parameters
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the MCP tool being called
    /// * `arguments` - Tool arguments (JSON object)
    ///
    /// # Returns
    ///
    /// Classification result with action type, risk level, and reasoning
    ///
    /// # Examples
    ///
    /// ```
    /// use luminaguard_orchestrator::approval::classifier::ToolClassifier;
    /// use serde_json::json;
    ///
    /// // Green action (read-only)
    /// let args = json!({"path": "test.txt"});
    /// let result = ToolClassifier::classify_tool("read_file", &args);
    /// assert!(!result.requires_approval);
    ///
    /// // Red action (destructive)
    /// let args = json!({"path": "test.txt"});
    /// let result = ToolClassifier::classify_tool("delete_file", &args);
    /// assert!(result.requires_approval);
    /// ```
    pub fn classify_tool(tool_name: &str, arguments: &Value) -> ClassificationResult {
        let tool_name_lower = tool_name.to_lowercase();

        // Check against known tool patterns
        match Self::classify_by_name(&tool_name_lower, arguments) {
            Some(result) => result,
            None => {
                // Unknown tool - default to RED for safety
                ClassificationResult {
                    requires_approval: true,
                    action_type: ActionType::Unknown,
                    risk_level: RiskLevel::Critical,
                    reason: format!(
                        "Unknown tool '{}': requires approval for security",
                        tool_name
                    ),
                }
            }
        }
    }

    /// Classify based on tool name pattern
    fn classify_by_name(tool_name: &str, arguments: &Value) -> Option<ClassificationResult> {
        // Green actions (read-only, safe)
        if Self::is_green_tool(tool_name) {
            return Some(ClassificationResult {
                requires_approval: false,
                action_type: Self::infer_action_type_from_name(tool_name),
                risk_level: RiskLevel::None,
                reason: format!(
                    "Tool '{}' is a read-only operation (Green action)",
                    tool_name
                ),
            });
        }

        // Red actions (destructive, external communication)
        if let Some(action_type) = Self::infer_red_action_type(tool_name, arguments) {
            return Some(ClassificationResult {
                requires_approval: true,
                action_type,
                risk_level: action_type.risk_level(),
                reason: format!(
                    "Tool '{}' is a destructive or external communication (Red action)",
                    tool_name
                ),
            });
        }

        // Unknown pattern
        None
    }

    /// Check if a tool name matches Green action patterns
    fn is_green_tool(tool_name: &str) -> bool {
        let green_keywords = [
            "read_file",
            "list_files",
            "list_directory",
            "list_directories",
            "search_files",
            "search",
            "grep",
            "get_file_info",
            "stat",
            "check_file",
            "read",
            "view",
            "show",
            "list",
            "get",
            "fetch",
            "find",
            "locate",
            "query",
            "inspect",
            "examine",
            "monitor",
            "status",
            "info",
            "get_info",
            "read_resource",
            "list_resources",
            "read_prompt",
            "list_prompts",
        ];

        // Check if tool name starts with, ends with, or contains any green keyword
        for keyword in green_keywords {
            if tool_name == keyword
                || tool_name.starts_with(&format!("{}_", keyword))
                || tool_name.ends_with(&format!("_{}", keyword))
                || tool_name.contains(&format!("_{}_", keyword))
            {
                return true;
            }
        }

        false
    }

    /// Infer action type from tool name
    fn infer_action_type_from_name(tool_name: &str) -> ActionType {
        // Check for exact matches first (highest priority)
        match tool_name {
            "read_file" => ActionType::ReadFile,
            "read_resource" | "read_prompt" => ActionType::ViewFile,
            "list_files" | "list_directory" | "list_directories" | "list_resources"
            | "list_prompts" => ActionType::ListDirectory,
            "search_files" | "grep" => ActionType::SearchWeb,
            "get_file_info" | "stat" | "get_info" | "info" | "status" => ActionType::ViewFile,
            "check_file" => ActionType::CheckLogs,
            _ => {
                // Pattern matching for prefixed/suffixed tool names
                // Check for list-related keywords
                if tool_name == "list"
                    || tool_name.starts_with("list_")
                    || tool_name.ends_with("_list")
                    || tool_name.contains("_list_")
                {
                    return ActionType::ListDirectory;
                }

                // Check for read-related keywords (but not resource/prompt)
                if tool_name == "read"
                    || tool_name.starts_with("read_")
                    || tool_name.ends_with("_read")
                    || tool_name.contains("_read_")
                {
                    // read_resource and read_prompt are handled above, so any other read_ is ReadFile
                    return ActionType::ReadFile;
                }

                // Check for view-related keywords
                if tool_name == "view"
                    || tool_name.starts_with("view_")
                    || tool_name.ends_with("_view")
                    || tool_name.contains("_view_")
                    || tool_name == "view_file"
                {
                    return ActionType::ViewFile;
                }

                // Check for search-related keywords
                if tool_name == "search"
                    || tool_name.starts_with("search_")
                    || tool_name.ends_with("_search")
                    || tool_name.contains("_search_")
                    || tool_name == "find"
                    || tool_name == "locate"
                {
                    return ActionType::SearchWeb;
                }

                // Check for monitor
                if tool_name == "monitor" {
                    return ActionType::CheckLogs;
                }

                // Default to ReadFile
                ActionType::ReadFile
            }
        }
    }

    /// Infer Red action type from tool name and parameters
    fn infer_red_action_type(tool_name: &str, arguments: &Value) -> Option<ActionType> {
        let tool_name_lower = tool_name.to_lowercase();

        // Delete/remove operations (Critical risk)
        if tool_name_lower.contains("delete")
            || tool_name_lower.contains("remove")
            || tool_name_lower.contains("unlink")
            || tool_name_lower.contains("rm")
        {
            return Some(ActionType::DeleteFile);
        }

        // Transfer/financial operations (Critical risk)
        if tool_name_lower.contains("transfer")
            || tool_name_lower.contains("pay")
            || tool_name_lower.contains("withdraw")
            || tool_name_lower.contains("deposit")
            || tool_name_lower.contains("send_payment")
            || tool_name_lower.contains("crypto")
            || tool_name_lower.contains("bitcoin")
            || tool_name_lower.contains("ethereum")
        {
            return Some(ActionType::TransferAsset);
        }

        // Execute/run operations (High risk)
        if tool_name_lower.contains("execute")
            || tool_name_lower.contains("run")
            || tool_name_lower.contains("exec")
            || tool_name_lower.contains("spawn")
            || tool_name_lower.contains("launch")
            || tool_name_lower.contains("start")
        {
            return Some(ActionType::ExecuteCommand);
        }

        // System modifications (High risk) - check this BEFORE edit/modify
        if tool_name_lower.contains("system")
            || tool_name_lower.contains("config")
            || tool_name_lower.contains("setting")
        {
            return Some(ActionType::ModifySystem);
        }

        // Write/create/modify operations (High risk)
        // Use word boundaries to avoid false positives
        if tool_name_lower == "write"
            || tool_name_lower.starts_with("write_")
            || tool_name_lower.ends_with("_write")
            || tool_name_lower.contains("_write_")
            || tool_name_lower == "edit"
            || tool_name_lower.starts_with("edit_")
            || tool_name_lower.ends_with("_edit")
            || tool_name_lower.contains("_edit_")
            || tool_name_lower == "modify"
            || tool_name_lower.starts_with("modify_")
            || tool_name_lower.ends_with("_modify")
            || tool_name_lower.contains("_modify_")
            || tool_name_lower == "update"
            || tool_name_lower.starts_with("update_")
            || tool_name_lower.ends_with("_update")
            || tool_name_lower.contains("_update_")
            || tool_name_lower == "change"
            || tool_name_lower.starts_with("change_")
            || tool_name_lower.ends_with("_change")
            || tool_name_lower.contains("_change_")
            || tool_name_lower.contains("patch")
        {
            return Some(ActionType::EditFile);
        }

        // Send/post/communicate operations (Medium risk)
        // Use word boundaries for 'post' to avoid false positives with 'post' in other words
        if tool_name_lower.contains("send")
            || tool_name_lower == "post"
            || tool_name_lower.starts_with("post_")
            || tool_name_lower.ends_with("_post")
            || tool_name_lower.contains("_post_")
            || tool_name_lower.contains("message")
            || tool_name_lower.contains("email")
            || tool_name_lower.contains("mail")
            || tool_name_lower.contains("slack")
            || tool_name_lower.contains("discord")
            || tool_name_lower.contains("telegram")
            || tool_name_lower.contains("whatsapp")
        {
            return Some(ActionType::SendEmail);
        }

        // Create operations (Medium risk)
        if tool_name_lower.contains("create")
            || tool_name_lower.contains("new")
            || tool_name_lower.contains("add")
            || tool_name_lower.contains("insert")
        {
            return Some(ActionType::CreateFile);
        }

        // Deploy/publish operations (Medium risk)
        if tool_name_lower.contains("publish") {
            return Some(ActionType::Publish);
        }
        if tool_name_lower.contains("deploy") || tool_name_lower.contains("release") {
            return Some(ActionType::Deploy);
        }

        // External API calls (Medium risk)
        if tool_name_lower.contains("call")
            || tool_name_lower.contains("request")
            || tool_name_lower.contains("invoke")
            || tool_name_lower.contains("api")
        {
            // Check if this is a known read-only external call
            if Self::is_read_only_external_call(&tool_name_lower) {
                return None; // Treat as Green
            }
            return Some(ActionType::ExternalCall);
        }

        // Install/uninstall operations (Low risk)
        if tool_name_lower.contains("install")
            || tool_name_lower.contains("uninstall")
            || tool_name_lower.contains("setup")
            || tool_name_lower.contains("configure")
        {
            return Some(ActionType::Install);
        }

        // Version control operations (Low risk)
        if tool_name_lower.contains("commit") {
            return Some(ActionType::Commit);
        }
        if tool_name_lower.contains("push") {
            return Some(ActionType::Push);
        }

        // Parameter-based classification
        Self::classify_by_parameters(arguments)
    }

    /// Check if an external call is read-only (safe)
    fn is_read_only_external_call(tool_name: &str) -> bool {
        // Check for read-only prefixes
        if tool_name.starts_with("get")
            || tool_name.starts_with("read")
            || tool_name.starts_with("list")
            || tool_name.starts_with("fetch")
            || tool_name.starts_with("query")
            || tool_name.starts_with("search")
        {
            return true;
        }

        // Check for read-only anywhere in the name
        if tool_name.contains("_get_")
            || tool_name.contains("_read_")
            || tool_name.contains("_list_")
            || tool_name.contains("_fetch_")
            || tool_name.contains("_query_")
            || tool_name.contains("_search_")
        {
            return true;
        }

        false
    }

    /// Classify based on parameter analysis
    fn classify_by_parameters(arguments: &Value) -> Option<ActionType> {
        if let Some(obj) = arguments.as_object() {
            // Check for dangerous parameters
            if obj.contains_key("delete")
                || obj.contains_key("remove")
                || obj.contains_key("destructive")
                || obj.contains_key("force")
            {
                return Some(ActionType::DeleteFile);
            }

            // Check for write operations
            if obj.contains_key("write")
                || obj.contains_key("content")
                || obj.contains_key("data")
                || obj.contains_key("body")
            {
                return Some(ActionType::EditFile);
            }

            // Check for financial operations
            if obj.contains_key("amount")
                || obj.contains_key("payment")
                || obj.contains_key("transfer")
                || obj.contains_key("crypto")
            {
                return Some(ActionType::TransferAsset);
            }
        }

        None
    }

    /// Classify a batch of tool calls
    ///
    /// Returns RED if any tool in the batch requires approval.
    pub fn classify_batch(tools: Vec<(&str, &Value)>) -> ClassificationResult {
        for (tool_name, arguments) in tools {
            let result = Self::classify_tool(tool_name, arguments);
            if result.requires_approval {
                return ClassificationResult {
                    requires_approval: true,
                    action_type: result.action_type,
                    risk_level: result.risk_level,
                    reason: format!(
                        "Batch contains tool '{}' which requires approval",
                        tool_name
                    ),
                };
            }
        }

        ClassificationResult {
            requires_approval: false,
            action_type: ActionType::ReadFile,
            risk_level: RiskLevel::None,
            reason: "All tools in batch are Green actions".to_string(),
        }
    }

    /// Get a list of known Green tools
    pub fn get_known_green_tools() -> Vec<&'static str> {
        vec![
            "read_file",
            "list_files",
            "list_directory",
            "list_directories",
            "search_files",
            "search",
            "get_file_info",
            "check_file",
            "read_resource",
            "list_resources",
            "read_prompt",
            "list_prompts",
        ]
    }

    /// Get a list of known Red tools
    pub fn get_known_red_tools() -> Vec<&'static str> {
        vec![
            "write_file",
            "edit_file",
            "delete_file",
            "create_file",
            "execute_command",
            "run_script",
            "send_email",
            "transfer_asset",
            "deploy",
            "install",
            "commit",
            "push",
            "publish",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Green action tests
    #[test]
    fn test_classify_read_file() {
        let result = ToolClassifier::classify_tool("read_file", &json!({"path": "test.txt"}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ReadFile);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_list_directory() {
        let result = ToolClassifier::classify_tool("list_directory", &json!({"path": "/tmp"}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ListDirectory);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_search() {
        let result = ToolClassifier::classify_tool("search", &json!({"query": "test"}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::SearchWeb);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_check_logs() {
        let result = ToolClassifier::classify_tool("check_file", &json!({"path": "log.txt"}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::CheckLogs);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_view_file() {
        let result = ToolClassifier::classify_tool("view", &json!({"path": "test.txt"}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ViewFile);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_list_resources() {
        let result = ToolClassifier::classify_tool("list_resources", &json!({}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ListDirectory);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_read_resource() {
        let result = ToolClassifier::classify_tool("read_resource", &json!({"uri": "file:///tmp"}));
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ViewFile);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    // Red action tests
    #[test]
    fn test_classify_write_file() {
        let result = ToolClassifier::classify_tool(
            "write_file",
            &json!({"path": "test.txt", "content": "hello"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::EditFile);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_edit_file() {
        let result =
            ToolClassifier::classify_tool("edit_file", &json!({"path": "test.txt", "changes": []}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::EditFile);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_delete_file() {
        let result = ToolClassifier::classify_tool("delete_file", &json!({"path": "test.txt"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::DeleteFile);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_classify_remove_file() {
        let result = ToolClassifier::classify_tool("remove_file", &json!({"path": "test.txt"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::DeleteFile);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_classify_create_file() {
        let result = ToolClassifier::classify_tool("create_file", &json!({"path": "new.txt"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::CreateFile);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_classify_execute_command() {
        let result =
            ToolClassifier::classify_tool("execute_command", &json!({"command": "ls -la"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::ExecuteCommand);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_run_script() {
        let result = ToolClassifier::classify_tool("run_script", &json!({"script": "test.sh"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::ExecuteCommand);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_send_email() {
        let result = ToolClassifier::classify_tool(
            "send_email",
            &json!({"to": "test@example.com", "subject": "test"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::SendEmail);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_classify_transfer_asset() {
        let result = ToolClassifier::classify_tool(
            "transfer_crypto",
            &json!({"to": "0x123", "amount": "1.0"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::TransferAsset);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_classify_deploy() {
        let result = ToolClassifier::classify_tool("deploy", &json!({"environment": "prod"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::Deploy);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_classify_install() {
        let result = ToolClassifier::classify_tool("install", &json!({"package": "test"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::Install);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_classify_commit() {
        let result = ToolClassifier::classify_tool("commit", &json!({"message": "test"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::Commit);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_classify_push() {
        let result = ToolClassifier::classify_tool("push", &json!({"remote": "origin"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::Push);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_classify_publish() {
        let result = ToolClassifier::classify_tool("publish", &json!({"target": "npm"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::Publish);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }

    // Unknown tool test
    #[test]
    fn test_classify_unknown_tool() {
        let result = ToolClassifier::classify_tool("unknown_tool_xyz", &json!({}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::Unknown);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    // Case insensitivity test
    #[test]
    fn test_classify_case_insensitive() {
        let result1 = ToolClassifier::classify_tool("READ_FILE", &json!({"path": "test.txt"}));
        let result2 = ToolClassifier::classify_tool("Read_File", &json!({"path": "test.txt"}));
        let result3 = ToolClassifier::classify_tool("read_file", &json!({"path": "test.txt"}));

        assert!(!result1.requires_approval);
        assert!(!result2.requires_approval);
        assert!(!result3.requires_approval);

        assert_eq!(result1.action_type, result2.action_type);
        assert_eq!(result2.action_type, result3.action_type);
    }

    // Batch classification test
    #[test]
    fn test_classify_batch_all_green() {
        let arg1 = json!({"path": "test.txt"});
        let arg2 = json!({"path": "/tmp"});
        let arg3 = json!({"query": "test"});

        let tools = vec![
            ("read_file", &arg1),
            ("list_directory", &arg2),
            ("search", &arg3),
        ];

        let result = ToolClassifier::classify_batch(tools);
        assert!(!result.requires_approval);
        assert_eq!(result.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_classify_batch_contains_red() {
        let arg1 = json!({"path": "test.txt"});
        let arg2 = json!({"path": "dangerous.txt"});
        let arg3 = json!({"path": "/tmp"});

        let tools = vec![
            ("read_file", &arg1),
            ("delete_file", &arg2),
            ("list_directory", &arg3),
        ];

        let result = ToolClassifier::classify_batch(tools);
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::DeleteFile);
        assert!(result.reason.contains("delete_file"));
    }

    // Slack-specific tool tests
    #[test]
    fn test_classify_slack_send_message() {
        let result = ToolClassifier::classify_tool(
            "slack_send_message",
            &json!({"channel": "#general", "text": "hello"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::SendEmail);
    }

    // GitHub-specific tool tests
    #[test]
    fn test_classify_github_create_issue() {
        let result = ToolClassifier::classify_tool(
            "github_create_issue",
            &json!({"owner": "test", "repo": "test", "title": "test"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::CreateFile);
    }

    #[test]
    fn test_classify_github_list_issues() {
        let result = ToolClassifier::classify_tool(
            "github_list_issues",
            &json!({"owner": "test", "repo": "test"}),
        );
        // List operations should be Green (read-only)
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ListDirectory);
    }

    // Parameter-based classification tests
    #[test]
    fn test_classify_by_parameters_destructive() {
        let result =
            ToolClassifier::classify_tool("some_tool", &json!({"delete": true, "force": true}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::DeleteFile);
    }

    #[test]
    fn test_classify_by_parameters_write() {
        let result =
            ToolClassifier::classify_tool("some_tool", &json!({"write": true, "content": "test"}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::EditFile);
    }

    #[test]
    fn test_classify_by_parameters_transfer() {
        let result = ToolClassifier::classify_tool(
            "some_tool",
            &json!({"amount": "100", "transfer": "to@example.com"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::TransferAsset);
    }

    // Known tools lists
    #[test]
    fn test_get_known_green_tools() {
        let green_tools = ToolClassifier::get_known_green_tools();
        assert!(green_tools.contains(&"read_file"));
        assert!(green_tools.contains(&"list_directory"));
        assert!(green_tools.contains(&"search"));
        assert!(!green_tools.contains(&"delete_file"));
    }

    #[test]
    fn test_get_known_red_tools() {
        let red_tools = ToolClassifier::get_known_red_tools();
        assert!(red_tools.contains(&"write_file"));
        assert!(red_tools.contains(&"delete_file"));
        assert!(red_tools.contains(&"send_email"));
        assert!(!red_tools.contains(&"read_file"));
    }

    // Classification reason tests
    #[test]
    fn test_green_action_reason() {
        let result = ToolClassifier::classify_tool("read_file", &json!({"path": "test.txt"}));
        assert!(result.reason.contains("read-only operation"));
        assert!(result.reason.contains("Green action"));
    }

    #[test]
    fn test_red_action_reason() {
        let result = ToolClassifier::classify_tool("delete_file", &json!({"path": "test.txt"}));
        assert!(result.reason.contains("destructive"));
        assert!(result.reason.contains("Red action"));
    }

    #[test]
    fn test_unknown_tool_reason() {
        let result = ToolClassifier::classify_tool("unknown_xyz", &json!({}));
        assert!(result.reason.contains("Unknown tool"));
        assert!(result.reason.contains("requires approval"));
    }

    // Risk level tests
    #[test]
    fn test_green_tools_have_no_risk() {
        let green_tools = ToolClassifier::get_known_green_tools();
        for tool in green_tools {
            let result = ToolClassifier::classify_tool(tool, &json!({}));
            assert_eq!(result.risk_level, RiskLevel::None);
        }
    }

    #[test]
    fn test_delete_tools_have_critical_risk() {
        let result1 = ToolClassifier::classify_tool("delete_file", &json!({}));
        let result2 = ToolClassifier::classify_tool("transfer_crypto", &json!({}));
        assert_eq!(result1.risk_level, RiskLevel::Critical);
        assert_eq!(result2.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_edit_tools_have_high_risk() {
        let result1 = ToolClassifier::classify_tool("edit_file", &json!({}));
        let result2 = ToolClassifier::classify_tool("execute_command", &json!({}));
        assert_eq!(result1.risk_level, RiskLevel::High);
        assert_eq!(result2.risk_level, RiskLevel::High);
    }

    // External API call tests
    #[test]
    fn test_read_only_external_call() {
        let result = ToolClassifier::classify_tool(
            "api_get_data",
            &json!({"url": "https://api.example.com/data"}),
        );
        assert!(!result.requires_approval);
        assert_eq!(result.action_type, ActionType::ReadFile);
    }

    #[test]
    fn test_write_external_call() {
        // Use invoke instead of post to avoid SendEmail classification
        let result = ToolClassifier::classify_tool(
            "api_invoke_method",
            &json!({"url": "https://api.example.com/data"}),
        );
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::ExternalCall);
    }

    // System modification tests
    #[test]
    fn test_system_modification_requires_approval() {
        let result = ToolClassifier::classify_tool("modify_system_config", &json!({}));
        assert!(result.requires_approval);
        assert_eq!(result.action_type, ActionType::ModifySystem);
    }
}
