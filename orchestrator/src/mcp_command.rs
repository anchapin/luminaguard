//! MCP Command Module
//!
//! This module encapsulates MCP operations for the Orchestrator.
//! It provides a clean abstraction layer between the CLI (main.rs)
//! and the MCP client implementation.
//!
//! # Design
//!
//! - Separation of concerns: CLI logic in main.rs, business logic here
//! - Reusable operations: Can be called by multiple commands
//! - Testable in isolation: Easy to unit test without CLI machinery
//! - Structured results: Returns clear success/error information

use crate::mcp::{McpClient, StdioTransport};
use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{info, warn};

/// Result of an MCP test operation
#[derive(Debug)]
pub struct McpTestResult {
    /// Whether the test succeeded
    pub success: bool,

    /// Number of tools discovered
    pub tool_count: usize,

    /// List of tool names discovered
    pub tool_names: Vec<String>,

    /// Test tool call result (if attempted)
    pub tool_call_result: Option<Value>,

    /// Error message (if failed)
    pub error: Option<String>,
}

/// Execute an MCP connection test
///
/// This function tests the MCP client by:
/// 1. Spawning a filesystem MCP server using npx
/// 2. Initializing the client connection
/// 3. Listing available tools
/// 4. Optionally calling a test tool
/// 5. Cleaning up the connection
///
/// # Arguments
///
/// * `test_tool_call` - If true, attempts to call a test tool (e.g., write_file)
///
/// # Returns
///
/// Returns `McpTestResult` with detailed information about the test
///
/// # Example
///
/// ```ignore
/// use luminaguard_orchestrator::mcp_command::execute_mcp_test;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let result = execute_mcp_test(false).await?;
///     println!("Test result: {:?}", result);
///     Ok(())
/// }
/// ```
pub async fn execute_mcp_test(test_tool_call: bool) -> Result<McpTestResult> {
    info!("🔌 Starting MCP connection test...");

    // Step 1: Spawn filesystem MCP server
    info!("📦 Spawning filesystem MCP server via npx...");
    let transport = StdioTransport::spawn(
        "npx",
        &["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
    )
    .await
    .context("Failed to spawn MCP server")?;

    // Step 2: Create and initialize client
    info!("🔧 Initializing MCP client...");
    let mut client = McpClient::new(transport);

    client
        .initialize()
        .await
        .context("Failed to initialize MCP client")?;

    info!("✅ MCP client initialized successfully");

    // Step 3: List available tools
    info!("🔍 Listing available tools...");
    let tools = client.list_tools().await.context("Failed to list tools")?;

    let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();

    info!("✅ Discovered {} tools:", tools.len());
    for name in &tool_names {
        info!("  - {}", name);
    }

    // Step 4: Optionally test a tool call
    let tool_call_result = if test_tool_call {
        info!("🧪 Testing tool call (write_file)...");
        match test_write_file(&mut client).await {
            Ok(result) => {
                info!("✅ Tool call succeeded");
                Some(result)
            }
            Err(e) => {
                warn!("⚠️  Tool call failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Step 5: Client cleanup happens automatically on drop
    info!("🧹 MCP test complete, cleaning up...");

    Ok(McpTestResult {
        success: true,
        tool_count: tools.len(),
        tool_names,
        tool_call_result,
        error: None,
    })
}

/// Test a write_file tool call
///
/// This creates a small test file to verify tool execution works
async fn test_write_file(client: &mut McpClient<StdioTransport>) -> Result<Value> {
    use std::time::SystemTime;

    // Create unique test filename
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    let test_path = format!("/tmp/luminaguard_test_{}.txt", timestamp);

    info!("📝 Writing test file to: {}", test_path);

    let result = client
        .call_tool(
            "write_file",
            serde_json::json!({
                "path": test_path,
                "content": "LuminaGuard MCP test\n"
            }),
        )
        .await
        .context("Failed to call write_file tool")?;

    // Clean up test file
    info!("🗑️  Cleaning up test file...");
    let _ = client
        .call_tool("delete_file", serde_json::json!({"path": test_path}))
        .await;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require npx and the MCP server to be installed
    // Run with: cargo test --lib mcp_command -- --ignored

    #[tokio::test]
    #[ignore] // Integration test - requires npx
    async fn test_execute_mcp_test_basic() {
        let result = execute_mcp_test(false)
            .await
            .expect("MCP test should succeed");

        assert!(result.success);
        assert!(result.tool_count > 0);
        assert!(!result.tool_names.is_empty());
        assert!(result.tool_call_result.is_none());
    }

    #[tokio::test]
    #[ignore] // Integration test - requires npx
    async fn test_execute_mcp_test_with_tool_call() {
        let result = execute_mcp_test(true)
            .await
            .expect("MCP test should succeed");

        assert!(result.success);
        assert!(result.tool_count > 0);
        assert!(result.tool_call_result.is_some());
    }

    #[test]
    fn test_mcp_test_result_structure() {
        // Verify the result structure is correct
        let result = McpTestResult {
            success: true,
            tool_count: 5,
            tool_names: vec!["tool1".to_string(), "tool2".to_string()],
            tool_call_result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        };

        assert_eq!(result.tool_count, 5);
        assert_eq!(result.tool_names.len(), 2);
        assert!(result.tool_call_result.is_some());
    }
}
