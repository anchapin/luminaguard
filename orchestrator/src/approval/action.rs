//! Action Classification for Approval Cliff
//!
//! This module classifies actions as "Green" (autonomous, safe) or "Red" (requires approval).
//! Unknown actions default to RED for safety (fail-secure principle).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Action type with detailed classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionType {
    // Green Actions (Autonomous - no approval needed)
    /// Read file contents
    ReadFile,
    /// List directory contents
    ListDirectory,
    /// Search the web or knowledge bases
    SearchWeb,
    /// Check system logs
    CheckLogs,
    /// Get system information
    GetSystemInfo,
    /// View file contents
    ViewFile,
    /// Display information
    DisplayInfo,
    /// Find/locate resources
    Find,
    /// Query data
    Query,
    /// Fetch remote data
    Fetch,
    /// Inspect objects
    Inspect,
    /// Examine resources
    Examine,
    /// Monitor status
    Monitor,
    /// Get status information
    Status,

    // Red Actions (Require approval)
    /// Create new file
    CreateFile,
    /// Edit/modify file
    EditFile,
    /// Delete file
    DeleteFile,
    /// Execute command
    ExecuteCommand,
    /// Send email or message
    SendEmail,
    /// Transfer assets or funds
    TransferAsset,
    /// Modify system configuration
    ModifySystem,
    /// Make external API call
    ExternalCall,
    /// Run script or program
    RunScript,
    /// Deploy code or container
    Deploy,
    /// Install/uninstall software
    Install,
    /// Commit to version control
    Commit,
    /// Push changes to remote
    Push,
    /// Publish content
    Publish,

    /// Unknown action type (defaults to RED for safety)
    Unknown,
}

impl ActionType {
    /// Determine if this action type requires approval
    ///
    /// Returns:
    /// - `false` for Green actions (safe, autonomous)
    /// - `true` for Red actions (destructive, external)
    ///
    /// # Principle: Conservative Default
    /// Unknown actions always require approval (fail-secure).
    pub fn requires_approval(self) -> bool {
        matches!(
            self,
            ActionType::CreateFile
                | ActionType::EditFile
                | ActionType::DeleteFile
                | ActionType::ExecuteCommand
                | ActionType::SendEmail
                | ActionType::TransferAsset
                | ActionType::ModifySystem
                | ActionType::ExternalCall
                | ActionType::RunScript
                | ActionType::Deploy
                | ActionType::Install
                | ActionType::Commit
                | ActionType::Push
                | ActionType::Publish
                | ActionType::Unknown
        )
    }

    /// Classify action from a string description (keyword matching)
    ///
    /// Uses keyword matching to determine action type.
    /// Returns `Unknown` if no keywords match.
    pub fn from_description(description: &str) -> Self {
        let lower = description.to_lowercase();

        // Check for Green keywords first
        for keyword in &[
            "read", "list", "search", "check", "get", "show", "view", "display", "find", "locate",
            "query", "fetch", "inspect", "examine", "monitor", "status", "info",
        ] {
            if lower.contains(keyword) {
                return match *keyword {
                    "read" => ActionType::ReadFile,
                    "list" => ActionType::ListDirectory,
                    "search" => ActionType::SearchWeb,
                    "check" => ActionType::CheckLogs,
                    "get" | "show" | "view" | "display" | "info" => ActionType::ViewFile,
                    "find" | "locate" => ActionType::Find,
                    "query" => ActionType::Query,
                    "fetch" => ActionType::Fetch,
                    "inspect" => ActionType::Inspect,
                    "examine" => ActionType::Examine,
                    "monitor" => ActionType::Monitor,
                    "status" => ActionType::Status,
                    _ => ActionType::GetSystemInfo,
                };
            }
        }

        // Check for Red keywords
        for keyword in &[
            "delete",
            "remove",
            "write",
            "edit",
            "modify",
            "create",
            "update",
            "change",
            "send",
            "post",
            "transfer",
            "execute",
            "run",
            "deploy",
            "install",
            "uninstall",
            "commit",
            "push",
            "publish",
        ] {
            if lower.contains(keyword) {
                return match *keyword {
                    "create" => ActionType::CreateFile,
                    "write" | "edit" | "modify" | "update" | "change" => ActionType::EditFile,
                    "delete" | "remove" => ActionType::DeleteFile,
                    "send" | "post" => ActionType::SendEmail,
                    "transfer" => ActionType::TransferAsset,
                    "execute" => ActionType::ExecuteCommand,
                    "run" => ActionType::RunScript,
                    "deploy" => ActionType::Deploy,
                    "install" => ActionType::Install,
                    "uninstall" => ActionType::Install,
                    "commit" => ActionType::Commit,
                    "push" => ActionType::Push,
                    "publish" => ActionType::Publish,
                    _ => ActionType::ExternalCall,
                };
            }
        }

        // Default: Unknown (requires approval)
        ActionType::Unknown
    }

    /// Get the risk level of this action type
    pub fn risk_level(self) -> RiskLevel {
        match self {
            // Critical risk (immediate harm)
            ActionType::DeleteFile | ActionType::TransferAsset => RiskLevel::Critical,

            // High risk (significant impact)
            ActionType::EditFile | ActionType::ExecuteCommand | ActionType::ModifySystem => {
                RiskLevel::High
            }

            // Medium risk (moderate impact)
            ActionType::CreateFile
            | ActionType::SendEmail
            | ActionType::ExternalCall
            | ActionType::Deploy => RiskLevel::Medium,

            // Low risk (minimal impact)
            ActionType::RunScript | ActionType::Install | ActionType::Commit | ActionType::Push => {
                RiskLevel::Low
            }

            // Green actions (no risk, but include for completeness)
            ActionType::ReadFile
            | ActionType::ListDirectory
            | ActionType::SearchWeb
            | ActionType::CheckLogs
            | ActionType::GetSystemInfo
            | ActionType::ViewFile
            | ActionType::DisplayInfo
            | ActionType::Find
            | ActionType::Query
            | ActionType::Fetch
            | ActionType::Inspect
            | ActionType::Examine
            | ActionType::Monitor
            | ActionType::Status => RiskLevel::None,

            // Unknown - treat as Critical for safety
            ActionType::Unknown => RiskLevel::Critical,

            ActionType::Publish => RiskLevel::Medium,
        }
    }
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionType::ReadFile => write!(f, "ReadFile"),
            ActionType::ListDirectory => write!(f, "ListDirectory"),
            ActionType::SearchWeb => write!(f, "SearchWeb"),
            ActionType::CheckLogs => write!(f, "CheckLogs"),
            ActionType::GetSystemInfo => write!(f, "GetSystemInfo"),
            ActionType::ViewFile => write!(f, "ViewFile"),
            ActionType::DisplayInfo => write!(f, "DisplayInfo"),
            ActionType::Find => write!(f, "Find"),
            ActionType::Query => write!(f, "Query"),
            ActionType::Fetch => write!(f, "Fetch"),
            ActionType::Inspect => write!(f, "Inspect"),
            ActionType::Examine => write!(f, "Examine"),
            ActionType::Monitor => write!(f, "Monitor"),
            ActionType::Status => write!(f, "Status"),
            ActionType::CreateFile => write!(f, "CreateFile"),
            ActionType::EditFile => write!(f, "EditFile"),
            ActionType::DeleteFile => write!(f, "DeleteFile"),
            ActionType::ExecuteCommand => write!(f, "ExecuteCommand"),
            ActionType::SendEmail => write!(f, "SendEmail"),
            ActionType::TransferAsset => write!(f, "TransferAsset"),
            ActionType::ModifySystem => write!(f, "ModifySystem"),
            ActionType::ExternalCall => write!(f, "ExternalCall"),
            ActionType::RunScript => write!(f, "RunScript"),
            ActionType::Deploy => write!(f, "Deploy"),
            ActionType::Install => write!(f, "Install"),
            ActionType::Commit => write!(f, "Commit"),
            ActionType::Push => write!(f, "Push"),
            ActionType::Publish => write!(f, "Publish"),
            ActionType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Risk level of an action
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// No risk (green actions, read-only)
    None = 0,
    /// Low risk (minimal impact, but requires approval)
    Low = 1,
    /// Medium risk (moderate impact)
    Medium = 2,
    /// High risk (significant impact)
    High = 3,
    /// Critical risk (immediate harm, irreversible)
    Critical = 4,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::None => write!(f, "None"),
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_green_actions_no_approval() {
        let green_actions = vec![
            ActionType::ReadFile,
            ActionType::ListDirectory,
            ActionType::SearchWeb,
            ActionType::CheckLogs,
            ActionType::GetSystemInfo,
            ActionType::ViewFile,
            ActionType::DisplayInfo,
            ActionType::Find,
            ActionType::Query,
            ActionType::Fetch,
            ActionType::Inspect,
            ActionType::Examine,
            ActionType::Monitor,
            ActionType::Status,
        ];

        for action in green_actions {
            assert!(
                !action.requires_approval(),
                "Green action {} should not require approval",
                action
            );
        }
    }

    #[test]
    fn test_red_actions_require_approval() {
        let red_actions = vec![
            ActionType::CreateFile,
            ActionType::EditFile,
            ActionType::DeleteFile,
            ActionType::ExecuteCommand,
            ActionType::SendEmail,
            ActionType::TransferAsset,
            ActionType::ModifySystem,
            ActionType::ExternalCall,
            ActionType::RunScript,
            ActionType::Deploy,
            ActionType::Install,
            ActionType::Commit,
            ActionType::Push,
            ActionType::Publish,
        ];

        for action in red_actions {
            assert!(
                action.requires_approval(),
                "Red action {} should require approval",
                action
            );
        }
    }

    #[test]
    fn test_unknown_requires_approval() {
        assert!(ActionType::Unknown.requires_approval());
    }

    #[test]
    fn test_risk_levels() {
        assert_eq!(ActionType::ReadFile.risk_level(), RiskLevel::None);
        assert_eq!(ActionType::DeleteFile.risk_level(), RiskLevel::Critical);
        assert_eq!(ActionType::EditFile.risk_level(), RiskLevel::High);
        assert_eq!(ActionType::CreateFile.risk_level(), RiskLevel::Medium);
        assert_eq!(ActionType::Install.risk_level(), RiskLevel::Low);
        assert_eq!(ActionType::Unknown.risk_level(), RiskLevel::Critical);
    }

    #[test]
    fn test_classify_from_description() {
        assert_eq!(
            ActionType::from_description("read file"),
            ActionType::ReadFile
        );
        assert_eq!(
            ActionType::from_description("delete file"),
            ActionType::DeleteFile
        );
        assert_eq!(
            ActionType::from_description("edit code"),
            ActionType::EditFile
        );
        assert_eq!(
            ActionType::from_description("send email"),
            ActionType::SendEmail
        );
        assert_eq!(
            ActionType::from_description("unknown action"),
            ActionType::Unknown
        );
    }

    #[test]
    fn test_classify_case_insensitive() {
        assert_eq!(
            ActionType::from_description("READ FILE"),
            ActionType::ReadFile
        );
        assert_eq!(
            ActionType::from_description("Delete File"),
            ActionType::DeleteFile
        );
    }

    #[test]
    fn test_action_type_display() {
        assert_eq!(ActionType::ReadFile.to_string(), "ReadFile");
        assert_eq!(ActionType::DeleteFile.to_string(), "DeleteFile");
        assert_eq!(ActionType::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::None < RiskLevel::Low);
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }
}
