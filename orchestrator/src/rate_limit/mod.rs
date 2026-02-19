//! Rate Limiting and Quota Management Module
//!
//! This module provides per-agent and per-user rate limiting with configurable
//! quotas for API calls, VM spawning, and approval requests.
//!
//! # Features
//!
//! - Token bucket algorithm for smooth rate limiting
//! - Per-agent and per-user quota tracking
//! - Configurable limits for different operation types
//! - Admin dashboard support for monitoring and adjusting quotas
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Rate Limit Manager                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │ API Quotas  │  │ VM Quotas   │  │ Approval    │         │
//! │  │             │  │             │  │ Quotas      │         │
//! │  └─────────────┘  └─────────────┘  └─────────────┘         │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │              Quota Store (In-Memory + Persistent)    │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod config;
pub mod dashboard;
pub mod manager;
pub mod quota;
pub mod store;

pub use config::RateLimitConfig;
pub use dashboard::DashboardData;
pub use manager::RateLimitManager;
pub use quota::{Quota, QuotaType, UsageStats};
pub use store::QuotaStore;
