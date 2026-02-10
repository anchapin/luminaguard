//! MCP Integration Tests
//!
//! This module contains integration tests that run against real MCP servers.
//! These tests verify that the client works correctly with actual MCP implementations.
//!
//! # Requirements
//!
//! These tests require:
//! - Node.js and npm (for @modelcontextprotocol/server-filesystem)
//! - Network access (for some servers)
//! - Temporary directory access (for filesystem tests)
//!
//! # Running the Tests
//!
//! By default, integration tests are ignored to avoid slowing down regular test runs.
//! Run them with:
//!
//! ```bash
//! cargo test --lib -- --ignored
//! ```
//!
//! # Test Coverage
//!
//! - Full client lifecycle (spawn → initialize → list_tools → call_tool → cleanup)
//! - Real MCP server implementations
//! - Protocol compliance validation
//! - Error handling with real servers

use crate::mcp::{McpClient, StdioTransport};
use tokio::time::{timeout, Duration};

/// Helper to create a timeout for integration tests
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Test against the filesystem MCP server
///
/// This test verifies that our client can work with the official
/// @modelcontextprotocol/server-filesystem server.
#[tokio::test]
#[ignore = "integration test - requires npm and Node.js"]
async fn test_integration_filesystem_server() {
    // Create a temporary directory for testing
    let temp_dir = std::env::temp_dir();
    let test_dir = temp_dir.join("mcp_test_integration");
    std::fs::create_dir_all(&test_dir).expect("Failed to create test directory");

    // Spawn the filesystem server
    let transport = timeout(
        TEST_TIMEOUT,
        StdioTransport::spawn(
            "npx",
            &[
                "-y",
                "@modelcontextprotocol/server-filesystem",
                test_dir.to_str().unwrap(),
            ],
        ),
    )
    .await
    .expect("Spawn timeout")
    .expect("Failed to spawn filesystem server");

    // Create client
    let mut client = McpClient::new(transport);

    // Initialize the connection
    timeout(TEST_TIMEOUT, client.initialize())
        .await
        .expect("Initialize timeout")
        .expect("Failed to initialize");

    // Check server capabilities
    let caps = client.server_capabilities().expect("No capabilities");
    assert_eq!(caps.protocol_version, "2024-11-05");

    // List available tools
    let tools = timeout(TEST_TIMEOUT, client.list_tools())
        .await
        .expect("List tools timeout")
        .expect("Failed to list tools");

    // Filesystem server should provide these tools
    assert!(!tools.is_empty(), "No tools available");
    let tool_names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"read_file"), "Missing read_file tool");
    assert!(
        tool_names.contains(&"write_file"),
        "Missing write_file tool"
    );
    assert!(
        tool_names.contains(&"list_directory"),
        "Missing list_directory tool"
    );

    // Test calling a tool - list_directory
    let result = timeout(
        TEST_TIMEOUT,
        client.call_tool(
            "list_directory",
            serde_json::json!({
                "path": test_dir.to_str().unwrap()
            }),
        ),
    )
    .await
    .expect("Tool call timeout")
    .expect("Failed to call list_directory");

    // Verify the result
    assert!(
        result.is_object() || result.is_string(),
        "Invalid result format"
    );

    // Cleanup happens automatically via Drop trait
}

/// Test against a simple echo server
///
/// This test uses a custom echo server that simply echoes back JSON-RPC messages.
/// This tests the basic transport and protocol handling without external dependencies.
#[cfg(unix)]
#[tokio::test]
#[ignore = "integration test - uses bash script"]
async fn test_integration_echo_server() {
    // Create a simple echo server script
    let echo_script = r#"#!/bin/bash
# MCP Echo Server - echoes back JSON-RPC messages
# This simulates a minimal MCP server for testing

while IFS= read -r line; do
    # Extract the request ID and method
    id=$(echo "$line" | jq -r '.id // 1')
    method=$(echo "$line" | jq -r '.method // "unknown"')

    case "$method" in
        "initialize")
            # Return initialize response
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"echo-server\",\"version\":\"1.0.0\"}}}"
            ;;
        "tools/list")
            # Return list of available tools (simplified schema for testing)
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"echo\",\"description\":\"Echo back the input\",\"inputSchema\":{\"type\":\"object\"}}]}}"
            ;;
        "tools/call")
            # Echo back the arguments
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"echoed\":$(echo "$line" | jq -c '.params.arguments // {}')}}"
            ;;
        *)
            # Unknown method - return error
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"Method not found: $method\"}}"
            ;;
    esac
done
"#;

    let echo_path = "/tmp/mcp_echo_integration.sh";
    std::fs::write(echo_path, echo_script).expect("Failed to write echo script");

    #[cfg(unix)]
    {
        use tokio::process::Command;

        // Make the script executable
        Command::new("chmod")
            .args(["+x", echo_path])
            .output()
            .await
            .expect("Failed to make echo script executable");

        // Spawn the echo server
        let transport = timeout(TEST_TIMEOUT, StdioTransport::spawn(echo_path, &[]))
            .await
            .expect("Spawn timeout")
            .expect("Failed to spawn echo server");

        // Create client
        let mut client = McpClient::new(transport);

        // Test full lifecycle
        timeout(TEST_TIMEOUT, client.initialize())
            .await
            .expect("Initialize timeout")
            .expect("Failed to initialize");

        let tools = timeout(TEST_TIMEOUT, client.list_tools())
            .await
            .expect("List tools timeout")
            .expect("Failed to list tools");

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        // Test tool call
        let result = timeout(
            TEST_TIMEOUT,
            client.call_tool("echo", serde_json::json!({"message": "hello"})),
        )
        .await
        .expect("Tool call timeout")
        .expect("Failed to call echo tool");

        assert!(result.is_object());

        // Cleanup
        let _ = std::fs::remove_file(echo_path);
    }

    #[cfg(not(unix))]
    {
        println!("Skipping echo server test on non-Unix platform");
        let _ = std::fs::remove_file(echo_path);
    }
}

/// Test client error handling with a malformed server
///
/// This test verifies that the client handles errors gracefully when
/// the server returns invalid responses.
#[cfg(unix)]
#[tokio::test]
#[ignore = "integration test - uses bash script"]
async fn test_integration_malformed_server() {
    // Create a server that returns malformed JSON
    let malformed_script = r#"#!/bin/bash
# MCP Malformed Server - returns invalid JSON for testing error handling

while IFS= read -r line; do
    # Return invalid JSON (missing closing brace)
    echo '{"jsonrpc":"2.0","id":1,"result":{"test":'
done
"#;

    let malformed_path = "/tmp/mcp_malformed.sh";
    std::fs::write(malformed_path, malformed_script).expect("Failed to write malformed script");

    #[cfg(unix)]
    {
        use tokio::process::Command;

        Command::new("chmod")
            .args(["+x", malformed_path])
            .output()
            .await
            .expect("Failed to make malformed script executable");

        let transport = timeout(TEST_TIMEOUT, StdioTransport::spawn(malformed_path, &[]))
            .await
            .expect("Spawn timeout")
            .expect("Failed to spawn malformed server");

        let mut client = McpClient::new(transport);

        // Initialize should fail due to malformed JSON
        let result = timeout(TEST_TIMEOUT, client.initialize()).await;
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Expected initialization to fail"
        );

        // Cleanup
        let _ = std::fs::remove_file(malformed_path);
    }

    #[cfg(not(unix))]
    {
        println!("Skipping malformed server test on non-Unix platform");
        let _ = std::fs::remove_file(malformed_path);
    }
}

/// Test server disconnection handling
///
/// This test verifies that the client handles server disconnection gracefully.
#[cfg(unix)]
#[tokio::test]
#[ignore = "integration test - uses bash script"]
async fn test_integration_server_disconnect() {
    // Create a server that exits immediately
    let disconnect_script = r#"#!/bin/bash
# MCP Disconnect Server - exits after first message

while IFS= read -r line; do
    echo "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"disconnect\",\"version\":\"1.0\"}}}"
    # Exit after first response
    exit 0
done
"#;

    let disconnect_path = "/tmp/mcp_disconnect.sh";
    std::fs::write(disconnect_path, disconnect_script).expect("Failed to write disconnect script");

    #[cfg(unix)]
    {
        use tokio::process::Command;

        Command::new("chmod")
            .args(["+x", disconnect_path])
            .output()
            .await
            .expect("Failed to make disconnect script executable");

        let transport = timeout(TEST_TIMEOUT, StdioTransport::spawn(disconnect_path, &[]))
            .await
            .expect("Spawn timeout")
            .expect("Failed to spawn disconnect server");

        let mut client = McpClient::new(transport);

        // Initialize should succeed
        timeout(TEST_TIMEOUT, client.initialize())
            .await
            .expect("Initialize timeout")
            .expect("Failed to initialize");

        // Server should now be disconnected
        // Next operation should fail
        let result = timeout(TEST_TIMEOUT, client.list_tools()).await;
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Expected operation to fail after disconnect"
        );

        // Cleanup
        let _ = std::fs::remove_file(disconnect_path);
    }

    #[cfg(not(unix))]
    {
        println!("Skipping disconnect test on non-Unix platform");
        let _ = std::fs::remove_file(disconnect_path);
    }
}

/// Test rapid sequential tool calls
///
/// This test verifies that the client can handle multiple rapid tool calls
/// without issues, testing the monotonic ID counter.
#[cfg(unix)]
#[tokio::test]
#[ignore = "integration test - uses bash script"]
async fn test_integration_rapid_calls() {
    let rapid_script = r#"#!/bin/bash
# MCP Rapid Call Server - handles multiple requests

request_count=0

while IFS= read -r line; do
    request_count=$((request_count + 1))
    id=$(echo "$line" | jq -r '.id // "'"$request_count"'"')
    method=$(echo "$line" | jq -r '.method // "unknown"')

    case "$method" in
        "initialize")
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"rapid\",\"version\":\"1.0\"}}}"
            ;;
        "tools/list")
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[]}}"
            ;;
        *)
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"request_count\":$request_count}}"
            ;;
    esac
done
"#;

    let rapid_path = "/tmp/mcp_rapid.sh";
    std::fs::write(rapid_path, rapid_script).expect("Failed to write rapid script");

    #[cfg(unix)]
    {
        use tokio::process::Command;

        Command::new("chmod")
            .args(["+x", rapid_path])
            .output()
            .await
            .expect("Failed to make rapid script executable");

        let transport = timeout(TEST_TIMEOUT, StdioTransport::spawn(rapid_path, &[]))
            .await
            .expect("Spawn timeout")
            .expect("Failed to spawn rapid server");

        let mut client = McpClient::new(transport);

        timeout(TEST_TIMEOUT, client.initialize())
            .await
            .expect("Initialize timeout")
            .expect("Failed to initialize");

        // Make multiple rapid calls
        for i in 1..=10 {
            let result = timeout(
                TEST_TIMEOUT,
                client.call_tool("test", serde_json::json!({"iteration": i})),
            )
            .await
            .expect("Tool call timeout")
            .expect("Failed to call tool");

            assert!(result.is_object());
        }

        // Cleanup
        let _ = std::fs::remove_file(rapid_path);
    }

    #[cfg(not(unix))]
    {
        println!("Skipping rapid calls test on non-Unix platform");
        let _ = std::fs::remove_file(rapid_path);
    }
}
