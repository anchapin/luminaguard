// Configuration File Support
//
// This module provides configuration file parsing for the LuminaGuard orchestrator.
// Supports TOML format with environment variable overrides.
// Configuration files are loaded from XDG config directory: ~/.config/luminaguard/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Logging configuration
    pub logging: LoggingConfig,

    /// VM configuration
    pub vm: VmConfig,

    /// MCP server configurations
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Metrics configuration
    pub metrics: MetricsConfig,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log format (json, pretty, compact)
    pub format: String,

    /// Whether to log to file
    pub log_to_file: bool,

    /// Log file path (if log_to_file is true)
    pub log_file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "compact".to_string(),
            log_to_file: false,
            log_file: None,
        }
    }
}

/// VM configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VmConfig {
    /// Number of vCPUs for VMs
    pub vcpu_count: u8,

    /// Memory size in MB for VMs
    pub memory_mb: u32,

    /// Kernel image path
    pub kernel_path: String,

    /// Root filesystem path
    pub rootfs_path: String,

    /// Snapshot pool configuration
    pub pool: PoolConfig,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            vcpu_count: 1,
            memory_mb: 512,
            kernel_path: "./resources/vmlinux".to_string(),
            rootfs_path: "./resources/rootfs.ext4".to_string(),
            pool: PoolConfig::default(),
        }
    }
}

/// Snapshot pool configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PoolConfig {
    /// Number of snapshots to maintain in pool
    pub pool_size: usize,

    /// Snapshot storage location
    pub snapshot_path: String,

    /// Snapshot refresh interval in seconds
    pub refresh_interval_secs: u64,

    /// Maximum snapshot age before refresh
    pub max_snapshot_age_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            pool_size: 5,
            snapshot_path: "/var/lib/luminaguard/snapshots".to_string(),
            refresh_interval_secs: 3600,
            max_snapshot_age_secs: 3600,
        }
    }
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct McpServerConfig {
    /// Command to spawn the MCP server (e.g., "npx")
    pub command: String,

    /// Arguments for the MCP server
    pub args: Vec<String>,

    /// Transport type (stdio, http)
    #[serde(default = "default_transport_type")]
    pub transport: String,

    /// HTTP URL (if transport is http)
    pub url: Option<String>,

    /// Timeout in seconds for MCP requests
    pub timeout_secs: u64,

    /// Whether to retry failed requests
    pub retry: bool,
}

fn default_transport_type() -> String {
    "stdio".to_string()
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), "/tmp".to_string()],
            transport: default_transport_type(),
            url: None,
            timeout_secs: 30,
            retry: true,
        }
    }
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MetricsConfig {
    /// Whether to enable metrics collection
    pub enabled: bool,

    /// Port for metrics server
    pub port: u16,

    /// Metrics export interval in seconds
    pub export_interval_secs: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9090,
            export_interval_secs: 60,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            logging: LoggingConfig::default(),
            vm: VmConfig::default(),
            mcp_servers: HashMap::new(),
            metrics: MetricsConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from the default XDG config directory
    ///
    /// # Returns
    ///
    /// * `Result<Config>` - The loaded configuration with defaults applied
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be parsed.
    /// If the config file does not exist, returns default configuration.
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        Self::load_from_path(&config_path)
    }

    /// Load configuration from a specific path
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the configuration file
    ///
    /// # Returns
    ///
    /// * `Result<Config>` - The loaded configuration with defaults applied
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be parsed.
    /// If the config file does not exist, returns default configuration.
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            tracing::debug!("Config file not found at {:?}, using defaults", path);
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file from {:?}", path))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file from {:?}", path))?;

        // Apply environment variable overrides
        let config = config.apply_env_overrides();

        // Validate configuration
        config.validate()?;

        tracing::info!("Loaded configuration from {:?}", path);
        Ok(config)
    }

    /// Get the default configuration file path
    ///
    /// Returns `~/.config/luminaguard/config.toml` on Linux/Mac
    pub fn config_path() -> PathBuf {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "luminaguard", "LuminaGuard") {
            proj_dirs.config_dir().join("config.toml")
        } else {
            // Fallback if XDG dirs cannot be determined
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config").join("luminaguard").join("config.toml")
        }
    }

    /// Apply environment variable overrides to the configuration
    ///
    /// Environment variables take precedence over config file values:
    /// - LUMINAGUARD_LOG_LEVEL
    /// - LUMINAGUARD_LOG_FORMAT
    /// - LUMINAGUARD_POOL_SIZE
    /// - LUMINAGUARD_SNAPSHOT_PATH
    /// - LUMINAGUARD_SNAPSHOT_REFRESH_SECS
    fn apply_env_overrides(mut self) -> Self {
        // Logging overrides
        if let Ok(level) = std::env::var("LUMINAGUARD_LOG_LEVEL") {
            self.logging.level = level;
        }
        if let Ok(format) = std::env::var("LUMINAGUARD_LOG_FORMAT") {
            self.logging.format = format;
        }

        // Pool overrides
        if let Ok(size) = std::env::var("LUMINAGUARD_POOL_SIZE") {
            if let Ok(size) = size.parse::<usize>() {
                if size > 0 && size <= 20 {
                    self.vm.pool.pool_size = size;
                }
            }
        }
        if let Ok(path) = std::env::var("LUMINAGUARD_SNAPSHOT_PATH") {
            self.vm.pool.snapshot_path = path;
        }
        if let Ok(refresh) = std::env::var("LUMINAGUARD_SNAPSHOT_REFRESH_SECS") {
            if let Ok(refresh) = refresh.parse::<u64>() {
                if refresh >= 60 {
                    self.vm.pool.refresh_interval_secs = refresh;
                }
            }
        }

        // VM overrides
        if let Ok(vcpus) = std::env::var("LUMINAGUARD_VCPU_COUNT") {
            if let Ok(vcpus) = vcpus.parse::<u8>() {
                if vcpus > 0 {
                    self.vm.vcpu_count = vcpus;
                }
            }
        }
        if let Ok(memory) = std::env::var("LUMINAGUARD_MEMORY_MB") {
            if let Ok(memory) = memory.parse::<u32>() {
                if memory >= 128 {
                    self.vm.memory_mb = memory;
                }
            }
        }

        // Metrics overrides
        if let Ok(enabled) = std::env::var("LUMINAGUARD_METRICS_ENABLED") {
            self.metrics.enabled = enabled.parse().unwrap_or(self.metrics.enabled);
        }
        if let Ok(port) = std::env::var("LUMINAGUARD_METRICS_PORT") {
            if let Ok(port) = port.parse::<u16>() {
                self.metrics.port = port;
            }
        }

        self
    }

    /// Validate the configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> Result<()> {
        // Validate logging level
        match self.logging.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => anyhow::bail!("Invalid log level: {}. Must be one of: trace, debug, info, warn, error", self.logging.level),
        }

        // Validate logging format
        match self.logging.format.to_lowercase().as_str() {
            "json" | "pretty" | "compact" => {}
            _ => anyhow::bail!("Invalid log format: {}. Must be one of: json, pretty, compact", self.logging.format),
        }

        // Validate VM configuration
        if self.vm.vcpu_count == 0 {
            anyhow::bail!("VM vCPU count must be > 0");
        }
        if self.vm.memory_mb < 128 {
            anyhow::bail!("VM memory must be at least 128 MB");
        }

        // Validate pool configuration
        if self.vm.pool.pool_size == 0 {
            anyhow::bail!("Pool size must be > 0");
        }
        if self.vm.pool.pool_size > 20 {
            anyhow::bail!("Pool size must be <= 20");
        }
        if self.vm.pool.refresh_interval_secs < 60 {
            anyhow::bail!("Snapshot refresh interval must be at least 60 seconds");
        }

        // Validate metrics configuration
        if self.metrics.port == 0 {
            anyhow::bail!("Metrics port must be > 0");
        }

        // Validate MCP server configurations
        for (name, server) in &self.mcp_servers {
            if server.command.is_empty() {
                anyhow::bail!("MCP server '{}' has empty command", name);
            }
            match server.transport.to_lowercase().as_str() {
                "stdio" | "http" => {}
                _ => anyhow::bail!("MCP server '{}' has invalid transport: {}. Must be 'stdio' or 'http'", name, server.transport),
            }
            if server.transport.to_lowercase() == "http" && server.url.is_none() {
                anyhow::bail!("MCP server '{}' uses HTTP transport but has no URL configured", name);
            }
        }

        Ok(())
    }

    /// Convert log level string to tracing::Level
    pub fn log_level(&self) -> Result<tracing::Level> {
        self.logging.level.to_lowercase().parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse log level: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.vm.vcpu_count, 1);
        assert_eq!(config.vm.memory_mb, 512);
        assert_eq!(config.vm.pool.pool_size, 5);
        assert_eq!(config.metrics.enabled, false);
        assert_eq!(config.metrics.port, 9090);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let mut config = Config::default();
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_log_format() {
        let mut config = Config::default();
        config.logging.format = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_vcpu_count() {
        let mut config = Config::default();
        config.vm.vcpu_count = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_memory() {
        let mut config = Config::default();
        config.vm.memory_mb = 64;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_pool_size() {
        let mut config = Config::default();
        config.vm.pool.pool_size = 0;
        assert!(config.validate().is_err());

        config.vm.pool.pool_size = 25;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_refresh_interval() {
        let mut config = Config::default();
        config.vm.pool.refresh_interval_secs = 30;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_metrics_port() {
        let mut config = Config::default();
        config.metrics.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_from_nonexistent_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().with_extension(".nonexistent");
        let config = Config::load_from_path(&path);
        assert!(config.is_ok());
        assert_eq!(config.unwrap(), Config::default());
    }

    #[test]
    fn test_load_valid_toml_config() {
        // Clean up environment variables to ensure isolation
        std::env::remove_var("LUMINAGUARD_LOG_LEVEL");
        std::env::remove_var("LUMINAGUARD_LOG_FORMAT");
        std::env::remove_var("LUMINAGUARD_POOL_SIZE");
        std::env::remove_var("LUMINAGUARD_SNAPSHOT_PATH");
        std::env::remove_var("LUMINAGUARD_SNAPSHOT_REFRESH_SECS");
        std::env::remove_var("LUMINAGUARD_VCPU_COUNT");
        std::env::remove_var("LUMINAGUARD_MEMORY_MB");
        std::env::remove_var("LUMINAGUARD_METRICS_ENABLED");
        std::env::remove_var("LUMINAGUARD_METRICS_PORT");

        let temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[logging]
level = "debug"
format = "json"

[vm]
vcpu_count = 2
memory_mb = 1024
kernel_path = "/path/to/kernel"
rootfs_path = "/path/to/rootfs"

[vm.pool]
pool_size = 10
snapshot_path = "/custom/snapshots"
refresh_interval_secs = 1800

[metrics]
enabled = true
port = 8080
"#;

        fs::write(temp_file.path(), toml_content).unwrap();

        let config = Config::load_from_path(temp_file.path()).unwrap();
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.vm.vcpu_count, 2);
        assert_eq!(config.vm.memory_mb, 1024);
        assert_eq!(config.vm.pool.pool_size, 10);
        assert_eq!(config.vm.pool.snapshot_path, "/custom/snapshots");
        assert_eq!(config.metrics.enabled, true);
        assert_eq!(config.metrics.port, 8080);
    }

    #[test]
    fn test_load_invalid_toml_config() {
        let temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[logging
level = "debug"
"#; // Invalid TOML

        fs::write(temp_file.path(), toml_content).unwrap();

        let config = Config::load_from_path(temp_file.path());
        assert!(config.is_err());
    }

    #[test]
    fn test_env_overrides() {
        // Clean up environment variables first to ensure isolation
        std::env::remove_var("LUMINAGUARD_LOG_LEVEL");
        std::env::remove_var("LUMINAGUARD_LOG_FORMAT");
        std::env::remove_var("LUMINAGUARD_POOL_SIZE");
        std::env::remove_var("LUMINAGUARD_SNAPSHOT_PATH");
        std::env::remove_var("LUMINAGUARD_VCPU_COUNT");
        std::env::remove_var("LUMINAGUARD_METRICS_ENABLED");

        std::env::set_var("LUMINAGUARD_LOG_LEVEL", "debug");
        std::env::set_var("LUMINAGUARD_LOG_FORMAT", "json");
        std::env::set_var("LUMINAGUARD_POOL_SIZE", "10");
        std::env::set_var("LUMINAGUARD_SNAPSHOT_PATH", "/custom/path");
        std::env::set_var("LUMINAGUARD_VCPU_COUNT", "2");
        std::env::set_var("LUMINAGUARD_METRICS_ENABLED", "true");

        let config = Config::default().apply_env_overrides();

        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.vm.pool.pool_size, 10);
        assert_eq!(config.vm.pool.snapshot_path, "/custom/path");
        assert_eq!(config.vm.vcpu_count, 2);
        assert_eq!(config.metrics.enabled, true);

        // Clean up
        std::env::remove_var("LUMINAGUARD_LOG_LEVEL");
        std::env::remove_var("LUMINAGUARD_LOG_FORMAT");
        std::env::remove_var("LUMINAGUARD_POOL_SIZE");
        std::env::remove_var("LUMINAGUARD_SNAPSHOT_PATH");
        std::env::remove_var("LUMINAGUARD_VCPU_COUNT");
        std::env::remove_var("LUMINAGUARD_METRICS_ENABLED");
    }

    #[test]
    fn test_env_overrides_invalid_values() {
        // Clean up environment variables first to ensure isolation
        std::env::remove_var("LUMINAGUARD_POOL_SIZE");
        std::env::remove_var("LUMINAGUARD_VCPU_COUNT");

        std::env::set_var("LUMINAGUARD_POOL_SIZE", "25"); // Invalid (> 20)
        std::env::set_var("LUMINAGUARD_VCPU_COUNT", "0"); // Invalid (must be > 0)

        let config = Config::default().apply_env_overrides();

        // Should keep defaults for invalid values
        assert_eq!(config.vm.pool.pool_size, 5);
        assert_eq!(config.vm.vcpu_count, 1);

        // Clean up
        std::env::remove_var("LUMINAGUARD_POOL_SIZE");
        std::env::remove_var("LUMINAGUARD_VCPU_COUNT");
    }

    #[test]
    fn test_mcp_server_config_default() {
        let config = McpServerConfig::default();
        assert_eq!(config.command, "npx");
        assert_eq!(config.transport, "stdio");
        assert_eq!(config.timeout_secs, 30);
        assert!(config.retry);
    }

    #[test]
    fn test_config_with_mcp_servers() {
        let temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
transport = "stdio"
timeout_secs = 30

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
transport = "stdio"
"#;

        fs::write(temp_file.path(), toml_content).unwrap();

        let config = Config::load_from_path(temp_file.path()).unwrap();
        assert_eq!(config.mcp_servers.len(), 2);

        let fs_server = config.mcp_servers.get("filesystem").unwrap();
        assert_eq!(fs_server.command, "npx");
        assert_eq!(fs_server.transport, "stdio");

        let github_server = config.mcp_servers.get("github").unwrap();
        assert_eq!(github_server.command, "npx");
    }

    #[test]
    fn test_config_validation_mcp_server_empty_command() {
        let mut config = Config::default();
        config.mcp_servers.insert("test".to_string(), McpServerConfig {
            command: String::new(),
            ..Default::default()
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_mcp_server_invalid_transport() {
        let mut config = Config::default();
        config.mcp_servers.insert("test".to_string(), McpServerConfig {
            transport: "invalid".to_string(),
            ..Default::default()
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_mcp_server_http_no_url() {
        let mut config = Config::default();
        config.mcp_servers.insert("test".to_string(), McpServerConfig {
            transport: "http".to_string(),
            url: None,
            ..Default::default()
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path();
        assert!(path.ends_with("config.toml"));
    }

    #[test]
    fn test_log_level_parsing() {
        let mut config = Config::default();
        config.logging.level = "debug".to_string();
        assert_eq!(config.log_level().unwrap(), tracing::Level::DEBUG);

        config.logging.level = "info".to_string();
        assert_eq!(config.log_level().unwrap(), tracing::Level::INFO);
    }

    #[test]
    fn test_log_level_parsing_invalid() {
        let mut config = Config::default();
        config.logging.level = "invalid".to_string();
        assert!(config.log_level().is_err());
    }

    #[test]
    fn test_config_partial_toml() {
        let temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[logging]
level = "debug"
"#;

        fs::write(temp_file.path(), toml_content).unwrap();

        let config = Config::load_from_path(temp_file.path()).unwrap();
        assert_eq!(config.logging.level, "debug");
        // Other fields should have defaults
        assert_eq!(config.vm.vcpu_count, 1);
        assert_eq!(config.metrics.port, 9090);
    }

    // Property-based test: config validation is deterministic
    #[test]
    fn test_config_validation_deterministic() {
        let config = Config::default();
        let result1 = config.validate();
        let result2 = config.validate();
        assert_eq!(result1.is_ok(), result2.is_ok());
    }

    // Property-based test: valid log levels are always accepted
    #[test]
    fn test_valid_log_levels() {
        let levels = vec!["trace", "debug", "info", "warn", "error"];
        for level in levels {
            let mut config = Config::default();
            config.logging.level = level.to_string();
            assert!(config.validate().is_ok(), "Log level {} should be valid", level);
        }
    }
}
