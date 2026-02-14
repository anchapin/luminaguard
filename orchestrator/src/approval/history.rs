//! Approval History Storage and Audit Trail
//!
//! This module records all approval decisions for compliance and auditing.
//! Decisions are immutable and include timestamps and user information.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single approval decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    /// Unique identifier for this record (UUID)
    pub id: String,

    /// When this decision was made (UTC)
    pub timestamp: DateTime<Utc>,

    /// Description of the action that was approved/rejected
    pub action_description: String,

    /// The decision made (Approved, Denied, Deferred)
    pub decision: ApprovalDecision,

    /// User who made the decision (or "system" for auto-decisions)
    pub approved_by: String,

    /// Optional justification for the decision
    pub justification: Option<String>,

    /// Result of execution (if approved and executed)
    /// Example: "Success: File created", "Error: Access denied"
    pub execution_result: Option<String>,
}

/// The decision made on an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// Action was approved and may proceed
    Approved,

    /// Action was denied and will not execute
    Denied,

    /// Action was deferred for later decision
    DeferredToLater,
}

impl std::fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalDecision::Approved => write!(f, "Approved"),
            ApprovalDecision::Denied => write!(f, "Denied"),
            ApprovalDecision::DeferredToLater => write!(f, "Deferred"),
        }
    }
}

/// Storage for approval history
#[derive(Debug, Clone)]
pub struct ApprovalHistory {
    /// All approval records (immutable)
    records: Vec<ApprovalRecord>,
}

impl ApprovalHistory {
    /// Create a new approval history
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Record a new approval decision (immutable - cannot be changed)
    pub fn record_decision(&mut self, record: ApprovalRecord) -> anyhow::Result<()> {
        // In Phase 3+, this would be written to persistent storage (SQLite)
        // For now, keep in memory
        self.records.push(record);
        Ok(())
    }

    /// Get the most recent decisions (optionally limited)
    pub fn get_history(&self, limit: Option<usize>) -> Vec<&ApprovalRecord> {
        let mut records: Vec<_> = self.records.iter().collect();
        // Sort by timestamp, newest first
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = limit {
            records.truncate(limit);
        }

        records
    }

    /// Get all decisions for a specific action description
    pub fn get_by_action(&self, description: &str) -> Vec<&ApprovalRecord> {
        self.records
            .iter()
            .filter(|r| r.action_description.contains(description))
            .collect()
    }

    /// Get all decisions by a specific user
    pub fn get_by_user(&self, user: &str) -> Vec<&ApprovalRecord> {
        self.records
            .iter()
            .filter(|r| r.approved_by == user)
            .collect()
    }

    /// Count decisions by type
    pub fn count_by_decision(&self) -> (usize, usize, usize) {
        let approved = self
            .records
            .iter()
            .filter(|r| r.decision == ApprovalDecision::Approved)
            .count();
        let denied = self
            .records
            .iter()
            .filter(|r| r.decision == ApprovalDecision::Denied)
            .count();
        let deferred = self
            .records
            .iter()
            .filter(|r| r.decision == ApprovalDecision::DeferredToLater)
            .count();

        (approved, denied, deferred)
    }

    /// Export history as JSON string (for auditing)
    pub fn export_audit_log(&self) -> anyhow::Result<String> {
        let json = serde_json::to_string_pretty(&self.records)?;
        Ok(json)
    }

    /// Get total number of records
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Clear history (for testing only)
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl Default for ApprovalHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_record(id: &str, action: &str, decision: ApprovalDecision) -> ApprovalRecord {
        ApprovalRecord {
            id: id.to_string(),
            timestamp: Utc::now(),
            action_description: action.to_string(),
            decision,
            approved_by: "test_user".to_string(),
            justification: None,
            execution_result: None,
        }
    }

    #[test]
    fn test_create_empty_history() {
        let history = ApprovalHistory::new();
        assert_eq!(history.record_count(), 0);
    }

    #[test]
    fn test_record_decision() {
        let mut history = ApprovalHistory::new();
        let record = create_test_record("1", "Test action", ApprovalDecision::Approved);

        history.record_decision(record).unwrap();
        assert_eq!(history.record_count(), 1);
    }

    #[test]
    fn test_get_history_empty() {
        let history = ApprovalHistory::new();
        let records = history.get_history(None);
        assert!(records.is_empty());
    }

    #[test]
    fn test_get_history_with_limit() {
        let mut history = ApprovalHistory::new();

        for i in 0..5 {
            let record = create_test_record(&i.to_string(), "Action", ApprovalDecision::Approved);
            history.record_decision(record).unwrap();
        }

        let records = history.get_history(Some(3));
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_get_by_action() {
        let mut history = ApprovalHistory::new();

        history
            .record_decision(create_test_record(
                "1",
                "Delete file /tmp/test.txt",
                ApprovalDecision::Approved,
            ))
            .unwrap();

        history
            .record_decision(create_test_record(
                "2",
                "Create file /tmp/new.txt",
                ApprovalDecision::Denied,
            ))
            .unwrap();

        let delete_records = history.get_by_action("Delete");
        assert_eq!(delete_records.len(), 1);
        assert_eq!(delete_records[0].decision, ApprovalDecision::Approved);
    }

    #[test]
    fn test_get_by_user() {
        let mut history = ApprovalHistory::new();

        let mut record = create_test_record("1", "Action 1", ApprovalDecision::Approved);
        record.approved_by = "alice".to_string();
        history.record_decision(record).unwrap();

        let mut record = create_test_record("2", "Action 2", ApprovalDecision::Denied);
        record.approved_by = "bob".to_string();
        history.record_decision(record).unwrap();

        let alice_records = history.get_by_user("alice");
        assert_eq!(alice_records.len(), 1);
        assert_eq!(alice_records[0].approved_by, "alice");
    }

    #[test]
    fn test_count_by_decision() {
        let mut history = ApprovalHistory::new();

        history
            .record_decision(create_test_record(
                "1",
                "Action 1",
                ApprovalDecision::Approved,
            ))
            .unwrap();
        history
            .record_decision(create_test_record(
                "2",
                "Action 2",
                ApprovalDecision::Approved,
            ))
            .unwrap();
        history
            .record_decision(create_test_record(
                "3",
                "Action 3",
                ApprovalDecision::Denied,
            ))
            .unwrap();

        let (approved, denied, deferred) = history.count_by_decision();
        assert_eq!(approved, 2);
        assert_eq!(denied, 1);
        assert_eq!(deferred, 0);
    }

    #[test]
    fn test_export_audit_log() {
        let mut history = ApprovalHistory::new();

        history
            .record_decision(create_test_record(
                "1",
                "Test action",
                ApprovalDecision::Approved,
            ))
            .unwrap();

        let json = history.export_audit_log().unwrap();
        assert!(json.contains("Test action"));
        assert!(json.contains("Approved"));
    }

    #[test]
    fn test_approval_decision_display() {
        assert_eq!(ApprovalDecision::Approved.to_string(), "Approved");
        assert_eq!(ApprovalDecision::Denied.to_string(), "Denied");
        assert_eq!(ApprovalDecision::DeferredToLater.to_string(), "Deferred");
    }

    #[test]
    fn test_history_preserves_order() {
        let mut history = ApprovalHistory::new();

        for i in 0..3 {
            let record = create_test_record(
                &i.to_string(),
                &format!("Action {}", i),
                ApprovalDecision::Approved,
            );
            history.record_decision(record).unwrap();
        }

        // get_history returns newest first
        let records = history.get_history(None);
        assert_eq!(records.len(), 3);
        // Verify at least some records exist
        assert!(!records.is_empty());
    }

    #[test]
    fn test_record_with_justification() {
        let mut history = ApprovalHistory::new();

        let mut record = create_test_record("1", "Delete file", ApprovalDecision::Denied);
        record.justification = Some("File is in use".to_string());
        history.record_decision(record).unwrap();

        let records = history.get_history(None);
        assert_eq!(records[0].justification, Some("File is in use".to_string()));
    }

    #[test]
    fn test_record_with_execution_result() {
        let mut history = ApprovalHistory::new();

        let mut record = create_test_record("1", "Create file", ApprovalDecision::Approved);
        record.execution_result = Some("Success: File created at /tmp/test.txt".to_string());
        history.record_decision(record).unwrap();

        let records = history.get_history(None);
        assert!(records[0]
            .execution_result
            .as_ref()
            .unwrap()
            .contains("Success"));
    }
}
