//! MCP HTTP Transport Layer
//!
//! This module implements HTTP-based transport for communicating with MCP servers.
//!
//! # Architecture
//!
//! The HTTP transport uses standard HTTP POST requests to send JSON-RPC messages
//! to MCP servers. This is suitable for:
//!
//! - Remote MCP servers (cloud-hosted or network-accessible)
//! - Servers running behind HTTP reverse proxies
//! - Production deployments with standard HTTP infrastructure
//!
//! # Transport Modes
//!
//! MCP supports two HTTP transport modes:
//!
//! 1. **Simple HTTP**: One-shot POST requests with immediate responses
//! 2. **Streamable HTTP**: Long-lived connections with streaming responses
//!    (replaces the deprecated SSE transport)
//!
//! This implementation currently supports **Simple HTTP** mode, which is sufficient
//! for most use cases. Streamable HTTP support is planned for future phases.
//!
//! # Features
//!
//! - **Retry Logic**: Exponential backoff for transient failures
//! - **Load Balancing**: Round-robin selection across multiple server instances
//! - **TLS Validation**: Full certificate validation with optional custom CAs
//! - **Custom Headers**: Support for authentication and custom headers
//! - **Timeout Control**: Configurable request timeouts
//!
//! # Example
//!
//! ```ignore
//! use luminaguard_orchestrator::mcp::{McpClient, HttpTransport};
//! use std::time::Duration;
//!
//! // Create HTTP transport with retry
//! let transport = HttpTransport::new("https://api.example.com/mcp")
//!     .with_timeout(Duration::from_secs(60))
//!     .with_retry(true);
//!
//! // Create MCP client
//! let mut client = McpClient::new(transport);
//!
//! // Initialize connection
//! client.initialize().await?;
//!
//! // List available tools
//! let tools = client.list_tools().await?;
//! ```

use crate::mcp::protocol::{McpRequest, McpResponse};
use crate::mcp::retry::RetryConfig;
use crate::mcp::transport::Transport;
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Simple base64 encoding helper (no-std compatible)
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        
        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        
        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }
        
        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }
    
    result
}

/// HTTP transport for remote MCP servers
///
/// This transport uses HTTP POST requests to communicate with MCP servers.
/// Each request/response pair is a separate HTTP transaction.
///
/// # Configuration
///
/// - **urls**: The base URL(s) of the MCP server endpoint (supports load balancing)
/// - **timeout**: Request timeout (default: 30 seconds)
/// - **headers**: Optional custom HTTP headers (e.g., authentication)
/// - **retry**: Enable retry logic with exponential backoff (default: false)
/// - **tls_verify**: Verify TLS certificates (default: true)
///
/// # Example
///
/// ```ignore
/// let transport = HttpTransport::new("https://mcp.example.com")
///     .with_timeout(Duration::from_secs(60))
///     .with_retry(true);
/// let mut client = McpClient::new(transport);
/// client.initialize().await?;
/// ```
pub struct HttpTransport {
    /// Reqwest HTTP client
    client: reqwest::Client,

    /// MCP server endpoint URLs (supports load balancing across multiple servers)
    urls: Vec<String>,

    /// Current URL index for load balancing (round-robin)
    current_url_index: Arc<AtomicUsize>,

    /// Request timeout
    timeout: Duration,

    /// Buffered response from the last HTTP request
    /// (HTTP is synchronous, so we buffer the response to return in recv())
    buffered_response: Arc<Mutex<Option<McpResponse>>>,

    /// Connection state
    connected: bool,

    /// Enable retry logic with exponential backoff
    enable_retry: bool,

    /// Retry configuration
    retry_config: Option<RetryConfig>,

    /// Custom HTTP headers
    custom_headers: Vec<(String, String)>,

    /// Health status for each server (URL index -> last check time)
    server_health: Arc<Mutex<Vec<(Instant, bool)>>>,

    /// Health check interval
    health_check_interval: Duration,

    /// Enable automatic failover to healthy servers
    enable_failover: bool,
}

/// Health check configuration for load balancing
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Interval between health checks
    pub check_interval: Duration,
    /// Enable automatic failover
    pub enable_failover: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            enable_failover: true,
        }
    }
}

impl HttpTransport {
    /// Create a new HTTP transport for the given MCP server URL
    ///
    /// # Arguments
    ///
    /// * `url` - The base URL of the MCP server (e.g., "https://mcp.example.com")
    ///
    /// # Returns
    ///
    /// Returns a new `HttpTransport` instance
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        let url = url.into();

        // Build reqwest client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        let server_health = vec![(Instant::now(), true)];

        Self {
            client,
            urls: vec![url],
            current_url_index: Arc::new(AtomicUsize::new(0)),
            timeout: Duration::from_secs(30),
            buffered_response: Arc::new(Mutex::new(None)),
            connected: true,
            enable_retry: false,
            retry_config: None,
            custom_headers: Vec::new(),
            server_health: Arc::new(Mutex::new(server_health)),
            health_check_interval: Duration::from_secs(30),
            enable_failover: true,
        }
    }

    /// Create HTTP transport with multiple server URLs for load balancing
    ///
    /// # Arguments
    ///
    /// * `urls` - List of MCP server URLs for load balancing (round-robin)
    ///
    /// # Returns
    ///
    /// Returns a new `HttpTransport` instance with multiple endpoints
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::with_load_balancing(vec![
    ///     "https://mcp1.example.com",
    ///     "https://mcp2.example.com",
    ///     "https://mcp3.example.com",
    /// ]);
    /// ```
    pub fn with_load_balancing(urls: Vec<impl Into<String>>) -> Self {
        let urls: Vec<String> = urls.into_iter().map(|u| u.into()).collect();
        let url_count = urls.len();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        // Initialize health status for each server
        let server_health = vec![(Instant::now(), true); url_count];

        Self {
            client,
            urls,
            current_url_index: Arc::new(AtomicUsize::new(0)),
            timeout: Duration::from_secs(30),
            buffered_response: Arc::new(Mutex::new(None)),
            connected: true,
            enable_retry: false,
            retry_config: None,
            custom_headers: Vec::new(),
            server_health: Arc::new(Mutex::new(server_health)),
            health_check_interval: Duration::from_secs(30),
            enable_failover: true,
        }
    }

    /// Get the next URL in round-robin fashion (for load balancing)
    ///
    /// If failover is enabled, skips unhealthy servers.
    fn get_next_url(&self) -> &str {
        let start_idx = self.current_url_index.fetch_add(1, Ordering::SeqCst);
        let url_count = self.urls.len();

        if !self.enable_failover || url_count == 1 {
            // Simple round-robin without health checks
            let position = start_idx % url_count;
            return &self.urls[position];
        }

        // With failover enabled, find the next healthy server
        if let Ok(health) = self.server_health.lock() {
            for i in 0..url_count {
                let position = (start_idx + i) % url_count;
                if let Some((_, is_healthy)) = health.get(position) {
                    if *is_healthy {
                        return &self.urls[position];
                    }
                }
            }
        }

        // All servers unhealthy or couldn't check - use round-robin anyway
        let position = start_idx % url_count;
        &self.urls[position]
    }

    /// Mark a server as healthy or unhealthy
    fn update_server_health(&self, url_index: usize, healthy: bool) {
        if let Ok(mut health) = self.server_health.lock() {
            if let Some(entry) = health.get_mut(url_index) {
                entry.0 = Instant::now();
                entry.1 = healthy;
                let status = if healthy { "healthy" } else { "unhealthy" };
                debug!(
                    "Server {} ({}) marked as {}",
                    url_index,
                    self.urls.get(url_index).unwrap_or(&"unknown".to_string()),
                    status
                );
            }
        }
    }

    /// Check if a server is healthy and needs re-checking
    ///
    /// Used by periodic health check tasks (future enhancement for Phase 3)
    #[allow(dead_code)]
    fn should_check_health(&self, url_index: usize) -> bool {
        if !self.enable_failover {
            return false;
        }

        if let Ok(health) = self.server_health.lock() {
            if let Some((last_check, _)) = health.get(url_index) {
                let elapsed = last_check.elapsed();
                return elapsed > self.health_check_interval;
            }
        }

        false
    }

    /// Enable health checks and automatic failover
    pub fn with_health_checks(mut self, config: HealthCheckConfig) -> Self {
        self.health_check_interval = config.check_interval;
        self.enable_failover = config.enable_failover;
        self
    }

    /// Set health check interval
    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// Enable or disable automatic failover
    pub fn with_failover(mut self, enabled: bool) -> Self {
        self.enable_failover = enabled;
        self
    }

    /// Set the request timeout
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration for HTTP requests
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_timeout(Duration::from_secs(60));
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        // Rebuild client with new timeout
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to rebuild HTTP client");
        self
    }

    /// Enable retry logic with exponential backoff
    ///
    /// # Arguments
    ///
    /// * `enable` - Whether to enable retry logic
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_retry(true);
    /// ```
    pub fn with_retry(mut self, enable: bool) -> Self {
        self.enable_retry = enable;
        if enable {
            self.retry_config = Some(RetryConfig::default());
        }
        self
    }

    /// Set custom retry configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Custom retry configuration
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = RetryConfig::default()
    ///     .max_attempts(5)
    ///     .base_delay(Duration::from_millis(100));
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_retry_config(config);
    /// ```
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.enable_retry = true;
        self.retry_config = Some(config);
        self
    }

    /// Add a custom HTTP header
    ///
    /// # Arguments
    ///
    /// * `name` - Header name
    /// * `value` - Header value
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_header("Authorization", "Bearer token123")
    ///     .with_header("X-Custom-Header", "value");
    /// ```
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((name.into(), value.into()));
        self
    }

    /// Add Bearer token authentication
    ///
    /// # Arguments
    ///
    /// * `token` - Bearer token value
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_bearer_token("your-token-here");
    /// ```
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.custom_headers
            .push(("Authorization".to_string(), format!("Bearer {}", token.into())));
        self
    }

    /// Add Basic authentication
    ///
    /// # Arguments
    ///
    /// * `username` - Username
    /// * `password` - Password
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_basic_auth("user", "pass");
    /// ```
    pub fn with_basic_auth(mut self, username: &str, password: &str) -> Self {
        let credentials = format!("{}:{}", username, password);
        let encoded = base64_encode(credentials.as_bytes());
        self.custom_headers
            .push(("Authorization".to_string(), format!("Basic {}", encoded)));
        self
    }

    /// Add API key authentication (sent as a header)
    ///
    /// # Arguments
    ///
    /// * `header_name` - Header name for the API key (e.g., "X-API-Key")
    /// * `api_key` - The API key value
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = HttpTransport::new("https://mcp.example.com")
    ///     .with_api_key("X-API-Key", "your-api-key");
    /// ```
    pub fn with_api_key(mut self, header_name: &str, api_key: impl Into<String>) -> Self {
        self.custom_headers
            .push((header_name.to_string(), api_key.into()));
        self
    }

    /// Get the server URLs
    pub fn urls(&self) -> &[String] {
        &self.urls
    }

    /// Get the primary server URL
    pub fn url(&self) -> &str {
        self.urls.first().map(|s| s.as_str()).unwrap_or("")
    }

    /// Get the request timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Check if retry is enabled
    pub fn is_retry_enabled(&self) -> bool {
        self.enable_retry
    }

    /// Disconnect from the MCP server
    ///
    /// For HTTP transport, this just marks the transport as disconnected.
    pub async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }
}

#[allow(async_fn_in_trait)]
impl Transport for HttpTransport {
    /// Send a JSON-RPC request to the MCP server via HTTP POST
    ///
    /// The request is serialized to JSON and sent as an HTTP POST request body.
    /// If retry logic is enabled, transient failures will be retried with exponential backoff.
    /// If load balancing is configured, requests are distributed round-robin across servers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Request serialization fails
    /// - HTTP request fails
    /// - Request times out
    /// - All retry attempts are exhausted
    async fn send(&mut self, request: &McpRequest) -> Result<()> {
        if !self.connected {
            return Err(anyhow::anyhow!("Transport is not connected"));
        }

        // Serialize the request to JSON
        let json =
            serde_json::to_string(request).context("Failed to serialize MCP request to JSON")?;

        // Determine if we should use retry logic
        if self.enable_retry {
            if let Some(config) = self.retry_config.clone() {
                return self.send_with_retry(&json, &config).await;
            }
        }

        // Single attempt without retry
        self.send_request(&json).await
    }

    /// Receive a response from the MCP server
    ///
    /// For HTTP transport, responses are received synchronously with the request,
    /// so this method returns the response that was buffered during send().
    async fn recv(&mut self) -> Result<McpResponse> {
        if !self.connected {
            return Err(anyhow::anyhow!("Transport is not connected"));
        }

        // Retrieve the buffered response
        let mut buffer = self
            .buffered_response
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire response buffer lock: {}", e))?;

        match buffer.take() {
            Some(response) => {
                tracing::debug!("Returning buffered HTTP response");
                Ok(response)
            }
            None => Err(anyhow::anyhow!(
                "No buffered response available - HTTP request must be sent before receiving"
            )),
        }
    }

    /// Check if the transport is still connected
    fn is_connected(&self) -> bool {
        self.connected
    }
}

impl HttpTransport {
    /// Send a single HTTP request to the server
    ///
    /// This is the core HTTP request logic, separated for reuse by retry logic.
    async fn send_request(&self, json: &str) -> Result<()> {
        let url_index = self.current_url_index.load(Ordering::SeqCst);
        let url = self.get_next_url();
        debug!(
            "Sending HTTP POST to {} (server {}): {}",
            url, url_index, json
        );

        // Build request with custom headers
        let mut request_builder = self.client.post(url);
        request_builder = request_builder.header("Content-Type", "application/json");

        // Add custom headers
        for (name, value) in &self.custom_headers {
            request_builder = request_builder.header(name.clone(), value.clone());
        }

        // Send HTTP POST request
        let http_response = match request_builder.body(json.to_string()).send().await {
            Ok(resp) => resp,
            Err(e) => {
                warn!("HTTP request failed to server {}: {}", url, e);
                // Mark server as unhealthy on network error
                if self.enable_failover {
                    self.update_server_health(url_index % self.urls.len(), false);
                }
                return Err(anyhow::anyhow!("Failed to send HTTP request: {}", e));
            }
        };

        // Check HTTP status
        if !http_response.status().is_success() {
            warn!(
                "HTTP request to {} returned status: {}",
                url,
                http_response.status()
            );
            // Mark server as unhealthy on HTTP error
            if self.enable_failover {
                self.update_server_health(url_index % self.urls.len(), false);
            }
            return Err(anyhow::anyhow!(
                "HTTP request failed with status: {}",
                http_response.status()
            ));
        }

        // Read response body
        let response_text = http_response
            .text()
            .await
            .context("Failed to read HTTP response body")?;

        debug!("Received HTTP response from {}: {}", url, response_text);

        // Parse MCP response
        let mcp_response: McpResponse =
            serde_json::from_str(&response_text).with_context(|| {
                format!(
                    "Failed to deserialize MCP response from JSON: {}",
                    response_text
                )
            })?;

        // Mark server as healthy on successful request
        if self.enable_failover {
            self.update_server_health(url_index % self.urls.len(), true);
        }

        // Store the response in the buffer for recv() to retrieve
        let mut buffer = self
            .buffered_response
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire response buffer lock: {}", e))?;
        *buffer = Some(mcp_response);

        Ok(())
    }

    /// Send a request with retry logic and exponential backoff
    async fn send_with_retry(&self, json: &str, config: &RetryConfig) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..config.max_attempts {
            match self.send_request(json).await {
                Ok(()) => {
                    if attempt > 0 {
                        info!(
                            "Request succeeded on attempt {} after {} retries",
                            attempt + 1,
                            attempt
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // Check if we should retry this error
                    if attempt < config.max_attempts - 1 && config.should_retry_error(&e) {
                        let delay = config.calculate_delay(attempt);
                        tracing::warn!(
                            "Request attempt {} failed: {}, retrying after {:?}",
                            attempt + 1,
                            error_msg,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        last_error = Some(e);
                        continue;
                    }

                    // Don't retry or no more attempts
                    tracing::error!(
                        "Request failed after {} attempts: {}",
                        attempt + 1,
                        error_msg
                    );
                    last_error = Some(e);
                    break;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All request attempts failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_transport_creation() {
        let transport = HttpTransport::new("https://example.com/mcp");
        assert_eq!(transport.url(), "https://example.com/mcp");
        assert_eq!(transport.timeout(), Duration::from_secs(30));
        assert!(transport.is_connected());
        assert!(!transport.is_retry_enabled());
    }

    #[test]
    fn test_http_transport_with_timeout() {
        let transport =
            HttpTransport::new("https://example.com/mcp").with_timeout(Duration::from_secs(60));
        assert_eq!(transport.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_http_transport_url_getter() {
        let transport = HttpTransport::new("http://localhost:3000/mcp");
        assert_eq!(transport.url(), "http://localhost:3000/mcp");
    }

    #[test]
    fn test_http_transport_load_balancing_creation() {
        let urls = vec![
            "https://mcp1.example.com",
            "https://mcp2.example.com",
            "https://mcp3.example.com",
        ];
        let transport = HttpTransport::with_load_balancing(urls);
        assert_eq!(transport.urls().len(), 3);
        assert_eq!(transport.url(), "https://mcp1.example.com");
    }

    #[test]
    fn test_http_transport_with_retry_enabled() {
        let transport = HttpTransport::new("https://example.com/mcp").with_retry(true);
        assert!(transport.is_retry_enabled());
        assert!(transport.retry_config.is_some());
    }

    #[test]
    fn test_http_transport_with_custom_retry_config() {
        let config = RetryConfig::default()
            .max_attempts(5)
            .base_delay(Duration::from_millis(50));

        let transport = HttpTransport::new("https://example.com/mcp").with_retry_config(config);
        assert!(transport.is_retry_enabled());
        assert!(transport.retry_config.is_some());
    }

    #[test]
    fn test_http_transport_with_custom_headers() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_header("Authorization", "Bearer token123")
            .with_header("X-Custom-Header", "value");

        assert_eq!(transport.custom_headers.len(), 2);
        assert_eq!(transport.custom_headers[0].0, "Authorization");
        assert_eq!(transport.custom_headers[0].1, "Bearer token123");
    }

    #[tokio::test]
    async fn test_http_transport_send_when_disconnected() {
        let mut transport = HttpTransport::new("https://example.com/mcp");
        transport.connected = false;

        let request = McpRequest::new(1, "test", None);
        let result = transport.send(&request).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not connected"));
    }

    #[test]
    fn test_transport_trait_bounds() {
        // Verify that HttpTransport implements the required trait bounds
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HttpTransport>();
    }

    #[tokio::test]
    async fn test_http_transport_recv_when_disconnected() {
        let mut transport = HttpTransport::new("https://example.com/mcp");
        transport.connected = false;

        let result = transport.recv().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn test_http_transport_recv_without_send() {
        let mut transport = HttpTransport::new("https://example.com/mcp");

        // Try to receive without sending first
        let result = transport.recv().await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No buffered response"));
    }

    #[test]
    fn test_http_transport_round_robin_load_balancing() {
        let urls = vec!["https://server1", "https://server2", "https://server3"];
        let transport = HttpTransport::with_load_balancing(urls);

        // Verify round-robin behavior by calling get_next_url multiple times
        assert_eq!(transport.get_next_url(), "https://server1");
        assert_eq!(transport.get_next_url(), "https://server2");
        assert_eq!(transport.get_next_url(), "https://server3");
        assert_eq!(transport.get_next_url(), "https://server1"); // Cycles back
    }

    #[test]
    fn test_http_transport_builder_chaining() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_timeout(Duration::from_secs(60))
            .with_retry(true)
            .with_header("X-Key", "value");

        assert_eq!(transport.timeout(), Duration::from_secs(60));
        assert!(transport.is_retry_enabled());
        assert_eq!(transport.custom_headers.len(), 1);
    }

    #[tokio::test]
    async fn test_http_transport_disconnect() {
        let mut transport = HttpTransport::new("https://example.com/mcp");
        assert!(transport.is_connected());

        let disconnect_result = transport.disconnect().await;

        assert!(disconnect_result.is_ok());
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_http_transport_multiple_headers() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_header("Authorization", "Bearer xyz")
            .with_header("X-API-Version", "2")
            .with_header("X-Request-ID", "123");

        assert_eq!(transport.custom_headers.len(), 3);
        assert_eq!(transport.custom_headers[1].0, "X-API-Version");
        assert_eq!(transport.custom_headers[2].1, "123");
    }

    #[test]
    fn test_http_transport_retry_disabled_by_default() {
        let transport = HttpTransport::new("https://example.com/mcp");
        assert!(!transport.is_retry_enabled());
        assert!(transport.retry_config.is_none());
    }

    #[test]
    fn test_http_transport_single_url_load_balancing() {
        let transport = HttpTransport::with_load_balancing(vec!["https://single"]);
        assert_eq!(transport.urls().len(), 1);
        assert_eq!(transport.get_next_url(), "https://single");
        assert_eq!(transport.get_next_url(), "https://single"); // Should cycle
    }

    #[test]
    fn test_http_transport_bearer_token_auth() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_bearer_token("my-secret-token");
        
        assert_eq!(transport.custom_headers.len(), 1);
        assert_eq!(transport.custom_headers[0].0, "Authorization");
        assert_eq!(transport.custom_headers[0].1, "Bearer my-secret-token");
    }

    #[test]
    fn test_http_transport_basic_auth() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_basic_auth("user", "pass123");
        
        assert_eq!(transport.custom_headers.len(), 1);
        assert_eq!(transport.custom_headers[0].0, "Authorization");
        // Base64 of "user:pass123" = "dXNlcjpwYXNzMTIz"
        assert_eq!(transport.custom_headers[0].1, "Basic dXNlcjpwYXNzMTIz");
    }

    #[test]
    fn test_http_transport_api_key_auth() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_api_key("X-API-Key", "my-api-key-123");
        
        assert_eq!(transport.custom_headers.len(), 1);
        assert_eq!(transport.custom_headers[0].0, "X-API-Key");
        assert_eq!(transport.custom_headers[0].1, "my-api-key-123");
    }

    #[test]
    fn test_http_transport_multiple_auth_methods() {
        let transport = HttpTransport::new("https://example.com/mcp")
            .with_bearer_token("token")
            .with_api_key("X-Request-ID", "req-123");
        
        assert_eq!(transport.custom_headers.len(), 2);
        assert_eq!(transport.custom_headers[0].0, "Authorization");
        assert_eq!(transport.custom_headers[1].0, "X-Request-ID");
    }

    #[test]
    fn test_base64_encode() {
        // Test known base64 values
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
        assert_eq!(base64_encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }
}
