# Configuration File Support

LuminaGuard supports configuration files for customizing the orchestrator behavior.

## Configuration File Location

The configuration file is loaded from the XDG config directory:

- **Linux/Mac**: `~/.config/luminaguard/config.toml`
- **Windows**: `%APPDATA%\luminaguard\config.toml`

You can also specify a custom configuration file path using the `--config` command-line flag:

```bash
luminaguard --config /path/to/custom/config.toml
```

## Configuration File Format

LuminaGuard uses TOML format for configuration files. An example configuration file is provided at `config.example.toml`.

## Configuration Options

### Logging

```toml
[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# Log format: json, pretty, compact
format = "compact"

# Whether to log to file
log_to_file = false

# Log file path (if log_to_file is true)
# log_file = "/var/log/luminaguard/orchestrator.log"
```

**Environment Variables:**
- `LUMINAGUARD_LOG_LEVEL` - Override the log level
- `LUMINAGUARD_LOG_FORMAT` - Override the log format

### VM Configuration

```toml
[vm]
# Number of vCPUs for VMs
vcpu_count = 1

# Memory size in MB for VMs (minimum: 128)
memory_mb = 512

# Kernel image path
kernel_path = "./resources/vmlinux"

# Root filesystem path
rootfs_path = "./resources/rootfs.ext4"
```

**Environment Variables:**
- `LUMINAGUARD_VCPU_COUNT` - Override the vCPU count
- `LUMINAGUARD_MEMORY_MB` - Override the memory size

### Snapshot Pool Configuration

```toml
[vm.pool]
# Number of snapshots to maintain in pool (1-20)
pool_size = 5

# Snapshot storage location
snapshot_path = "/var/lib/luminaguard/snapshots"

# Snapshot refresh interval in seconds (minimum: 60)
refresh_interval_secs = 3600

# Maximum snapshot age before refresh (in seconds)
max_snapshot_age_secs = 3600
```

**Environment Variables:**
- `LUMINAGUARD_POOL_SIZE` - Override the pool size
- `LUMINAGUARD_SNAPSHOT_PATH` - Override the snapshot path
- `LUMINAGUARD_SNAPSHOT_REFRESH_SECS` - Override the refresh interval

### MCP Server Configuration

```toml
[mcp_servers.filesystem]
# Command to spawn the MCP server
command = "npx"

# Arguments for the MCP server
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]

# Transport type: stdio, http
transport = "stdio"

# Timeout in seconds for MCP requests
timeout_secs = 30

# Whether to retry failed requests
retry = true
```

You can configure multiple MCP servers by adding more sections under `mcp_servers`:

```toml
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
transport = "stdio"
timeout_secs = 30
retry = true

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
transport = "stdio"
timeout_secs = 30
retry = true

[mcp_servers.custom_http]
command = ""
args = []
transport = "http"
url = "https://api.example.com/mcp"
timeout_secs = 60
retry = true
```

**Transport Types:**
- `stdio` - Standard input/output transport (for local MCP servers)
- `http` - HTTP transport (for remote MCP servers, requires `url` field)

### Metrics Configuration

```toml
[metrics]
# Whether to enable metrics collection
enabled = false

# Port for metrics server
port = 9090

# Metrics export interval in seconds
export_interval_secs = 60
```

**Environment Variables:**
- `LUMINAGUARD_METRICS_ENABLED` - Override whether metrics are enabled
- `LUMINAGUARD_METRICS_PORT` - Override the metrics port

## Configuration Validation

The configuration is validated on load. Invalid configurations will cause an error to be displayed.

**Validation Rules:**
- Log level must be one of: `trace`, `debug`, `info`, `warn`, `error`
- Log format must be one of: `json`, `pretty`, `compact`
- VM vCPU count must be > 0
- VM memory must be >= 128 MB
- Pool size must be between 1 and 20
- Refresh interval must be >= 60 seconds
- Metrics port must be > 0
- MCP server command must not be empty
- MCP server transport must be `stdio` or `http`
- HTTP MCP servers must have a URL configured

## Environment Variable Overrides

Environment variables take precedence over configuration file values. This is useful for:
- Docker container configuration
- CI/CD environments
- One-off testing

Example:

```bash
# Override log level to debug
LUMINAGUARD_LOG_LEVEL=debug luminaguard run "my task"

# Use a custom pool size
LUMINAGUARD_POOL_SIZE=10 luminaguard daemon

# Enable metrics on a custom port
LUMINAGUARD_METRICS_ENABLED=true \
LUMINAGUARD_METRICS_PORT=8080 \
luminaguard daemon
```

## Default Configuration

If no configuration file is found, LuminaGuard uses sensible defaults:

```toml
[logging]
level = "info"
format = "compact"
log_to_file = false
log_file = null

[vm]
vcpu_count = 1
memory_mb = 512
kernel_path = "./resources/vmlinux"
rootfs_path = "./resources/rootfs.ext4"

[vm.pool]
pool_size = 5
snapshot_path = "/var/lib/luminaguard/snapshots"
refresh_interval_secs = 3600
max_snapshot_age_secs = 3600

[mcp_servers]

[metrics]
enabled = false
port = 9090
export_interval_secs = 60
```

## Configuration Module API

The configuration module provides the following API:

```rust
use luminaguard_orchestrator::config::Config;

// Load configuration from default location
let config = Config::load()?;

// Load configuration from specific path
let config = Config::load_from_path("/path/to/config.toml")?;

// Get default configuration file path
let path = Config::config_path();

// Validate configuration
config.validate()?;

// Convert log level string to tracing::Level
let level = config.log_level()?;
```

## Testing

The configuration module includes comprehensive tests:

```bash
# Run configuration tests
cargo test --lib config::tests

# Run tests with single thread (required for environment variable tests)
cargo test --lib config::tests -- --test-threads=1
```

## Security Considerations

1. **Configuration File Permissions**: Ensure your configuration file has appropriate permissions (e.g., `0600` for user-only access)
2. **Secrets**: Do not store sensitive information (API keys, passwords) in the configuration file
3. **File Paths**: Validate all file paths before use (the configuration module does this automatically)
4. **Environment Variables**: Environment variable overrides are useful but can be a security risk in production

## Troubleshooting

### Configuration File Not Found

If you see the message "Config file not found at ..., using defaults", LuminaGuard will use default values. To create a configuration file, copy `config.example.toml` to `~/.config/luminaguard/config.toml` and customize it.

### Invalid Configuration

If you see an error about invalid configuration, check:
- Log level is one of the valid values
- Log format is one of the valid values
- VM settings are within valid ranges
- MCP server configurations are complete
- HTTP MCP servers have URLs configured

### Environment Variables Not Working

Ensure that environment variables are set before running the orchestrator:

```bash
# Set environment variable in current shell
export LUMINAGUARD_LOG_LEVEL=debug
luminaguard run "my task"

# Set for single command
LUMINAGUARD_LOG_LEVEL=debug luminaguard run "my task"
```
