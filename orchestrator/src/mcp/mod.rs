//! MCP (Model Context Protocol) Client Implementation
//!
//! This module provides a pure Rust implementation of the MCP client,
//! built from scratch using Tokio and Hyper (no external SDK).
//!
//! # Architecture
//!
//! The implementation is organized into three layers:
//!
//! 1. **Protocol Layer** (`protocol`): JSON-RPC 2.0 message types
//! 2. **Transport Layer** (`transport`): stdio and HTTP transports
//! 3. **Client Layer** (`client`): High-level MCP client API
//!
//! # Design Principles
//!
//! - **Minimal Dependencies**: Only Tokio, Hyper, and Serde
//! - **Auditability**: ~900 LOC total, fully readable
//! - **Performance**: <100ms startup, <50ms round-trip (local)
//! - **Type Safety**: Leverages Rust's type system for correctness

// Protocol layer: JSON-RPC 2.0 message types
pub mod protocol;

// Transport layer: stdio and HTTP transports
pub mod transport;

// HTTP transport for remote MCP servers
pub mod http_transport;

// Client layer: High-level MCP client API
pub mod client;

// Retry logic and error resilience
pub mod retry;

// Re-export commonly used types for convenience
pub use protocol::{
    ClientCapabilities, ClientInfo, InitializeParams, McpError, McpMethod, McpRequest, McpResponse,
    ServerCapabilities, ServerInfo, Tool, ToolCallParams,
};

// Re-export transport types
pub use http_transport::HttpTransport;
pub use transport::StdioTransport;

// Re-export client types
pub use client::{ClientState, McpClient};

// Note: Old placeholder client removed - now using client::McpClient

// Integration tests module
// These tests are ignored by default - run with: cargo test --lib -- --ignored
#[cfg(test)]
mod integration;

// Property-based tests module
#[cfg(test)]
mod proptests;

#[cfg(test)]

#[cfg(test)]
mod tests {
    use crate::mcp::{McpError, McpRequest};

    #[test]
    fn test_protocol_module_available() {
        // Test that we can create basic MCP requests
        let req = McpRequest::new(1, "initialize", None);
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "initialize");
    }

    #[test]
    fn test_error_creation() {
        let err = McpError::method_not_found("test_method");
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("test_method"));
    }
}
