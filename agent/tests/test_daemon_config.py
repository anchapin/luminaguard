"""
Tests for Daemon Configuration System

Tests configuration loading, validation, and management
"""

import json
import os
import tempfile
from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest

from daemon_config import (
    DaemonConfig,
    ConfigLoader,
    ConfigValidator,
    ConfigManager,
    LogLevel,
    LoggingConfig,
    SecurityConfig,
    VmConfig,
    McpConfig,
)


class TestDaemonConfig:
    """Test DaemonConfig dataclass"""
    
    def test_default_config(self):
        """Test default configuration values"""
        config = DaemonConfig()
        
        assert config.name == "luminaguard"
        assert config.execution_mode == "host"
        assert config.port == 8000
        assert config.host == "127.0.0.1"
        assert config.enable_daemon is False
        assert config.logging.level == LogLevel.INFO
        assert config.security.approval_required_by_default is True
    
    def test_config_directories_created(self):
        """Test that default directories are set"""
        config = DaemonConfig()
        
        assert config.config_dir is not None
        assert config.log_dir is not None
        assert config.data_dir is not None
        assert ".luminaguard" in config.config_dir
    
    def test_config_timestamps(self):
        """Test that timestamps are set"""
        config = DaemonConfig()
        
        assert config.created_at is not None
        assert config.last_modified is not None
    
    def test_logging_config(self):
        """Test LoggingConfig dataclass"""
        logging_config = LoggingConfig(
            level=LogLevel.DEBUG,
            pretty_print=True
        )
        
        assert logging_config.level == LogLevel.DEBUG
        assert logging_config.pretty_print is True
        assert logging_config.max_size_mb == 100
    
    def test_security_config(self):
        """Test SecurityConfig dataclass"""
        security_config = SecurityConfig(
            approval_timeout_seconds=600
        )
        
        assert security_config.approval_timeout_seconds == 600
        assert security_config.approval_required_by_default is True
    
    def test_vm_config(self):
        """Test VmConfig dataclass"""
        vm_config = VmConfig(
            vcpu_count=4,
            memory_mb=1024
        )
        
        assert vm_config.vcpu_count == 4
        assert vm_config.memory_mb == 1024
        assert vm_config.timeout_seconds == 60
    
    def test_mcp_config(self):
        """Test McpConfig dataclass"""
        mcp_config = McpConfig(
            servers=[{"name": "filesystem", "command": "npx"}]
        )
        
        assert len(mcp_config.servers) == 1
        assert mcp_config.servers[0]["name"] == "filesystem"


class TestConfigLoader:
    """Test ConfigLoader"""
    
    def test_find_config_no_file(self):
        """Test finding config when no file exists"""
        with patch("pathlib.Path.exists", return_value=False):
            result = ConfigLoader._find_config()
            assert result is None
    
    def test_load_default_config(self):
        """Test loading default config when file not found"""
        loader = ConfigLoader("/nonexistent/path.yaml")
        config = loader.load()
        
        assert isinstance(config, DaemonConfig)
        assert config.name == "luminaguard"
    
    def test_load_json_config(self):
        """Test loading JSON configuration file"""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump({
                "name": "test-daemon",
                "port": 9000,
                "execution_mode": "vm",
                "logging": {
                    "level": "debug"
                }
            }, f)
            f.flush()
            
            try:
                loader = ConfigLoader(f.name)
                config = loader.load()
                
                assert config.name == "test-daemon"
                assert config.port == 9000
                assert config.execution_mode == "vm"
            finally:
                os.unlink(f.name)
    
    def test_load_yaml_config(self):
        """Test loading YAML configuration file"""
        pytest.importorskip("yaml")
        
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            f.write("""
name: yaml-daemon
port: 8080
execution_mode: vm
logging:
  level: warning
""")
            f.flush()
            
            try:
                loader = ConfigLoader(f.name)
                config = loader.load()
                
                assert config.name == "yaml-daemon"
                assert config.port == 8080
                assert config.execution_mode == "vm"
            finally:
                os.unlink(f.name)
    
    def test_apply_env_overrides(self):
        """Test environment variable overrides"""
        with patch.dict(os.environ, {
            "LUMINAGUARD_PORT": "9999",
            "LUMINAGUARD_MODE": "vm",
            "LUMINAGUARD_DAEMON": "true",
        }):
            loader = ConfigLoader()
            config = loader.load()
            
            assert config.port == 9999
            assert config.execution_mode == "vm"
            assert config.enable_daemon is True
    
    def test_save_json_config(self):
        """Test saving configuration to JSON"""
        config = DaemonConfig(name="test-save", port=8888)
        loader = ConfigLoader()
        
        with tempfile.TemporaryDirectory() as tmpdir:
            save_path = Path(tmpdir) / "config.json"
            saved_path = loader.save(config, str(save_path))
            
            assert Path(saved_path).exists()
            
            # Verify content
            with open(saved_path) as f:
                saved_config = json.load(f)
            
            assert saved_config["name"] == "test-save"
            assert saved_config["port"] == 8888
    
    def test_save_yaml_config(self):
        """Test saving configuration to YAML"""
        pytest.importorskip("yaml")
        
        config = DaemonConfig(name="yaml-save", port=7777)
        loader = ConfigLoader()
        
        with tempfile.TemporaryDirectory() as tmpdir:
            save_path = Path(tmpdir) / "config.yaml"
            saved_path = loader.save(config, str(save_path))
            
            assert Path(saved_path).exists()


class TestConfigValidator:
    """Test ConfigValidator"""
    
    def test_valid_config(self):
        """Test validation of valid configuration"""
        config = DaemonConfig()
        errors = ConfigValidator.validate(config)
        
        assert len(errors) == 0
    
    def test_invalid_log_level(self):
        """Test invalid logging level"""
        config = DaemonConfig()
        config.logging.level = "invalid"
        errors = ConfigValidator.validate(config)
        
        assert len(errors) > 0
        assert any("Invalid log level" in e for e in errors)
    
    def test_invalid_memory(self):
        """Test invalid VM memory"""
        config = DaemonConfig()
        config.vm.memory_mb = 64  # Too low
        errors = ConfigValidator.validate(config)
        
        assert len(errors) > 0
        assert any("memory" in e.lower() for e in errors)
    
    def test_invalid_port(self):
        """Test invalid port number"""
        config = DaemonConfig()
        config.port = 70000  # Out of range
        errors = ConfigValidator.validate(config)
        
        assert len(errors) > 0
        assert any("port" in e.lower() for e in errors)
    
    def test_invalid_execution_mode(self):
        """Test invalid execution mode"""
        config = DaemonConfig()
        config.execution_mode = "invalid-mode"
        errors = ConfigValidator.validate(config)
        
        assert len(errors) > 0
        assert any("execution_mode" in e for e in errors)


class TestConfigManager:
    """Test ConfigManager"""
    
    def test_create_manager(self):
        """Test creating configuration manager"""
        manager = ConfigManager()
        
        assert manager.config is not None
        assert isinstance(manager.config, DaemonConfig)
    
    def test_get_value(self):
        """Test getting configuration values"""
        manager = ConfigManager()
        
        assert manager.get("port") == 8000
        assert manager.get("execution_mode") == "host"
        assert manager.get("logging.level") == LogLevel.INFO
    
    def test_get_with_default(self):
        """Test getting value with default"""
        manager = ConfigManager()
        
        result = manager.get("nonexistent.key", "default_value")
        assert result == "default_value"
    
    def test_set_value(self):
        """Test setting configuration values"""
        manager = ConfigManager()
        
        manager.set("port", 9999)
        assert manager.get("port") == 9999
    
    def test_set_nested_value(self):
        """Test setting nested configuration values"""
        manager = ConfigManager()
        
        manager.set("logging.level", LogLevel.DEBUG)
        assert manager.get("logging.level") == LogLevel.DEBUG
    
    def test_export_config(self):
        """Test exporting configuration as dictionary"""
        manager = ConfigManager()
        exported = manager.export()
        
        assert exported["name"] == "luminaguard"
        assert exported["port"] == 8000
        assert exported["logging"]["level"] == "info"
    
    def test_reload_config(self):
        """Test reloading configuration"""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump({
                "name": "original",
                "port": 8000
            }, f)
            f.flush()
            
            try:
                manager = ConfigManager(f.name)
                original_name = manager.get("name")
                
                # Update the file
                with open(f.name, "w") as f2:
                    json.dump({
                        "name": "updated",
                        "port": 9999
                    }, f2)
                
                manager.reload()
                
                assert manager.get("name") == "updated"
                assert manager.get("port") == 9999
            finally:
                os.unlink(f.name)


class TestLogLevelEnum:
    """Test LogLevel enum"""
    
    def test_log_level_values(self):
        """Test log level enum values"""
        assert LogLevel.DEBUG.value == "debug"
        assert LogLevel.INFO.value == "info"
        assert LogLevel.WARNING.value == "warning"
        assert LogLevel.ERROR.value == "error"
        assert LogLevel.CRITICAL.value == "critical"
    
    def test_log_level_from_string(self):
        """Test creating log level from string"""
        level = LogLevel("debug")
        assert level == LogLevel.DEBUG
        
        level = LogLevel("warning")
        assert level == LogLevel.WARNING


class TestConfigIntegration:
    """Integration tests for configuration system"""
    
    def test_full_workflow(self):
        """Test complete configuration workflow"""
        with tempfile.TemporaryDirectory() as tmpdir:
            config_file = Path(tmpdir) / "config.json"
            
            # Create initial config
            config = DaemonConfig(
                name="integration-test",
                port=8765,
                execution_mode="vm"
            )
            
            # Save it
            loader = ConfigLoader()
            loader.save(config, str(config_file))
            
            # Load it back
            loader2 = ConfigLoader(str(config_file))
            loaded_config = loader2.load()
            
            assert loaded_config.name == "integration-test"
            assert loaded_config.port == 8765
            assert loaded_config.execution_mode == "vm"
            
            # Validate it
            errors = ConfigValidator.validate(loaded_config)
            assert len(errors) == 0
    
    def test_config_with_env_overrides(self):
        """Test configuration with environment overrides"""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump({
                "name": "env-test",
                "port": 8000,
                "execution_mode": "host"
            }, f)
            f.flush()
            
            try:
                with patch.dict(os.environ, {
                    "LUMINAGUARD_PORT": "9999",
                    "LUMINAGUARD_MODE": "vm"
                }):
                    loader = ConfigLoader(f.name)
                    config = loader.load()
                    
                    assert config.name == "env-test"
                    assert config.port == 9999  # Overridden
                    assert config.execution_mode == "vm"  # Overridden
            finally:
                os.unlink(f.name)
