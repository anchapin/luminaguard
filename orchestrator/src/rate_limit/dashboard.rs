//! Admin Dashboard Support
//!
//! Provides data structures and utilities for monitoring and adjusting quotas
//! via an admin dashboard.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::RateLimitConfig;
use super::manager::RateLimitManager;
use super::quota::{QuotaType, UsageStats};
use super::store::{EntityType, UsageRecord};

/// Dashboard data for admin interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    /// Timestamp of data generation
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Summary statistics
    pub summary: QuotaSummary,

    /// Per-user statistics
    pub user_stats: Vec<EntityStats>,

    /// Per-agent statistics
    pub agent_stats: Vec<EntityStats>,

    /// Recent usage history
    pub recent_history: Vec<UsageRecord>,

    /// Current configuration
    pub config: RateLimitConfig,

    /// Admin users
    pub admin_users: Vec<String>,
}

/// Summary of quota usage across all entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaSummary {
    /// Total number of users tracked
    pub total_users: usize,

    /// Total number of agents tracked
    pub total_agents: usize,

    /// Total API calls in current period
    pub total_api_calls: u64,

    /// Total VM spawns in current period
    pub total_vm_spawns: u64,

    /// Total approval requests in current period
    pub total_approval_requests: u64,

    /// Number of rate-limited requests
    pub rate_limited_count: u64,

    /// Average utilization percentage
    pub avg_utilization_percent: f64,
}

/// Statistics for a single entity (user or agent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityStats {
    /// Entity ID
    pub entity_id: String,

    /// Entity type
    pub entity_type: EntityType,

    /// Quota statistics
    pub quotas: Vec<UsageStats>,

    /// Is this entity an admin (users only)
    pub is_admin: bool,

    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Dashboard builder for constructing dashboard data
pub struct DashboardBuilder {
    manager: RateLimitManager,
}

impl DashboardBuilder {
    /// Create a new dashboard builder
    pub fn new(manager: RateLimitManager) -> Self {
        Self { manager }
    }

    /// Build dashboard data
    pub async fn build(&self) -> DashboardData {
        let all_stats = self.manager.get_all_stats().await;
        let admin_users = self.manager.get_admin_users().await;
        let config = self.manager.get_config().await;
        let history = self.manager.store().get_all_usage_history().await;

        // Group stats by entity
        let mut user_stats_map: HashMap<String, Vec<UsageStats>> = HashMap::new();
        let mut agent_stats_map: HashMap<String, Vec<UsageStats>> = HashMap::new();

        for stat in all_stats {
            // Parse entity from quota_id (format: "EntityType-entity_id-QuotaType")
            let parts: Vec<&str> = stat.quota_id.split('-').collect();
            if parts.len() >= 2 {
                let entity_type_str = parts[0];
                let entity_id = parts[1].to_string();

                if entity_type_str == "User" {
                    user_stats_map.entry(entity_id).or_default().push(stat);
                } else if entity_type_str == "Agent" {
                    agent_stats_map.entry(entity_id).or_default().push(stat);
                }
            }
        }

        // Build entity stats
        let user_stats: Vec<EntityStats> = user_stats_map
            .into_iter()
            .map(|(entity_id, quotas)| {
                let is_admin = admin_users.contains(&entity_id);
                let last_activity = self.get_last_activity(&entity_id, EntityType::User, &history);
                EntityStats {
                    entity_id,
                    entity_type: EntityType::User,
                    quotas,
                    is_admin,
                    last_activity,
                }
            })
            .collect();

        let agent_stats: Vec<EntityStats> = agent_stats_map
            .into_iter()
            .map(|(entity_id, quotas)| {
                let last_activity = self.get_last_activity(&entity_id, EntityType::Agent, &history);
                EntityStats {
                    entity_id,
                    entity_type: EntityType::Agent,
                    quotas,
                    is_admin: false,
                    last_activity,
                }
            })
            .collect();

        // Build summary
        let summary = self.build_summary(&user_stats, &agent_stats, &history);

        // Get recent history (last 100 records)
        let recent_history: Vec<UsageRecord> = history.into_iter().rev().take(100).collect();

        DashboardData {
            timestamp: chrono::Utc::now(),
            summary,
            user_stats,
            agent_stats,
            recent_history,
            config,
            admin_users,
        }
    }

    /// Get last activity for an entity
    fn get_last_activity(
        &self,
        entity_id: &str,
        entity_type: EntityType,
        history: &[UsageRecord],
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        history
            .iter()
            .filter(|r| r.key.entity_type == entity_type && r.key.entity_id == entity_id)
            .map(|r| r.timestamp)
            .max()
    }

    /// Build summary statistics
    fn build_summary(
        &self,
        user_stats: &[EntityStats],
        agent_stats: &[EntityStats],
        history: &[UsageRecord],
    ) -> QuotaSummary {
        let total_users = user_stats.len();
        let total_agents = agent_stats.len();

        let mut total_api_calls = 0u64;
        let mut total_vm_spawns = 0u64;
        let mut total_approval_requests = 0u64;
        let mut total_utilization = 0.0;
        let mut quota_count = 0;

        for entity in user_stats.iter().chain(agent_stats.iter()) {
            for quota in &entity.quotas {
                match quota.quota_type {
                    QuotaType::ApiCalls => total_api_calls += quota.period_usage as u64,
                    QuotaType::VmSpawn => total_vm_spawns += quota.period_usage as u64,
                    QuotaType::ApprovalRequest => {
                        total_approval_requests += quota.period_usage as u64
                    }
                }
                total_utilization += quota.utilization_percent;
                quota_count += 1;
            }
        }

        let rate_limited_count = history.iter().filter(|r| !r.success).count() as u64;

        let avg_utilization_percent = if quota_count > 0 {
            total_utilization / quota_count as f64
        } else {
            0.0
        };

        QuotaSummary {
            total_users,
            total_agents,
            total_api_calls,
            total_vm_spawns,
            total_approval_requests,
            rate_limited_count,
            avg_utilization_percent,
        }
    }
}

/// Admin action for adjusting quotas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdminAction {
    /// Set quota for a user
    SetUserQuota {
        user_id: String,
        quota_type: QuotaType,
        max_tokens: u32,
        refill_rate: f64,
    },

    /// Set quota for an agent
    SetAgentQuota {
        agent_id: String,
        quota_type: QuotaType,
        max_tokens: u32,
        refill_rate: f64,
    },

    /// Add admin user
    AddAdminUser { user_id: String },

    /// Remove admin user
    RemoveAdminUser { user_id: String },

    /// Reset all period usage
    ResetAllPeriods,

    /// Update configuration
    UpdateConfig { config: RateLimitConfig },
}

impl AdminAction {
    /// Execute the admin action
    pub async fn execute(&self, manager: &RateLimitManager) -> anyhow::Result<()> {
        match self {
            AdminAction::SetUserQuota {
                user_id,
                quota_type,
                max_tokens,
                refill_rate,
            } => {
                manager
                    .set_user_quota(user_id, *quota_type, *max_tokens, *refill_rate)
                    .await?;
            }
            AdminAction::SetAgentQuota {
                agent_id,
                quota_type,
                max_tokens,
                refill_rate,
            } => {
                manager
                    .set_agent_quota(agent_id, *quota_type, *max_tokens, *refill_rate)
                    .await?;
            }
            AdminAction::AddAdminUser { user_id } => {
                manager.add_admin_user(user_id).await;
            }
            AdminAction::RemoveAdminUser { user_id } => {
                manager.remove_admin_user(user_id).await;
            }
            AdminAction::ResetAllPeriods => {
                manager.reset_all_periods().await;
            }
            AdminAction::UpdateConfig { config } => {
                manager.update_config(config.clone()).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_builder() {
        let manager = RateLimitManager::default_config();
        manager
            .check_user_limit("user-1", QuotaType::ApiCalls, 10)
            .await
            .unwrap();

        let builder = DashboardBuilder::new(manager);
        let dashboard = builder.build().await;

        assert!(dashboard.summary.total_users >= 1);
        assert!(dashboard.timestamp.timestamp() > 0);
    }

    #[tokio::test]
    async fn test_dashboard_with_admin() {
        let manager = RateLimitManager::default_config();
        manager.add_admin_user("admin-1").await;

        let builder = DashboardBuilder::new(manager);
        let dashboard = builder.build().await;

        assert!(dashboard.admin_users.contains(&"admin-1".to_string()));
    }

    #[tokio::test]
    async fn test_admin_action_add_admin() {
        let manager = RateLimitManager::default_config();

        let action = AdminAction::AddAdminUser {
            user_id: "new-admin".to_string(),
        };

        action.execute(&manager).await.unwrap();

        let admins = manager.get_admin_users().await;
        assert!(admins.contains(&"new-admin".to_string()));
    }

    #[tokio::test]
    async fn test_admin_action_set_user_quota() {
        let manager = RateLimitManager::default_config();

        let action = AdminAction::SetUserQuota {
            user_id: "user-custom".to_string(),
            quota_type: QuotaType::ApiCalls,
            max_tokens: 25,
            refill_rate: 0.5,
        };

        action.execute(&manager).await.unwrap();

        // Verify quota was set
        let result = manager
            .check_user_limit("user-custom", QuotaType::ApiCalls, 25)
            .await
            .unwrap();
        assert!(result.allowed);

        let result = manager
            .check_user_limit("user-custom", QuotaType::ApiCalls, 1)
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_admin_action_reset_periods() {
        let manager = RateLimitManager::default_config();

        manager
            .check_user_limit("user-1", QuotaType::ApiCalls, 50)
            .await
            .unwrap();

        let action = AdminAction::ResetAllPeriods;
        action.execute(&manager).await.unwrap();

        // Period usage should be reset
    }

    #[test]
    fn test_quota_summary_serialization() {
        let summary = QuotaSummary {
            total_users: 10,
            total_agents: 5,
            total_api_calls: 1000,
            total_vm_spawns: 50,
            total_approval_requests: 200,
            rate_limited_count: 15,
            avg_utilization_percent: 45.5,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: QuotaSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.total_users, 10);
        assert_eq!(parsed.avg_utilization_percent, 45.5);
    }

    #[test]
    fn test_entity_stats_serialization() {
        let stats = EntityStats {
            entity_id: "user-123".to_string(),
            entity_type: EntityType::User,
            quotas: vec![],
            is_admin: false,
            last_activity: None,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let parsed: EntityStats = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.entity_id, "user-123");
        assert_eq!(parsed.entity_type, EntityType::User);
    }
}
