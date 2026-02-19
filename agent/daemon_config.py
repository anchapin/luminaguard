#!/usr/bin/env python3
"""
Daemon Configuration System for LuminaGuard

Provides:
- YAML/JSON configuration file support
- Environment variable overrides
- Hot-reload configuration without restart
- Configuration validation and defaults
- CLI flag support for common options
"""

import json
import os
import sys
from dataclasses import dataclass, field, asdict
from enum import Enum
from pathlib import Path
from typing import Any, Dict, Optional, List
from datetime import datetime
import logging

try:
    import yaml

    HAS_YAML = True
except ImportError:
    HAS_YAML = False


class LogLevel(Enum):
    """Logging levels"""

    DEBUG = "debug"
    INFO = "info"
    WARNING = "warning"
    ERROR = "error"
    CRITICAL = "critical"


@dataclass
class LoggingConfig:
    """Logging configuration"""

    level: LogLevel = LogLevel.INFO
    format: str = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    file: Optional[str] = None
    max_size_mb: int = 100
    backup_count: int = 5
    pretty_print: bool = False


@dataclass
class SecurityConfig:
    """Security configuration"""

    approval_required_by_default: bool = True
    approval_timeout_seconds: int = 300
    audit_log_path: Optional[str] = None
    enable_audit_logging: bool = True
    secure_subprocess: bool = True


@dataclass
class VmConfig:
    """VM/Firecracker configuration"""

    enabled: bool = True
    vcpu_count: int = 2
    memory_mb: int = 512
    kernel_path: Optional[str] = None
    rootfs_path: Optional[str] = None
    timeout_seconds: int = 60


@dataclass
class McpConfig:
    """MCP (Model Context Protocol) configuration"""

    enabled: bool = True
    servers: List[Dict[str, Any]] = field(default_factory=list)
    timeout_seconds: int = 30
    auto_start: bool = True


@dataclass
class DaemonConfig:
    """Main daemon configuration"""

    # Basic settings
    name: str = "luminaguard"
    version: str = "1.0.0"

    # Paths
    config_dir: Optional[str] = None
    log_dir: Optional[str] = None
    data_dir: Optional[str] = None

    # Mode and features
    execution_mode: str = "host"  # "host" or "vm"
    enable_daemon: bool = False
    port: int = 8000
    host: str = "127.0.0.1"

    # Sub-configurations
    logging: LoggingConfig = field(default_factory=LoggingConfig)
    security: SecurityConfig = field(default_factory=SecurityConfig)
    vm: VmConfig = field(default_factory=VmConfig)
    mcp: McpConfig = field(default_factory=McpConfig)

    # Metadata
    created_at: Optional[str] = None
    last_modified: Optional[str] = None

    def __post_init__(self):
        """Initialize defaults and timestamp"""
        if self.created_at is None:
            self.created_at = datetime.now().isoformat()
        self.last_modified = datetime.now().isoformat()

        # Set default directories if not specified
        if self.config_dir is None:
            self.config_dir = str(Path.home() / ".luminaguard" / "config")
        if self.log_dir is None:
            self.log_dir = str(Path.home() / ".luminaguard" / "logs")
        if self.data_dir is None:
            self.data_dir = str(Path.home() / ".luminaguard" / "data")


class ConfigLoader:
    """Load and manage daemon configuration"""

    def __init__(self, config_path: Optional[str] = None):
        """
        Initialize configuration loader

        Args:
            config_path: Path to config file (YAML or JSON)
                        If None, searches in standard locations
        """
        self.config_path = config_path or self._find_config()
        self.logger = logging.getLogger(__name__)
        self._watchers: List[callable] = []

    @staticmethod
    def _find_config() -> Optional[str]:
        """Find configuration file in standard locations"""
        search_paths = [
            Path.cwd() / "luminaguard.yaml",
            Path.cwd() / "luminaguard.json",
            Path.home() / ".luminaguard" / "config.yaml",
            Path.home() / ".luminaguard" / "config.json",
            Path("/etc/luminaguard/config.yaml"),
            Path("/etc/luminaguard/config.json"),
        ]

        for path in search_paths:
            if path.exists():
                return str(path)
        return None

    def load(self) -> DaemonConfig:
        """Load configuration from file or use defaults"""
        config = DaemonConfig()

        # Apply environment overrides to defaults first
        config = self._apply_env_overrides_to_config(config)

        if not self.config_path:
            return config

        try:
            path = Path(self.config_path)
            if not path.exists():
                raise FileNotFoundError(f"Config file not found: {self.config_path}")

            # Load file based on extension
            if path.suffix.lower() == ".yaml" or path.suffix.lower() == ".yml":
                if not HAS_YAML:
                    raise ImportError(
                        "PyYAML required for YAML config. Install: pip install pyyaml"
                    )
                config_dict = self._load_yaml(path)
            elif path.suffix.lower() == ".json":
                config_dict = self._load_json(path)
            else:
                raise ValueError(f"Unsupported config format: {path.suffix}")

            # Merge with environment variables
            config_dict = self._apply_env_overrides(config_dict)

            # Merge with defaults
            config = self._merge_config(config, config_dict)

            self.logger.info(f"Configuration loaded from {self.config_path}")
            return config

        except Exception as e:
            self.logger.warning(f"Failed to load config: {e}, using defaults")
            return config

    def _load_yaml(self, path: Path) -> Dict[str, Any]:
        """Load YAML configuration file"""
        with open(path, "r") as f:
            return yaml.safe_load(f) or {}

    def _load_json(self, path: Path) -> Dict[str, Any]:
        """Load JSON configuration file"""
        with open(path, "r") as f:
            return json.load(f)

    def _apply_env_overrides(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Apply environment variable overrides to dictionary"""
        # Top-level overrides
        env_overrides = {
            "LUMINAGUARD_MODE": "execution_mode",
            "LUMINAGUARD_PORT": "port",
            "LUMINAGUARD_HOST": "host",
            "LUMINAGUARD_DAEMON": "enable_daemon",
            "LUMINAGUARD_LOG_LEVEL": "logging.level",
            "LUMINAGUARD_LOG_FILE": "logging.file",
        }

        for env_var, config_key in env_overrides.items():
            if env_var in os.environ:
                value = os.environ[env_var]

                # Convert types
                if config_key == "port":
                    value = int(value)
                elif config_key == "enable_daemon":
                    value = value.lower() in ("true", "1", "yes")

                # Handle nested keys (e.g., "logging.level")
                if "." in config_key:
                    parts = config_key.split(".")
                    curr = config
                    for part in parts[:-1]:
                        if part not in curr:
                            curr[part] = {}
                        curr = curr[part]
                    curr[parts[-1]] = value
                else:
                    config[config_key] = value

        return config

    def _apply_env_overrides_to_config(self, config: DaemonConfig) -> DaemonConfig:
        """Apply environment variable overrides to DaemonConfig object"""
        env_overrides = {
            "LUMINAGUARD_PORT": ("port", int),
            "LUMINAGUARD_HOST": ("host", str),
            "LUMINAGUARD_MODE": ("execution_mode", str),
            "LUMINAGUARD_DAEMON": (
                "enable_daemon",
                lambda x: x.lower() in ("true", "1", "yes"),
            ),
        }

        for env_var, (config_key, type_fn) in env_overrides.items():
            if env_var in os.environ:
                value = type_fn(os.environ[env_var])
                setattr(config, config_key, value)

        return config

    def _merge_config(
        self, base: DaemonConfig, overrides: Dict[str, Any]
    ) -> DaemonConfig:
        """Merge configuration overrides into base config"""
        base_dict = asdict(base)

        # Deep merge
        for key, value in overrides.items():
            if (
                key in base_dict
                and isinstance(value, dict)
                and isinstance(base_dict[key], dict)
            ):
                base_dict[key] = {**base_dict[key], **value}
            else:
                base_dict[key] = value

        # Reconstruct nested dataclasses
        if "logging" in base_dict and isinstance(base_dict["logging"], dict):
            base_dict["logging"] = LoggingConfig(**base_dict["logging"])
        if "security" in base_dict and isinstance(base_dict["security"], dict):
            base_dict["security"] = SecurityConfig(**base_dict["security"])
        if "vm" in base_dict and isinstance(base_dict["vm"], dict):
            base_dict["vm"] = VmConfig(**base_dict["vm"])
        if "mcp" in base_dict and isinstance(base_dict["mcp"], dict):
            base_dict["mcp"] = McpConfig(**base_dict["mcp"])

        return DaemonConfig(**base_dict)

    def save(self, config: DaemonConfig, path: Optional[str] = None) -> str:
        """Save configuration to file"""
        save_path = Path(
            path or self.config_path or Path.home() / ".luminaguard" / "config.yaml"
        )
        save_path.parent.mkdir(parents=True, exist_ok=True)

        config_dict = asdict(config)

        # Convert enums to strings
        config_dict["logging"]["level"] = config.logging.level.value

        # Save based on file extension
        if save_path.suffix.lower() in (".yaml", ".yml"):
            if not HAS_YAML:
                raise ImportError("PyYAML required. Install: pip install pyyaml")
            with open(save_path, "w") as f:
                yaml.dump(config_dict, f, default_flow_style=False)
        else:
            with open(save_path, "w") as f:
                json.dump(config_dict, f, indent=2)

        self.logger.info(f"Configuration saved to {save_path}")
        return str(save_path)

    def watch(self, callback: callable) -> None:
        """Register a callback for configuration changes"""
        self._watchers.append(callback)

    def notify_watchers(self, config: DaemonConfig) -> None:
        """Notify all watchers of configuration changes"""
        for callback in self._watchers:
            try:
                callback(config)
            except Exception as e:
                self.logger.error(f"Watcher callback failed: {e}")


class ConfigValidator:
    """Validate configuration values"""

    @staticmethod
    def validate(config: DaemonConfig) -> List[str]:
        """
        Validate configuration and return list of errors

        Returns:
            List of validation error messages (empty if valid)
        """
        errors = []

        # Validate logging
        # Handle both LogLevel enum and string values (from JSON loading)
        log_level = config.logging.level
        if isinstance(log_level, str):
            # Check if string is a valid log level value
            valid_levels = [level.value for level in LogLevel]
            if log_level.lower() not in valid_levels:
                errors.append(f"Invalid log level: {config.logging.level}")
        elif not isinstance(log_level, LogLevel):
            errors.append(f"Invalid log level type: {type(config.logging.level)}")
        if config.logging.max_size_mb < 1:
            errors.append("log max_size_mb must be >= 1")
        if config.logging.backup_count < 0:
            errors.append("log backup_count must be >= 0")

        # Validate security
        if config.security.approval_timeout_seconds < 0:
            errors.append("approval_timeout_seconds must be >= 0")

        # Validate VM
        if config.vm.vcpu_count < 1:
            errors.append("vm vcpu_count must be >= 1")
        if config.vm.memory_mb < 128:
            errors.append("vm memory must be >= 128 MB")
        if config.vm.timeout_seconds < 0:
            errors.append("vm timeout_seconds must be >= 0")

        # Validate networking
        if not (0 < config.port < 65536):
            errors.append(f"port must be 1-65535, got {config.port}")

        # Validate execution mode
        if config.execution_mode not in ("host", "vm"):
            errors.append(
                f"execution_mode must be 'host' or 'vm', got {config.execution_mode}"
            )

        return errors


class ConfigManager:
    """High-level configuration management"""

    def __init__(self, config_path: Optional[str] = None):
        """Initialize configuration manager"""
        self.loader = ConfigLoader(config_path)
        self.config = self.loader.load()
        self.logger = logging.getLogger(__name__)

        # Validate on load
        errors = ConfigValidator.validate(self.config)
        if errors:
            for error in errors:
                self.logger.warning(f"Config validation: {error}")

    def get(self, key: str, default: Any = None) -> Any:
        """Get configuration value by dot-notation key"""
        keys = key.split(".")
        value = self.config

        for k in keys:
            if hasattr(value, k):
                value = getattr(value, k)
            elif isinstance(value, dict) and k in value:
                value = value[k]
            else:
                return default

        return value

    def set(self, key: str, value: Any) -> None:
        """Set configuration value by dot-notation key"""
        keys = key.split(".")
        obj = self.config

        # Navigate to parent
        for k in keys[:-1]:
            if hasattr(obj, k):
                obj = getattr(obj, k)
            else:
                raise KeyError(f"Configuration key not found: {key}")

        # Set value
        setattr(obj, keys[-1], value)
        self.config.last_modified = datetime.now().isoformat()

    def reload(self) -> None:
        """Reload configuration from file"""
        self.config = self.loader.load()
        self.loader.notify_watchers(self.config)
        self.logger.info("Configuration reloaded")

    def export(self) -> Dict[str, Any]:
        """Export configuration as dictionary"""
        config_dict = asdict(self.config)
        config_dict["logging"]["level"] = self.config.logging.level.value
        return config_dict


# Global configuration manager instance
_config_manager: Optional[ConfigManager] = None


def get_config_manager(config_path: Optional[str] = None) -> ConfigManager:
    """Get or create global configuration manager"""
    global _config_manager
    if _config_manager is None:
        _config_manager = ConfigManager(config_path)
    return _config_manager


def get_config() -> DaemonConfig:
    """Get current daemon configuration"""
    return get_config_manager().config


if __name__ == "__main__":
    # Example usage
    logging.basicConfig(level=logging.INFO)

    # Load and display configuration
    manager = ConfigManager()
    print("Current Configuration:")
    print(json.dumps(manager.export(), indent=2))
