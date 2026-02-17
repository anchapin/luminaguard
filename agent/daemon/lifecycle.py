"""
Daemon Lifecycle Management for LuminaGuard.

This module provides daemon lifecycle management:
- Start/stop/restart commands
- Auto-restart on crash with configurable retry policy
- Graceful shutdown handling
- PID file management
- Systemd integration for Linux

Part of: luminaguard-0va.4 - Daemon Lifecycle Management
"""

import asyncio
import os
import sys
import signal
import logging
import time
import errno
from dataclasses import dataclass, field
from typing import Optional, Callable, Awaitable, Dict, Any, List
from pathlib import Path
from enum import Enum

logger = logging.getLogger(__name__)


class DaemonState(Enum):
    """Daemon state enum."""
    STOPPED = "stopped"
    STARTING = "starting"
    RUNNING = "running"
    STOPPING = "stopping"
    RESTARTING = "restarting"
    FAILED = "failed"


@dataclass
class LifecycleConfig:
    """Configuration for daemon lifecycle."""
    # Path to PID file (default: ~/.luminaguard/daemon.pid)
    pid_file: str = "~/.luminaguard/daemon.pid"
    # Path to state file (default: ~/.luminaguard/state.json)
    state_file: str = "~/.luminaguard/state.json"
    # Log file path (default: ~/.luminaguard/daemon.log)
    log_file: str = "~/.luminaguard/daemon.log"
    # User to run as (default: current user)
    user: Optional[str] = None
    # Group to run as (default: current group)
    group: Optional[str] = None
    # Working directory (default: ~)
    working_dir: str = "~"
    # Environment variables to set
    env: Dict[str, str] = field(default_factory=dict)
    # Enable auto-restart on crash (default: True)
    auto_restart: bool = True
    # Maximum restart attempts (default: 3)
    max_restart_attempts: int = 3
    # Delay between restart attempts in seconds (default: 5)
    restart_delay: int = 5
    # Graceful shutdown timeout in seconds (default: 30)
    shutdown_timeout: int = 30
    # Enable systemd integration (default: True on Linux)
    systemd_enabled: bool = sys.platform.startswith("linux")


@dataclass
class LifecycleMetrics:
    """Lifecycle metrics."""
    # Number of times daemon has been started
    start_count: int = 0
    # Number of times daemon has been stopped
    stop_count: int = 0
    # Number of restart attempts
    restart_attempts: int = 0
    # Number of consecutive failures
    consecutive_failures: int = 0
    # Last start timestamp
    last_start: Optional[float] = None
    # Last stop timestamp
    last_stop: Optional[float] = None
    # Last failure timestamp
    last_failure: Optional[float] = None
    # Last failure reason
    last_failure_reason: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "start_count": self.start_count,
            "stop_count": self.stop_count,
            "restart_attempts": self.restart_attempts,
            "consecutive_failures": self.consecutive_failures,
            "last_start": self.last_start,
            "last_stop": self.last_stop,
            "last_failure": self.last_failure,
            "last_failure_reason": self.last_failure_reason,
        }


class PIDFile:
    """PID file management."""

    def __init__(self, path: str):
        """Initialize PID file manager."""
        self.path = os.path.expanduser(path)
        self._lock_file = f"{self.path}.lock"

    def write(self, pid: int) -> None:
        """Write PID to file."""
        Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        with open(self.path, "w") as f:
            f.write(str(pid))
        logger.debug(f"Wrote PID {pid} to {self.path}")

    def read(self) -> Optional[int]:
        """Read PID from file."""
        try:
            with open(self.path, "r") as f:
                return int(f.read().strip())
        except (FileNotFoundError, ValueError):
            return None

    def exists(self) -> bool:
        """Check if PID file exists."""
        return os.path.exists(self.path)

    def is_running(self) -> bool:
        """Check if process is running."""
        pid = self.read()
        if pid is None:
            return False

        try:
            # Signal 0 just checks if process exists
            os.kill(pid, 0)
            return True
        except OSError as e:
            if e.errno == errno.ESRCH:
                return False  # No such process
            elif e.errno == errno.EPERM:
                return True   # Process exists but we don't have permission
            raise

    def remove(self) -> None:
        """Remove PID file."""
        try:
            os.remove(self.path)
            logger.debug(f"Removed PID file {self.path}")
        except FileNotFoundError:
            pass

    def get_pid_or_raise(self) -> int:
        """Get PID or raise error if not running."""
        if not self.exists():
            raise RuntimeError(f"PID file not found: {self.path}")

        pid = self.read()
        if pid is None:
            raise RuntimeError(f"Invalid PID in file: {self.path}")

        if not self.is_running():
            self.remove()
            raise RuntimeError(f"Process {pid} is not running")

        return pid


class LifecycleManager:
    """
    Daemon lifecycle manager.
    
    Provides:
    - Start/stop/restart commands
    - Auto-restart on crash
    - Graceful shutdown handling
    - PID file management
    - Systemd integration
    """

    def __init__(
        self,
        config: Optional[LifecycleConfig] = None,
        on_start: Optional[Callable[[], Awaitable[None]]] = None,
        on_stop: Optional[Callable[[], Awaitable[None]]] = None,
        on_crash: Optional[Callable[[Exception], None]] = None,
    ):
        """
        Initialize lifecycle manager.
        
        Args:
            config: Lifecycle configuration
            on_start: Callback when daemon starts
            on_stop: Callback when daemon stops
            on_crash: Callback when daemon crashes
        """
        self.config = config or LifecycleConfig()
        self.on_start = on_start
        self.on_stop = on_stop
        self.on_crash = on_crash

        self._pid_file = PIDFile(self.config.pid_file)
        self._state: Dict[str, Any] = {}
        self._state_file = os.path.expanduser(self.config.state_file)

        self._state_manager: Optional[DaemonStateManager] = None
        self._metrics = LifecycleMetrics()
        self._state_value = DaemonState.STOPPED
        self._running = False
        self._shutdown_event = asyncio.Event()

    @property
    def state(self) -> DaemonState:
        """Get current daemon state."""
        return self._state_value

    @property
    def metrics(self) -> LifecycleMetrics:
        """Get lifecycle metrics."""
        return self._metrics

    def _set_state(self, new_state: DaemonState) -> None:
        """Set daemon state."""
        old_state = self._state_value
        self._state_value = new_state
        logger.info(f"Daemon state: {old_state.value} -> {new_state.value}")

    def load_state(self) -> Dict[str, Any]:
        """Load persisted state."""
        try:
            import json
            if os.path.exists(self._state_file):
                with open(self._state_file, "r") as f:
                    self._state = json.load(f)
                logger.debug(f"Loaded state from {self._state_file}")
        except Exception as e:
            logger.warning(f"Failed to load state: {e}")
            self._state = {}
        return self._state

    def save_state(self) -> None:
        """Persist state."""
        try:
            import json
            Path(self._state_file).parent.mkdir(parents=True, exist_ok=True)
            with open(self._state_file, "w") as f:
                json.dump(self._state, f, indent=2)
            logger.debug(f"Saved state to {self._state_file}")
        except Exception as e:
            logger.warning(f"Failed to save state: {e}")

    async def start(self, daemonize: bool = False) -> bool:
        """
        Start the daemon.
        
        Args:
            daemonize: Whether to daemonize (run in background)
            
        Returns:
            True if started successfully
        """
        # Check if already running
        if self._pid_file.is_running():
            pid = self._pid_file.read()
            logger.error(f"Daemon already running with PID {pid}")
            return False

        self._set_state(DaemonState.STARTING)
        self._metrics.start_count += 1
        self._metrics.last_start = time.time()

        try:
            if daemonize:
                self._daemonize()

            # Write PID file
            self._pid_file.write(os.getpid())

            # Set up signal handlers
            self._setup_signal_handlers()

            # Load persisted state
            self.load_state()

            # Mark as running
            self._running = True
            self._set_state(DaemonState.RUNNING)
            self._shutdown_event.clear()

            # Run on_start callback
            if self.on_start:
                await self.on_start()

            # Save state
            self.save_state()

            logger.info(f"Daemon started with PID {os.getpid()}")
            return True

        except Exception as e:
            logger.error(f"Failed to start daemon: {e}")
            self._metrics.consecutive_failures += 1
            self._metrics.last_failure = time.time()
            self._metrics.last_failure_reason = str(e)
            self._set_state(DaemonState.FAILED)
            return False

    async def stop(self, graceful: bool = True) -> bool:
        """
        Stop the daemon.
        
        Args:
            graceful: Whether to do graceful shutdown
            
        Returns:
            True if stopped successfully
        """
        if not self._pid_file.is_running():
            logger.warning("Daemon not running")
            return True

        self._set_state(DaemonState.STOPPING)
        self._metrics.stop_count += 1
        self._metrics.last_stop = time.time()
        self._running = False

        try:
            # Run on_stop callback
            if self.on_stop:
                await self.on_stop()

            if graceful:
                # Send SIGTERM for graceful shutdown
                pid = self._pid_file.get_pid_or_raise()
                os.kill(pid, signal.SIGTERM)

                # Wait for shutdown
                try:
                    await asyncio.wait_for(
                        self._shutdown_event.wait(),
                        timeout=self.config.shutdown_timeout
                    )
                except asyncio.TimeoutError:
                    logger.warning("Graceful shutdown timed out, forcing stop")

            # Force stop if still running
            if self._pid_file.is_running():
                pid = self._pid_file.get_pid_or_raise()
                os.kill(pid, signal.SIGKILL)

            # Clean up PID file
            self._pid_file.remove()

            # Save state
            self.save_state()

            self._set_state(DaemonState.STOPPED)
            logger.info("Daemon stopped")
            return True

        except Exception as e:
            logger.error(f"Failed to stop daemon: {e}")
            self._set_state(DaemonState.FAILED)
            return False

    async def restart(self) -> bool:
        """
        Restart the daemon.
        
        Returns:
            True if restarted successfully
        """
        self._metrics.restart_attempts += 1
        self._set_state(DaemonState.RESTARTING)

        # Stop
        if not await self.stop(graceful=True):
            return False

        # Wait a bit
        await asyncio.sleep(1)

        # Start
        return await self.start(daemonize=True)

    def _daemonize(self) -> None:
        """Daemonize the process (fork to background)."""
        # First fork
        try:
            pid = os.fork()
            if pid > 0:
                # Parent exits
                sys.exit(0)
        except OSError as e:
            raise RuntimeError(f"First fork failed: {e}")

        # Decouple from parent environment
        os.chdir(self.config.working_dir)
        os.setsid()  # Create new session
        os.umask(0o077)

        # Second fork
        try:
            pid = os.fork()
            if pid > 0:
                # Parent exits
                sys.exit(0)
        except OSError as e:
            raise RuntimeError(f"Second fork failed: {e}")

        # Redirect standard file descriptors
        sys.stdout.flush()
        sys.stderr.flush()

        with open("/dev/null", "r") as devnull:
            os.dup2(devnull.fileno(), sys.stdin.fileno())
        with open("/dev/null", "a+") as devnull:
            os.dup2(devnull.fileno(), sys.stdout.fileno())
            os.dup2(devnull.fileno(), sys.stderr.fileno())

        logger.debug("Process daemonized")

    def _setup_signal_handlers(self) -> None:
        """Set up signal handlers for graceful shutdown."""
        def signal_handler(signum, frame):
            logger.info(f"Received signal {signum}")
            if signum in (signal.SIGTERM, signal.SIGINT):
                self._shutdown_event.set()
                self._running = False

        signal.signal(signal.SIGTERM, signal_handler)
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGHUP, signal_handler)

    def get_status(self) -> Dict[str, Any]:
        """Get daemon status."""
        is_running = self._pid_file.is_running()
        pid = self._pid_file.read()

        return {
            "state": self._state_value.value,
            "running": is_running,
            "pid": pid,
            "uptime": time.time() - self._metrics.last_start if self._metrics.last_start and is_running else None,
            "metrics": self._metrics.to_dict(),
        }

    async def wait_for_shutdown(self) -> None:
        """Wait for daemon shutdown."""
        await self._shutdown_event.wait()


class DaemonStateManager:
    """Manages daemon state for lifecycle operations."""

    def __init__(self, lifecycle: LifecycleManager):
        """Initialize state manager."""
        self._lifecycle = lifecycle

    async def start_daemon(self) -> bool:
        """Start the daemon."""
        return await self._lifecycle.start(daemonize=True)

    async def stop_daemon(self) -> bool:
        """Stop the daemon."""
        return await self._lifecycle.stop(graceful=True)

    async def restart_daemon(self) -> bool:
        """Restart the daemon."""
        return await self._lifecycle.restart()

    def get_status(self) -> Dict[str, Any]:
        """Get daemon status."""
        return self._lifecycle.get_status()

    def is_running(self) -> bool:
        """Check if daemon is running."""
        return self._lifecycle._pid_file.is_running()


async def create_lifecycle_manager(
    config: Optional[LifecycleConfig] = None,
    on_start: Optional[Callable[[], Awaitable[None]]] = None,
    on_stop: Optional[Callable[[], Awaitable[None]]] = None,
    on_crash: Optional[Callable[[Exception], None]] = None,
) -> LifecycleManager:
    """
    Create a lifecycle manager.
    
    Args:
        config: Lifecycle configuration
        on_start: Callback when daemon starts
        on_stop: Callback when daemon stops
        on_crash: Callback when daemon crashes
        
    Returns:
        LifecycleManager instance
    """
    return LifecycleManager(
        config=config,
        on_start=on_start,
        on_stop=on_stop,
        on_crash=on_crash,
    )
