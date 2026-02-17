// Webhook manager - orchestrates delivery with retries and DLQ
//
// Manages:
// - Webhook registration and lifecycle
// - Event delivery scheduling with retry logic
// - Dead letter queue for failed deliveries
// - Metrics and monitoring

use crate::webhooks::delivery::{DeliveryExecutor, WebhookConfig, WebhookEvent};
use crate::webhooks::queue::DeadLetterQueue;
use crate::webhooks::retry::RetryDecision;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Webhook manager
pub struct WebhookManager {
    webhooks: Arc<RwLock<HashMap<String, WebhookConfig>>>,
    dlq: Arc<RwLock<DeadLetterQueue>>,
    executor: DeliveryExecutor,
}

impl WebhookManager {
    /// Create a new webhook manager
    pub fn new() -> Self {
        Self {
            webhooks: Arc::new(RwLock::new(HashMap::new())),
            dlq: Arc::new(RwLock::new(DeadLetterQueue::new())),
            executor: DeliveryExecutor::new(),
        }
    }

    /// Register a new webhook
    pub async fn register(&self, config: WebhookConfig) {
        info!("Registering webhook: {}", config.id);
        let mut webhooks = self.webhooks.write().await;
        webhooks.insert(config.id.clone(), config);
    }

    /// Unregister a webhook
    pub async fn unregister(&self, webhook_id: &str) {
        info!("Unregistering webhook: {}", webhook_id);
        let mut webhooks = self.webhooks.write().await;
        webhooks.remove(webhook_id);
    }

    /// Get webhook by ID
    pub async fn get(&self, webhook_id: &str) -> Option<WebhookConfig> {
        let webhooks = self.webhooks.read().await;
        webhooks.get(webhook_id).cloned()
    }

    /// List all webhooks
    pub async fn list(&self) -> Vec<WebhookConfig> {
        let webhooks = self.webhooks.read().await;
        webhooks.values().cloned().collect()
    }

    /// Deliver an event to all matching webhooks
    pub async fn deliver_event(&self, event: WebhookEvent) -> usize {
        let webhooks = self.webhooks.read().await;
        let matching: Vec<WebhookConfig> = webhooks
            .values()
            .filter(|w| w.handles_event(&event.event_type))
            .cloned()
            .collect();

        let mut delivered = 0;
        for webhook in matching {
            if self.deliver_to_webhook(&webhook, &event).await {
                delivered += 1;
            }
        }

        delivered
    }

    /// Deliver event to a specific webhook with retries
    async fn deliver_to_webhook(&self, webhook: &WebhookConfig, event: &WebhookEvent) -> bool {
        let mut attempt = 0;
        let max_retries = webhook.retry_config.max_retries;

        loop {
            debug!(
                "Webhook delivery attempt {} for {}",
                attempt + 1,
                webhook.id
            );

            let result = self.executor.deliver(webhook, event, attempt).await;

            if result.success {
                info!("Webhook {} delivered successfully", webhook.id);
                return true;
            }

            // Determine retry action
            match result.retry_decision {
                RetryDecision::Retry(delay_ms) => {
                    attempt += 1;
                    debug!("Retrying webhook {} in {} ms", webhook.id, delay_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                RetryDecision::GiveUp => {
                    // Move to DLQ
                    let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                    let dlq_entry = crate::webhooks::queue::DeadLetterEntry::new(
                        webhook.id.clone(),
                        event.event_type.clone(),
                        event.data.clone(),
                        attempt + 1,
                        error_msg,
                    );

                    let mut dlq = self.dlq.write().await;
                    let dlq_id = dlq.add(dlq_entry);
                    
                    info!(
                        "Webhook {} moved to DLQ: {}",
                        webhook.id, dlq_id
                    );
                    return false;
                }
            }
        }
    }

    /// Get DLQ stats
    pub async fn get_dlq_stats(&self) -> crate::webhooks::queue::DLQStats {
        let dlq = self.dlq.read().await;
        dlq.stats()
    }

    /// List pending DLQ entries
    pub async fn get_pending_dlq_entries(&self) -> Vec<crate::webhooks::queue::DeadLetterEntry> {
        let dlq = self.dlq.read().await;
        dlq.pending_review()
    }

    /// Archive old DLQ entries
    pub async fn archive_old_dlq_entries(&self) -> usize {
        let mut dlq = self.dlq.write().await;
        dlq.archive_old_entries()
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_webhook() {
        let manager = WebhookManager::new();
        let config = WebhookConfig::new(
            "webhook-1".to_string(),
            "https://example.com/webhook".to_string(),
        );

        manager.register(config).await;

        let retrieved = manager.get("webhook-1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().url, "https://example.com/webhook");
    }

    #[tokio::test]
    async fn test_unregister_webhook() {
        let manager = WebhookManager::new();
        let config = WebhookConfig::new(
            "webhook-1".to_string(),
            "https://example.com/webhook".to_string(),
        );

        manager.register(config).await;
        assert!(manager.get("webhook-1").await.is_some());

        manager.unregister("webhook-1").await;
        assert!(manager.get("webhook-1").await.is_none());
    }

    #[tokio::test]
    async fn test_list_webhooks() {
        let manager = WebhookManager::new();

        for i in 0..3 {
            let config = WebhookConfig::new(
                format!("webhook-{}", i),
                format!("https://example.com/webhook-{}", i),
            );
            manager.register(config).await;
        }

        let webhooks = manager.list().await;
        assert_eq!(webhooks.len(), 3);
    }
}
