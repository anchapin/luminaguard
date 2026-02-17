// Webhook delivery executor with retry logic
//
// Handles actual HTTP delivery of webhook events with automatic retries

use crate::webhooks::retry::{calculate_retry_delay, get_attempt_timeout, RetryConfig, RetryDecision};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Unique webhook ID
    pub id: String,
    /// URL to POST events to
    pub url: String,
    /// Custom HTTP headers to include
    pub headers: Option<std::collections::HashMap<String, String>>,
    /// Bearer token for authentication
    pub auth_token: Option<String>,
    /// Enabled/disabled flag
    pub enabled: bool,
    /// Event types to subscribe to
    pub event_types: Vec<String>,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl WebhookConfig {
    /// Create a new webhook configuration
    pub fn new(id: String, url: String) -> Self {
        Self {
            id,
            url,
            headers: None,
            auth_token: None,
            enabled: true,
            event_types: vec!["*".to_string()], // Subscribe to all events by default
            retry_config: RetryConfig::default(),
        }
    }

    /// Check if this webhook should handle an event
    pub fn handles_event(&self, event_type: &str) -> bool {
        if !self.enabled {
            return false;
        }
        self.event_types.contains(&"*".to_string()) || self.event_types.contains(&event_type.to_string())
    }
}

/// Result of a delivery attempt
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub success: bool,
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error: Option<String>,
    pub retry_decision: RetryDecision,
}

/// Webhook event to be delivered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    pub event_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: serde_json::Value,
}

/// Webhook delivery executor
pub struct DeliveryExecutor {
    client: reqwest::Client,
}

impl DeliveryExecutor {
    /// Create a new delivery executor
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Attempt to deliver a webhook event
    ///
    /// # Arguments
    /// * `config` - Webhook configuration
    /// * `event` - Event to deliver
    /// * `attempt` - Current attempt number (0-indexed)
    ///
    /// # Returns
    /// Delivery result with retry decision
    pub async fn deliver(
        &self,
        config: &WebhookConfig,
        event: &WebhookEvent,
        attempt: u32,
    ) -> DeliveryResult {
        let timeout = get_attempt_timeout(attempt, 5);
        let start = Instant::now();

        // Build request
        let mut request = self.client
            .post(&config.url)
            .timeout(timeout)
            .json(&event);

        // Add custom headers
        if let Some(headers) = &config.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        // Add authentication
        if let Some(token) = &config.auth_token {
            request = request.bearer_auth(token);
        }

        // Send request
        let response_time_ms = {
            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let elapsed = start.elapsed().as_millis() as u64;

                    debug!(
                        "Webhook {} attempt {} - status {}, time {} ms",
                        config.id, attempt, status, elapsed
                    );

                    let success = status >= 200 && status < 300;
                    let retry_decision = if success {
                        RetryDecision::GiveUp // Success, no retry needed
                    } else if is_retryable_status(status) {
                        calculate_retry_delay(attempt, &config.retry_config)
                    } else {
                        RetryDecision::GiveUp // Non-retryable error
                    };

                    return DeliveryResult {
                        success,
                        status_code: Some(status),
                        response_time_ms: elapsed,
                        error: if success { None } else { Some(format!("HTTP {}", status)) },
                        retry_decision,
                    };
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let error_msg = e.to_string();

                    warn!(
                        "Webhook {} attempt {} failed: {} (time {} ms)",
                        config.id, attempt, error_msg, elapsed
                    );

                    let retry_decision = calculate_retry_delay(attempt, &config.retry_config);

                    return DeliveryResult {
                        success: false,
                        status_code: None,
                        response_time_ms: elapsed,
                        error: Some(error_msg),
                        retry_decision,
                    };
                }
            }
        };
    }
}

impl Default for DeliveryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if an HTTP status code is retryable
fn is_retryable_status(status: u16) -> bool {
    matches!(
        status,
        408 | // Request Timeout
        429 | // Too Many Requests
        500 | // Internal Server Error
        502 | // Bad Gateway
        503 | // Service Unavailable
        504   // Gateway Timeout
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_config_new() {
        let config = WebhookConfig::new(
            "webhook-1".to_string(),
            "https://example.com/webhook".to_string(),
        );

        assert_eq!(config.id, "webhook-1");
        assert_eq!(config.url, "https://example.com/webhook");
        assert!(config.enabled);
        assert_eq!(config.event_types, vec!["*"]);
    }

    #[test]
    fn test_webhook_config_handles_event() {
        let mut config = WebhookConfig::new(
            "webhook-1".to_string(),
            "https://example.com/webhook".to_string(),
        );

        // With "*", handles all events
        assert!(config.handles_event("task.completed"));
        assert!(config.handles_event("approval.granted"));

        // Specific event types
        config.event_types = vec!["task.completed".to_string(), "task.failed".to_string()];
        assert!(config.handles_event("task.completed"));
        assert!(!config.handles_event("approval.granted"));

        // Disabled
        config.enabled = false;
        assert!(!config.handles_event("task.completed"));
    }

    #[test]
    fn test_is_retryable_status() {
        // Retryable statuses
        assert!(is_retryable_status(408));
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(503));

        // Non-retryable statuses
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
    }

    #[test]
    fn test_webhook_event_creation() {
        let event = WebhookEvent {
            id: "evt-123".to_string(),
            event_type: "task.completed".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"task_id": "task-456"}),
        };

        assert_eq!(event.event_type, "task.completed");
        assert_eq!(event.data.get("task_id").and_then(|v| v.as_str()), Some("task-456"));
    }
}
