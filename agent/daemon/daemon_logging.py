"""
Daemon Logging and Monitoring Module.

This module provides comprehensive logging for daemon mode:
- Structured logging with levels (debug, info, warn, error)
- Log rotation and retention policies
- Integration with external monitoring systems
- Metrics collection (uptime, tasks, errors)
- Log forwarding to external systems (stdout, file, syslog)

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
Issue: #446 - Daemon Logging and Monitoring
"""

from __future__ import annotations

import os
import sys
import json
import logging
import time
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional, Dict, Any, List, Callable
from enum import Enum
from datetime import datetime, timedelta
import threading
import queue
import io
from collections import deque
import statistics

try:
    import syslog
except ImportError:
    syslog = None


class LogLevel(Enum):
    """Log levels"""
    DEBUG = "DEBUG"
    INFO = "INFO"
    WARNING = "WARNING"
    ERROR = "ERROR"
    CRITICAL = "CRITICAL"


class LogFormat(Enum):
    """Log output format"""
    PLAIN = "plain"
    JSON = "json"
    STRUCTURED = "structured"


@dataclass
class LogRotationConfig:
    """Configuration for log rotation"""
    enabled: bool = True
    max_bytes: int = 10_000_000  # 10MB
    backup_count: int = 5
    rotate_on_startup: bool = False
    compress_rotated: bool = True


@dataclass
class MetricsConfig:
    """Configuration for metrics collection"""
    enabled: bool = True
    collection_interval_seconds: int = 60
    retention_minutes: int = 60
    export_interval_seconds: int = 300


@dataclass
class DaemonMetrics:
    """Daemon performance and health metrics"""
    # Uptime metrics
    start_time: float = field(default_factory=time.time)
    last_start_time: Optional[float] = None
    restart_count: int = 0
    
    # Task metrics
    tasks_completed: int = 0
    tasks_failed: int = 0
    tasks_in_progress: int = 0
    
    # Error metrics
    errors_total: int = 0
    errors_by_type: Dict[str, int] = field(default_factory=dict)
    
    # Resource metrics
    memory_usage_mb: float = 0.0
    cpu_usage_percent: float = 0.0
    
    # Request metrics
    requests_total: int = 0
    requests_by_endpoint: Dict[str, int] = field(default_factory=dict)
    
    # Health metrics
    health_check_failures: int = 0
    last_health_check_time: Optional[float] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert metrics to dictionary"""
        data = asdict(self)
        # Calculate uptime
        data['uptime_seconds'] = time.time() - self.start_time
        data['uptime_human'] = str(timedelta(seconds=int(time.time() - self.start_time)))
        return data


class StructuredLogger(logging.Logger):
    """Enhanced logger with structured logging support"""
    
    def __init__(self, name: str, level: int = logging.INFO):
        super().__init__(name, level)
        self._context: Dict[str, Any] = {}
    
    def set_context(self, **kwargs) -> None:
        """Set contextual information for all log messages"""
        self._context.update(kwargs)
    
    def clear_context(self) -> None:
        """Clear all contextual information"""
        self._context = {}
    
    def _format_message(
        self,
        level: int,
        msg: str,
        extra: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Format log message as structured data"""
        data = {
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "level": logging.getLevelName(level),
            "logger": self.name,
            "message": msg,
            "context": dict(self._context),
        }
        
        if extra:
            data["context"].update(extra)
        
        return data
    
    def log(
        self,
        level: int,
        msg: str,
        exc_info: bool = False,
        extra: Optional[Dict[str, Any]] = None,
        **kwargs
    ):
        """Log a message with structured data"""
        if extra:
            combined = {**self._context, **extra}
        else:
            combined = dict(self._context)
        
        # Add any keyword arguments as context
        if kwargs:
            combined.update(kwargs)
        
        super().log(level, msg, exc_info=exc_info)


class RotatingFileHandler(logging.Handler):
    """Custom rotating file handler with compression support"""
    
    def __init__(
        self,
        filename: str,
        max_bytes: int = 10_000_000,
        backup_count: int = 5,
        compress: bool = True,
    ):
        super().__init__()
        self.filename = Path(filename)
        self.max_bytes = max_bytes
        self.backup_count = backup_count
        self.compress = compress
        self._lock = threading.Lock()
        
        # Ensure directory exists
        self.filename.parent.mkdir(parents=True, exist_ok=True)
    
    def emit(self, record: logging.LogRecord):
        """Emit a log record with rotation"""
        try:
            with self._lock:
                self._emit(record)
        except Exception:
            self.handleError(record)
    
    def _emit(self, record: logging.LogRecord):
        """Emit a single record"""
        # Check if rotation needed
        if self.max_bytes > 0 and self.filename.exists():
            if self.filename.stat().st_size >= self.max_bytes:
                self._rotate()
        
        # Format and write message
        msg = self.format(record) + "\n"
        with self.filename.open("a") as f:
            f.write(msg)
    
    def _rotate(self):
        """Rotate log files"""
        # Close current file and rename
        for i in range(self.backup_count - 1, 0, -1):
            src = self.filename.with_suffix(f".{i}")
            dst = self.filename.with_suffix(f".{i + 1}")
            if dst.exists():
                dst.unlink()
            if src.exists():
                src.rename(dst)
        
        # Move current to .1
        if self.filename.exists():
            dst = self.filename.with_suffix(".1")
            if dst.exists():
                dst.unlink()
            self.filename.rename(dst)


class QueueLogHandler(logging.Handler):
    """Log handler that writes to a queue for async processing"""
    
    def __init__(self, queue_size: int = 1000):
        super().__init__()
        self._queue: queue.Queue = queue.Queue(maxsize=queue_size)
    
    def emit(self, record: logging.LogRecord):
        try:
            msg = self.format(record)
            self._queue.put_nowait(msg)
        except queue.Full:
            pass  # Drop message if queue is full
    
    def get_message(self, timeout: float = 1.0) -> Optional[str]:
        """Get a log message from the queue"""
        try:
            return self._queue.get(timeout=timeout)
        except queue.Empty:
            return None


class SyslogHandler(logging.Handler):
    """Syslog handler with structured data support"""
    
    def __init__(
        self,
        address: str = "/dev/log",
        facility: int = None,
    ):
        super().__init__()
        self.address = address
        self.facility = facility or syslog.LOG_DAEMON if syslog else 0
        self._connected = False
        
        if syslog:
            try:
                syslog.openlog("luminaguard", 0, self.facility)
                self._connected = True
            except Exception:
                pass
    
    def emit(self, record: logging.LogRecord):
        if not self._connected or not syslog:
            return
        
        try:
            msg = self.format(record)
            # Map log level to syslog
            level_map = {
                logging.DEBUG: syslog.LOG_DEBUG,
                logging.INFO: syslog.LOG_INFO,
                logging.WARNING: syslog.LOG_WARNING,
                logging.ERROR: syslog.LOG_ERR,
                logging.CRITICAL: syslog.LOG_CRIT,
            }
            level = level_map.get(record.levelno, syslog.LOG_INFO)
            syslog.syslog(level, msg)
        except Exception:
            self.handleError(record)


class DaemonLogger:
    """
    Comprehensive logging system for daemon mode.
    
    Features:
    - Structured logging
    - Log rotation
    - Multiple output targets
    - Metrics collection
    - Async log processing
    """
    
    def __init__(
        self,
        name: str = "luminaguard",
        level: str = "INFO",
        log_format: str = "json",
        log_file: Optional[str] = None,
        rotation: Optional[LogRotationConfig] = None,
        metrics: Optional[MetricsConfig] = None,
    ):
        self.name = name
        self.level = getattr(logging, level.upper())
        self.log_format = LogFormat(log_format)
        self.log_file = log_file
        self.rotation = rotation or LogRotationConfig()
        self.metrics_config = metrics or MetricsConfig()
        
        # Setup logger
        self.logger = logging.getLogger(name)
        self.logger.setLevel(self.level)
        self.logger.handlers = []
        
        # Add handlers
        self._setup_handlers()
        
        # Metrics
        self.metrics = DaemonMetrics()
        self._metrics_lock = threading.Lock()
        self._metrics_history: deque = deque(maxlen=60)  # Keep last 60 data points
        self._metrics_thread: Optional[threading.Thread] = None
        self._stop_metrics = threading.Event()
        
        # Start metrics collection if enabled
        if self.metrics_config.enabled:
            self._start_metrics_collection()
    
    def _setup_handlers(self):
        """Setup log handlers"""
        # Console handler
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setLevel(self.level)
        console_handler.setFormatter(self._create_formatter())
        self.logger.addHandler(console_handler)
        
        # File handler with rotation
        if self.log_file:
            if self.rotation.enabled:
                handler = RotatingFileHandler(
                    self.log_file,
                    self.rotation.max_bytes,
                    self.rotation.backup_count,
                    self.rotation.compress_rotated,
                )
            else:
                handler = logging.FileHandler(self.log_file)
            
            handler.setLevel(self.level)
            handler.setFormatter(self._create_formatter())
            self.logger.addHandler(handler)
        
        # Syslog handler (Linux only)
        if syslog and os.path.exists("/dev/log"):
            try:
                syslog_handler = SyslogHandler()
                syslog_handler.setLevel(self.level)
                syslog_handler.setFormatter(self._create_formatter())
                self.logger.addHandler(syslog_handler)
            except Exception:
                pass
    
    def _create_formatter(self) -> logging.Formatter:
        """Create log formatter based on format type"""
        if self.log_format == LogFormat.JSON:
            return JsonFormatter()
        elif self.log_format == LogFormat.STRUCTURED:
            return StructuredFormatter()
        else:
            return logging.Formatter(
                "%(asctime)s - %(name)s - %(levelname)s - %(message)s",
                datefmt="%Y-%m-%d %H:%M:%S"
            )
    
    def _start_metrics_collection(self):
        """Start background metrics collection"""
        self._metrics_thread = threading.Thread(
            target=self._metrics_collection_loop,
            daemon=True,
        )
        self._metrics_thread.start()
    
    def _metrics_collection_loop(self):
        """Background loop for metrics collection"""
        while not self._stop_metrics.wait(self.metrics_config.collection_interval_seconds):
            self._collect_metrics()
    
    def _collect_metrics(self):
        """Collect current metrics"""
        with self._metrics_lock:
            # Collect memory usage
            try:
                import psutil
                process = psutil.Process(os.getpid())
                self.metrics.memory_usage_mb = process.memory_info().rss / 1024 / 1024
                self.metrics.cpu_usage_percent = process.cpu_percent()
            except ImportError:
                pass
            
            # Store metrics snapshot
            self._metrics_history.append({
                "timestamp": time.time(),
                "tasks_completed": self.metrics.tasks_completed,
                "tasks_failed": self.metrics.tasks_failed,
                "errors_total": self.metrics.errors_total,
                "memory_usage_mb": self.metrics.memory_usage_mb,
                "cpu_usage_percent": self.metrics.cpu_usage_percent,
            })
    
    def get_metrics(self) -> Dict[str, Any]:
        """Get current metrics"""
        with self._metrics_lock:
            return self.metrics.to_dict()
    
    def get_metrics_history(self) -> List[Dict[str, Any]]:
        """Get metrics history"""
        with self._metrics_lock:
            return list(self._metrics_history)
    
    def record_task_completed(self):
        """Record a completed task"""
        with self._metrics_lock:
            self.metrics.tasks_completed += 1
            self.metrics.tasks_in_progress = max(0, self.metrics.tasks_in_progress - 1)
    
    def record_task_failed(self):
        """Record a failed task"""
        with self._metrics_lock:
            self.metrics.tasks_failed += 1
            self.metrics.tasks_in_progress = max(0, self.metrics.tasks_in_progress - 1)
    
    def record_task_started(self):
        """Record a started task"""
        with self._metrics_lock:
            self.metrics.tasks_in_progress += 1
    
    def record_error(self, error_type: str):
        """Record an error"""
        with self._metrics_lock:
            self.metrics.errors_total += 1
            self.metrics.errors_by_type[error_type] = \
                self.metrics.errors_by_type.get(error_type, 0) + 1
    
    def record_request(self, endpoint: str):
        """Record an API request"""
        with self._metrics_lock:
            self.metrics.requests_total += 1
            self.metrics.requests_by_endpoint[endpoint] = \
                self.metrics.requests_by_endpoint.get(endpoint, 0) + 1
    
    def record_health_check_failure(self):
        """Record a health check failure"""
        with self._metrics_lock:
            self.metrics.health_check_failures += 1
            self.metrics.last_health_check_time = time.time()
    
    def shutdown(self):
        """Shutdown the logging system"""
        self._stop_metrics.set()
        if self._metrics_thread:
            self._metrics_thread.join(timeout=5)
        logging.shutdown()


class JsonFormatter(logging.Formatter):
    """JSON log formatter"""
    
    def format(self, record: logging.LogRecord) -> str:
        data = {
            "timestamp": datetime.utcfromtimestamp(record.created).isoformat() + "Z",
            "level": record.levelname,
            "logger": record.name,
            "message": record.getMessage(),
            "module": record.module,
            "function": record.funcName,
            "line": record.lineno,
        }
        
        if record.exc_info:
            data["exception"] = self.formatException(record.exc_info)
        
        return json.dumps(data)


class StructuredFormatter(logging.Formatter):
    """Structured log formatter with key-value pairs"""
    
    def format(self, record: logging.LogRecord) -> str:
        parts = [
            f"ts={datetime.utcfromtimestamp(record.created).isoformat()}Z",
            f"level={record.levelname}",
            f"logger={record.name}",
            f"msg=\"{record.getMessage()}\"",
        ]
        
        if record.module != record.name:
            parts.append(f"module={record.module}")
        
        if record.funcName != "?":
            parts.append(f"func={record.funcName}")
        
        parts.append(f"line={record.lineno}")
        
        if record.exc_info:
            parts.append(f"exc=\"{self.formatException(record.exc_info)}\"")
        
        return " ".join(parts)


class LogForwarder:
    """Forwards logs to external systems"""
    
    def __init__(self):
        self._handlers: List[Callable[[str], None]] = []
        self._queue: queue.Queue = queue.Queue(maxsize=1000)
        self._thread: Optional[threading.Thread] = None
        self._running = False
    
    def add_handler(self, handler: Callable[[str], None]) -> None:
        """Add a log forwarder handler"""
        self._handlers.append(handler)
    
    def forward(self, message: str) -> None:
        """Forward a log message"""
        try:
            self._queue.put_nowait(message)
        except queue.Full:
            pass  # Drop if queue full
    
    def start(self) -> None:
        """Start the forwarder"""
        self._running = True
        self._thread = threading.Thread(target=self._forward_loop, daemon=True)
        self._thread.start()
    
    def stop(self) -> None:
        """Stop the forwarder"""
        self._running = False
        if self._thread:
            self._thread.join(timeout=5)
    
    def _forward_loop(self):
        """Main forwarder loop"""
        while self._running:
            try:
                msg = self._queue.get(timeout=1)
                for handler in self._handlers:
                    try:
                        handler(msg)
                    except Exception:
                        pass
            except queue.Empty:
                continue


def create_logger(
    name: str = "luminaguard",
    level: str = "INFO",
    log_file: Optional[str] = None,
) -> DaemonLogger:
    """Factory function to create a DaemonLogger"""
    return DaemonLogger(
        name=name,
        level=level,
        log_file=log_file,
    )


# CLI support
def main():
    """CLI for daemon logging management"""
    import argparse
    
    parser = argparse.ArgumentParser(description="LuminaGuard Logging Management")
    subparsers = parser.add_subparsers(dest="command", help="Commands")
    
    # Test logging
    test_parser = subparsers.add_parser("test", help="Test logging configuration")
    test_parser.add_argument("--level", default="INFO", help="Log level")
    test_parser.add_argument("--file", help="Log file path")
    
    # Show metrics
    metrics_parser = subparsers.add_parser("metrics", help="Show daemon metrics")
    metrics_parser.add_argument("--json", action="store_true", help="Output as JSON")
    
    # Rotate logs
    rotate_parser = subparsers.add_parser("rotate", help="Rotate log files")
    rotate_parser.add_argument("--file", required=True, help="Log file to rotate")
    
    args = parser.parse_args()
    
    if args.command == "test":
        logger = create_logger(level=args.level, log_file=args.file)
        logger.logger.debug("Debug message")
        logger.logger.info("Info message")
        logger.logger.warning("Warning message")
        logger.logger.error("Error message")
        logger.logger.critical("Critical message")
        
        # Test metrics
        logger.record_task_started()
        logger.record_task_completed()
        logger.record_error("TestError")
        
        print("Metrics:", logger.get_metrics())
        logger.shutdown()
    
    elif args.command == "metrics":
        # This would connect to a running daemon to get metrics
        print("Metrics not available - daemon not running")
    
    elif args.command == "rotate":
        # Manually rotate logs
        path = Path(args.file)
        if not path.exists():
            print(f"Log file not found: {args.file}")
            return 1
        
        for i in range(4, 0, -1):
            src = path.with_suffix(f".{i}")
            dst = path.with_suffix(f".{i + 1}")
            if dst.exists():
                dst.unlink()
            if src.exists():
                src.rename(dst)
        
        if path.exists():
            dst = path.with_suffix(".1")
            if dst.exists():
                dst.unlink()
            path.rename(dst)
        
        print(f"Rotated {args.file}")
    
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
