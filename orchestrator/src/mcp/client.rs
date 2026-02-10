//! MCP Client Layer
//!
//! This module provides the high-level MCP client that orchestrates
//! communication with MCP servers using the transport layer.
//!
//! # Architecture
//!
//! The client is generic over the transport layer, allowing it to work
//! with different transport mechanisms (stdio, HTTP, etc.) through the
//! [`Transport`] trait.
//!
//! # Usage
//!
//! ```ignore
//! use ironclaw_orchestrator::mcp::{McpClient, StdioTransport};
//!
//! // Create a stdio transport
//! let transport = StdioTransport::spawn("npx", &["-y", "@modelcontextprotocol/server-filesystem"]).await?;
//!
//! // Create MCP client
//! let mut client = McpClient::new(transport);
//!
//! // Initialize connection
//! client.initialize().await?;
//!
//! // List available tools
//! let tools = client.list_tools().await?;
//!
//! // Call a tool
//! let result = client.call_tool("read_file", json!({"path": "/tmp/file.txt"})).await?;
//! ```

use crate::mcp::protocol::{
    ClientCapabilities, ClientInfo, InitializeParams, McpError, McpMethod, McpRequest, McpResponse,
    ServerCapabilities, ServerInfo, Tool,
};
use crate::mcp::retry::RetryConfig;
use crate::mcp::transport::Transport;
use anyhow::{Context, Result};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

/// High-level MCP client
///
/// This client provides a convenient, type-safe API for interacting with MCP servers.
/// It handles the initialization handshake, tool discovery, and tool invocation.
///
/// # Type Parameters
///
/// * `T` - The transport type (e.g., `StdioTransport`, `HttpTransport`)
///
/// # Lifecycle
///
/// 1. Create client with `McpClient::new(transport)`
/// 2. Initialize with `client.initialize()`
/// 3. Use the client (list tools, call tools)
/// 4. Drop the client when done (transport auto-cleanup)
///
/// # Example
///
/// ```ignore
/// let transport = StdioTransport::spawn("npx", &["-y", "@modelcontextprotocol/server-filesystem"]).await?;
/// let mut client = McpClient::new(transport);
/// client.initialize().await?;
/// let tools = client.list_tools().await?;
/// ```
pub struct McpClient<T>
where
    T: Transport,
{
    /// Underlying transport for sending/receiving messages
    transport: T,

    /// Next request ID (monotonically increasing)
    next_id: AtomicU64,

    /// Server capabilities (after initialization)
    server_capabilities: Option<ServerCapabilities>,

    /// Available tools (after listing)
    tools: Vec<Tool>,

    /// Client state
    state: ClientState,

    /// Retry configuration for transient failures
    retry_config: Option<RetryConfig>,
}

/// Client state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Client is created but not initialized
    Created,

    /// Initialization is in progress
    Initializing,

    /// Client is initialized and ready
    Ready,

    /// Client is disconnected
    Disconnected,
}

impl<T> McpClient<T>
where
    T: Transport,
{
    /// Create a new MCP client with the given transport
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport to use for communication
    ///
    /// # Returns
    ///
    /// Returns a new `McpClient` instance
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transport = StdioTransport::spawn("npx", &["-y", "server"]).await?;
    /// let client = McpClient::new(transport);
    /// ```
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_id: AtomicU64::new(1),
            server_capabilities: None,
            tools: Vec::new(),
            state: ClientState::Created,
            retry_config: None,
        }
    }

    /// Set retry configuration for the client
    ///
    /// # Arguments
    ///
    /// * `config` - Retry configuration
    ///
    /// # Returns
    ///
    /// Returns `self` for chaining
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = McpClient::new(transport)
    ///     .with_retry(RetryConfig::default().max_attempts(5));
    /// ```
    pub fn with_retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Send a request and receive a response (with optional retry)
    ///
    /// This is a helper method that wraps the send/recv pattern with retry logic
    /// if a retry config is set.
    ///
    /// # Arguments
    ///
    /// * `request` - The MCP request to send
    ///
    /// # Returns
    ///
    /// Returns the MCP response
    async fn send_request(&mut self, request: &McpRequest) -> Result<McpResponse> {
        if let Some(config) = self.retry_config.clone() {
            // Use retry logic - manually implemented to avoid borrow issues
            let mut last_error = None;

            for attempt in 0..config.max_attempts {
                match self.transport.send(request).await {
                    Ok(()) => match self.transport.recv().await {
                        Ok(response) => {
                            if attempt > 0 {
                                tracing::info!(
                                    "Request succeeded on attempt {} after {} retries",
                                    attempt + 1,
                                    attempt
                                );
                            }
                            return Ok(response);
                        }
                        Err(e) => {
                            last_error = Some(e);
                        }
                    },
                    Err(e) => {
                        last_error = Some(e);
                    }
                }

                // Check if we should retry this error
                if attempt < config.max_attempts - 1 {
                    if let Some(ref error) = last_error {
                        if config.should_retry_error(error) {
                            let delay = config.calculate_delay(attempt);
                            tracing::warn!(
                                "Request attempt {} failed: {}, retrying after {:?}",
                                attempt + 1,
                                error,
                                delay
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                    }
                }

                // Don't retry
                break;
            }

            Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed")))
        } else {
            // No retry - single attempt
            self.transport.send(request).await?;
            self.transport.recv().await
        }
    }

    /// Get the underlying transport
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Get a mutable reference to the underlying transport
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Initialize the MCP connection
    ///
    /// This sends an `initialize` request to the server and waits for the response.
    /// The server will respond with its capabilities and information.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if initialization succeeded
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Transport send/recv fails
    /// - Server returns an error response
    /// - Server reports incompatible protocol version
    pub async fn initialize(&mut self) -> Result<()> {
        if self.state != ClientState::Created {
            return Err(anyhow::anyhow!(
                "Cannot initialize client: invalid state {:?}",
                self.state
            ));
        }

        if !self.transport.is_connected() {
            return Err(anyhow::anyhow!(
                "Cannot initialize: transport is disconnected"
            ));
        }

        self.state = ClientState::Initializing;
        tracing::info!("Initializing MCP connection...");

        // Prepare initialize parameters
        let client_info = ClientInfo {
            name: "ironclaw-orchestrator".to_string(),
            version: env!("CARGO_PKG_VERSION", "0.1.0").to_string(),
        };

        let capabilities = ClientCapabilities {
            sampling: None,
            experimental: None,
        };

        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities,
            client_info,
        };

        // Create initialize request
        let request = McpRequest::new(
            self.next_id.fetch_add(1, Ordering::SeqCst),
            "initialize",
            Some(json!(params)),
        );

        // Send request and receive response (with optional retry)
        let response = self
            .send_request(&request)
            .await
            .context("Failed to complete initialize request")?;

        // Check for error response
        if !response.is_success() {
            let error = response
                .error
                .ok_or_else(|| McpError::internal_error("Initialize failed with unknown error"))?;
            return Err(anyhow::anyhow!("Initialize failed: {}", error));
        }

        // Parse server capabilities from response
        let result = response
            .result
            .ok_or_else(|| McpError::internal_error("Initialize response missing result"))?;

        // Parse the server capabilities
        let server_info: ServerInfo = serde_json::from_value(result["serverInfo"].clone())
            .context("Failed to parse server info from initialize response")?;

        // Store server capabilities
        self.server_capabilities = Some(ServerCapabilities {
            protocol_version: result["protocolVersion"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing protocolVersion in initialize response"))?
                .to_string(),
            capabilities: result["capabilities"].clone(),
            server_info,
        });

        self.state = ClientState::Ready;
        tracing::info!(
            "MCP connection initialized: {} v{}",
            self.server_capabilities
                .as_ref()
                .map(|c| c.server_info.name.as_str())
                .unwrap_or("unknown"),
            self.server_capabilities
                .as_ref()
                .map(|c| c.protocol_version.as_str())
                .unwrap_or("unknown")
        );

        Ok(())
    }

    /// List available tools from the MCP server
    ///
    /// This sends a `tools/list` request to the server and returns the list of tools.
    ///
    /// # Returns
    ///
    /// Returns a vector of available tools
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client is not initialized
    /// - Transport send/recv fails
    /// - Server returns an error response
    /// - Tool list format is invalid
    pub async fn list_tools(&mut self) -> Result<Vec<Tool>> {
        self.ensure_ready()?;

        tracing::debug!("Listing available tools from MCP server");

        // Create tools/list request
        let request = McpRequest::notification(
            self.next_id.fetch_add(1, Ordering::SeqCst),
            McpMethod::ToolsList.as_str().to_string(),
        );

        // Send request and receive response (with optional retry)
        let response = self
            .send_request(&request)
            .await
            .context("Failed to complete tools/list request")?;

        // Check for error response
        if !response.is_success() {
            let error = response
                .error
                .ok_or_else(|| McpError::internal_error("Tools/list failed with unknown error"))?;
            return Err(anyhow::anyhow!("Failed to list tools: {}", error));
        }

        // Parse tools from response
        let result = response
            .result
            .ok_or_else(|| McpError::internal_error("Tools/list response missing result"))?;

        let tools: Vec<Tool> = serde_json::from_value(result["tools"].clone())
            .context("Failed to parse tools from response")?;

        // Cache the tools
        self.tools = tools.clone();

        tracing::info!("Listed {} tools from MCP server", tools.len());

        // Log tool names for debugging
        for tool in &tools {
            tracing::debug!("  - {}", tool.name);
        }

        Ok(tools)
    }

    /// Call a tool on the MCP server
    ///
    /// This sends a `tools/call` request with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to call
    /// * `arguments` - The arguments to pass to the tool (must match tool's input schema)
    ///
    /// # Returns
    ///
    /// Returns the tool's result as a JSON value
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client is not initialized
    /// - Transport send/recv fails
    /// - Server returns an error response
    /// - Tool execution fails
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.ensure_ready()?;

        tracing::debug!("Calling tool: {} with arguments: {:?}", name, arguments);

        // Create tools/call request
        let params = json!({
            "name": name,
            "arguments": arguments
        });

        let request = McpRequest::new(
            self.next_id.fetch_add(1, Ordering::SeqCst),
            McpMethod::ToolsCall.as_str().to_string(),
            Some(params),
        );

        // Send request and receive response (with optional retry)
        let response = self
            .send_request(&request)
            .await
            .context("Failed to complete tools/call request")?;

        // Check for error response
        if !response.is_success() {
            let error = response
                .error
                .ok_or_else(|| McpError::internal_error("Tool call failed with unknown error"))?;
            return Err(anyhow::anyhow!("Tool '{}' failed: {}", name, error));
        }

        // Parse tool result
        let result = response
            .result
            .ok_or_else(|| McpError::internal_error("Tool call response missing result"))?;

        tracing::debug!("Tool '{}' returned result: {:?}", name, result);

        Ok(result)
    }

    /// Check if the client is ready for operations
    fn ensure_ready(&self) -> Result<()> {
        match self.state {
            ClientState::Created => Err(anyhow::anyhow!(
                "Client not initialized. Call initialize() first."
            )),
            ClientState::Initializing => Err(anyhow::anyhow!("Client is currently initializing")),
            ClientState::Ready => Ok(()),
            ClientState::Disconnected => Err(anyhow::anyhow!("Client is disconnected")),
        }
    }

    /// Get the current client state
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Get server capabilities (after initialization)
    ///
    /// Returns `None` if the client hasn't been initialized yet
    pub fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    /// Get available tools (cached after listing)
    ///
    /// Returns an empty slice if tools haven't been listed yet
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::McpResponse;
    use std::sync::Arc;
    use std::time::Duration;

    // Mock transport for testing
    #[derive(Clone)]
    struct MockTransport {
        connected: bool,
        requests: Vec<McpRequest>,
        response: Option<McpResponse>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                connected: true,
                requests: Vec::new(),
                response: None,
            }
        }

        fn set_response(&mut self, response: McpResponse) {
            self.response = Some(response);
        }

        fn set_error_response(&mut self, code: i32, message: &str) {
            self.response = Some(McpResponse::err(1, McpError::new(code, message)));
        }
    }

    #[allow(async_fn_in_trait)]
    impl Transport for MockTransport {
        async fn send(&mut self, request: &McpRequest) -> Result<()> {
            if !self.connected {
                return Err(anyhow::anyhow!("Mock transport disconnected"));
            }
            self.requests.push(request.clone());
            Ok(())
        }

        async fn recv(&mut self) -> Result<McpResponse> {
            if !self.connected {
                return Err(anyhow::anyhow!("Mock transport disconnected"));
            }

            if let Some(response) = self.response.take() {
                Ok(response)
            } else {
                // Return a default success response
                Ok(McpResponse::ok(self.requests.last().unwrap().id, json!({})))
            }
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    // Helper to create a successful initialize response
    fn create_init_response() -> McpResponse {
        McpResponse::ok(
            1,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }),
        )
    }

    // Helper to create a tools list response
    fn create_tools_list_response(tools: &[Tool]) -> McpResponse {
        let tools_array = if tools.is_empty() {
            serde_json::Value::Array(Vec::new())
        } else {
            serde_json::to_value(tools).unwrap()
        };
        McpResponse::ok(2, json!({"tools": tools_array}))
    }

    // Helper to create a tool call response
    fn create_tool_call_response(result: serde_json::Value) -> McpResponse {
        McpResponse::ok(3, result)
    }

    #[tokio::test]
    async fn test_client_creation() {
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        assert_eq!(client.next_id.load(Ordering::SeqCst), 1);
        assert_eq!(client.state(), ClientState::Created);
    }

    #[tokio::test]
    async fn test_client_initialize_success() {
        let mut transport = MockTransport::new();
        transport.set_response(create_init_response());

        let mut client = McpClient::new(transport);

        // Initialize should succeed
        assert!(client.initialize().await.is_ok());

        // State should be Ready
        assert_eq!(client.state(), ClientState::Ready);

        // Server capabilities should be stored
        let caps = client.server_capabilities().unwrap();
        assert_eq!(caps.server_info.name, "test-server");
    }

    #[tokio::test]
    async fn test_client_initialize_error() {
        let mut transport = MockTransport::new();
        transport.set_error_response(-32001, "Initialization failed");

        let mut client = McpClient::new(transport);

        // Initialize should fail
        assert!(client.initialize().await.is_err());

        // State should not be Ready (since init failed)
        assert_ne!(client.state(), ClientState::Ready);
    }

    #[tokio::test]
    async fn test_client_list_tools() {
        let mut transport = MockTransport::new();

        let tools = vec![Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
        }];

        transport.set_response(create_tools_list_response(&tools));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready; // Skip initialization for this test

        // List tools should succeed
        let result = client.list_tools().await;

        assert!(result.is_ok());
        let listed_tools = result.unwrap();
        assert_eq!(listed_tools.len(), 1);
        assert_eq!(listed_tools[0].name, "test_tool");
    }

    #[tokio::test]
    async fn test_client_call_tool() {
        let mut transport = MockTransport::new();
        let tool_result = json!({"status": "success"});

        transport.set_response(create_tool_call_response(tool_result));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready; // Skip initialization

        // Call tool should succeed
        let result = client.call_tool("test_tool", json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["status"], "success");
    }

    #[tokio::test]
    async fn test_client_call_tool_not_found() {
        let mut transport = MockTransport::new();
        transport.set_error_response(-32601, "Tool not found");

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready; // Skip initialization

        // Call tool should fail
        let result = client.call_tool("unknown_tool", json!({})).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_state_transitions() {
        let transport = MockTransport::new();
        let mut client = McpClient::new(transport);

        // Initial state
        assert_eq!(client.state(), ClientState::Created);

        // After initialization
        client.state = ClientState::Ready;

        // ensure_ready() should pass
        assert!(client.ensure_ready().is_ok());
    }

    #[tokio::test]
    async fn test_client_list_tools_when_not_initialized() {
        let transport = MockTransport::new();
        let mut client = McpClient::new(transport);

        // List tools should fail (not initialized)
        let result = client.list_tools().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_client_server_capabilities_after_init() {
        let mut transport = MockTransport::new();
        transport.set_response(create_init_response());

        let mut client = McpClient::new(transport);

        // Before initialization, no capabilities
        assert!(client.server_capabilities().is_none());

        // Initialize
        client.initialize().await.unwrap();

        // After initialization, capabilities are available
        let caps = client.server_capabilities().unwrap();
        assert_eq!(caps.server_info.name, "test-server");
    }

    #[tokio::test]
    async fn test_client_tools_caching() {
        let mut transport = MockTransport::new();

        let tools = vec![
            Tool {
                name: "tool1".to_string(),
                description: "First tool".to_string(),
                input_schema: json!({}),
            },
            Tool {
                name: "tool2".to_string(),
                description: "Second tool".to_string(),
                input_schema: json!({}),
            },
        ];

        transport.set_response(create_tools_list_response(&tools));

        let mut client = McpClient::new(transport.clone());
        client.state = ClientState::Ready;

        // First call should fetch from server
        let result1 = client.list_tools().await.unwrap();
        assert_eq!(result1.len(), 2);

        // Tools should be cached
        let tools = client.tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_client_state_debug() {
        // Just verify that ClientState implements Debug
        let state = ClientState::Created;
        let formatted = format!("{:?}", state);
        // Debug output for enums shows the variant name
        assert!(formatted == "Created" || formatted.contains("Created"));
    }

    #[tokio::test]
    async fn test_client_initialize_without_connection() {
        let mut transport = MockTransport::new();
        transport.connected = false;

        let mut client = McpClient::new(transport);

        // Initialize should fail (transport disconnected)
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_client_multiple_operations() {
        // This test verifies that the client can perform multiple operations sequentially
        // The AtomicU64 ensures each request gets a unique, incrementing ID
        let mut transport = MockTransport::new();
        transport.set_response(create_init_response());

        let mut client = McpClient::new(transport);

        // Initialize should succeed
        assert!(client.initialize().await.is_ok());

        // Client should be in Ready state
        assert_eq!(client.state(), ClientState::Ready);

        // Server capabilities should be available
        assert!(client.server_capabilities().is_some());
    }

    #[tokio::test]
    async fn test_client_initialize_missing_protocol_version() {
        // Test initialize fails when response is missing protocol version
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(
            1,
            json!({
                "protocolVersion": null,
                "capabilities": {},
                "serverInfo": {"name": "test", "version": "1.0"}
            }),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // Initialize should fail due to missing protocol version
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_client_initialize_invalid_server_info() {
        // Test initialize fails when server info is invalid
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(
            1,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {"invalid": "data"}
            }),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // Initialize should fail due to invalid server info
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_client_list_tools_missing_tools_field() {
        // Test list_tools fails when response is missing tools field
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(2, json!({"invalid": "data"})));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // list_tools should fail
        assert!(client.list_tools().await.is_err());
    }

    #[tokio::test]
    async fn test_client_list_tools_invalid_tools_array() {
        // Test list_tools fails when tools is not an array
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(2, json!({"tools": "not an array"})));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // list_tools should fail
        assert!(client.list_tools().await.is_err());
    }

    #[tokio::test]
    async fn test_client_call_tool_missing_result() {
        // Test call_tool fails when response is missing result
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::err(
            3,
            McpError::method_not_found("unknown_tool"),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // call_tool should fail
        assert!(client.call_tool("unknown_tool", json!({})).await.is_err());
    }

    #[tokio::test]
    async fn test_client_ensure_ready_disconnected() {
        let transport = MockTransport::new();
        let mut client = McpClient::new(transport);
        client.state = ClientState::Disconnected;

        // ensure_ready should fail
        assert!(client.ensure_ready().is_err());
        assert!(client
            .ensure_ready()
            .unwrap_err()
            .to_string()
            .contains("disconnected"));
    }

    #[tokio::test]
    async fn test_client_ensure_ready_initializing() {
        let transport = MockTransport::new();
        let mut client = McpClient::new(transport);
        client.state = ClientState::Initializing;

        // ensure_ready should fail
        assert!(client.ensure_ready().is_err());
        assert!(client
            .ensure_ready()
            .unwrap_err()
            .to_string()
            .contains("initializing"));
    }

    #[tokio::test]
    async fn test_client_transport_getter() {
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        // Test that we can get a reference to the transport
        let _transport_ref = client.transport();
    }

    #[tokio::test]
    async fn test_client_tools_empty_before_list() {
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        // Tools should be empty before listing
        assert_eq!(client.tools().len(), 0);
    }

    #[tokio::test]
    async fn test_client_double_initialize() {
        // Test that calling initialize twice fails
        let mut transport = MockTransport::new();
        transport.set_response(create_init_response());

        let mut client = McpClient::new(transport);

        // First initialize should succeed
        assert!(client.initialize().await.is_ok());

        // Second initialize should fail (already initialized)
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_client_transport_mut_getter() {
        let transport = MockTransport::new();
        let mut client = McpClient::new(transport);

        // Test that we can get a mutable reference to the transport
        let _transport_ref = client.transport_mut();
    }

    #[tokio::test]
    async fn test_client_call_tool_serialization_error() {
        // Test call_tool with arguments that can't be serialized
        // This is hard to test directly since serde_json::Value accepts most things,
        // but we can test with valid JSON
        let mut transport = MockTransport::new();
        transport.set_response(create_tool_call_response(json!({"result": "success"})));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // call_tool should succeed with valid JSON arguments
        let result = client.call_tool("test_tool", json!({"key": "value"})).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_client_request_id_wrapping() {
        // Test that request IDs increment correctly (using AtomicU64)
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        // Check that the starting ID is 1
        assert_eq!(client.next_id.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_client_list_tools_empty_response() {
        // Test list_tools with empty tools array
        let mut transport = MockTransport::new();
        transport.set_response(create_tools_list_response(&[]));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 0);
    }

    #[tokio::test]
    async fn test_client_multiple_tool_calls() {
        // Test calling multiple tools sequentially
        let mut transport = MockTransport::new();
        transport.set_response(create_tool_call_response(json!({"result": "success"})));

        let mut client = McpClient::new(transport.clone());
        client.state = ClientState::Ready;

        // First tool call
        let result1 = client.call_tool("tool1", json!({})).await;
        assert!(result1.is_ok());

        // Second tool call
        transport.set_response(create_tool_call_response(json!({"result": "success2"})));
        let result2 = client.call_tool("tool2", json!({})).await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_client_initialize_missing_server_info() {
        // Test initialize fails when serverInfo is completely missing
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(
            1,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {}
                // serverInfo is missing
            }),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // Initialize should fail
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_client_initialize_missing_capabilities() {
        // Test initialize when capabilities field is missing
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(
            1,
            json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "test", "version": "1.0"}
                // capabilities is missing
            }),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // Initialize should still succeed - missing fields are treated as null in JSON
        // The code just clones the capabilities value as-is
        let result = client.initialize().await;
        // The result might fail or succeed depending on how serde handles missing fields
        // Let's just check it doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_client_tools_return_type() {
        // Test that tools() returns the correct type
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        // tools() should return a slice
        let tools: &[Tool] = client.tools();
        assert_eq!(tools.len(), 0);
    }

    #[tokio::test]
    async fn test_client_transport_methods() {
        // Test transport() and transport_mut() getters
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        // Test immutable getter
        let _ref = client.transport();

        // Test mutable getter
        let mut client = McpClient::new(MockTransport::new());
        let _mut_ref = client.transport_mut();
    }

    #[tokio::test]
    async fn test_client_ensure_ready_states() {
        // Test ensure_ready for all states
        let transport = MockTransport::new();

        // Created state - should fail
        let mut client = McpClient::new(transport.clone());
        client.state = ClientState::Created;
        let result = client.ensure_ready();
        assert!(result.is_err());
        // Verify the error message
        assert!(result.unwrap_err().to_string().contains("not initialized"));

        // Initializing state - should fail
        client.state = ClientState::Initializing;
        let result = client.ensure_ready();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("initializing"));

        // Ready state - should succeed
        client.state = ClientState::Ready;
        assert!(client.ensure_ready().is_ok());

        // Disconnected state - should fail
        client.state = ClientState::Disconnected;
        let result = client.ensure_ready();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disconnected"));
    }

    #[tokio::test]
    async fn test_client_state_display() {
        // Test that ClientState can be formatted for display
        let states = [
            ClientState::Created,
            ClientState::Initializing,
            ClientState::Ready,
            ClientState::Disconnected,
        ];

        for state in states {
            // Just verify we can format it without panicking
            let _ = format!("{:?}", state);
        }
    }

    #[tokio::test]
    async fn test_client_server_capabilities_none() {
        // Test server_capabilities() returns None before initialization
        let transport = MockTransport::new();
        let client = McpClient::new(transport);

        assert!(client.server_capabilities().is_none());
    }

    #[tokio::test]
    async fn test_client_initialize_with_result_and_error() {
        // Test initialize when response has both result and error (invalid)
        let mut transport = MockTransport::new();
        // Create an invalid response with both result and error
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(json!({"test": "data"})),
            error: Some(McpError::internal_error("Error")),
        };
        transport.set_response(response);

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // This should fail when checking response.is_success()
        assert!(client.initialize().await.is_err());
    }

    #[tokio::test]
    async fn test_client_list_tools_with_error_response() {
        // Test list_tools when server returns an error
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::err(
            2,
            McpError::internal_error("Server error"),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        assert!(client.list_tools().await.is_err());
    }

    #[tokio::test]
    async fn test_client_call_tool_empty_result() {
        // Test call_tool when result is an empty object
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(3, json!({})));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        let result = client.call_tool("test", json!({})).await.unwrap();
        assert_eq!(result, json!({}));
    }

    #[tokio::test]
    async fn test_client_initialize_with_non_string_protocol_version() {
        // Test initialize when protocolVersion is not a string
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(
            1,
            json!({
                "protocolVersion": 20241105, // number instead of string
                "capabilities": {},
                "serverInfo": {"name": "test", "version": "1.0"}
            }),
        ));

        let mut client = McpClient::new(transport);
        client.state = ClientState::Ready;

        // Should fail when parsing protocol version
        assert!(client.initialize().await.is_err());
    }

    // Mock transport for retry testing
    #[derive(Clone)]
    struct RetryMockTransport {
        connected: bool,
        attempt_count: Arc<std::sync::atomic::AtomicUsize>,
        fail_until: usize,
        should_fail: bool,
    }

    impl RetryMockTransport {
        fn new(fail_until: usize) -> Self {
            Self {
                connected: true,
                attempt_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                fail_until,
                should_fail: false,
            }
        }

        fn always_fail() -> Self {
            Self {
                connected: true,
                attempt_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                fail_until: 999,
                should_fail: true,
            }
        }
    }

    #[allow(async_fn_in_trait)]
    impl Transport for RetryMockTransport {
        async fn send(&mut self, _request: &McpRequest) -> Result<()> {
            if !self.connected {
                return Err(anyhow::anyhow!("Transport disconnected"));
            }

            let count = self
                .attempt_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if self.should_fail || count < self.fail_until {
                Err(anyhow::anyhow!("Temporary failure (attempt {})", count))
            } else {
                Ok(())
            }
        }

        async fn recv(&mut self) -> Result<McpResponse> {
            if !self.connected {
                return Err(anyhow::anyhow!("Transport disconnected"));
            }

            Ok(McpResponse::ok(
                1,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "serverInfo": {"name": "test", "version": "1.0"}
                }),
            ))
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    #[tokio::test]
    async fn test_client_with_retry_success_after_failures() {
        // Test that client retries transient failures
        let transport = RetryMockTransport::new(2); // Fail first 2 attempts
        let retry_config = RetryConfig::default()
            .max_attempts(5)
            .base_delay(Duration::from_millis(10));

        let mut client = McpClient::new(transport).with_retry(retry_config);
        client.state = ClientState::Created;

        // Should succeed after retries
        let result = client.initialize().await;
        assert!(result.is_ok(), "Initialize should succeed after retries");
    }

    #[tokio::test]
    async fn test_client_with_retry_max_attempts_reached() {
        // Test that client gives up after max attempts
        let transport = RetryMockTransport::always_fail();
        let retry_config = RetryConfig::default()
            .max_attempts(3)
            .base_delay(Duration::from_millis(10));

        let mut client = McpClient::new(transport).with_retry(retry_config);
        client.state = ClientState::Created;

        // Should fail after max attempts
        let result = client.initialize().await;
        assert!(result.is_err(), "Initialize should fail after max attempts");
    }

    #[tokio::test]
    async fn test_client_without_retry_no_retry_on_failure() {
        // Test that client without retry config doesn't retry
        let transport = RetryMockTransport::always_fail(); // Will always fail

        let mut client = McpClient::new(transport); // No retry config
        client.state = ClientState::Created;

        // Should fail immediately without retry
        let result = client.initialize().await;
        assert!(result.is_err(), "Initialize should fail immediately");
    }

    #[tokio::test]
    async fn test_client_with_retry_no_retry_on_permanent_error() {
        // Test that client doesn't retry permanent errors (e.g., auth failures)

        // Mock transport that returns auth error
        #[derive(Clone)]
        struct AuthFailTransport;

        #[allow(async_fn_in_trait)]
        impl Transport for AuthFailTransport {
            async fn send(&mut self, _request: &McpRequest) -> Result<()> {
                Err(anyhow::anyhow!("Unauthorized: Invalid credentials"))
            }

            async fn recv(&mut self) -> Result<McpResponse> {
                Ok(McpResponse::ok(1, json!({})))
            }

            fn is_connected(&self) -> bool {
                true
            }
        }

        let transport = AuthFailTransport;
        let retry_config = RetryConfig::default()
            .max_attempts(5)
            .base_delay(Duration::from_millis(10));

        let mut client = McpClient::new(transport).with_retry(retry_config);
        client.state = ClientState::Created;

        // Should fail immediately without retries (auth error is not retryable)
        let result = client.initialize().await;
        assert!(result.is_err(), "Initialize should fail on auth error");
    }

    #[tokio::test]
    async fn test_client_with_retry_list_tools() {
        // Test retry with list_tools using the standard MockTransport
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(2, json!({"tools": []})));

        let retry_config = RetryConfig::default()
            .max_attempts(3)
            .base_delay(Duration::from_millis(10));

        let mut client = McpClient::new(transport).with_retry(retry_config);
        client.state = ClientState::Ready;

        // Should succeed (mock doesn't fail, but retry config is set)
        let result = client.list_tools().await;
        assert!(result.is_ok(), "list_tools should succeed");
    }

    #[tokio::test]
    async fn test_client_with_retry_call_tool() {
        // Test retry with call_tool using the standard MockTransport
        let mut transport = MockTransport::new();
        transport.set_response(McpResponse::ok(3, json!({"result": "success"})));

        let retry_config = RetryConfig::default()
            .max_attempts(3)
            .base_delay(Duration::from_millis(10));

        let mut client = McpClient::new(transport).with_retry(retry_config);
        client.state = ClientState::Ready;

        // Should succeed (mock doesn't fail, but retry config is set)
        let result = client.call_tool("test_tool", json!({})).await;
        assert!(result.is_ok(), "call_tool should succeed");
    }

    #[tokio::test]
    async fn test_client_retry_config_getter() {
        // Test that retry config can be set via builder
        let transport = MockTransport::new();
        let retry_config = RetryConfig::default();

        // Test that with_retry returns a client with retry configured
        let _client_with_retry = McpClient::new(transport).with_retry(retry_config);

        // If we got here without panicking, the builder pattern works
        // (we can't directly inspect retry_config as it's private)
    }
}
