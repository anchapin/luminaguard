//! MCP Protocol Types (JSON-RPC 2.0)
//!
//! This module defines the core protocol types for the Model Context Protocol (MCP).
//! MCP is built on top of JSON-RPC 2.0, which is a simple stateless RPC protocol.
//!
//! # Protocol Specification
//!
//! - JSON-RPC 2.0: <https://www.jsonrpc.org/specification>
//! - MCP Spec: <https://modelcontextprotocol.io/specification/2025-03-26>
//!
//! # Architecture
//!
//! The protocol layer is responsible only for serialization/deserialization of MCP messages.
//! Transport concerns (stdio, HTTP) are handled in the transport layer.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 version constant
pub const JSONRPC_VERSION: &str = "2.0";

/// A JSON-RPC 2.0 request message
///
/// Requests are sent from the client to the MCP server to invoke methods.
/// Each request has a unique ID (monotonically increasing) to match responses.
///
/// # Example
///
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": 1,
///   "method": "tools/list",
///   "params": {}
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpRequest {
    /// JSON-RPC version (always "2.0")
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,

    /// Request identifier (used to match responses)
    pub id: u64,

    /// Method name to invoke
    pub method: String,

    /// Method parameters (optional, depends on method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Create a new MCP request
    ///
    /// # Arguments
    ///
    /// * `id` - Unique request identifier
    /// * `method` - Method name to invoke
    /// * `params` - Optional method parameters
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }

    /// Create a request without parameters
    pub fn notification(id: u64, method: impl Into<String>) -> Self {
        Self::new(id, method, None)
    }
}

/// A JSON-RPC 2.0 response message
///
/// Responses are sent from the MCP server back to the client.
/// A response either contains a `result` or an `error`, but never both.
///
/// # Example (Success)
///
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": 1,
///   "result": {"tools": [...]}
/// }
/// ```
///
/// # Example (Error)
///
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": 1,
///   "error": {"code": -32601, "message": "Method not found"}
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpResponse {
    /// JSON-RPC version (always "2.0")
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,

    /// Request identifier (must match the request's ID)
    pub id: u64,

    /// Result payload (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error information (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl McpResponse {
    /// Create a successful response
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn err(id: u64, error: McpError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Check if the response is successful
    pub fn is_success(&self) -> bool {
        self.result.is_some() && self.error.is_none()
    }

    /// Get the result, or the error if unsuccessful
    pub fn into_result(self) -> Result<serde_json::Value, McpError> {
        match (self.result, self.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error),
            _ => Err(McpError::internal_error(
                "Invalid response: both result and error present",
            )),
        }
    }
}

/// A JSON-RPC 2.0 error object
///
/// Errors follow the JSON-RPC 2.0 specification with MCP-specific extensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpError {
    /// Error code (JSON-RPC defined or MCP-specific)
    pub code: i32,

    /// Human-readable error message
    pub message: String,

    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpError {
    /// Create a new error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    // JSON-RPC standard errors
    /// Parse error (-32700): Invalid JSON was received
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(-32700, message)
    }

    /// Invalid request (-32600): The JSON sent is not a valid Request object
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(-32600, message)
    }

    /// Method not found (-32601): The method does not exist / is not available
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::new(-32601, format!("Method not found: {}", method.into()))
    }

    /// Invalid params (-32602): Invalid method parameter(s)
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }

    /// Internal error (-32603): Internal JSON-RPC error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(-32603, message)
    }

    // MCP-specific errors (negative numbers beyond JSON-RPC range)
    /// Server error (-32000): MCP server error
    pub fn server_error(message: impl Into<String>) -> Self {
        Self::new(-32000, message)
    }

    /// Initialization error (-32001): Failed to initialize connection
    pub fn initialization_error(message: impl Into<String>) -> Self {
        Self::new(-32001, message)
    }
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Error {}] {}", self.code, self.message)
    }
}

impl std::error::Error for McpError {}

/// MCP method identifiers
///
/// MCP defines a set of standard methods that all servers must support.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum McpMethod {
    /// Initialize the connection (must be called first)
    Initialize,

    /// List available tools
    ToolsList,

    /// Call a specific tool
    ToolsCall,

    /// List available resources
    ResourcesList,

    /// Read a resource
    ResourcesRead,

    /// List available prompts
    PromptsList,

    /// Get a prompt
    PromptsGet,

    /// Custom method (for extensibility)
    Custom(String),
}

impl McpMethod {
    /// Convert to string for JSON-RPC method field
    pub fn as_str(&self) -> &str {
        match self {
            Self::Initialize => "initialize",
            Self::ToolsList => "tools/list",
            Self::ToolsCall => "tools/call",
            Self::ResourcesList => "resources/list",
            Self::ResourcesRead => "resources/read",
            Self::PromptsList => "prompts/list",
            Self::PromptsGet => "prompts/get",
            Self::Custom(s) => s.as_str(),
        }
    }
}

impl From<String> for McpMethod {
    fn from(s: String) -> Self {
        match s.as_str() {
            "initialize" => Self::Initialize,
            "tools/list" => Self::ToolsList,
            "tools/call" => Self::ToolsCall,
            "resources/list" => Self::ResourcesList,
            "resources/read" => Self::ResourcesRead,
            "prompts/list" => Self::PromptsList,
            "prompts/get" => Self::PromptsGet,
            _ => Self::Custom(s),
        }
    }
}

impl From<&str> for McpMethod {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

/// Initialization parameters
///
/// Sent during the initialize handshake to negotiate capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitializeParams {
    /// Client protocol version
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,

    /// Client capabilities
    pub capabilities: ClientCapabilities,

    /// Client information
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

/// Client capabilities advertised during initialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientCapabilities {
    /// Sampling capability (object or null)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<serde_json::Value>,

    /// Experimental features
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

/// Client identification information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientInfo {
    /// Client name
    pub name: String,

    /// Client version
    pub version: String,
}

/// Server capabilities (returned during initialization)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerCapabilities {
    /// Server protocol version
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,

    /// Server capabilities
    pub capabilities: serde_json::Value,

    /// Server information
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

/// Server identification information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerInfo {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tool {
    /// Tool name (unique identifier)
    pub name: String,

    /// Tool description
    pub description: String,

    /// Tool input schema (JSON Schema)
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Tool call parameters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCallParams {
    /// Name of the tool to call
    pub name: String,

    /// Tool arguments (must match input schema)
    pub arguments: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_request() {
        let req = McpRequest::new(1, "tools/list", None);
        let json = serde_json::to_string(&req).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"tools/list\""));
    }

    #[test]
    fn test_deserialize_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let req: McpRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "tools/list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_serialize_response_success() {
        let result = serde_json::json!({"tools": []});
        let resp = McpResponse::ok(1, result.clone());
        let json = serde_json::to_string(&resp).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_serialize_response_error() {
        let err = McpError::method_not_found("unknown_method");
        let resp = McpResponse::err(1, err);
        let json = serde_json::to_string(&resp).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"error\""));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_response_is_success() {
        let ok_resp = McpResponse::ok(1, serde_json::json!({}));
        let err_resp = McpResponse::err(1, McpError::internal_error("failed"));

        assert!(ok_resp.is_success());
        assert!(!err_resp.is_success());
    }

    #[test]
    fn test_response_into_result() {
        let result = serde_json::json!({"status": "ok"});
        let ok_resp = McpResponse::ok(1, result.clone());

        assert_eq!(ok_resp.into_result().unwrap(), result);

        let err = McpError::invalid_params("bad params");
        let err_resp = McpResponse::err(1, err.clone());

        assert_eq!(err_resp.into_result().unwrap_err(), err);
    }

    #[test]
    fn test_error_codes() {
        let parse_err = McpError::parse_error("invalid json");
        assert_eq!(parse_err.code, -32700);

        let invalid_req = McpError::invalid_request("bad request");
        assert_eq!(invalid_req.code, -32600);

        let method_nf = McpError::method_not_found("test");
        assert_eq!(method_nf.code, -32601);

        let invalid_params = McpError::invalid_params("bad params");
        assert_eq!(invalid_params.code, -32602);

        let internal = McpError::internal_error("server error");
        assert_eq!(internal.code, -32603);
    }

    #[test]
    fn test_mcp_method_conversion() {
        assert_eq!(McpMethod::Initialize.as_str(), "initialize");
        assert_eq!(McpMethod::ToolsList.as_str(), "tools/list");
        assert_eq!(McpMethod::ToolsCall.as_str(), "tools/call");

        // String to McpMethod
        let method: McpMethod = "tools/list".into();
        assert_eq!(method, McpMethod::ToolsList);

        // Custom method
        let custom: McpMethod = "custom/method".into();
        assert!(matches!(custom, McpMethod::Custom(_)));

        // Test all method variants for coverage
        assert_eq!(McpMethod::ResourcesList.as_str(), "resources/list");
        assert_eq!(McpMethod::ResourcesRead.as_str(), "resources/read");
        assert_eq!(McpMethod::PromptsList.as_str(), "prompts/list");
        assert_eq!(McpMethod::PromptsGet.as_str(), "prompts/get");

        // Test Custom variant with as_str()
        let custom_method = McpMethod::Custom("my/custom".to_string());
        assert_eq!(custom_method.as_str(), "my/custom");
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"name\":\"test_tool\""));
        assert!(json.contains("\"description\":\"A test tool\""));
    }

    #[test]
    fn test_round_trip_request() {
        let original = McpRequest::new(
            42,
            "tools/call",
            Some(serde_json::json!({"name": "test", "args": {}})),
        );

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: McpRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_request_with_params() {
        let params = serde_json::json!({"query": "test"});
        let req = McpRequest::new(1, "resources/read", Some(params));

        assert!(req.params.is_some());
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"params\""));
    }

    #[test]
    fn test_request_notification() {
        let req = McpRequest::notification(1, "tools/list");
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "tools/list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_error_with_data() {
        let data = serde_json::json!({"details": "Additional error info"});
        let err = McpError::with_data(-32000, "Server error", data.clone());

        assert_eq!(err.code, -32000);
        assert_eq!(err.message, "Server error");
        assert_eq!(err.data, Some(data));
    }

    #[test]
    fn test_error_server_error() {
        let err = McpError::server_error("Connection failed");
        assert_eq!(err.code, -32000);
        assert!(err.message.contains("Connection failed"));
    }

    #[test]
    fn test_response_into_result_invalid() {
        // Edge case: response with both result and error (invalid)
        let invalid_resp = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"status": "ok"})),
            error: Some(McpError::internal_error("Error")),
        };

        let result = invalid_resp.into_result();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32603);
        assert!(err.message.contains("Invalid response"));
    }

    #[test]
    fn test_error_new() {
        let err = McpError::new(-32001, "Custom error");
        assert_eq!(err.code, -32001);
        assert_eq!(err.message, "Custom error");
        assert!(err.data.is_none());
    }

    #[test]
    fn test_error_invalid_params() {
        let err = McpError::invalid_params("Missing required field");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing required field"));
    }

    #[test]
    fn test_error_parse_error() {
        let err = McpError::parse_error("Unexpected token");
        assert_eq!(err.code, -32700);
        assert!(err.message.contains("Unexpected token"));
    }

    #[test]
    fn test_error_invalid_request() {
        let err = McpError::invalid_request("Missing jsonrpc field");
        assert_eq!(err.code, -32600);
        assert!(err.message.contains("Missing jsonrpc field"));
    }
}
