//! Agent RPC Module
//!
//! This module provides JSON-RPC 2.0 communication between the Python agent loop
//! and the Rust orchestrator over stdin/stdout.
//!
//! # Architecture
//!
//! ```text
//! Python Agent Loop (loop.py)
//!      ↓ (JSON-RPC over stdin/stdout)
//! Rust Orchestrator (this module)
//!      ↓ (MCP over stdio/HTTP)
//! MCP Server (filesystem, github, etc.)
//! ```
//!
//! # Protocol
//!
//! The orchestrator acts as an MCP proxy, exposing standard MCP methods:
//! - `initialize`: Initialize the orchestrator
//! - `tools/list`: List available tools from connected MCP servers
//! - `tools/call`: Execute a tool call
//!
//! # Usage
//!
//! Start orchestrator in agent mode:
//! ```bash
//! luminaguard agent-mode --server filesystem --command "npx -y @modelcontextprotocol/server-filesystem /tmp"
//! ```

use crate::mcp::{McpClient, ServerCapabilities, ServerInfo, StdioTransport};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, BufReader, Write};
use tracing::{debug, error, info, warn};

/// JSON-RPC 2.0 Request
#[derive(Debug, Deserialize)]
struct JsonRequest {
    /// JSON-RPC version (must be "2.0")
    jsonrpc: String,
    /// Method name (e.g., "initialize", "tools/list", "tools/call")
    method: String,
    /// Method parameters (optional)
    #[serde(default)]
    params: Option<serde_json::Value>,
    /// Request ID
    id: serde_json::Value,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Serialize)]
struct JsonResponse {
    /// JSON-RPC version
    jsonrpc: &'static str,
    /// Result (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    /// Error (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    /// Request ID (must match request)
    id: serde_json::Value,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Serialize)]
struct JsonRpcError {
    /// Error code
    code: i32,
    /// Error message
    message: String,
    /// Additional data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create an invalid request error
    fn invalid_request(msg: String) -> Self {
        Self {
            code: -32600,
            message: format!("Invalid request: {}", msg),
            data: None,
        }
    }

    /// Create a method not found error
    #[allow(dead_code)]
    fn method_not_found(method: String) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    /// Create an internal error
    fn internal_error(msg: String) -> Self {
        Self {
            code: -32603,
            message: format!("Internal error: {}", msg),
            data: None,
        }
    }
}

/// Agent RPC server configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// MCP server name (for logging)
    pub server_name: String,

    /// Command to spawn MCP server
    pub command: Vec<String>,
}

impl AgentConfig {
    /// Create new agent configuration
    pub fn new(server_name: String, command: Vec<String>) -> Self {
        Self {
            server_name,
            command,
        }
    }
}

/// Agent RPC server state
struct AgentServer {
    /// MCP client (initialized when ready)
    mcp_client: Option<McpClient<StdioTransport>>,
    /// Server capabilities
    capabilities: Option<ServerCapabilities>,
    /// Server info
    server_info: Option<ServerInfo>,
}

impl AgentServer {
    /// Create new agent server
    fn new() -> Self {
        Self {
            mcp_client: None,
            capabilities: None,
            server_info: None,
        }
    }

    /// Initialize MCP connection
    async fn initialize(&mut self, config: &AgentConfig) -> Result<()> {
        info!(
            "🔌 Initializing MCP connection to {}...",
            config.server_name
        );

        // Split command into program and args
        if config.command.is_empty() {
            return Err(anyhow::anyhow!("Command cannot be empty"));
        }

        let program = &config.command[0];
        let args: Vec<&str> = config.command[1..].iter().map(|s| s.as_str()).collect();

        debug!("Spawning MCP server: {} {:?}", program, args);

        // Spawn MCP server via stdio transport
        let transport = StdioTransport::spawn(program, &args)
            .await
            .context("Failed to spawn MCP server")?;

        // Create and initialize MCP client
        let mut client = McpClient::new(transport);

        client
            .initialize()
            .await
            .context("Failed to initialize MCP client")?;

        // Get server info
        let server_info = ServerInfo {
            name: config.server_name.clone(),
            version: "1.0.0".to_string(),
        };

        // Create capabilities with tools support
        let capabilities = ServerCapabilities {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({
                "tools": {},
            }),
            server_info: server_info.clone(),
        };

        self.mcp_client = Some(client);
        self.server_info = Some(server_info);
        self.capabilities = Some(capabilities);

        info!("✅ MCP connection initialized");

        Ok(())
    }

    /// Handle "initialize" method
    async fn handle_initialize(
        &mut self,
        config: &AgentConfig,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        info!("📋 Handling initialize request");

        // Initialize MCP connection
        self.initialize(config).await?;

        // Return initialized response
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": self.capabilities,
            "serverInfo": self.server_info,
        }))
    }

    /// Handle "tools/list" method
    async fn handle_tools_list(
        &mut self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        info!("🔍 Handling tools/list request");

        let client = self
            .mcp_client
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP client not initialized"))?;

        let tools = client.list_tools().await.context("Failed to list tools")?;

        let tools_json: Vec<serde_json::Value> = tools
            .into_iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            })
            .collect();

        Ok(json!({ "tools": tools_json }))
    }

    /// Handle "tools/call" method
    async fn handle_tools_call(
        &mut self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let params = params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;

        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'name' parameter"))?;

        let arguments = params
            .get("arguments")
            .ok_or_else(|| anyhow::anyhow!("Missing 'arguments' parameter"))?;

        info!("🔧 Handling tools/call request: {}", tool_name);

        let client = self
            .mcp_client
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP client not initialized"))?;

        let result = client
            .call_tool(tool_name, arguments.clone())
            .await
            .context("Failed to call tool")?;

        Ok(json!({
            "content": result,
            "isError": false
        }))
    }
}

/// Run the agent RPC server (synchronous wrapper)
///
/// This is a convenience function that creates a tokio runtime and blocks on
/// the async `run_agent_rpc_server` function.
///
/// # Errors
///
/// Returns an error if runtime creation or server execution fails
pub fn run_agent_rpc_server_sync(config: AgentConfig) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

    rt.block_on(run_agent_rpc_server(config))
}

/// Run the agent RPC server
///
/// This function:
/// 1. Reads JSON-RPC requests from stdin
/// 2. Routes to appropriate handler
/// 3. Writes JSON-RPC responses to stdout
///
/// # Errors
///
/// Returns an error if:
/// - stdin/stdout communication fails
/// - JSON parsing fails
/// - MCP operations fail
pub async fn run_agent_rpc_server(config: AgentConfig) -> Result<()> {
    info!("🤖 LuminaGuard Agent RPC Server starting...");
    info!("📦 MCP server: {}", config.server_name);
    info!("🔧 Command: {:?}", config.command);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lock = BufReader::new(stdin.lock());
    let mut stdout_lock = stdout.lock();

    let mut server = AgentServer::new();

    info!("✅ Ready to receive requests");

    // Read line by line from stdin
    let mut line = String::new();
    loop {
        line.clear();

        // Read request from stdin
        let bytes_read = stdin_lock
            .read_line(&mut line)
            .context("Failed to read from stdin")?;

        if bytes_read == 0 {
            info!("📭 EOF received, shutting down");
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue; // Skip empty lines
        }

        debug!("📨 Received: {}", line);

        // Parse JSON-RPC request
        let request: JsonRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(e) => {
                error!("❌ Failed to parse JSON: {}", e);
                let error_response = JsonResponse {
                    jsonrpc: "2.0",
                    result: None,
                    error: Some(JsonRpcError::invalid_request(e.to_string())),
                    id: json!(null),
                };
                write_response(&mut stdout_lock, &error_response);
                continue;
            }
        };

        // Verify JSON-RPC version
        if request.jsonrpc != "2.0" {
            warn!("⚠️  Unsupported JSON-RPC version: {}", request.jsonrpc);
            let error_response = JsonResponse {
                jsonrpc: "2.0",
                result: None,
                error: Some(JsonRpcError::invalid_request(
                    "Unsupported JSON-RPC version".to_string(),
                )),
                id: request.id,
            };
            write_response(&mut stdout_lock, &error_response);
            continue;
        }

        // Route to handler (async)
        let result = match request.method.as_str() {
            "initialize" => server.handle_initialize(&config, request.params).await,
            "tools/list" => server.handle_tools_list(request.params).await,
            "tools/call" => server.handle_tools_call(request.params).await,
            _ => {
                error!("❌ Unknown method: {}", request.method);
                Err(anyhow::anyhow!("Unknown method: {}", request.method))
            }
        };

        // Build response
        let response = match result {
            Ok(result_value) => JsonResponse {
                jsonrpc: "2.0",
                result: Some(result_value),
                error: None,
                id: request.id,
            },
            Err(e) => {
                error!("❌ Error handling request: {}", e);
                JsonResponse {
                    jsonrpc: "2.0",
                    result: None,
                    error: Some(JsonRpcError::internal_error(e.to_string())),
                    id: request.id,
                }
            }
        };

        // Write response to stdout
        write_response(&mut stdout_lock, &response);
    }

    info!("👋 Agent RPC server shutting down");
    Ok(())
}

/// Write JSON-RPC response to stdout
fn write_response(stdout: &mut io::StdoutLock<'_>, response: &JsonResponse) {
    let json = serde_json::to_string(response).unwrap_or_else(|e| {
        error!("❌ Failed to serialize response: {}", e);
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Failed to serialize response"},"id":null}"#.to_string()
    });

    if let Err(e) = writeln!(stdout, "{}", json) {
        error!("❌ Failed to write to stdout: {}", e);
    }

    if let Err(e) = stdout.flush() {
        error!("❌ Failed to flush stdout: {}", e);
    }

    debug!("📤 Sent: {}", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_creation() {
        let config = AgentConfig::new(
            "filesystem".to_string(),
            vec!["npx".to_string(), "-y".to_string(), "@server".to_string()],
        );

        assert_eq!(config.server_name, "filesystem");
        assert_eq!(config.command.len(), 3);
    }

    #[test]
    fn test_json_rpc_error_creation() {
        let err = JsonRpcError::method_not_found("test_method".to_string());
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("test_method"));
    }

    #[test]
    fn test_agent_server_creation() {
        let server = AgentServer::new();
        assert!(server.mcp_client.is_none());
        assert!(server.capabilities.is_none());
    }
}
