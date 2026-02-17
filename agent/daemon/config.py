"""
Daemon Configuration System for LuminaGuard.

This module provides configuration management for daemon mode:
- YAML/JSON configuration file support
- Environment variable overrides
- Hot-reload configuration
- Configuration validation and defaults
- CLI flags for common options

Part of: luminaguard-0va.5 - Daemon Configuration System
"""

import os
import sys
import json
import logging
from dataclasses import dataclass, field, asdict
from typing import Optional, Dict, Any, List, Callable, Union
from pathlib import Path
from enum import Enum
import argparse
import threading
from datetime import timedelta

logger = logging.getLogger(__name__)


class LogLevel(Enum):
    """Log level enum."""
    DEBUG = "debug"
    INFO = "info"
    WARNING = "warning"
    ERROR = "error"
    CRITICAL = "critical"


@dataclass
class HealthConfig:
    """Health check configuration."""
    ping_interval: int = 60
    timeout: int = 10
    max_failures: int = 3
    ping_url: Optional[str] = None
    http_port: int = 8080
    http_enabled: bool = True
    vsock_cid: int = 2
    vsock_port: int = 5050
    vsock_enabled: bool = True


@dataclass
class SchedulerConfig:
    """Job scheduler configuration."""
    persistence_path: str = "~/.luminaguard/jobs.json"
    default_timeout: int = 300
    max_concurrent: int = 5
    persistence_enabled: bool = True


@dataclass
class LifecycleConfig:
    """Lifecycle configuration."""
    pid_file: str = "~/.luminaguard/daemon.pid"
    state_file: str = "~/.luminaguard/state.json"
    log_file: str = "~/.luminaguard/daemon.log"
    user: Optional[str] = None
    group: Optional[str] = None
    working_dir: str = "~"
    auto_restart: bool = True
    max_restart_attempts: int = 3
    restart_delay: int = 5
    shutdown_timeout: int = 30


@dataclass
class MessengerConfig:
    """Messenger configuration."""
    discord_enabled: bool = False
    discord_token: Optional[str] = None
    discord_channel_id: Optional[str] = None
    telegram_enabled: bool = False
    telegram_token: Optional[str] = None
    telegram_chat_id: Optional[str] = None
    whatsapp_enabled: bool = False
    whatsapp_phone: Optional[str] = None
    whatsapp_api_url: Optional[str] = None


@dataclass
class Config:
    """
    Main daemon configuration.
    
    This is the root configuration object that contains all
    sub-configurations for the daemon.
    """
    # Daemon settings
    daemon_name: str = "luminaguard"
    version: str = "0.1.0"
    debug: bool = False
    
    # Logging
    log_level: str = "info"
    log_file: Optional[str] = None
    
    # Sub-configurations
    health: HealthConfig = field(default_factory=HealthConfig)
    scheduler: SchedulerConfig = field(default_factory=SchedulerConfig)
    lifecycle: LifecycleConfig = field(default_factory=LifecycleConfig)
    messenger: MessengerConfig = field(default_factory=MessengerConfig)
    
    # API settings
    api_host: str = "127.0.0.1"
    api_port: int = 8081
    api_enabled: bool = True
    
    # MCP settings
    mcp_servers: Dict[str, Dict[str, Any]] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        result = asdict(self)
        return result
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Config":
        """Create from dictionary."""
        config = cls()
        
        # Top-level settings
        for key in ["daemon_name", "version", "debug", "log_level", "log_file", 
                    "api_host", "api_port", "api_enabled"]:
            if key in data:
                setattr(config, key, data[key])
        
        # Sub-configurations
        if "health" in data:
            config.health = HealthConfig(**data["health"])
        if "scheduler" in data:
            config.scheduler = SchedulerConfig(**data["scheduler"])
        if "lifecycle" in data:
            config.lifecycle = LifecycleConfig(**data["lifecycle"])
        if "messenger" in data:
            config.messenger = MessengerConfig(**data["messenger"])
        if "mcp_servers" in data:
            config.mcp_servers = data["mcp_servers"]
        
        return config


class ConfigSource(Enum):
    """Configuration source priority."""
    DEFAULT = 0
    ENV = 1
    FILE = 2
    CLI = 3


class ConfigManager:
    """
    Configuration manager for daemon mode.
    
    Provides:
    - YAML/JSON configuration file support
    - Environment variable overrides
    - Hot-reload configuration
    - Configuration validation
    - CLI argument parsing
    """
    
    # Environment variable prefix
    ENV_PREFIX = "LUMINAGUARD_"
    
    # Default config file paths
    DEFAULT_CONFIG_PATHS = [
        "~/.luminaguard/config.json",
        "~/.luminaguard/config.yaml",
        "/etc/luminaguard/config.json",
        "/etc/luminaguard/config.yaml",
    ]
    
    def __init__(self, config_path: Optional[str] = None):
        """
        Initialize configuration manager.
        
        Args:
            config_path: Path to config file (optional)
        """
        self._config_path = config_path
        self._config: Optional[Config] = None
        self._lock = threading.RLock()
        self._watchers: List[Callable[[Config], None]] = []
        self._file_watcher: Optional[threading.Thread] = None
        self._last_modified: float = 0
    
    @property
    def config(self) -> Config:
        """Get current configuration."""
        with self._lock:
            if self._config is None:
                self._config = self._load_config()
            return self._config
    
    def _load_config(self) -> Config:
        """Load configuration from all sources."""
        # Start with defaults
        config = Config()
        
        # Load from file
        if self._config_path:
            config = self._merge_config(config, self._load_file(self._config_path))
        else:
            for path in self.DEFAULT_CONFIG_PATHS:
                if os.path.exists(os.path.expanduser(path)):
                    config = self._merge_config(config, self._load_file(path))
                    break
        
        # Override with environment variables
        config = self._merge_config(config, self._load_env())
        
        return config
    
    def _load_file(self, path: str) -> Dict[str, Any]:
        """Load configuration from file."""
        path = os.path.expanduser(path)
        
        try:
            with open(path, "r") as f:
                if path.endswith(".yaml") or path.endswith(".yml"):
                    try:
                        import yaml
                        return yaml.safe_load(f) or {}
                    except ImportError:
                        logger.warning("PyYAML not installed, skipping YAML config")
                        return {}
                else:
                    return json.load(f)
        except Exception as e:
            logger.warning(f"Failed to load config from {path}: {e}")
            return {}
    
    def _load_env(self) -> Dict[str, Any]:
        """Load configuration from environment variables."""
        config = {}
        
        for key, value in os.environ.items():
            if not key.startswith(self.ENV_PREFIX):
                continue
            
            # Parse key path
            key = key[len(self.ENV_PREFIX):].lower()
            parts = key.split("_")
            
            # Build nested structure
            current = config
            for part in parts[:-1]:
                if part not in current:
                    current[part] = {}
                current = current[part]
            
            # Set value
            value = self._parse_env_value(value)
            current[parts[-1]] = value
        
        return config
    
    def _parse_env_value(self, value: str) -> Any:
        """Parse environment variable value."""
        # Boolean
        if value.lower() in ("true", "yes", "1"):
            return True
        if value.lower() in ("false", "no", "0"):
            return False
        
        # Number
        try:
            if "." in value:
                return float(value)
            return int(value)
        except ValueError:
            pass
        
        # String
        return value
    
    def _merge_config(self, base: Config, override: Dict[str, Any]) -> Config:
        """Merge override config into base."""
        base_dict = base.to_dict()
        
        # Deep merge
        def merge(base: Dict, over: Dict) -> Dict:
            result = base.copy()
            for key, value in over.items():
                if key in result and isinstance(result[key], dict) and isinstance(value, dict):
                    result[key] = merge(result[key], value)
                else:
                    result[key] = value
            return result
        
        merged = merge(base_dict, override)
        return Config.from_dict(merged)
    
    def reload(self) -> Config:
        """Reload configuration from sources."""
        with self._lock:
            old_config = self._config
            self._config = self._load_config()
            
            # Notify watchers
            for watcher in self._watchers:
                try:
                    watcher(self._config)
                except Exception as e:
                    logger.error(f"Error in config watcher: {e}")
            
            return self._config
    
    def watch(self, callback: Callable[[Config], None]) -> None:
        """
        Register a callback for config changes.
        
        Args:
            callback: Called when config is reloaded
        """
        self._watchers.append(callback)
    
    def start_file_watcher(self, interval: float = 5.0) -> None:
        """
        Start watching config file for changes.
        
        Args:
            interval: Check interval in seconds
        """
        if self._file_watcher is not None:
            return
        
        def watcher():
            while True:
                try:
                    if self._config_path:
                        path = os.path.expanduser(self._config_path)
                        if os.path.exists(path):
                            mtime = os.path.getmtime(path)
                            if mtime > self._last_modified:
                                self._last_modified = mtime
                                logger.info("Config file changed, reloading...")
                                self.reload()
                except Exception as e:
                    logger.error(f"Error in file watcher: {e}")
                
                import time
                time.sleep(interval)
        
        self._file_watcher = threading.Thread(target=watcher, daemon=True)
        self._file_watcher.start()
    
    def stop_file_watcher(self) -> None:
        """Stop watching config file."""
        if self._file_watcher:
            self._file_watcher = None
    
    def get(self, path: str, default: Any = None) -> Any:
        """
        Get config value by path.
        
        Args:
            path: Dot-separated path (e.g., "health.http_port")
            default: Default value if not found
            
        Returns:
            Config value or default
        """
        config = self.config.to_dict()
        
        for key in path.split("."):
            if isinstance(config, dict) and key in config:
                config = config[key]
            else:
                return default
        
        return config
    
    def set(self, path: str, value: Any) -> None:
        """
        Set config value by path.
        
        Args:
            path: Dot-separated path
            value: Value to set
        """
        with self._lock:
            config_dict = self.config.to_dict()
            
            # Navigate to nested location
            parts = path.split(".")
            current = config_dict
            for key in parts[:-1]:
                if key not in current:
                    current[key] = {}
                current = current[key]
            
            # Set value
            current[parts[-1]] = value
            
            # Rebuild config
            self._config = Config.from_dict(config_dict)


class ConfigCLI:
    """CLI argument parser for daemon configuration."""
    
    def __init__(self, prog: Optional[str] = None):
        """Initialize CLI parser."""
        self._parser = argparse.ArgumentParser(prog=prog or "luminaguard")
        self._setup_args()
    
    def _setup_args(self) -> None:
        """Set up CLI arguments."""
        # Daemon options
        daemon_group = self._parser.add_argument_group("Daemon Options")
        daemon_group.add_argument(
            "-d", "--daemon",
            action="store_true",
            help="Run as daemon"
        )
        daemon_group.add_argument(
            "--pid-file",
            default=None,
            help="PID file path"
        )
        daemon_group.add_argument(
            "--config",
            default=None,
            help="Config file path"
        )
        
        # Logging options
        log_group = self._parser.add_argument_group("Logging Options")
        log_group.add_argument(
            "-v", "--verbose",
            action="count",
            help="Increase verbosity (-v, -vv, -vvv)"
        )
        log_group.add_argument(
            "--log-level",
            choices=["debug", "info", "warning", "error", "critical"],
            help="Set log level"
        )
        log_group.add_argument(
            "--log-file",
            help="Log file path"
        )
        
        # API options
        api_group = self._parser.add_argument_group("API Options")
        api_group.add_argument(
            "--api-host",
            default=None,
            help="API host"
        )
        api_group.add_argument(
            "--api-port",
            type=int,
            help="API port"
        )
        api_group.add_argument(
            "--no-api",
            action="store_true",
            help="Disable API"
        )
        
        # Health options
        health_group = self._parser.add_argument_group("Health Check Options")
        health_group.add_argument(
            "--health-port",
            type=int,
            help="Health check HTTP port"
        )
        health_group.add_argument(
            "--health-ping-url",
            help="External health ping URL"
        )
        
        # Misc options
        self._parser.add_argument(
            "--version",
            action="store_true",
            help="Show version"
        )
    
    def parse_args(self, args: Optional[List[str]] = None) -> argparse.Namespace:
        """Parse CLI arguments."""
        return self._parser.parse_args(args)
    
    def to_config_overrides(self, args: argparse.Namespace) -> Dict[str, Any]:
        """
        Convert parsed args to config overrides.
        
        Returns:
            Dictionary of config overrides
        """
        overrides = {}
        
        if args.verbose:
            levels = ["warning", "info", "debug"]
            idx = min(args.verbose, len(levels) - 1)
            overrides["log_level"] = levels[idx]
        
        if args.log_level:
            overrides["log_level"] = args.log_level
        
        if args.log_file:
            overrides["log_file"] = args.log_file
        
        if args.api_host:
            overrides["api_host"] = args.api_host
        
        if args.api_port:
            overrides["api_port"] = args.api_port
        
        if args.no_api:
            overrides["api_enabled"] = False
        
        if args.health_port:
            overrides.setdefault("health", {})["http_port"] = args.health_port
        
        if args.health_ping_url:
            overrides.setdefault("health", {})["ping_url"] = args.health_ping_url
        
        if args.pid_file:
            overrides.setdefault("lifecycle", {})["pid_file"] = args.pid_file
        
        return overrides


def create_config_manager(
    config_path: Optional[str] = None,
    cli_args: Optional[List[str]] = None,
) -> ConfigManager:
    """
    Create a configured config manager.
    
    Args:
        config_path: Path to config file
        cli_args: CLI arguments to parse
        
    Returns:
        Configured ConfigManager
    """
    # Get config path from CLI if not provided
    if config_path is None and cli_args is not None:
        parser = ConfigCLI()
        args = parser.parse_args(cli_args)
        if args.config:
            config_path = args.config
    
    # Create config manager
    manager = ConfigManager(config_path=config_path)
    
    # Apply CLI overrides if provided
    if cli_args is not None:
        parser = ConfigCLI()
        args = parser.parse_args(cli_args)
        overrides = parser.to_config_overrides(args)
        
        if overrides:
            config = manager.config
            config_dict = config.to_dict()
            
            # Deep merge
            def merge(base: Dict, over: Dict) -> Dict:
                result = base.copy()
                for key, value in over.items():
                    if key in result and isinstance(result[key], dict) and isinstance(value, dict):
                        result[key] = merge(result[key], value)
                    else:
                        result[key] = value
                return result
            
            merged = merge(config_dict, overrides)
            manager._config = Config.from_dict(merged)
    
    return manager
