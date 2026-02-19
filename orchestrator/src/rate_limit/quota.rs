//! Quota Types and Token Bucket Implementation
//!
//! This module provides the core quota types and token bucket algorithm
//! for rate limiting.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Types of operations that can be rate-limited
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuotaType {
    /// API calls (requests per minute)
    ApiCalls,
    /// VM spawning (VMs per hour)
    VmSpawn,
    /// Approval requests (requests per hour)
    ApprovalRequest,
}

impl QuotaType {
    /// Get the default limit for this quota type
    pub fn default_limit(&self) -> u32 {
        match self {
            QuotaType::ApiCalls => 100,
            QuotaType::VmSpawn => 10,
            QuotaType::ApprovalRequest => 50,
        }
    }

    /// Get the refill rate (tokens per second)
    pub fn default_refill_rate(&self) -> f64 {
        match self {
            QuotaType::ApiCalls => 100.0 / 60.0,      // 100 per minute
            QuotaType::VmSpawn => 10.0 / 3600.0,      // 10 per hour
            QuotaType::ApprovalRequest => 50.0 / 3600.0, // 50 per hour
        }
    }
}

/// Quota configuration for a specific entity (user or agent)
#[derive(Debug, Clone)]
pub struct Quota {
    /// Unique identifier for this quota
    pub id: String,

    /// Type of quota
    pub quota_type: QuotaType,

    /// Maximum tokens (burst capacity)
    pub max_tokens: u32,

    /// Current token count
    pub current_tokens: Arc<AtomicU64>,

    /// Tokens added per second
    pub refill_rate: f64,

    /// Last refill timestamp
    pub last_refill: Arc<std::sync::Mutex<Instant>>,

    /// Total requests allowed in the period
    pub period_limit: u32,

    /// Current usage in the period
    pub period_usage: Arc<AtomicU64>,
}

impl Quota {
    /// Create a new quota with default settings for the given type
    pub fn new(quota_type: QuotaType, id: String) -> Self {
        Self {
            id,
            quota_type,
            max_tokens: quota_type.default_limit(),
            current_tokens: Arc::new(AtomicU64::new(quota_type.default_limit() as u64)),
            refill_rate: quota_type.default_refill_rate(),
            last_refill: Arc::new(std::sync::Mutex::new(Instant::now())),
            period_limit: quota_type.default_limit(),
            period_usage: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a new quota with custom settings
    pub fn with_limits(
        quota_type: QuotaType,
        id: String,
        max_tokens: u32,
        refill_rate: f64,
        period_limit: u32,
    ) -> Self {
        Self {
            id,
            quota_type,
            max_tokens,
            current_tokens: Arc::new(AtomicU64::new(max_tokens as u64)),
            refill_rate,
            last_refill: Arc::new(std::sync::Mutex::new(Instant::now())),
            period_limit,
            period_usage: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Try to consume tokens from the bucket
    ///
    /// Returns true if tokens were successfully consumed,
    /// false if insufficient tokens available.
    pub fn try_consume(&self, tokens: u32) -> bool {
        self.refill();

        let tokens_u64 = tokens as u64;
        let mut current = self.current_tokens.load(Ordering::SeqCst);

        loop {
            if current < tokens_u64 {
                return false;
            }

            let new_value = current - tokens_u64;
            match self.current_tokens.compare_exchange_weak(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    // Track period usage
                    self.period_usage.fetch_add(tokens_u64, Ordering::SeqCst);
                    return true;
                }
                Err(actual) => {
                    current = actual;
                }
            }
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let mut last_refill = self.last_refill.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);

        if elapsed < Duration::from_millis(100) {
            // Don't refill too frequently
            return;
        }

        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate).floor() as u64;

        if tokens_to_add > 0 {
            let mut current = self.current_tokens.load(Ordering::SeqCst);
            loop {
                let new_value = std::cmp::min(current + tokens_to_add, self.max_tokens as u64);
                match self.current_tokens.compare_exchange_weak(
                    current,
                    new_value,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        *last_refill = now;
                        break;
                    }
                    Err(actual) => {
                        current = actual;
                    }
                }
            }
        }
    }

    /// Get current token count
    pub fn current_tokens(&self) -> u32 {
        self.current_tokens.load(Ordering::SeqCst) as u32
    }

    /// Get remaining tokens
    pub fn remaining_tokens(&self) -> u32 {
        self.current_tokens()
    }

    /// Get usage statistics
    pub fn usage_stats(&self) -> UsageStats {
        UsageStats {
            quota_id: self.id.clone(),
            quota_type: self.quota_type,
            max_tokens: self.max_tokens,
            current_tokens: self.current_tokens(),
            period_limit: self.period_limit,
            period_usage: self.period_usage.load(Ordering::SeqCst) as u32,
            utilization_percent: self.utilization_percent(),
        }
    }

    /// Calculate utilization percentage
    fn utilization_percent(&self) -> f64 {
        let usage = self.period_usage.load(Ordering::SeqCst) as f64;
        let limit = self.period_limit as f64;
        if limit > 0.0 {
            (usage / limit) * 100.0
        } else {
            0.0
        }
    }

    /// Reset period usage
    pub fn reset_period(&self) {
        self.period_usage.store(0, Ordering::SeqCst);
    }

    /// Force set token count (for admin adjustments)
    pub fn set_tokens(&self, tokens: u32) {
        self.current_tokens
            .store(tokens.min(self.max_tokens) as u64, Ordering::SeqCst);
    }
}

/// Usage statistics for a quota
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// Quota identifier
    pub quota_id: String,

    /// Type of quota
    pub quota_type: QuotaType,

    /// Maximum tokens
    pub max_tokens: u32,

    /// Current tokens available
    pub current_tokens: u32,

    /// Period limit
    pub period_limit: u32,

    /// Usage in current period
    pub period_usage: u32,

    /// Utilization percentage
    pub utilization_percent: f64,
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    /// Maximum capacity
    capacity: u32,

    /// Current tokens
    tokens: Arc<AtomicU64>,

    /// Refill rate (tokens per second)
    refill_rate: f64,

    /// Last refill time
    last_refill: Arc<std::sync::Mutex<Instant>>,
}

impl TokenBucket {
    /// Create a new token bucket
    pub fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: Arc::new(AtomicU64::new(capacity as u64)),
            refill_rate,
            last_refill: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }

    /// Try to consume tokens
    pub fn try_consume(&self, tokens: u32) -> bool {
        self.refill();

        let tokens_u64 = tokens as u64;
        let mut current = self.tokens.load(Ordering::SeqCst);

        loop {
            if current < tokens_u64 {
                return false;
            }

            let new_value = current - tokens_u64;
            match self.tokens.compare_exchange_weak(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(actual) => {
                    current = actual;
                }
            }
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let mut last_refill = self.last_refill.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);

        if elapsed < Duration::from_millis(100) {
            return;
        }

        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate).floor() as u64;

        if tokens_to_add > 0 {
            let mut current = self.tokens.load(Ordering::SeqCst);
            loop {
                let new_value = std::cmp::min(current + tokens_to_add, self.capacity as u64);
                match self.tokens.compare_exchange_weak(
                    current,
                    new_value,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        *last_refill = now;
                        break;
                    }
                    Err(actual) => {
                        current = actual;
                    }
                }
            }
        }
    }

    /// Get current token count
    pub fn available(&self) -> u32 {
        self.tokens.load(Ordering::SeqCst) as u32
    }

    /// Get time until tokens are available
    pub fn time_until_available(&self, tokens: u32) -> Duration {
        let current = self.available();
        if current >= tokens {
            return Duration::ZERO;
        }

        let needed = (tokens - current) as f64;
        let seconds = needed / self.refill_rate;
        Duration::from_secs_f64(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_creation() {
        let quota = Quota::new(QuotaType::ApiCalls, "test-quota".to_string());
        assert_eq!(quota.quota_type, QuotaType::ApiCalls);
        assert_eq!(quota.max_tokens, 100);
    }

    #[test]
    fn test_quota_consume() {
        let quota = Quota::new(QuotaType::ApiCalls, "test-quota".to_string());

        // Should succeed
        assert!(quota.try_consume(10));
        assert_eq!(quota.current_tokens(), 90);

        // Should succeed
        assert!(quota.try_consume(10));
        assert_eq!(quota.current_tokens(), 80);
    }

    #[test]
    fn test_quota_consume_insufficient() {
        let quota = Quota::new(QuotaType::ApiCalls, "test-quota".to_string());

        // Consume all tokens
        assert!(quota.try_consume(100));
        assert_eq!(quota.current_tokens(), 0);

        // Should fail - no tokens
        assert!(!quota.try_consume(1));
    }

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(100, 10.0);
        assert_eq!(bucket.available(), 100);
    }

    #[test]
    fn test_token_bucket_consume() {
        let bucket = TokenBucket::new(100, 10.0);

        assert!(bucket.try_consume(50));
        assert_eq!(bucket.available(), 50);
    }

    #[test]
    fn test_token_bucket_insufficient() {
        let bucket = TokenBucket::new(100, 10.0);

        assert!(bucket.try_consume(100));
        assert!(!bucket.try_consume(1));
    }

    #[test]
    fn test_usage_stats() {
        let quota = Quota::new(QuotaType::ApiCalls, "test-quota".to_string());
        quota.try_consume(25);

        let stats = quota.usage_stats();
        assert_eq!(stats.current_tokens, 75);
        assert_eq!(stats.period_usage, 25);
    }

    #[test]
    fn test_quota_reset() {
        let quota = Quota::new(QuotaType::ApiCalls, "test-quota".to_string());
        quota.try_consume(50);

        quota.reset_period();
        assert_eq!(quota.period_usage.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_quota_set_tokens() {
        let quota = Quota::new(QuotaType::ApiCalls, "test-quota".to_string());
        quota.try_consume(50);

        quota.set_tokens(100);
        assert_eq!(quota.current_tokens(), 100);
    }

    #[test]
    fn test_quota_type_defaults() {
        assert_eq!(QuotaType::ApiCalls.default_limit(), 100);
        assert_eq!(QuotaType::VmSpawn.default_limit(), 10);
        assert_eq!(QuotaType::ApprovalRequest.default_limit(), 50);
    }

    #[test]
    fn test_time_until_available() {
        let bucket = TokenBucket::new(100, 10.0); // 10 tokens per second
        bucket.try_consume(100);

        let time = bucket.time_until_available(20);
        // Should need 2 seconds to get 20 tokens at 10 tokens/sec
        assert!(time.as_secs_f64() >= 1.9 && time.as_secs_f64() <= 2.1);
    }
}