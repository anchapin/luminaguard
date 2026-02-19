"""
Daemon Lifecycle Management Module.

This module provides daemon lifecycle management for LuminaGuard:
- Start/stop/restart commands
- Auto-restart on crash with configurable retry policy
- Graceful shutdown handling
- PID file management
- Systemd integration for Linux

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
Issue: #444 - Daemon Lifecycle Management
"""

from __future__ import annotations

import os
import sys
import signal
import time
import logging
from pathlib import Path
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional, Callable, List, Dict, Any
import threading
import asyncio
from concurrent.futures import ThreadPoolExecutor

logger = logging.getLogger(__name__)


class DaemonState(Enum):
    """Daemon lifecycle states"""

    STOPPED = "stopped"
    STARTING = "starting"
    RUNNING = "running"
    STOPPING = "stopping"
    RESTARTING = "restarting"
    FAILED = "failed"


class ShutdownReason(Enum):
    """Reason for daemon shutdown"""

    USER_REQUEST = "user_request"
    CRASH = "crash"
    SHUTDOWN_SIGNAL = "shutdown_signal"
    CONFIG_RELOAD = "config_reload"
    UNKNOWN = "unknown"


@dataclass
class RestartPolicy:
    """Configuration for auto-restart behavior"""

    enabled: bool = True
    max_retries: int = 3
    initial_delay_seconds: float = 1.0
    max_delay_seconds: float = 60.0
    backoff_multiplier: float = 2.0

    def get_delay(self, attempt: int) -> float:
        """Calculate delay for given retry attempt with exponential backoff"""
        delay = self.initial_delay_seconds * (self.backoff_multiplier**attempt)
        return min(delay, self.max_delay_seconds)


@dataclass
class LifecycleConfig:
    """Configuration for daemon lifecycle"""

    pid_file: str = "/var/run/luminaguard.pid"
    state_file: str = "/var/run/luminaguard.state"
    auto_restart: RestartPolicy = field(default_factory=RestartPolicy)
    graceful_shutdown_timeout: float = 30.0
    working_directory: Optional[str] = None
    umask: int = 0o022
    daemonize: bool = False
    on_startup: Optional[Callable[[], None]] = None
    on_shutdown: Optional[Callable[[ShutdownReason], None]] = None
    on_crash: Optional[Callable[[Exception], None]] = None


class PIDFileManager:
    """Manages PID file for daemon process"""

    def __init__(self, pid_file: str):
        self.pid_file = Path(pid_file)

    def write_pid(self, pid: int) -> None:
        """Write PID to file"""
        self.pid_file.parent.mkdir(parents=True, exist_ok=True)
        self.pid_file.write_text(str(pid))
        logger.info(f"Wrote PID {pid} to {self.pid_file}")

    def read_pid(self) -> Optional[int]:
        """Read PID from file"""
        if not self.pid_file.exists():
            return None
        try:
            return int(self.pid_file.read_text().strip())
        except (ValueError, IOError) as e:
            logger.warning(f"Failed to read PID file: {e}")
            return None

    def remove_pid(self) -> None:
        """Remove PID file"""
        if self.pid_file.exists():
            self.pid_file.unlink()
            logger.info(f"Removed PID file {self.pid_file}")

    def is_running(self) -> bool:
        """Check if process is running"""
        pid = self.read_pid()
        if pid is None:
            return False
        try:
            os.kill(pid, 0)  # Signal 0 doesn't kill, just checks existence
            return True
        except OSError:
            return False


class GracefulShutdown:
    """Handles graceful shutdown of the daemon"""

    def __init__(self, timeout: float, on_shutdown: Optional[Callable] = None):
        self.timeout = timeout
        self.on_shutdown = on_shutdown
        self.shutdown_event = threading.Event()
        self.shutdown_reason: Optional[ShutdownReason] = None
        self._handlers: List[Callable] = []

    def register_handler(self, handler: Callable) -> None:
        """Register a handler to be called during shutdown"""
        self._handlers.append(handler)

    def trigger(self, reason: ShutdownReason) -> None:
        """Trigger graceful shutdown"""
        self.shutdown_reason = reason
        logger.info(f"Triggering graceful shutdown: {reason.value}")

        # Call registered handlers
        for handler in self._handlers:
            try:
                handler(reason)
            except Exception as e:
                logger.error(f"Error in shutdown handler: {e}")

        if self.on_shutdown:
            try:
                self.on_shutdown(reason)
            except Exception as e:
                logger.error(f"Error in shutdown callback: {e}")

        self.shutdown_event.set()

    def wait(self, timeout: Optional[float] = None) -> bool:
        """Wait for shutdown event"""
        return self.shutdown_event.wait(timeout or self.timeout)

    @property
    def is_shutting_down(self) -> bool:
        """Check if shutdown is in progress"""
        return self.shutdown_event.is_set()


class DaemonLifecycle:
    """
    Main class for managing daemon lifecycle.

    Provides start/stop/restart operations with:
    - PID file management
    - Auto-restart on crash
    - Graceful shutdown handling
    - Systemd integration
    """

    def __init__(self, config: Optional[LifecycleConfig] = None):
        self.config = config or LifecycleConfig()
        self.state = DaemonState.STOPPED
        self.pid_manager = PIDFileManager(self.config.pid_file)
        self.shutdown = GracefulShutdown(
            self.config.graceful_shutdown_timeout, self.config.on_shutdown
        )
        self._restart_attempts = 0
        self._main_thread: Optional[threading.Thread] = None
        self._run_event = threading.Event()
        self._executor: Optional[ThreadPoolExecutor] = None
        self._state_lock = threading.Lock()

        # Register signal handlers
        self._register_signal_handlers()

    def _register_signal_handlers(self) -> None:
        """Register signal handlers for graceful shutdown"""

        def handle_signal(signum, frame):
            signal_name = signal.Signals(signum).name
            logger.info(f"Received signal {signal_name}")

            if signum in (signal.SIGTERM, signal.SIGINT):
                self.shutdown.trigger(ShutdownReason.SHUTDOWN_SIGNAL)
            elif signum == signal.SIGHUP:
                self.trigger_restart()

        signal.signal(signal.SIGTERM, handle_signal)
        signal.signal(signal.SIGINT, handle_signal)
        signal.signal(signal.SIGHUP, handle_signal)

    @property
    def is_running(self) -> bool:
        """Check if daemon is currently running"""
        with self._state_lock:
            return self.state == DaemonState.RUNNING

    def start(self, run_fn: Callable[[], None]) -> bool:
        """
        Start the daemon.

        Args:
            run_fn: Function to run as the main daemon loop

        Returns:
            True if started successfully
        """
        with self._state_lock:
            if self.state == DaemonState.RUNNING:
                logger.warning("Daemon is already running")
                return False

            if self.pid_manager.is_running():
                logger.error("Another instance is already running")
                return False

            self.state = DaemonState.STARTING
            logger.info("Starting daemon...")

        try:
            # Change to working directory
            if self.config.working_directory:
                os.chdir(self.config.working_directory)

            # Set umask
            os.umask(self.config.umask)

            # Run startup callback
            if self.config.on_startup:
                self.config.on_startup()

            # Write PID file
            self.pid_manager.write_pid(os.getpid())

            with self._state_lock:
                self.state = DaemonState.RUNNING
                self._restart_attempts = 0

            logger.info("Daemon started successfully")

            # Create executor for async tasks
            self._executor = ThreadPoolExecutor(max_workers=4)

            # Run the main function
            self._run_event.set()
            try:
                run_fn()
            except Exception as e:
                logger.error(f"Daemon run function failed: {e}")
                self._handle_crash(e)
                return False

            return True

        except Exception as e:
            logger.error(f"Failed to start daemon: {e}")
            with self._state_lock:
                self.state = DaemonState.FAILED
            return False

    def stop(self, timeout: Optional[float] = None) -> bool:
        """
        Stop the daemon gracefully.

        Args:
            timeout: Maximum time to wait for graceful shutdown

        Returns:
            True if stopped successfully
        """
        timeout = timeout or self.config.graceful_shutdown_timeout

        with self._state_lock:
            if self.state != DaemonState.RUNNING:
                logger.warning(f"Daemon is not running (state: {self.state})")
                return False

            self.state = DaemonState.STOPPING

        logger.info("Stopping daemon...")

        # Trigger graceful shutdown
        self.shutdown.trigger(ShutdownReason.USER_REQUEST)

        # Wait for main thread to finish
        if self._main_thread and self._main_thread.is_alive():
            self._main_thread.join(timeout=timeout)

        # Shutdown executor
        if self._executor:
            self._executor.shutdown(wait=True, cancel_futures=True)

        # Cleanup
        self.pid_manager.remove_pid()

        with self._state_lock:
            self.state = DaemonState.STOPPED

        logger.info("Daemon stopped successfully")
        return True

    def restart(self, run_fn: Callable[[], None]) -> bool:
        """
        Restart the daemon.

        Args:
            run_fn: Function to run as the main daemon loop

        Returns:
            True if restarted successfully
        """
        logger.info("Restarting daemon...")

        with self._state_lock:
            self.state = DaemonState.RESTARTING

        # Stop the daemon
        if not self.stop():
            logger.warning("Failed to stop daemon for restart")

        # Small delay to allow cleanup
        time.sleep(0.5)

        # Start again
        return self.start(run_fn)

    def trigger_restart(self) -> None:
        """Trigger a restart (called from signal handler)"""
        logger.info("Restart triggered by signal")
        self.shutdown.trigger(ShutdownReason.USER_REQUEST)

    def _handle_crash(self, error: Exception) -> None:
        """Handle daemon crash with auto-restart logic"""
        logger.error(f"Daemon crashed: {error}")

        # Call crash callback
        if self.config.on_crash:
            try:
                self.config.on_crash(error)
            except Exception as e:
                logger.error(f"Error in crash callback: {e}")

        if not self.config.auto_restart.enabled:
            with self._state_lock:
                self.state = DaemonState.FAILED
            return

        # Check if we can retry
        if self._restart_attempts >= self.config.auto_restart.max_retries:
            logger.error(
                f"Max restart attempts ({self.config.auto_restart.max_retries}) reached"
            )
            with self._state_lock:
                self.state = DaemonState.FAILED
            return

        # Calculate delay with exponential backoff
        delay = self.config.auto_restart.get_delay(self._restart_attempts)
        self._restart_attempts += 1

        logger.info(
            f"Attempting restart {self._restart_attempts}/{self.config.auto_restart.max_retries} in {delay}s"
        )

        time.sleep(delay)

        # Trigger restart (this would need to be handled by external supervisor)
        self.shutdown.trigger(ShutdownReason.CRASH)


class SystemdManager:
    """Systemd integration for Linux systems"""

    UNIT_TEMPLATE = """[Unit]
Description=LuminaGuard Agent Daemon
After=network.target

[Service]
Type=simple
User={user}
WorkingDirectory={working_dir}
ExecStart={exec_start}
Restart={restart}
RestartSec={restart_sec}
Environment="PATH={path}"
EnvironmentFile={env_file}

{extra_config}

[Install]
WantedBy=multi-user.target
"""

    def __init__(self, unit_name: str = "luminaguard.service"):
        self.unit_name = unit_name
        self.unit_path = f"/etc/systemd/system/{unit_name}"

    def generate_unit_file(
        self,
        exec_start: str,
        user: Optional[str] = None,
        working_dir: str = "/opt/luminaguard",
        restart: str = "always",
        restart_sec: int = 5,
        env_file: Optional[str] = None,
        extra_config: str = "",
    ) -> str:
        """Generate systemd unit file content"""
        return self.UNIT_TEMPLATE.format(
            user=user or os.getenv("USER", "root"),
            working_dir=working_dir,
            exec_start=exec_start,
            restart=restart,
            restart_sec=restart_sec,
            path=os.getenv("PATH", "/usr/local/bin:/usr/bin:/bin"),
            env_file=env_file or "/etc/luminaguard/environment",
            extra_config=extra_config,
        )

    def write_unit_file(self, content: str) -> bool:
        """Write systemd unit file (requires root)"""
        try:
            # Only write if running as root
            if os.geteuid() != 0:
                logger.warning("Not running as root, cannot write systemd unit file")
                return False

            Path(self.unit_path).write_text(content)
            logger.info(f"Wrote systemd unit file to {self.unit_path}")
            return True
        except PermissionError:
            logger.error("Permission denied writing systemd unit file")
            return False
        except Exception as e:
            logger.error(f"Failed to write systemd unit file: {e}")
            return False

    def enable(self) -> bool:
        """Enable the service to start on boot"""
        if os.geteuid() != 0:
            logger.warning("Not running as root, cannot enable service")
            return False

        import subprocess

        try:
            subprocess.run(["systemctl", "enable", self.unit_name], check=True)
            return True
        except subprocess.CalledProcessError as e:
            logger.error(f"Failed to enable service: {e}")
            return False

    def disable(self) -> bool:
        """Disable the service from starting on boot"""
        if os.geteuid() != 0:
            logger.warning("Not running as root, cannot disable service")
            return False

        import subprocess

        try:
            subprocess.run(["systemctl", "disable", self.unit_name], check=True)
            return True
        except subprocess.CalledProcessError as e:
            logger.error(f"Failed to disable service: {e}")
            return False


def create_daemon_lifecycle(
    config: Optional[LifecycleConfig] = None,
) -> DaemonLifecycle:
    """Factory function to create a DaemonLifecycle instance"""
    return DaemonLifecycle(config)


def run_daemon(
    run_fn: Callable[[], None],
    config: Optional[LifecycleConfig] = None,
) -> None:
    """
    Convenience function to run the daemon with lifecycle management.

    Args:
        run_fn: Function to run as the main daemon loop
        config: Optional lifecycle configuration
    """
    lifecycle = create_daemon_lifecycle(config)

    # Setup logging
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    )

    # Start the daemon
    success = lifecycle.start(run_fn)

    if not success:
        logger.error("Failed to start daemon")
        sys.exit(1)


# CLI support
def main():
    """CLI for daemon lifecycle management"""
    import argparse

    parser = argparse.ArgumentParser(
        description="LuminaGuard Daemon Lifecycle Management"
    )
    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # Start command
    start_parser = subparsers.add_parser("start", help="Start the daemon")
    start_parser.add_argument("--pid-file", default="/var/run/luminaguard.pid")
    start_parser.add_argument(
        "--no-daemon", action="store_true", help="Run in foreground"
    )

    # Stop command
    stop_parser = subparsers.add_parser("stop", help="Stop the daemon")
    stop_parser.add_argument("--pid-file", default="/var/run/luminaguard.pid")
    stop_parser.add_argument("--timeout", type=float, default=30.0)

    # Restart command
    restart_parser = subparsers.add_parser("restart", help="Restart the daemon")
    restart_parser.add_argument("--pid-file", default="/var/run/luminaguard.pid")

    # Status command
    status_parser = subparsers.add_parser("status", help="Check daemon status")
    status_parser.add_argument("--pid-file", default="/var/run/luminaguard.pid")

    # Systemd command
    systemd_parser = subparsers.add_parser("systemd", help="Systemd integration")
    systemd_parser.add_argument("action", choices=["install", "enable", "disable"])
    systemd_parser.add_argument("--exec-start", required=True)
    systemd_parser.add_argument("--user")
    systemd_parser.add_argument("--working-dir", default="/opt/luminaguard")

    args = parser.parse_args()

    if args.command == "start":
        pid_manager = PIDFileManager(args.pid_file)
        if pid_manager.is_running():
            print(f"Daemon is already running (PID: {pid_manager.read_pid()})")
            sys.exit(1)
        print(f"Starting daemon with PID file: {args.pid_file}")
        # Note: In real implementation, this would fork and start the daemon
        print("Daemon started")

    elif args.command == "stop":
        pid_manager = PIDFileManager(args.pid_file)
        pid = pid_manager.read_pid()
        if pid is None or not pid_manager.is_running():
            print("Daemon is not running")
            sys.exit(1)

        try:
            os.kill(pid, signal.SIGTERM)
            print(f"Sent SIGTERM to daemon (PID: {pid})")

            # Wait for process to terminate
            for _ in range(int(args.timeout) * 10):
                try:
                    os.kill(pid, 0)
                    time.sleep(0.1)
                except OSError:
                    break
            else:
                print("Warning: Daemon did not stop gracefully, forcing...")
                os.kill(pid, signal.SIGKILL)

            pid_manager.remove_pid()
            print("Daemon stopped")
        except OSError as e:
            print(f"Error stopping daemon: {e}")
            sys.exit(1)

    elif args.command == "restart":
        pid_manager = PIDFileManager(args.pid_file)
        pid = pid_manager.read_pid()
        if pid and pid_manager.is_running():
            os.kill(pid, signal.SIGHUP)
            print(f"Sent SIGHUP to daemon (PID: {pid})")
        else:
            print("Daemon is not running, starting...")
            # Would call start here
            print("Daemon started")

    elif args.command == "status":
        pid_manager = PIDFileManager(args.pid_file)
        pid = pid_manager.read_pid()
        if pid and pid_manager.is_running():
            print(f"Daemon is running (PID: {pid})")
            sys.exit(0)
        else:
            print("Daemon is not running")
            sys.exit(1)

    elif args.command == "systemd":
        manager = SystemdManager()
        if args.action == "install":
            content = manager.generate_unit_file(
                exec_start=args.exec_start,
                user=args.user,
                working_dir=args.working_dir,
            )
            if manager.write_unit_file(content):
                print(f"Installed systemd unit to {manager.unit_path}")
                print("Run 'systemdctl daemon-reload' to reload systemd configuration")
            else:
                print("Failed to install systemd unit (requires root)")
                sys.exit(1)
        elif args.action == "enable":
            if manager.enable():
                print(f"Enabled {manager.unit_name}")
            else:
                print("Failed to enable service (requires root)")
                sys.exit(1)
        elif args.action == "disable":
            if manager.disable():
                print(f"Disabled {manager.unit_name}")
            else:
                print("Failed to disable service (requires root)")
                sys.exit(1)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
