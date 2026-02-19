//! Rate Limit Configuration
//!
//! Configuration for rate limiting and quota management.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default rate limits
pub const DEFAULT_API_RATE_LIMIT: u32 = 100; // requests per minute
pub const DEFAULT_VM_SPAWN_LIMIT: u32 = 10; // VMs per hour
pub const DEFAULT_APPROVAL_LIMIT: u32 = 50; // approvals per hour
pub const DEFAULT_BURST_SIZE: u32 = 20; // burst allowance

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,

    /// Default API rate limit (requests per minute)
    pub default_api_limit: u32,

    /// Default VM spawn limit (VMs per hour)
    pub default_vm_spawn_limit: u32,

    /// Default approval request limit (requests per hour)
    pub default_approval_limit: u32,

    /// Default burst size (allows temporary spikes)
    pub default_burst_size: u32,

    /// Token refill interval in seconds
    pub refill_interval_secs: u64,

    /// Quota reset interval in hours
    pub quota_reset_interval_hours: u64,

    /// Per-user quota overrides
    #[serde(default)]
    pub user_overrides: std::collections::HashMap<String, UserQuotaOverride>,

    /// Per-agent quota overrides
    #[serde(default)]
    pub agent_overrides: std::collections::HashMap<String, AgentQuotaOverride>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_api_limit: DEFAULT_API_RATE_LIMIT,
            default_vm_spawn_limit: DEFAULT_VM_SPAWN_LIMIT,
            default_approval_limit: DEFAULT_APPROVAL_LIMIT,
            default_burst_size: DEFAULT_BURST_SIZE,
            refill_interval_secs: 60,
            quota_reset_interval_hours: 24,
            user_overrides: std::collections::HashMap::new(),
            agent_overrides: std::collections::HashMap::new(),
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("LUMINAGUARD_RATE_LIMIT_ENABLED") {
            config.enabled = val.parse().unwrap_or(true);
        }

        if let Ok(val) = std::env::var("LUMINAGUARD_API_RATE_LIMIT") {
            if let Ok(limit) = val.parse() {
                config.default_api_limit = limit;
            }
        }

        if let Ok(val) = std::env::var("LUMINAGUARD_VM_SPAWN_LIMIT") {
            if let Ok(limit) = val.parse() {
                config.default_vm_spawn_limit = limit;
            }
        }

        if let Ok(val) = std::env::var("LUMINAGUARD_APPROVAL_LIMIT") {
            if let Ok(limit) = val.parse() {
                config.default_approval_limit = limit;
            }
        }

        if let Ok(val) = std::env::var("LUMINAGUARD_BURST_SIZE") {
            if let Ok(size) = val.parse() {
                config.default_burst_size = size;
            }
        }

        config
    }

    /// Get refill duration
    pub fn refill_duration(&self) -> Duration {
        Duration::from_secs(self.refill_interval_secs)
    }

    /// Get quota reset duration
    pub fn quota_reset_duration(&self) -> Duration {
        Duration::from_secs(self.quota_reset_interval_hours * 3600)
    }

    /// Disable rate limiting (for testing)
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

/// Per-user quota override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuotaOverride {
    /// User ID
    pub user_id: String,

    /// API rate limit override
    pub api_limit: Option<u32>,

    /// VM spawn limit override
    pub vm_spawn_limit: Option<u32>,

    /// Approval limit override
    pub approval_limit: Option<u32>,

    /// Burst size override
    pub burst_size: Option<u32>,

    /// Is this user an admin (exempt from limits)
    pub is_admin: bool,
}

/// Per-agent quota override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentQuotaOverride {
    /// Agent ID
    pub agent_id: String,

    /// API rate limit override
    pub api_limit: Option<u32>,

    /// VM spawn limit override
    pub vm_spawn_limit: Option<u32>,

    /// Approval limit override
    pub approval_limit: Option<u32>,

    /// Burst size override
    pub burst_size: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RateLimitConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_api_limit, DEFAULT_API_RATE_LIMIT);
        assert_eq!(config.default_vm_spawn_limit, DEFAULT_VM_SPAWN_LIMIT);
    }

    #[test]
    fn test_disabled_config() {
        let config = RateLimitConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_refill_duration() {
        let config = RateLimitConfig::default();
        assert_eq!(config.refill_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_quota_reset_duration() {
        let config = RateLimitConfig::default();
        assert_eq!(config.quota_reset_duration(), Duration::from_secs(24 * 3600));
    }

    #[test]
    fn test_config_serialization() {
        let config = RateLimitConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: RateLimitConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.default_api_limit, parsed.default_api_limit);
    }
}