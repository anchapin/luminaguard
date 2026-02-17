"""
Daemon Configuration System Module.

This module provides configuration management for daemon mode:
- YAML/JSON configuration file support
- Environment variable overrides
- Hot-reload configuration without restart
- Configuration validation and defaults
- CLI flags for common options

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
Issue: #445 - Daemon Configuration System
"""

from __future__ import annotations

import os
import json
import logging
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional, Dict, Any, List, Union
from enum import Enum
import copy
import threading
import time

try:
    from watchdog.observers import Observer
    from watchdog.events import FileSystemEventHandler
    WATCHDOG_AVAILABLE = True
except ImportError:
    WATCHDOG_AVAILABLE = False
    Observer = object
    FileSystemEventHandler = object

logger = logging.getLogger(__name__)


class ConfigFormat(Enum):
    """Configuration file format"""
    YAML = "yaml"
    JSON = "json"


@dataclass
class HealthConfigData:
    """Health check configuration"""
    enabled: bool = True
    interval_seconds: int = 60
    timeout_seconds: int = 10
    endpoint: str = "/health"
    http_port: Optional[int] = None
    vsock_port: Optional[int] = None


@dataclass
class SchedulerConfigData:
    """Job scheduler configuration"""
    enabled: bool = True
    max_concurrent_jobs: int = 4
    default_timeout_seconds: int = 300
    retry_failed_jobs: bool = True
    max_retries: int = 3


@dataclass
class LoggingConfigData:
    """Logging configuration"""
    level: str = "INFO"
    format: str = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    file: Optional[str] = None
    max_bytes: int = 10_000_000  # 10MB
    backup_count: int = 5
    syslog_enabled: bool = False
    syslog_address: str = "/dev/log"


@dataclass
class LifecycleConfigData:
    """Lifecycle configuration"""
    pid_file: str = "/var/run/luminaguard.pid"
    state_file: str = "/var/run/luminaguard.state"
    auto_restart: bool = True
    max_retries: int = 3
    graceful_shutdown_timeout: float = 30.0


@dataclass
class MessengerConfigData:
    """Messenger configuration"""
    telegram_enabled: bool = False
    discord_enabled: bool = False
    whatsapp_enabled: bool = False
    slack_enabled: bool = False


@dataclass
class StateConfigData:
    """State persistence configuration"""
    enabled: bool = True
    directory: str = "/var/lib/luminaguard/state"
    encryption_enabled: bool = False
    encryption_key: Optional[str] = None
    max_history_messages: int = 1000
    snapshot_interval_seconds: int = 300


@dataclass
class DaemonConfig:
    """
    Main daemon configuration class.
    
    This is the root configuration object that contains all
    sub-configurations for the daemon.
    """
    # General settings
    name: str = "luminaguard"
    version: str = "0.1.0"
    working_directory: str = "/opt/luminaguard"
    
    # Server settings
    host: str = "127.0.0.1"
    port: int = 8080
    
    # Sub-configurations
    health: HealthConfigData = field(default_factory=HealthConfigData)
    scheduler: SchedulerConfigData = field(default_factory=SchedulerConfigData)
    logging: LoggingConfigData = field(default_factory=LoggingConfigData)
    lifecycle: LifecycleConfigData = field(default_factory=LifecycleConfigData)
    messenger: MessengerConfigData = field(default_factory=MessengerConfigData)
    state: StateConfigData = field(default_factory=StateConfigData)
    
    # Runtime settings (not persisted)
    _config_file: Optional[str] = field(default=None, repr=False)
    _config_format: ConfigFormat = field(default=ConfigFormat.YAML, repr=False)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert configuration to dictionary"""
        result = {}
        for key, value in asdict(self).items():
            if not key.startswith('_'):
                if hasattr(value, '__dataclass_fields__'):
                    result[key] = asdict(value)
                else:
                    result[key] = value
        return result
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> DaemonConfig:
        """Create configuration from dictionary"""
        # Extract known sub-configurations
        sub_configs = {}
        main_config = {}
        
        for key, value in data.items():
            if key == 'health' and isinstance(value, dict):
                sub_configs['health'] = HealthConfigData(**value)
            elif key == 'scheduler' and isinstance(value, dict):
                sub_configs['scheduler'] = SchedulerConfigData(**value)
            elif key == 'logging' and isinstance(value, dict):
                sub_configs['logging'] = LoggingConfigData(**value)
            elif key == 'lifecycle' and isinstance(value, dict):
                sub_configs['lifecycle'] = LifecycleConfigData(**value)
            elif key == 'messenger' and isinstance(value, dict):
                sub_configs['messenger'] = MessengerConfigData(**value)
            elif key == 'state' and isinstance(value, dict):
                sub_configs['state'] = StateConfigData(**value)
            else:
                main_config[key] = value
        
        return cls(**{**main_config, **sub_configs})
    
    def validate(self) -> List[str]:
        """
        Validate the configuration.
        
        Returns:
            List of validation error messages (empty if valid)
        """
        errors = []
        
        # Validate port
        if not 1 <= self.port <= 65535:
            errors.append(f"Invalid port: {self.port} (must be 1-65535)")
        
        # Validate health config
        if self.health.interval_seconds < 1:
            errors.append("health.interval_seconds must be >= 1")
        
        if self.health.timeout_seconds < 1:
            errors.append("health.timeout_seconds must be >= 1")
        
        # Validate scheduler config
        if self.scheduler.max_concurrent_jobs < 1:
            errors.append("scheduler.max_concurrent_jobs must be >= 1")
        
        # Validate logging config
        valid_levels = {"DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"}
        if self.logging.level.upper() not in valid_levels:
            errors.append(f"logging.level must be one of: {valid_levels}")
        
        # Validate lifecycle config
        if self.lifecycle.max_retries < 0:
            errors.append("lifecycle.max_retries must be >= 0")
        
        if self.lifecycle.graceful_shutdown_timeout < 0:
            errors.append("lifecycle.graceful_shutdown_timeout must be >= 0")
        
        # Validate state config
        if self.state.max_history_messages < 0:
            errors.append("state.max_history_messages must be >= 0")
        
        return errors


class ConfigLoader:
    """Loads configuration from files and environment variables"""
    
    # Environment variable prefix
    ENV_PREFIX = "LUMINAGUARD_"
    
    # Default configuration file paths (in order of preference)
    DEFAULT_CONFIG_PATHS = [
        "/etc/luminaguard/config.yaml",
        "/etc/luminaguard/config.json",
        "~/.config/luminaguard/config.yaml",
        "~/.config/luminaguard/config.json",
        "./config.yaml",
        "./config.json",
    ]
    
    def __init__(self):
        self._config: Optional[DaemonConfig] = None
    
    def load(
        self,
        config_file: Optional[str] = None,
        config_format: Optional[ConfigFormat] = None,
    ) -> DaemonConfig:
        """
        Load configuration from file and environment variables.
        
        Args:
            config_file: Path to configuration file (optional)
            config_format: Configuration format (auto-detected if not provided)
            
        Returns:
            Loaded configuration
        """
        # Start with defaults
        config = DaemonConfig()
        
        # Try to load from file
        if config_file:
            config = self._load_from_file(config_file, config_format)
        else:
            # Try default paths
            for path in self.DEFAULT_CONFIG_PATHS:
                expanded = os.path.expanduser(path)
                if os.path.exists(expanded):
                    config = self._load_from_file(expanded)
                    break
        
        # Apply environment variable overrides
        config = self._apply_env_overrides(config)
        
        self._config = config
        return config
    
    def _load_from_file(
        self,
        path: str,
        config_format: Optional[ConfigFormat] = None,
    ) -> DaemonConfig:
        """Load configuration from a file"""
        path = Path(path)
        
        if not path.exists():
            raise FileNotFoundError(f"Configuration file not found: {path}")
        
        # Determine format
        if config_format is None:
            if path.suffix in ('.yaml', '.yml'):
                config_format = ConfigFormat.YAML
            elif path.suffix == '.json':
                config_format = ConfigFormat.JSON
            else:
                raise ValueError(f"Unknown configuration format: {path.suffix}")
        
        # Load based on format
        content = path.read_text()
        
        if config_format == ConfigFormat.YAML:
            try:
                import yaml
                data = yaml.safe_load(content)
            except ImportError:
                logger.warning("PyYAML not installed, falling back to JSON")
                data = json.loads(content)
        else:
            data = json.loads(content)
        
        # Create config and set file path
        config = DaemonConfig.from_dict(data)
        config._config_file = str(path)
        config._config_format = config_format
        
        return config
    
    def _apply_env_overrides(self, config: DaemonConfig) -> DaemonConfig:
        """Apply environment variable overrides to configuration"""
        # Map of environment variables to config paths
        env_mappings = {
            # General
            "LUMINAGUARD_NAME": ("name", str),
            "LUMINAGUARD_VERSION": ("version", str),
            "LUMINAGUARD_WORKING_DIRECTORY": ("working_directory", str),
            "LUMINAGUARD_HOST": ("host", str),
            "LUMINAGUARD_PORT": ("port", int),
            
            # Health
            "LUMINAGUARD_HEALTH_ENABLED": ("health.enabled", bool),
            "LUMINAGUARD_HEALTH_INTERVAL": ("health.interval_seconds", int),
            "LUMINAGUARD_HEALTH_TIMEOUT": ("health.timeout_seconds", int),
            "LUMINAGUARD_HEALTH_ENDPOINT": ("health.endpoint", str),
            "LUMINAGUARD_HEALTH_HTTP_PORT": ("health.http_port", int),
            
            # Scheduler
            "LUMINAGUARD_SCHEDULER_ENABLED": ("scheduler.enabled", bool),
            "LUMINAGUARD_SCHEDULER_MAX_JOBS": ("scheduler.max_concurrent_jobs", int),
            "LUMINAGUARD_SCHEDULER_TIMEOUT": ("scheduler.default_timeout_seconds", int),
            
            # Logging
            "LUMINAGUARD_LOG_LEVEL": ("logging.level", str),
            "LUMINAGUARD_LOG_FILE": ("logging.file", str),
            
            # Lifecycle
            "LUMINAGUARD_PID_FILE": ("lifecycle.pid_file", str),
            "LUMINAGUARD_STATE_FILE": ("lifecycle.state_file", str),
            "LUMINAGUARD_AUTO_RESTART": ("lifecycle.auto_restart", bool),
            "LUMINAGUARD_MAX_RETRIES": ("lifecycle.max_retries", int),
            "LUMINAGUARD_SHUTDOWN_TIMEOUT": ("lifecycle.graceful_shutdown_timeout", float),
            
            # State
            "LUMINAGUARD_STATE_DIR": ("state.directory", str),
            "LUMINAGUARD_STATE_ENCRYPTION": ("state.encryption_enabled", bool),
            "LUMINAGUARD_STATE_MAX_MESSAGES": ("state.max_history_messages", int),
        }
        
        config_dict = config.to_dict()
        
        for env_var, (path, type_fn) in env_mappings.items():
            value = os.environ.get(env_var)
            if value is not None:
                try:
                    # Convert value to appropriate type
                    if type_fn == bool:
                        value = value.lower() in ('true', '1', 'yes', 'on')
                    else:
                        value = type_fn(value)
                    
                    # Set nested value
                    self._set_nested(config_dict, path, value)
                except Exception as e:
                    logger.warning(f"Invalid value for {env_var}: {e}")
        
        return DaemonConfig.from_dict(config_dict)
    
    def _set_nested(self, d: Dict, path: str, value: Any) -> None:
        """Set a nested dictionary value using dot notation"""
        keys = path.split('.')
        current = d
        
        for key in keys[:-1]:
            if key not in current:
                current[key] = {}
            current = current[key]
        
        current[keys[-1]] = value
    
    @property
    def config(self) -> Optional[DaemonConfig]:
        """Get the current configuration"""
        return self._config


class ConfigWatcher(FileSystemEventHandler):
    """Watches configuration file for changes and triggers reload"""
    
    def __init__(
        self,
        config_loader: ConfigLoader,
        on_reload: Optional[callable] = None,
    ):
        super().__init__()
        self.config_loader = config_loader
        self.on_reload = on_reload
        self._last_reload = 0
        self._debounce_seconds = 1.0
    
    def on_modified(self, event):
        if event.is_directory:
            return
        
        path = Path(event.src_path)
        if self.config_loader._config and self.config_loader._config._config_file:
            config_path = Path(self.config_loader._config._config_file)
            if path.resolve() == config_path.resolve():
                self._trigger_reload()
    
    def _trigger_reload(self):
        """Debounce and trigger reload"""
        now = time.time()
        if now - self._last_reload < self._debounce_seconds:
            return
        
        self._last_reload = now
        
        try:
            logger.info("Configuration file changed, reloading...")
            new_config = self.config_loader.load(
                self.config_loader._config._config_file,
                self.config_loader._config._config_format,
            )
            
            if self.on_reload:
                self.on_reload(new_config)
                
            logger.info("Configuration reloaded successfully")
        except Exception as e:
            logger.error(f"Failed to reload configuration: {e}")


class HotReloadConfig:
    """
    Configuration manager with hot-reload support.
    
    Watches the configuration file and automatically reloads
    when changes are detected.
    """
    
    def __init__(
        self,
        config_loader: Optional[ConfigLoader] = None,
    ):
        self.config_loader = config_loader or ConfigLoader()
        self._observer: Optional[Observer] = None
        self._lock = threading.RLock()
        self._current_config: Optional[DaemonConfig] = None
        self._reload_callbacks: List[callable] = []
    
    def load(
        self,
        config_file: Optional[str] = None,
        config_format: Optional[ConfigFormat] = None,
    ) -> DaemonConfig:
        """Load configuration and start watching for changes"""
        self._current_config = self.config_loader.load(config_file, config_format)
        return self._current_config
    
    def start_watching(
        self,
        on_reload: Optional[callable] = None,
    ) -> None:
        """Start watching configuration file for changes"""
        if not self._current_config or not self._current_config._config_file:
            logger.warning("No configuration file to watch")
            return
        
        config_path = Path(self._current_config._config_file)
        if not config_path.exists():
            logger.warning(f"Configuration file does not exist: {config_path}")
            return
        
        # Create watcher
        watcher = ConfigWatcher(self.config_loader, on_reload)
        
        # Start observer
        self._observer = Observer()
        self._observer.schedule(
            watcher,
            str(config_path.parent),
            recursive=False,
        )
        self._observer.start()
        
        logger.info(f"Watching configuration file: {config_path}")
    
    def stop_watching(self) -> None:
        """Stop watching configuration file"""
        if self._observer:
            self._observer.stop()
            self._observer.join()
            self._observer = None
            logger.info("Stopped watching configuration file")
    
    def register_reload_callback(self, callback: callable) -> None:
        """Register a callback to be called on configuration reload"""
        self._reload_callbacks.append(callback)
    
    def get_config(self) -> Optional[DaemonConfig]:
        """Get current configuration (thread-safe)"""
        with self._lock:
            return copy.deepcopy(self._current_config)
    
    def update_config(self, config: DaemonConfig) -> None:
        """Update configuration (thread-safe)"""
        with self._lock:
            self._current_config = config
            
            # Call reload callbacks
            for callback in self._reload_callbacks:
                try:
                    callback(config)
                except Exception as e:
                    logger.error(f"Error in reload callback: {e}")


def create_config_loader() -> ConfigLoader:
    """Factory function to create a ConfigLoader"""
    return ConfigLoader()


def create_hot_reload_config() -> HotReloadConfig:
    """Factory function to create a HotReloadConfig"""
    return HotReloadConfig()


def load_config(
    config_file: Optional[str] = None,
    enable_hot_reload: bool = False,
) -> Union[DaemonConfig, HotReloadConfig]:
    """
    Convenience function to load daemon configuration.
    
    Args:
        config_file: Path to configuration file
        enable_hot_reload: Whether to enable hot-reload support
        
    Returns:
        DaemonConfig if hot_reload is False, HotReloadConfig otherwise
    """
    if enable_hot_reload:
        manager = create_hot_reload_config()
        manager.load(config_file)
        manager.start_watching()
        return manager
    else:
        loader = create_config_loader()
        return loader.load(config_file)


# CLI support
def main():
    """CLI for daemon configuration management"""
    import argparse
    
    parser = argparse.ArgumentParser(description="LuminaGuard Configuration Management")
    subparsers = parser.add_subparsers(dest="command", help="Commands")
    
    # Validate command
    validate_parser = subparsers.add_parser("validate", help="Validate configuration file")
    validate_parser.add_argument("config_file", help="Path to configuration file")
    validate_parser.add_argument("--format", choices=["yaml", "json"], help="Configuration format")
    
    # Generate command
    generate_parser = subparsers.add_parser("generate", help="Generate default configuration")
    generate_parser.add_argument("--format", choices=["yaml", "json"], default="yaml", help="Output format")
    generate_parser.add_argument("--output", help="Output file (stdout if not specified)")
    
    # Show command
    show_parser = subparsers.add_parser("show", help="Show current configuration")
    show_parser.add_argument("--format", choices=["yaml", "json"], default="yaml", help="Output format")
    show_parser.add_argument("--output", help="Output file (stdout if not specified)")
    
    args = parser.parse_args()
    
    if args.command == "validate":
        try:
            loader = ConfigLoader()
            config_format = ConfigFormat(args.format) if args.format else None
            config = loader._load_from_file(args.config_file, config_format)
            errors = config.validate()
            
            if errors:
                print("Configuration validation failed:")
                for error in errors:
                    print(f"  - {error}")
                return 1
            else:
                print("Configuration is valid")
                return 0
        except Exception as e:
            print(f"Error: {e}")
            return 1
    
    elif args.command == "generate":
        config = DaemonConfig()
        
        if args.format == "yaml":
            try:
                import yaml
                output = yaml.dump(config.to_dict(), default_flow_style=False)
            except ImportError:
                print("Error: PyYAML not installed")
                return 1
        else:
            output = json.dumps(config.to_dict(), indent=2)
        
        if args.output:
            Path(args.output).write_text(output)
            print(f"Configuration written to {args.output}")
        else:
            print(output)
    
    elif args.command == "show":
        loader = ConfigLoader()
        try:
            config = loader.load()
        except Exception as e:
            print(f"Error loading configuration: {e}")
            return 1
        
        if args.format == "yaml":
            try:
                import yaml
                output = yaml.dump(config.to_dict(), default_flow_style=False)
            except ImportError:
                output = json.dumps(config.to_dict(), indent=2)
        else:
            output = json.dumps(config.to_dict(), indent=2)
        
        if args.output:
            Path(args.output).write_text(output)
            print(f"Configuration written to {args.output}")
        else:
            print(output)
    
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
