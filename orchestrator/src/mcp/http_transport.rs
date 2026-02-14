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
//! # Example
//!
//! ```ignore
//! use luminaguard_orchestrator::mcp::{McpClient, HttpTransport};
//!
//! // Create HTTP transport
//! let transport = HttpTransport::new("https://api.example.com/mcp");
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
use crate::mcp::transport::Transport;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

/// HTTP transport for remote MCP servers
///
/// This transport uses HTTP POST requests to communicate with MCP servers.
/// Each request/response pair is a separate HTTP transaction.
///
/// # Configuration
///
/// - **url**: The base URL of the MCP server endpoint
/// - **timeout**: Request timeout (default: 30 seconds)
/// - **headers**: Optional custom HTTP headers (e.g., authentication)
///
/// # Example
///
/// ```ignore
/// let transport = HttpTransport::new("https://mcp.example.com");
/// let mut client = McpClient::new(transport);
/// client.initialize().await?;
/// ```
pub struct HttpTransport {
    /// Reqwest HTTP client
    client: reqwest::Client,

    /// MCP server endpoint URL
    url: String,

    /// Request timeout
    timeout: Duration,

    /// Buffered response from the last HTTP request
    /// (HTTP is synchronous, so we buffer the response to return in recv())
    buffered_response: Arc<Mutex<Option<McpResponse>>>,

    /// Connection state
    connected: bool,
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

        Self {
            client,
            url,
            timeout: Duration::from_secs(30),
            buffered_response: Arc::new(Mutex::new(None)),
            connected: true,
        }
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

    /// Get the server URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the request timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
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
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Request serialization fails
    /// - HTTP request fails
    /// - Request times out
    async fn send(&mut self, request: &McpRequest) -> Result<()> {
        if !self.connected {
            return Err(anyhow::anyhow!("Transport is not connected"));
        }

        // Serialize the request to JSON
        let json =
            serde_json::to_string(request).context("Failed to serialize MCP request to JSON")?;

        tracing::debug!("Sending HTTP POST to {}: {}", self.url, json);

        // Send HTTP POST request
        let http_response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .context("Failed to send HTTP request")?;

        // Check HTTP status
        if !http_response.status().is_success() {
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

        tracing::debug!("Received HTTP response: {}", response_text);

        // Parse MCP response
        let mcp_response: McpResponse =
            serde_json::from_str(&response_text).with_context(|| {
                format!(
                    "Failed to deserialize MCP response from JSON: {}",
                    response_text
                )
            })?;

        // Store the response in the buffer for recv() to retrieve
        let mut buffer = self
            .buffered_response
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire response buffer lock: {}", e))?;
        *buffer = Some(mcp_response);

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_transport_creation() {
        let transport = HttpTransport::new("https://example.com/mcp");
        assert_eq!(transport.url(), "https://example.com/mcp");
        assert_eq!(transport.timeout(), Duration::from_secs(30));
        assert!(transport.is_connected());
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
}
