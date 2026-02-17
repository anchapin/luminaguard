// Exponential backoff retry logic for webhook delivery
//
// Implements retry strategy with:
// - Exponential backoff (1s, 2s, 4s, 8s, 16s)
// - Maximum 5 retry attempts
// - Jitter to prevent thundering herd
// - Configurable timeout per retry

use serde::{Deserialize, Serialize};
use std::time::Duration;
use rand::Rng;
use tracing::debug;

/// Retry strategy configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Base delay in milliseconds (default 1000ms)
    pub base_delay_ms: u64,
    /// Maximum number of retries (default 5)
    pub max_retries: u32,
    /// Maximum total delay in milliseconds (default 60000ms)
    pub max_total_delay_ms: u64,
    /// Use exponential backoff (default true)
    pub use_exponential_backoff: bool,
    /// Add jitter to delay (default true)
    pub use_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            base_delay_ms: 1000,
            max_retries: 5,
            max_total_delay_ms: 60000,
            use_exponential_backoff: true,
            use_jitter: true,
        }
    }
}

/// Retry decision for a failed delivery attempt
#[derive(Debug, Clone, PartialEq)]
pub enum RetryDecision {
    /// Retry with this delay in milliseconds
    Retry(u64),
    /// Give up and move to dead letter queue
    GiveUp,
}

/// Calculate retry delay based on attempt number
///
/// Formula with exponential backoff and optional jitter:
/// delay = min(base * 2^attempt, max_total) + jitter
///
/// # Arguments
/// * `attempt` - Zero-indexed attempt number (0 = first retry)
/// * `config` - Retry configuration
///
/// # Returns
/// Retry decision with delay or final failure
pub fn calculate_retry_delay(attempt: u32, config: &RetryConfig) -> RetryDecision {
    if attempt >= config.max_retries {
        debug!("Max retries ({}) exceeded, giving up", config.max_retries);
        return RetryDecision::GiveUp;
    }

    let base_delay = config.base_delay_ms;
    
    // Calculate exponential backoff: base * 2^attempt
    let delay_ms = if config.use_exponential_backoff {
        base_delay.saturating_mul(2_u64.pow(attempt))
    } else {
        base_delay
    };

    // Cap at maximum total delay
    let delay_ms = delay_ms.min(config.max_total_delay_ms);

    // Add jitter (±20% of delay)
    let delay_ms = if config.use_jitter && delay_ms > 0 {
        let jitter_percent = 0.2;
        let jitter = (delay_ms as f64 * jitter_percent) as u64;
        let mut rng = rand::thread_rng();
        let random_jitter = rng.gen_range(0..=jitter);
        
        // 50% chance to add or subtract jitter
        if rng.gen_bool(0.5) {
            delay_ms.saturating_add(random_jitter)
        } else {
            delay_ms.saturating_sub(random_jitter)
        }
    } else {
        delay_ms
    };

    debug!(
        "Retry attempt {} - delay {} ms (max: {} ms)",
        attempt, delay_ms, config.max_total_delay_ms
    );

    RetryDecision::Retry(delay_ms)
}

/// Get timeout duration for a retry attempt
///
/// Timeout increases with attempt number to account for slower responses
/// Formula: base_timeout * (1 + attempt * 0.5)
///
/// # Arguments
/// * `attempt` - Zero-indexed attempt number
/// * `base_timeout_secs` - Base timeout in seconds (default 5)
///
/// # Returns
/// Duration for this attempt's timeout
pub fn get_attempt_timeout(attempt: u32, base_timeout_secs: u64) -> Duration {
    let multiplier = 1.0 + (attempt as f64 * 0.5);
    let timeout_secs = (base_timeout_secs as f64 * multiplier) as u64;
    Duration::from_secs(timeout_secs.min(60)) // Max 60s timeout
}

/// Webhook delivery status tracking
#[derive(Debug, Clone, PartialEq)]
pub enum DeliveryStatus {
    /// Initial state, pending first attempt
    Pending,
    /// Currently being delivered
    InProgress,
    /// Delivered successfully
    Delivered,
    /// Failed after all retries
    Failed,
    /// Dead lettered (moved to DLQ)
    DeadLettered,
}

/// Webhook delivery attempt record
#[derive(Debug, Clone)]
pub struct DeliveryAttempt {
    pub attempt_number: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let config = RetryConfig::default();
        
        // First retry: 1s
        let delay0 = calculate_retry_delay(0, &config);
        assert!(matches!(delay0, RetryDecision::Retry(d) if d >= 800 && d <= 1200));
        
        // Second retry: 2s
        let delay1 = calculate_retry_delay(1, &config);
        assert!(matches!(delay1, RetryDecision::Retry(d) if d >= 1600 && d <= 2400));
        
        // Third retry: 4s
        let delay2 = calculate_retry_delay(2, &config);
        assert!(matches!(delay2, RetryDecision::Retry(d) if d >= 3200 && d <= 4800));
    }

    #[test]
    fn test_max_retries_exceeded() {
        let config = RetryConfig::default();
        
        let decision = calculate_retry_delay(config.max_retries, &config);
        assert_eq!(decision, RetryDecision::GiveUp);
    }

    #[test]
    fn test_max_delay_capped() {
        let mut config = RetryConfig::default();
        config.max_total_delay_ms = 5000;
        config.use_jitter = false;
        
        // Very high attempt number should be capped
        let delay = calculate_retry_delay(10, &config);
        assert!(matches!(delay, RetryDecision::Retry(d) if d <= 5000));
    }

    #[test]
    fn test_attempt_timeout() {
        // Base timeout is 5 seconds
        let timeout0 = get_attempt_timeout(0, 5);
        assert_eq!(timeout0.as_secs(), 5);
        
        // Second attempt: 5 * (1 + 0.5) = 7.5s
        let timeout1 = get_attempt_timeout(1, 5);
        assert_eq!(timeout1.as_secs(), 7);
        
        // Very high attempt number is capped at 60s
        let timeout_high = get_attempt_timeout(1000, 5);
        assert_eq!(timeout_high.as_secs(), 60);
    }

    #[test]
    fn test_no_exponential_backoff() {
        let mut config = RetryConfig::default();
        config.use_exponential_backoff = false;
        config.use_jitter = false;
        
        let delay0 = calculate_retry_delay(0, &config);
        let delay1 = calculate_retry_delay(1, &config);
        
        // Both should be base delay
        assert_eq!(delay0, RetryDecision::Retry(1000));
        assert_eq!(delay1, RetryDecision::Retry(1000));
    }

    #[test]
    fn test_delivery_status_transitions() {
        let mut status = DeliveryStatus::Pending;
        assert_eq!(status, DeliveryStatus::Pending);
        
        status = DeliveryStatus::InProgress;
        assert_eq!(status, DeliveryStatus::InProgress);
        
        status = DeliveryStatus::Delivered;
        assert_eq!(status, DeliveryStatus::Delivered);
    }
}
