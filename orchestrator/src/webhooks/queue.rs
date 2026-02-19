// Dead letter queue for failed webhook deliveries
//
// Tracks webhook events that failed after all retries for manual review

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Dead letter entry for a failed webhook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    /// Unique ID for this DLQ entry
    pub id: String,
    /// Original webhook ID
    pub webhook_id: String,
    /// Event that failed to deliver
    pub event_type: String,
    /// Event payload
    pub payload: serde_json::Value,
    /// Total delivery attempts
    pub attempt_count: u32,
    /// Timestamp when moved to DLQ
    pub timestamp: DateTime<Utc>,
    /// Last error message
    pub last_error: String,
    /// Status: awaiting_review, reviewed, resolved, archived
    pub status: String,
    /// Optional notes from reviewer
    pub notes: Option<String>,
    /// Number of hours to retain before auto-archiving
    pub retention_hours: u32,
}

impl DeadLetterEntry {
    /// Create a new DLQ entry from a failed delivery
    pub fn new(
        webhook_id: String,
        event_type: String,
        payload: serde_json::Value,
        attempt_count: u32,
        last_error: String,
    ) -> Self {
        Self {
            id: format!("dlq-{}", Uuid::new_v4()),
            webhook_id,
            event_type,
            payload,
            attempt_count,
            timestamp: Utc::now(),
            last_error,
            status: "awaiting_review".to_string(),
            notes: None,
            retention_hours: 168, // 7 days default
        }
    }

    /// Mark this entry as reviewed
    pub fn mark_reviewed(&mut self, notes: String) {
        self.status = "reviewed".to_string();
        self.notes = Some(notes);
    }

    /// Mark this entry as resolved
    pub fn mark_resolved(&mut self) {
        self.status = "resolved".to_string();
    }

    /// Check if this entry should be archived
    pub fn should_archive(&self) -> bool {
        let age_hours = (Utc::now() - self.timestamp).num_hours() as u32;
        age_hours >= self.retention_hours
    }
}

/// Dead letter queue manager
#[derive(Debug, Clone)]
pub struct DeadLetterQueue {
    /// Map of DLQ entry ID to entry
    entries: HashMap<String, DeadLetterEntry>,
}

impl DeadLetterQueue {
    /// Create a new empty DLQ
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add an entry to the DLQ
    pub fn add(&mut self, entry: DeadLetterEntry) -> String {
        let id = entry.id.clone();
        self.entries.insert(id.clone(), entry);
        id
    }

    /// Get an entry by ID
    pub fn get(&self, id: &str) -> Option<DeadLetterEntry> {
        self.entries.get(id).cloned()
    }

    /// Update an entry
    pub fn update(&mut self, entry: DeadLetterEntry) -> bool {
        if self.entries.contains_key(&entry.id) {
            self.entries.insert(entry.id.clone(), entry);
            true
        } else {
            false
        }
    }

    /// List all pending review entries
    pub fn pending_review(&self) -> Vec<DeadLetterEntry> {
        self.entries
            .values()
            .filter(|e| e.status == "awaiting_review")
            .cloned()
            .collect()
    }

    /// List entries for a specific webhook
    pub fn for_webhook(&self, webhook_id: &str) -> Vec<DeadLetterEntry> {
        self.entries
            .values()
            .filter(|e| e.webhook_id == webhook_id)
            .cloned()
            .collect()
    }

    /// Archive old entries
    pub fn archive_old_entries(&mut self) -> usize {
        let to_archive: Vec<String> = self
            .entries
            .values()
            .filter(|e| e.should_archive())
            .map(|e| e.id.clone())
            .collect();

        let count = to_archive.len();
        for id in to_archive {
            self.entries.remove(&id);
        }
        count
    }

    /// Get statistics
    pub fn stats(&self) -> DLQStats {
        let entries = self.entries.values().collect::<Vec<_>>();
        let total = entries.len();
        let awaiting_review = entries
            .iter()
            .filter(|e| e.status == "awaiting_review")
            .count();
        let reviewed = entries.iter().filter(|e| e.status == "reviewed").count();
        let resolved = entries.iter().filter(|e| e.status == "resolved").count();

        DLQStats {
            total,
            awaiting_review,
            reviewed,
            resolved,
        }
    }
}

impl Default for DeadLetterQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// DLQ statistics
#[derive(Debug, Clone, Serialize)]
pub struct DLQStats {
    pub total: usize,
    pub awaiting_review: usize,
    pub reviewed: usize,
    pub resolved: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlq_create_entry() {
        let entry = DeadLetterEntry::new(
            "webhook-123".to_string(),
            "task.completed".to_string(),
            serde_json::json!({"task_id": "task-456"}),
            3,
            "Connection timeout".to_string(),
        );

        assert_eq!(entry.webhook_id, "webhook-123");
        assert_eq!(entry.event_type, "task.completed");
        assert_eq!(entry.attempt_count, 3);
        assert_eq!(entry.status, "awaiting_review");
    }

    #[test]
    fn test_dlq_add_and_get() {
        let mut dlq = DeadLetterQueue::new();
        let entry = DeadLetterEntry::new(
            "webhook-123".to_string(),
            "task.completed".to_string(),
            serde_json::json!({}),
            2,
            "Error".to_string(),
        );

        let id = entry.id.clone();
        dlq.add(entry);

        let retrieved = dlq.get(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().webhook_id, "webhook-123");
    }

    #[test]
    fn test_dlq_pending_review() {
        let mut dlq = DeadLetterQueue::new();

        let entry1 = DeadLetterEntry::new(
            "webhook-1".to_string(),
            "task.completed".to_string(),
            serde_json::json!({}),
            2,
            "Error".to_string(),
        );

        let mut entry2 = DeadLetterEntry::new(
            "webhook-2".to_string(),
            "task.failed".to_string(),
            serde_json::json!({}),
            2,
            "Error".to_string(),
        );
        entry2.status = "reviewed".to_string();

        dlq.add(entry1);
        dlq.add(entry2);

        let pending = dlq.pending_review();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].webhook_id, "webhook-1");
    }

    #[test]
    fn test_dlq_for_webhook() {
        let mut dlq = DeadLetterQueue::new();

        for i in 0..3 {
            dlq.add(DeadLetterEntry::new(
                "webhook-1".to_string(),
                format!("event-{}", i),
                serde_json::json!({}),
                2,
                "Error".to_string(),
            ));
        }

        dlq.add(DeadLetterEntry::new(
            "webhook-2".to_string(),
            "event-other".to_string(),
            serde_json::json!({}),
            1,
            "Error".to_string(),
        ));

        let entries = dlq.for_webhook("webhook-1");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_dlq_stats() {
        let mut dlq = DeadLetterQueue::new();

        dlq.add(DeadLetterEntry::new(
            "webhook-1".to_string(),
            "event-1".to_string(),
            serde_json::json!({}),
            2,
            "Error".to_string(),
        ));

        let mut entry2 = DeadLetterEntry::new(
            "webhook-2".to_string(),
            "event-2".to_string(),
            serde_json::json!({}),
            2,
            "Error".to_string(),
        );
        entry2.status = "resolved".to_string();
        dlq.add(entry2);

        let stats = dlq.stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.awaiting_review, 1);
        assert_eq!(stats.resolved, 1);
    }
}
