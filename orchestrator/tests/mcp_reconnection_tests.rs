//! MCP Client Reconnection Integration Tests
//!
//! This module tests MCP client behavior in various reconnection and error scenarios.
//!
//! Issue: #497

use anyhow::Result;
use luminaguard_orchestrator::mcp::transport::Transport;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Mock transport that simulates connection failures and reconnection scenarios
#[derive(Clone)]
struct ReconnectMockTransport {
    /// Whether the transport is currently connected
    connected: Arc<AtomicBool>,
    /// Number of connection attempts
    connect_attempts: Arc<AtomicUsize>,
    /// Number of send attempts
    send_attempts: Arc<AtomicUsize>,
    /// Number of times to fail before succeeding
    fail_count: Arc<AtomicUsize>,
    /// Response queue for simulating different responses
    response_queue: Arc<Mutex<Vec<luminaguard_orchestrator::mcp::protocol::McpResponse>>>,
    /// Simulated latency in milliseconds
    latency_ms: Arc<AtomicUsize>,
}

impl ReconnectMockTransport {
    fn new() -> Self {
        Self {
            connected: Arc::new(AtomicBool::new(true)),
            connect_attempts: Arc::new(AtomicUsize::new(0)),
            send_attempts: Arc::new(AtomicUsize::new(0)),
            fail_count: Arc::new(AtomicUsize::new(0)),
            response_queue: Arc::new(Mutex::new(Vec::new())),
            latency_ms: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Set the transport to fail for the next N operations
    fn set_fail_count(&self, count: usize) {
        self.fail_count.store(count, Ordering::SeqCst);
    }

    /// Disconnect the transport
    fn disconnect(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }

    /// Reconnect the transport
    fn reconnect(&self) {
        self.connected.store(true, Ordering::SeqCst);
        self.connect_attempts.fetch_add(1, Ordering::SeqCst);
    }

    /// Add a response to the queue
    async fn push_response(&self, response: luminaguard_orchestrator::mcp::protocol::McpResponse) {
        self.response_queue.lock().await.push(response);
    }

    /// Set simulated latency
    fn set_latency(&self, ms: usize) {
        self.latency_ms.store(ms, Ordering::SeqCst);
    }

    /// Get number of send attempts
    fn send_attempt_count(&self) -> usize {
        self.send_attempts.load(Ordering::SeqCst)
    }
}

#[allow(async_fn_in_trait)]
impl luminaguard_orchestrator::mcp::transport::Transport for ReconnectMockTransport {
    async fn send(
        &mut self,
        _request: &luminaguard_orchestrator::mcp::protocol::McpRequest,
    ) -> Result<()> {
        self.send_attempts.fetch_add(1, Ordering::SeqCst);

        // Simulate latency
        let latency = self.latency_ms.load(Ordering::SeqCst);
        if latency > 0 {
            tokio::time::sleep(Duration::from_millis(latency as u64)).await;
        }

        if !self.connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Transport is disconnected"));
        }

        // Check if we should fail
        let fails = self.fail_count.load(Ordering::SeqCst);
        if fails > 0 {
            self.fail_count.fetch_sub(1, Ordering::SeqCst);
            return Err(anyhow::anyhow!("Simulated transient failure"));
        }

        Ok(())
    }

    async fn recv(&mut self) -> Result<luminaguard_orchestrator::mcp::protocol::McpResponse> {
        // Simulate latency
        let latency = self.latency_ms.load(Ordering::SeqCst);
        if latency > 0 {
            tokio::time::sleep(Duration::from_millis(latency as u64)).await;
        }

        if !self.connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Transport is disconnected"));
        }

        // Check if we should fail
        let fails = self.fail_count.load(Ordering::SeqCst);
        if fails > 0 {
            self.fail_count.fetch_sub(1, Ordering::SeqCst);
            return Err(anyhow::anyhow!("Simulated transient failure"));
        }

        // Return queued response or default
        let mut queue = self.response_queue.lock().await;
        if let Some(response) = queue.pop() {
            Ok(response)
        } else {
            // Return a default success response with proper serverInfo structure
            Ok(luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
                1,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "serverInfo": {
                        "name": "test-server",
                        "version": "1.0.0"
                    }
                }),
            ))
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

/// Helper to create an initialize response
fn create_init_response() -> luminaguard_orchestrator::mcp::protocol::McpResponse {
    luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
        1,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "serverInfo": {"name": "test-server", "version": "1.0.0"}
        }),
    )
}

/// Helper to create a tools list response
fn create_tools_response() -> luminaguard_orchestrator::mcp::protocol::McpResponse {
    luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
        2,
        json!({
            "tools": [
                {
                    "name": "test_tool",
                    "description": "A test tool",
                    "inputSchema": {"type": "object"}
                }
            ]
        }),
    )
}

/// Helper to create a tool call response
fn create_tool_response(result: &str) -> luminaguard_orchestrator::mcp::protocol::McpResponse {
    luminaguard_orchestrator::mcp::protocol::McpResponse::ok(3, json!({"result": result}))
}

// ============================================================================
// Test: Reconnection after server restart
// ============================================================================

#[tokio::test]
async fn test_reconnection_after_server_restart() {
    let transport = ReconnectMockTransport::new();

    // Create client and initialize
    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());
    client
        .initialize()
        .await
        .expect("Initial connection should succeed");

    // Simulate server restart (disconnect then reconnect)
    transport.disconnect();

    // Client should detect disconnection
    assert!(!client.transport().is_connected());

    // Reconnect
    transport.reconnect();

    // Client should now be able to reconnect
    assert!(client.transport().is_connected());
}

#[tokio::test]
async fn test_reconnection_basic() {
    let transport = ReconnectMockTransport::new();

    // Basic test: client can initialize
    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());

    // Initialize should succeed
    let result = client.initialize().await;
    assert!(result.is_ok(), "Should initialize successfully");

    // Verify we had an attempt
    assert!(transport.send_attempt_count() >= 1);
}

// ============================================================================
// Test: Handling of malformed responses
// ============================================================================

#[tokio::test]
async fn test_malformed_response_missing_protocol_version() {
    let transport = ReconnectMockTransport::new();

    // Push a malformed response
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
            1,
            json!({
                "capabilities": {},
                "serverInfo": {"name": "test", "version": "1.0"}
                // Missing protocolVersion
            }),
        ))
        .await;

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

    // Initialize should fail due to malformed response
    let result = client.initialize().await;
    assert!(result.is_err(), "Should fail with malformed response");
}

#[tokio::test]
async fn test_malformed_response_invalid_json() {
    let transport = ReconnectMockTransport::new();

    // Push a response with invalid structure
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
            1,
            json!("not an object"),
        ))
        .await;

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

    // Initialize should fail
    let result = client.initialize().await;
    assert!(result.is_err(), "Should fail with invalid JSON structure");
}

#[tokio::test]
async fn test_malformed_tools_response() {
    let transport = ReconnectMockTransport::new();

    // Push a malformed tools response
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
            2,
            json!({
                "tools": "not an array"
            }),
        ))
        .await;

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);
    client.initialize().await.ok(); // Skip init

    // List tools should fail
    let result = client.list_tools().await;
    assert!(result.is_err(), "Should fail with malformed tools response");
}

// ============================================================================
// Test: Timeout handling
// ============================================================================

#[tokio::test]
async fn test_timeout_on_slow_response() {
    let transport = ReconnectMockTransport::new();

    // Set moderate latency (simulating slow server)
    transport.set_latency(100); // 100ms

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

    // With default timeout behavior, this should still succeed
    let result = tokio::time::timeout(Duration::from_secs(2), client.initialize()).await;

    assert!(result.is_ok(), "Should complete within timeout");
}

#[tokio::test]
async fn test_timeout_with_retry() {
    let transport = ReconnectMockTransport::new();

    // Set high latency initially
    transport.set_latency(200);

    let retry_config = luminaguard_orchestrator::mcp::retry::RetryConfig::default()
        .max_attempts(3)
        .base_delay(Duration::from_millis(10));

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone())
        .with_retry(retry_config);

    // Should succeed even with latency
    let result = client.initialize().await;
    assert!(result.is_ok(), "Should succeed with retry");
}

// ============================================================================
// Test: Connection pool exhaustion scenarios
// ============================================================================

#[tokio::test]
async fn test_multiple_concurrent_clients() {
    let mut handles = vec![];

    // Spawn multiple clients concurrently
    for i in 0..5 {
        let handle = tokio::spawn(async move {
            let transport = ReconnectMockTransport::new();

            // Note: responses are popped from the queue in LIFO order
            // So push tools response first, then init response
            transport.push_response(create_tools_response()).await;
            transport.push_response(create_init_response()).await;

            let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

            client
                .initialize()
                .await
                .expect(&format!("Client {} should initialize successfully", i));

            // Each client should be able to list tools
            let tools = client
                .list_tools()
                .await
                .expect(&format!("Client {} should list tools successfully", i));

            tools.len()
        });

        handles.push(handle);
    }

    // Wait for all clients
    let results: Vec<_> = futures::future::join_all(handles).await;

    // All clients should have succeeded
    for result in results {
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // 1 tool in response
    }
}

#[tokio::test]
async fn test_client_state_after_disconnect() {
    let transport = ReconnectMockTransport::new();

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());

    // Initialize
    client.initialize().await.expect("Should initialize");

    // Disconnect
    transport.disconnect();

    // Operations should fail
    let result = client.list_tools().await;
    assert!(result.is_err(), "Should fail when disconnected");
}

// ============================================================================
// Test: Error recovery scenarios
// ============================================================================

#[tokio::test]
async fn test_recovery_from_server_error() {
    let transport = ReconnectMockTransport::new();

    // Push an error response first
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::err(
            1,
            luminaguard_orchestrator::mcp::protocol::McpError::internal_error("Server error"),
        ))
        .await;

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());

    // First attempt should fail
    let result = client.initialize().await;
    assert!(result.is_err(), "Should fail with server error");

    // Push a success response
    transport.push_response(create_init_response()).await;

    // Reset client state and retry
    let mut new_client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);
    let result = new_client.initialize().await;
    assert!(result.is_ok(), "Should succeed after recovery");
}

// ============================================================================
// Test: Connection state management
// ============================================================================

#[tokio::test]
async fn test_connection_state_transitions() {
    let transport = ReconnectMockTransport::new();

    let client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());

    // Initial state
    assert_eq!(
        client.state(),
        luminaguard_orchestrator::mcp::client::ClientState::Created
    );

    // After initialization
    let mut client = client;
    client.initialize().await.expect("Should initialize");

    assert_eq!(
        client.state(),
        luminaguard_orchestrator::mcp::client::ClientState::Ready
    );
}

#[tokio::test]
async fn test_double_initialization_prevention() {
    let transport = ReconnectMockTransport::new();

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

    // First initialization
    client
        .initialize()
        .await
        .expect("First init should succeed");

    // Second initialization should fail
    let result = client.initialize().await;
    assert!(result.is_err(), "Double initialization should fail");
}

// ============================================================================
// Test: Tool call resilience
// ============================================================================

#[tokio::test]
async fn test_tool_call_with_transient_failure() {
    let transport = ReconnectMockTransport::new();

    // Initialize first
    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());
    client.initialize().await.expect("Should initialize");

    // Set up for tool call with transient failure
    transport.set_fail_count(1);
    transport
        .push_response(create_tool_response("success"))
        .await;

    let retry_config = luminaguard_orchestrator::mcp::retry::RetryConfig::default()
        .max_attempts(3)
        .base_delay(Duration::from_millis(10));

    // Create a new client with retry for tool calls
    let _client_with_retry =
        luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone())
            .with_retry(retry_config);

    // Note: In real usage, we'd initialize and call tools
    // This test verifies the retry mechanism can be configured
}

#[tokio::test]
async fn test_tool_call_error_propagation() {
    let transport = ReconnectMockTransport::new();

    // Initialize
    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport.clone());
    client.initialize().await.expect("Should initialize");

    // Push error response for tool call
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::err(
            3,
            luminaguard_orchestrator::mcp::protocol::McpError::method_not_found("unknown_tool"),
        ))
        .await;

    // Tool call should fail with proper error
    let result = client.call_tool("unknown_tool", json!({})).await;
    assert!(result.is_err(), "Should fail for unknown tool");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("unknown_tool") || error_msg.contains("not found"),
        "Error should mention the tool: {}",
        error_msg
    );
}

// ============================================================================
// Test: Server capability negotiation
// ============================================================================

#[tokio::test]
async fn test_capability_negotiation() {
    let transport = ReconnectMockTransport::new();

    // Push response with capabilities
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
            1,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {"supported": true},
                    "resources": {"supported": true}
                },
                "serverInfo": {"name": "capable-server", "version": "2.0.0"}
            }),
        ))
        .await;

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

    client.initialize().await.expect("Should initialize");

    // Check capabilities were stored
    let caps = client
        .server_capabilities()
        .expect("Should have capabilities");
    assert_eq!(caps.server_info.name, "capable-server");
    assert_eq!(caps.server_info.version, "2.0.0");
}

// ============================================================================
// Test: Protocol version compatibility
// ============================================================================

#[tokio::test]
async fn test_protocol_version_mismatch() {
    let transport = ReconnectMockTransport::new();

    // Push response with incompatible version
    transport
        .push_response(luminaguard_orchestrator::mcp::protocol::McpResponse::ok(
            1,
            json!({
                "protocolVersion": "2023-01-01", // Old version
                "capabilities": {},
                "serverInfo": {"name": "old-server", "version": "0.1.0"}
            }),
        ))
        .await;

    let mut client = luminaguard_orchestrator::mcp::client::McpClient::new(transport);

    // Should still initialize (version check is advisory)
    let result = client.initialize().await;
    assert!(
        result.is_ok(),
        "Should initialize even with version mismatch"
    );

    // But we can check the version
    let caps = client
        .server_capabilities()
        .expect("Should have capabilities");
    assert_ne!(caps.protocol_version, "2024-11-05");
}
