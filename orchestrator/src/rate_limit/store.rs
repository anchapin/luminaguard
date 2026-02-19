//! Quota Store
//!
//! In-memory and persistent storage for quota data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::quota::{Quota, QuotaType, UsageStats};

/// Key for identifying a quota owner
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuotaKey {
    /// Entity type (user or agent)
    pub entity_type: EntityType,

    /// Entity ID
    pub entity_id: String,

    /// Quota type
    pub quota_type: QuotaType,
}

impl QuotaKey {
    /// Create a new quota key
    pub fn new(entity_type: EntityType, entity_id: String, quota_type: QuotaType) -> Self {
        Self {
            entity_type,
            entity_id,
            quota_type,
        }
    }

    /// Create a key for a user's API quota
    pub fn user_api(user_id: &str) -> Self {
        Self::new(EntityType::User, user_id.to_string(), QuotaType::ApiCalls)
    }

    /// Create a key for a user's VM spawn quota
    pub fn user_vm(user_id: &str) -> Self {
        Self::new(EntityType::User, user_id.to_string(), QuotaType::VmSpawn)
    }

    /// Create a key for a user's approval quota
    pub fn user_approval(user_id: &str) -> Self {
        Self::new(
            EntityType::User,
            user_id.to_string(),
            QuotaType::ApprovalRequest,
        )
    }

    /// Create a key for an agent's API quota
    pub fn agent_api(agent_id: &str) -> Self {
        Self::new(EntityType::Agent, agent_id.to_string(), QuotaType::ApiCalls)
    }

    /// Create a key for an agent's VM spawn quota
    pub fn agent_vm(agent_id: &str) -> Self {
        Self::new(EntityType::Agent, agent_id.to_string(), QuotaType::VmSpawn)
    }

    /// Create a key for an agent's approval quota
    pub fn agent_approval(agent_id: &str) -> Self {
        Self::new(
            EntityType::Agent,
            agent_id.to_string(),
            QuotaType::ApprovalRequest,
        )
    }
}

/// Type of entity that owns a quota
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum EntityType {
    /// User quota
    User,
    /// Agent quota
    Agent,
}

/// In-memory quota store
#[derive(Debug, Clone)]
pub struct QuotaStore {
    /// Quota storage
    quotas: Arc<RwLock<HashMap<QuotaKey, Quota>>>,

    /// Usage history (for analytics)
    usage_history: Arc<RwLock<Vec<UsageRecord>>>,
}

/// Record of quota usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Quota key
    pub key: QuotaKey,

    /// Timestamp of usage
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Amount consumed
    pub amount: u32,

    /// Whether it was successful
    pub success: bool,
}

impl QuotaStore {
    /// Create a new quota store
    pub fn new() -> Self {
        Self {
            quotas: Arc::new(RwLock::new(HashMap::new())),
            usage_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get or create a quota for the given key
    pub async fn get_or_create(&self, key: &QuotaKey) -> Quota {
        let mut quotas = self.quotas.write().await;

        if !quotas.contains_key(key) {
            let quota_id = format!(
                "{:?}-{}-{:?}",
                key.entity_type, key.entity_id, key.quota_type
            );
            let quota = Quota::new(key.quota_type, quota_id);
            quotas.insert(key.clone(), quota);
        }

        quotas.get(key).cloned().unwrap()
    }

    /// Get a quota if it exists
    pub async fn get(&self, key: &QuotaKey) -> Option<Quota> {
        let quotas = self.quotas.read().await;
        quotas.get(key).cloned()
    }

    /// Set a quota
    pub async fn set(&self, key: QuotaKey, quota: Quota) {
        let mut quotas = self.quotas.write().await;
        quotas.insert(key, quota);
    }

    /// Remove a quota
    pub async fn remove(&self, key: &QuotaKey) -> Option<Quota> {
        let mut quotas = self.quotas.write().await;
        quotas.remove(key)
    }

    /// Record usage
    pub async fn record_usage(&self, key: &QuotaKey, amount: u32, success: bool) {
        let record = UsageRecord {
            key: key.clone(),
            timestamp: chrono::Utc::now(),
            amount,
            success,
        };

        let mut history = self.usage_history.write().await;
        history.push(record);

        // Keep only last 10000 records
        if history.len() > 10000 {
            let excess = history.len() - 10000;
            history.drain(0..excess);
        }
    }

    /// Get usage history for a key
    pub async fn get_usage_history(&self, key: &QuotaKey) -> Vec<UsageRecord> {
        let history = self.usage_history.read().await;
        history.iter().filter(|r| &r.key == key).cloned().collect()
    }

    /// Get all usage history
    pub async fn get_all_usage_history(&self) -> Vec<UsageRecord> {
        let history = self.usage_history.read().await;
        history.clone()
    }

    /// Get all quotas
    pub async fn get_all_quotas(&self) -> HashMap<QuotaKey, Quota> {
        let quotas = self.quotas.read().await;
        quotas.clone()
    }

    /// Get quotas for a specific entity
    pub async fn get_entity_quotas(&self, entity_type: EntityType, entity_id: &str) -> Vec<Quota> {
        let quotas = self.quotas.read().await;
        quotas
            .iter()
            .filter(|(k, _)| k.entity_type == entity_type && k.entity_id == entity_id)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Get usage stats for all quotas
    pub async fn get_all_stats(&self) -> Vec<UsageStats> {
        let quotas = self.quotas.read().await;
        quotas.values().map(|q| q.usage_stats()).collect()
    }

    /// Reset all period usage
    pub async fn reset_all_periods(&self) {
        let quotas = self.quotas.read().await;
        for quota in quotas.values() {
            quota.reset_period();
        }
    }

    /// Clear all quotas
    pub async fn clear(&self) {
        let mut quotas = self.quotas.write().await;
        quotas.clear();
    }

    /// Get quota count
    pub async fn count(&self) -> usize {
        let quotas = self.quotas.read().await;
        quotas.len()
    }
}

impl Default for QuotaStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quota_store_creation() {
        let store = QuotaStore::new();
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_get_or_create() {
        let store = QuotaStore::new();
        let key = QuotaKey::user_api("user-123");

        let quota = store.get_or_create(&key).await;
        assert_eq!(quota.quota_type, QuotaType::ApiCalls);
        assert_eq!(store.count().await, 1);
    }

    #[tokio::test]
    async fn test_get_existing() {
        let store = QuotaStore::new();
        let key = QuotaKey::user_api("user-123");

        // Create quota
        store.get_or_create(&key).await;

        // Get existing
        let quota = store.get(&key).await;
        assert!(quota.is_some());
    }

    #[tokio::test]
    async fn test_remove() {
        let store = QuotaStore::new();
        let key = QuotaKey::user_api("user-123");

        store.get_or_create(&key).await;
        assert_eq!(store.count().await, 1);

        let removed = store.remove(&key).await;
        assert!(removed.is_some());
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_record_usage() {
        let store = QuotaStore::new();
        let key = QuotaKey::user_api("user-123");

        store.record_usage(&key, 10, true).await;
        store.record_usage(&key, 5, false).await;

        let history = store.get_usage_history(&key).await;
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_entity_quotas() {
        let store = QuotaStore::new();

        // Create quotas for user
        store.get_or_create(&QuotaKey::user_api("user-123")).await;
        store.get_or_create(&QuotaKey::user_vm("user-123")).await;

        // Create quota for different user
        store.get_or_create(&QuotaKey::user_api("user-456")).await;

        let quotas = store.get_entity_quotas(EntityType::User, "user-123").await;
        assert_eq!(quotas.len(), 2);
    }

    #[tokio::test]
    async fn test_reset_all_periods() {
        let store = QuotaStore::new();

        let key1 = QuotaKey::user_api("user-123");
        let key2 = QuotaKey::user_vm("user-123");

        let quota1 = store.get_or_create(&key1).await;
        let quota2 = store.get_or_create(&key2).await;

        quota1.try_consume(10);
        quota2.try_consume(5);

        store.reset_all_periods().await;

        let stats1 = quota1.usage_stats();
        let stats2 = quota2.usage_stats();

        assert_eq!(stats1.period_usage, 0);
        assert_eq!(stats2.period_usage, 0);
    }

    #[tokio::test]
    async fn test_quota_key_helpers() {
        let key = QuotaKey::user_api("user-1");
        assert_eq!(key.entity_type, EntityType::User);
        assert_eq!(key.quota_type, QuotaType::ApiCalls);

        let key = QuotaKey::agent_vm("agent-1");
        assert_eq!(key.entity_type, EntityType::Agent);
        assert_eq!(key.quota_type, QuotaType::VmSpawn);
    }
}
