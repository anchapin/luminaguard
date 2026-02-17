//! LuminaGuard
//!
//! This library provides the core functionality for the LuminaGuard Orchestrator,
//! including MCP client implementation, VM spawning, approval cliff, and memory management.

pub mod agent_rpc;
pub mod approval;
pub mod mcp;
pub mod mcp_command;
pub mod vm;
pub mod metrics;
pub mod metrics_server;
