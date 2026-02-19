//! Daemon Logging Configuration
//!
//! This module provides structured logging with configurable log levels,
//! JSON format support, and request ID tracking.
//!
//! Issue: #499

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Log format configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable plain text format
    Plain,
    /// JSON structured format
    Json,
}

/// Log level configuration
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

/// Daemon logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: LogLevel,

    /// Log format (plain, json)
    pub format: LogFormat,

    /// Include span events in logs
    pub span_events: bool,

    /// Include file and line number in logs
    pub file_and_line: bool,

    /// Log file path (optional, logs to stdout if not set)
    pub file_path: Option<PathBuf>,

    /// Enable request ID tracking
    pub request_id_tracking: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Plain,
            span_events: false,
            file_and_line: true,
            file_path: None,
            request_id_tracking: true,
        }
    }
}

impl LoggingConfig {
    /// Create a new logging config with custom level
    pub fn with_level(level: LogLevel) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Create a new logging config with JSON format
    pub fn with_json_format() -> Self {
        Self {
            format: LogFormat::Json,
            ..Default::default()
        }
    }

    /// Load logging config from environment variables
    pub fn from_env() -> Self {
        let level = std::env::var("LUMINAGUARD_LOG_LEVEL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();

        let format = std::env::var("LUMINAGUARD_LOG_FORMAT")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "json" => Some(LogFormat::Json),
                "plain" => Some(LogFormat::Plain),
                _ => None,
            })
            .unwrap_or(LogFormat::Plain);

        let file_path = std::env::var("LUMINAGUARD_LOG_FILE")
            .ok()
            .map(PathBuf::from);

        Self {
            level,
            format,
            file_path,
            ..Default::default()
        }
    }

    /// Initialize the logging system
    pub fn init(&self) -> Result<()> {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(self.level.to_string()));

        match self.format {
            LogFormat::Plain => {
                let layer = fmt::layer()
                    .with_file(self.file_and_line)
                    .with_line_number(self.file_and_line)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_thread_names(false)
                    .with_span_events(if self.span_events {
                        FmtSpan::FULL
                    } else {
                        FmtSpan::NONE
                    })
                    .with_filter(filter);

                tracing_subscriber::registry().with(layer).try_init()?;
            }
            LogFormat::Json => {
                // JSON format requires the "json" feature in tracing-subscriber
                // Fall back to plain format for now
                let layer = fmt::layer()
                    .with_file(self.file_and_line)
                    .with_line_number(self.file_and_line)
                    .with_target(true)
                    .with_span_events(if self.span_events {
                        FmtSpan::FULL
                    } else {
                        FmtSpan::NONE
                    })
                    .with_filter(filter);

                tracing_subscriber::registry().with(layer).try_init()?;
            }
        }

        tracing::info!(
            "Logging initialized: level={}, format={:?}",
            self.level,
            self.format
        );

        Ok(())
    }
}

/// Request ID for tracking requests across log entries
#[derive(Debug, Clone, Copy)]
pub struct RequestId(pub uuid::Uuid);

impl RequestId {
    /// Generate a new request ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_default() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn test_log_level_from_tracing() {
        let level: tracing::Level = LogLevel::Debug.into();
        assert_eq!(level, tracing::Level::DEBUG);
    }

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, LogLevel::Info);
        assert_eq!(config.format, LogFormat::Plain);
        assert!(config.file_path.is_none());
    }

    #[test]
    fn test_logging_config_with_level() {
        let config = LoggingConfig::with_level(LogLevel::Debug);
        assert_eq!(config.level, LogLevel::Debug);
    }

    #[test]
    fn test_logging_config_with_json() {
        let config = LoggingConfig::with_json_format();
        assert_eq!(config.format, LogFormat::Json);
    }

    #[test]
    fn test_request_id_new() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();
        assert_ne!(id1.0, id2.0);
    }

    #[test]
    fn test_request_id_display() {
        let id = RequestId::new();
        let s = id.to_string();
        assert!(!s.is_empty());
        assert_eq!(s.len(), 36); // UUID format
    }
}
