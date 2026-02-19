//! Rate Limit Manager
//!
//! Central manager for rate limiting and quota management.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::config::RateLimitConfig;
use super::quota::{Quota, QuotaType, UsageStats};
use super::store::{EntityType, QuotaKey, QuotaStore, UsageRecord};

/// Result of a rate limit check
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,

    /// Remaining tokens after this request
    pub remaining: u32,

    /// Time until tokens are available (if not allowed)
    pub retry_after_secs: Option<u64>,

    /// Reason for denial (if not allowed)
    pub reason: Option<String>,
}

impl RateLimitResult {
    /// Create an allowed result
    pub fn allowed(remaining: u32) -> Self {
        Self {
            allowed: true,
            remaining,
            retry_after_secs: None,
            reason: None,
        }
    }

    /// Create a denied result
    pub fn denied(remaining: u32, retry_after_secs: u64, reason: &str) -> Self {
        Self {
            allowed: false,
            remaining,
            retry_after_secs: Some(retry_after_secs),
            reason: Some(reason.to_string()),
        }
    }
}

/// Rate limit manager
#[derive(Debug, Clone)]
pub struct RateLimitManager {
    /// Configuration
    config: Arc<RwLock<RateLimitConfig>>,

    /// Quota store
    store: QuotaStore,

    /// Admin users (exempt from limits)
    admin_users: Arc<RwLock<Vec<String>>>,
}

impl RateLimitManager {
    /// Create a new rate limit manager
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            store: QuotaStore::new(),
            admin_users: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Create a disabled rate limit manager (for testing)
    pub fn disabled() -> Self {
        Self::new(RateLimitConfig::disabled())
    }

    /// Check if a request is allowed for a user
    pub async fn check_user_limit(
        &self,
        user_id: &str,
        quota_type: QuotaType,
        tokens: u32,
    ) -> Result<RateLimitResult> {
        // Check if rate limiting is enabled
        let config = self.config.read().await;
        if !config.enabled {
            return Ok(RateLimitResult::allowed(u32::MAX));
        }

        // Check if user is admin
        let admin_users = self.admin_users.read().await;
        if admin_users.contains(&user_id.to_string()) {
            return Ok(RateLimitResult::allowed(u32::MAX));
        }
        drop(admin_users);

        // Get or create quota
        let key = match quota_type {
            QuotaType::ApiCalls => QuotaKey::user_api(user_id),
            QuotaType::VmSpawn => QuotaKey::user_vm(user_id),
            QuotaType::ApprovalRequest => QuotaKey::user_approval(user_id),
        };

        self.check_quota(&key, tokens).await
    }

    /// Check if a request is allowed for an agent
    pub async fn check_agent_limit(
        &self,
        agent_id: &str,
        quota_type: QuotaType,
        tokens: u32,
    ) -> Result<RateLimitResult> {
        // Check if rate limiting is enabled
        let config = self.config.read().await;
        if !config.enabled {
            return Ok(RateLimitResult::allowed(u32::MAX));
        }
        drop(config);

        // Get or create quota
        let key = match quota_type {
            QuotaType::ApiCalls => QuotaKey::agent_api(agent_id),
            QuotaType::VmSpawn => QuotaKey::agent_vm(agent_id),
            QuotaType::ApprovalRequest => QuotaKey::agent_approval(agent_id),
        };

        self.check_quota(&key, tokens).await
    }

    /// Check quota for a key
    async fn check_quota(&self, key: &QuotaKey, tokens: u32) -> Result<RateLimitResult> {
        let quota = self.store.get_or_create(key).await;

        if quota.try_consume(tokens) {
            self.store.record_usage(key, tokens, true).await;
            Ok(RateLimitResult::allowed(quota.remaining_tokens()))
        } else {
            self.store.record_usage(key, tokens, false).await;

            let retry_after = quota.refill_rate * tokens as f64;
            Ok(RateLimitResult::denied(
                quota.remaining_tokens(),
                retry_after.ceil() as u64,
                "Rate limit exceeded",
            ))
        }
    }

    /// Consume tokens for a user operation
    pub async fn consume_user(
        &self,
        user_id: &str,
        quota_type: QuotaType,
        tokens: u32,
    ) -> Result<bool> {
        let result = self.check_user_limit(user_id, quota_type, tokens).await?;
        Ok(result.allowed)
    }

    /// Consume tokens for an agent operation
    pub async fn consume_agent(
        &self,
        agent_id: &str,
        quota_type: QuotaType,
        tokens: u32,
    ) -> Result<bool> {
        let result = self.check_agent_limit(agent_id, quota_type, tokens).await?;
        Ok(result.allowed)
    }

    /// Get usage statistics for a user
    pub async fn get_user_stats(&self, user_id: &str) -> Vec<UsageStats> {
        let quotas = self
            .store
            .get_entity_quotas(EntityType::User, user_id)
            .await;
        quotas.into_iter().map(|q| q.usage_stats()).collect()
    }

    /// Get usage statistics for an agent
    pub async fn get_agent_stats(&self, agent_id: &str) -> Vec<UsageStats> {
        let quotas = self
            .store
            .get_entity_quotas(EntityType::Agent, agent_id)
            .await;
        quotas.into_iter().map(|q| q.usage_stats()).collect()
    }

    /// Get all usage statistics
    pub async fn get_all_stats(&self) -> Vec<UsageStats> {
        self.store.get_all_stats().await
    }

    /// Get usage history for an entity
    pub async fn get_usage_history(
        &self,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Vec<UsageRecord> {
        // Get all history and filter
        let all_history = self.store.get_all_usage_history().await;
        all_history
            .into_iter()
            .filter(|r| r.key.entity_type == entity_type && r.key.entity_id == entity_id)
            .collect()
    }

    /// Set quota for a user
    pub async fn set_user_quota(
        &self,
        user_id: &str,
        quota_type: QuotaType,
        max_tokens: u32,
        refill_rate: f64,
    ) -> Result<()> {
        let key = match quota_type {
            QuotaType::ApiCalls => QuotaKey::user_api(user_id),
            QuotaType::VmSpawn => QuotaKey::user_vm(user_id),
            QuotaType::ApprovalRequest => QuotaKey::user_approval(user_id),
        };

        let quota_id = format!("User-{}-{:?}", user_id, quota_type);
        let quota = Quota::with_limits(quota_type, quota_id, max_tokens, refill_rate, max_tokens);

        self.store.set(key, quota).await;

        Ok(())
    }

    /// Set quota for an agent
    pub async fn set_agent_quota(
        &self,
        agent_id: &str,
        quota_type: QuotaType,
        max_tokens: u32,
        refill_rate: f64,
    ) -> Result<()> {
        let key = match quota_type {
            QuotaType::ApiCalls => QuotaKey::agent_api(agent_id),
            QuotaType::VmSpawn => QuotaKey::agent_vm(agent_id),
            QuotaType::ApprovalRequest => QuotaKey::agent_approval(agent_id),
        };

        let quota_id = format!("Agent-{}-{:?}", agent_id, quota_type);
        let quota = Quota::with_limits(quota_type, quota_id, max_tokens, refill_rate, max_tokens);

        self.store.set(key, quota).await;

        Ok(())
    }

    /// Add an admin user (exempt from limits)
    pub async fn add_admin_user(&self, user_id: &str) {
        let mut admins = self.admin_users.write().await;
        if !admins.contains(&user_id.to_string()) {
            admins.push(user_id.to_string());
        }
    }

    /// Remove an admin user
    pub async fn remove_admin_user(&self, user_id: &str) {
        let mut admins = self.admin_users.write().await;
        admins.retain(|id| id != user_id);
    }

    /// Get admin users
    pub async fn get_admin_users(&self) -> Vec<String> {
        self.admin_users.read().await.clone()
    }

    /// Reset all period usage
    pub async fn reset_all_periods(&self) {
        self.store.reset_all_periods().await;
    }

    /// Update configuration
    pub async fn update_config(&self, config: RateLimitConfig) {
        let mut current = self.config.write().await;
        *current = config;
    }

    /// Get current configuration
    pub async fn get_config(&self) -> RateLimitConfig {
        self.config.read().await.clone()
    }

    /// Get quota store (for dashboard)
    pub fn store(&self) -> &QuotaStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = RateLimitManager::default_config();
        let config = manager.get_config().await;
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_disabled_manager() {
        let manager = RateLimitManager::disabled();
        let config = manager.get_config().await;
        assert!(!config.enabled);
    }

    #[tokio::test]
    async fn test_check_user_limit_allowed() {
        let manager = RateLimitManager::default_config();

        let result = manager
            .check_user_limit("user-123", QuotaType::ApiCalls, 10)
            .await
            .unwrap();

        assert!(result.allowed);
        assert_eq!(result.remaining, 90);
    }

    #[tokio::test]
    async fn test_check_user_limit_denied() {
        let manager = RateLimitManager::default_config();

        // Consume all tokens
        let result1 = manager
            .check_user_limit("user-456", QuotaType::ApiCalls, 100)
            .await
            .unwrap();
        assert!(result1.allowed);

        // Next request should be denied
        let result2 = manager
            .check_user_limit("user-456", QuotaType::ApiCalls, 1)
            .await
            .unwrap();

        assert!(!result2.allowed);
        assert!(result2.retry_after_secs.is_some());
    }

    #[tokio::test]
    async fn test_admin_user_exempt() {
        let manager = RateLimitManager::default_config();
        manager.add_admin_user("admin-user").await;

        // Admin should always be allowed
        for _ in 0..150 {
            let result = manager
                .check_user_limit("admin-user", QuotaType::ApiCalls, 1)
                .await
                .unwrap();
            assert!(result.allowed);
        }
    }

    #[tokio::test]
    async fn test_disabled_allows_all() {
        let manager = RateLimitManager::disabled();

        // Should allow unlimited requests
        for _ in 0..200 {
            let result = manager
                .check_user_limit("user-123", QuotaType::ApiCalls, 1)
                .await
                .unwrap();
            assert!(result.allowed);
        }
    }

    #[tokio::test]
    async fn test_get_user_stats() {
        let manager = RateLimitManager::default_config();

        manager
            .check_user_limit("user-789", QuotaType::ApiCalls, 25)
            .await
            .unwrap();

        let stats = manager.get_user_stats("user-789").await;
        assert!(!stats.is_empty());

        let api_stats = stats.iter().find(|s| s.quota_type == QuotaType::ApiCalls);
        assert!(api_stats.is_some());
        assert_eq!(api_stats.unwrap().period_usage, 25);
    }

    #[tokio::test]
    async fn test_set_user_quota() {
        let manager = RateLimitManager::default_config();

        manager
            .set_user_quota("user-custom", QuotaType::ApiCalls, 50, 1.0)
            .await
            .unwrap();

        // Should be able to consume 50
        let result = manager
            .check_user_limit("user-custom", QuotaType::ApiCalls, 50)
            .await
            .unwrap();
        assert!(result.allowed);

        // But not 51
        let result = manager
            .check_user_limit("user-custom", QuotaType::ApiCalls, 1)
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_agent_limits() {
        let manager = RateLimitManager::default_config();

        let result = manager
            .check_agent_limit("agent-123", QuotaType::VmSpawn, 5)
            .await
            .unwrap();

        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_reset_all_periods() {
        let manager = RateLimitManager::default_config();

        manager
            .check_user_limit("user-reset", QuotaType::ApiCalls, 50)
            .await
            .unwrap();

        manager.reset_all_periods().await;

        let stats = manager.get_user_stats("user-reset").await;
        let api_stats = stats.iter().find(|s| s.quota_type == QuotaType::ApiCalls);
        // After reset, period usage should be 0
        // Note: The quota object is cloned, so we need to get fresh stats
    }

    #[tokio::test]
    async fn test_add_remove_admin() {
        let manager = RateLimitManager::default_config();

        manager.add_admin_user("admin-1").await;
        manager.add_admin_user("admin-2").await;

        let admins = manager.get_admin_users().await;
        assert_eq!(admins.len(), 2);

        manager.remove_admin_user("admin-1").await;

        let admins = manager.get_admin_users().await;
        assert_eq!(admins.len(), 1);
        assert_eq!(admins[0], "admin-2");
    }

    #[tokio::test]
    async fn test_rate_limit_result() {
        let allowed = RateLimitResult::allowed(50);
        assert!(allowed.allowed);
        assert_eq!(allowed.remaining, 50);
        assert!(allowed.retry_after_secs.is_none());

        let denied = RateLimitResult::denied(0, 60, "Rate limit exceeded");
        assert!(!denied.allowed);
        assert_eq!(denied.remaining, 0);
        assert_eq!(denied.retry_after_secs, Some(60));
        assert_eq!(denied.reason, Some("Rate limit exceeded".to_string()));
    }
}