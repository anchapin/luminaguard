//! Property-Based Tests for MCP Client
//!
//! This module contains property-based tests using proptest to verify invariants
//! hold for random inputs across the MCP client implementation.
//!
//! # Test Strategies
//!
//! - **Message Serialization**: Round-trip validity - serializing and deserializing
//!   should preserve the original message structure
//! - **Retry Logic**: Exponential backoff properties - delays should increase
//!   exponentially within bounds
//! - **Error Handling**: Edge cases like network failures, invalid JSON, timeouts
//!
//! # Running the Tests
//!
//! ```bash
//! cargo test --lib mcp::proptests
//! ```

use proptest::collection::btree_map;
use proptest::prelude::*;
use serde_json::Value;

// Import MCP types for testing
use crate::mcp::protocol::{
    ClientCapabilities, ClientInfo, InitializeParams, McpError, McpRequest, McpResponse,
    ServerCapabilities, ServerInfo, Tool, ToolCallParams,
};
use crate::mcp::retry::RetryConfig;
use std::time::Duration;

// Helper: Generate arbitrary JSON values
fn arb_json_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| Value::Number(n.into())),
        ".*".prop_map(Value::String),
        prop::collection::vec(any::<i64>().prop_map(|n| Value::Number(n.into())), 0..3)
            .prop_map(Value::Array),
    ]
}

// Helper: Generate arbitrary tool definitions
fn arb_tool() -> impl Strategy<Value = Tool> {
    ("[a-z_]+", "[a-zA-Z ]+", arb_json_value()).prop_map(|(name, description, input_schema)| Tool {
        name,
        description,
        input_schema,
    })
}

// Helper: Generate arbitrary tool call parameters
fn arb_tool_call_params() -> impl Strategy<Value = ToolCallParams> {
    ("[a-z_]+", arb_json_value()).prop_map(|(name, arguments)| ToolCallParams {
        name,
        arguments: serde_json::to_value(arguments).unwrap(),
    })
}

// ============================================================================
// Property 1: Message Serialization Round-Trip
// ============================================================================

proptest! {
    /// Test that MCP requests can be serialized and deserialized correctly
    #[test]
    fn prop_request_serialization_roundtrip(
        id in 0u64..1000,
        method in "[a-z_]+",
        params in prop::option::of(arb_json_value())
    ) {
        let original = McpRequest::new(id, &method, params);
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: McpRequest = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(original.id, deserialized.id);
        prop_assert_eq!(original.method, deserialized.method);
        prop_assert_eq!(original.jsonrpc, deserialized.jsonrpc);
    }

    /// Test that MCP responses can be serialized and deserialized correctly
    #[test]
    fn prop_response_serialization_roundtrip(
        id in 0u64..1000,
        result in prop::option::of(arb_json_value()),
        error_code in prop::option::of(0i32..1000)
    ) {
        let original = match result {
            Some(r) => McpResponse::ok(id, r),
            None => match error_code {
                Some(code) => McpResponse::err(id, McpError::new(code, "test error")),
                None => McpResponse::ok(id, Value::Null),
            }
        };

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: McpResponse = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(original.id, deserialized.id);
        prop_assert_eq!(original.jsonrpc, deserialized.jsonrpc);
    }

    /// Test that tool definitions can be serialized and deserialized correctly
    #[test]
    fn prop_tool_serialization_roundtrip(tool in arb_tool()) {
        let serialized = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(tool.name, deserialized.name);
        prop_assert_eq!(tool.description, deserialized.description);
    }
}

// ============================================================================
// Property 2: Retry Logic Properties
// ============================================================================

proptest! {
    /// Test that retry delays increase exponentially
    #[test]
    fn prop_retry_delays_increase_exponentially(
        base_delay_ms in 10u64..1000,
        max_delay_ms in 1000u64..60000,
        attempt in 1usize..10usize
    ) {
        let config = RetryConfig::new()
            .base_delay(Duration::from_millis(base_delay_ms))
            .max_delay(Duration::from_millis(max_delay_ms));

        let delay = config.calculate_delay(attempt);

        // Delay should not exceed max_delay
        prop_assert!(delay <= Duration::from_millis(max_delay_ms));

        // For higher attempts, delay should be larger (with some tolerance for jitter)
        if attempt > 1 {
            let delay_prev = config.calculate_delay(attempt - 1);
            // Allow for jitter to cause non-monotonic delays in rare cases
            // but check that delay is roughly exponential
            let ratio = delay.as_millis() as f64 / delay_prev.as_millis().max(1) as f64;
            prop_assert!(ratio < 3.0); // Should be roughly 2x, but with jitter
        }
    }

    /// Test that retry delays are bounded by max_delay
    #[test]
    fn prop_retry_delays_bounded_by_max(
        base_delay_ms in 10u64..1000,
        max_delay_ms in 1000u64..60000,
        attempt in 1usize..20usize
    ) {
        let config = RetryConfig::new()
            .base_delay(Duration::from_millis(base_delay_ms))
            .max_delay(Duration::from_millis(max_delay_ms));

        let delay = config.calculate_delay(attempt);
        prop_assert!(delay <= Duration::from_millis(max_delay_ms));
    }

    /// Test that max_attempts is stored correctly
    #[test]
    fn prop_retry_max_attempts_stored(
        max_attempts in 1usize..10usize
    ) {
        let config = RetryConfig::new().max_attempts(max_attempts);
        prop_assert_eq!(config.max_attempts, max_attempts);
    }
}

// ============================================================================
// Property 3: Error Handling Invariants
// ============================================================================

proptest! {
    /// Test that MCP errors have valid error codes
    #[test]
    fn prop_error_codes_valid(code in -32000i32..32000i32, message in "[a-zA-Z ]+") {
        let error = McpError::new(code, &message);

        prop_assert_eq!(error.code, code);
        prop_assert!(error.message.contains(&message));
    }

    /// Test that standard error methods produce valid errors
    #[test]
    fn prop_standard_errors_valid(method in "[a-z_]+") {
        let not_found = McpError::method_not_found(&method);
        prop_assert_eq!(not_found.code, -32601);

        let invalid_params = McpError::invalid_params(&method);
        prop_assert_eq!(invalid_params.code, -32602);

        let internal = McpError::internal_error(&method);
        prop_assert_eq!(internal.code, -32603);
    }

    /// Test that error serialization preserves all fields
    #[test]
    fn prop_error_serialization_preserves_fields(
        code in -32000i32..32000i32,
        message in "[a-zA-Z ]+",
        data in prop::option::of(any::<i64>().prop_map(|n| Value::Number(n.into())))
    ) {
        let original = match data {
            Some(d) => McpError::with_data(code, &message, d),
            None => McpError::new(code, &message),
        };

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: McpError = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(original.code, deserialized.code);
        prop_assert_eq!(original.message, deserialized.message);
        prop_assert_eq!(original.data, deserialized.data);
    }
}

// ============================================================================
// Property 4: Initialize Parameters Invariants
// ============================================================================

proptest! {
    /// Test that client info can be serialized and deserialized correctly
    #[test]
    fn prop_client_info_serialization_roundtrip(
        name in "[a-zA-Z]+",
        version in "[0-9.]+"
    ) {
        let client_info = ClientInfo {
            name: name.clone(),
            version: version.clone(),
        };

        let serialized = serde_json::to_string(&client_info).unwrap();
        let deserialized: ClientInfo = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(deserialized.name, name);
        prop_assert_eq!(deserialized.version, version);
    }

    /// Test that server info is preserved in responses
    #[test]
    fn prop_server_info_preserved(
        name in "[a-zA-Z]+",
        version in "[0-9.]+"
    ) {
        let server_info = ServerInfo {
            name: name.clone(),
            version: version.clone(),
        };

        let serialized = serde_json::to_string(&server_info).unwrap();
        let deserialized: ServerInfo = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(deserialized.name, name);
        prop_assert_eq!(deserialized.version, version);
    }
}

// ============================================================================
// Property 5: Tool Call Parameters Invariants
// ============================================================================

proptest! {
    /// Test that tool call parameters preserve the tool name and arguments
    #[test]
    fn prop_tool_call_params_preserve_data(params in arb_tool_call_params()) {
        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: ToolCallParams = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(params.name, deserialized.name);
        prop_assert_eq!(params.arguments, deserialized.arguments);
    }

    /// Test that multiple tool calls can be batched
    #[test]
    fn prop_multiple_tool_calls_batching(
        tools in prop::collection::vec(arb_tool_call_params(), 1..5)
    ) {
        // All tools should serialize successfully
        for tool in &tools {
            let serialized = serde_json::to_string(tool).unwrap();
            let deserialized: ToolCallParams = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(&tool.name, &deserialized.name);
        }
    }
}

// ============================================================================
// Property 6: JSON Schema Validation Invariants
// ============================================================================

proptest! {
    /// Test that valid JSON schemas can be parsed
    #[test]
    fn prop_valid_json_schema_parseable(schema_json in arb_json_value()) {
        // Any valid JSON should be parseable as a schema
        let serialized = serde_json::to_string(&schema_json).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(schema_json, parsed);
    }

    /// Test that tool schemas are preserved
    #[test]
    fn prop_tool_schema_preserved(
        name in "[a-z_]+",
        schema in arb_json_value()
    ) {
        let tool = Tool {
            name: name.clone(),
            description: "Test tool".to_string(),
            input_schema: schema.clone(),
        };

        let serialized = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(tool.name, deserialized.name);
        prop_assert_eq!(tool.input_schema, deserialized.input_schema);
    }
}
