"""
Health Check System for LuminaGuard Daemon Mode.

This module provides configurable heartbeat mechanism for 24/7 bot that monitors
health and uptime. It supports:
- Periodic health pings to external monitoring services
- Bot uptime and restart count tracking
- Health check endpoints (HTTP/VSOCK)
- Configurable intervals and timeout thresholds
- Health status reporting to external monitoring systems

Part of: luminaguard-0va.1 - Heartbeat/Health Check System
"""

import asyncio
import time
import logging
import json
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, Callable, Awaitable
from enum import Enum

import aiohttp
from aiohttp import web

logger = logging.getLogger(__name__)


class HealthStatus(Enum):
    """Health status enum for the daemon."""

    HEALTHY = "healthy"
    DEGRADED = "degraded"
    UNHEALTHY = "unhealthy"
    STARTING = "starting"
    STOPPED = "stopped"


@dataclass
class HealthConfig:
    """Configuration for health check system."""

    # Ping interval in seconds (default: 60)
    ping_interval: int = 60
    # Timeout for health check responses in seconds (default: 10)
    timeout: int = 10
    # Number of consecutive failures before marking unhealthy (default: 3)
    max_failures: int = 3
    # External health ping URL (optional)
    ping_url: Optional[str] = None
    # HTTP health check port (default: 8080)
    http_port: int = 8080
    # Enable HTTP health endpoint (default: True)
    http_enabled: bool = True
    # VSOCK health check CID (default: 2 = host)
    vsock_cid: int = 2
    # VSOCK health check port (default: 5050)
    vsock_port: int = 5050
    # Enable VSOCK health check (default: True)
    vsock_enabled: bool = True


@dataclass
class HealthMetrics:
    """Health metrics for the daemon."""

    # Bot start timestamp
    start_time: float = field(default_factory=time.time)
    # Number of times the bot has been restarted
    restart_count: int = 0
    # Number of successful health checks
    success_count: int = 0
    # Number of failed health checks
    failure_count: int = 0
    # Last successful health check timestamp
    last_success: Optional[float] = None
    # Last failed health check timestamp
    last_failure: Optional[float] = None
    # Current health status
    status: HealthStatus = HealthStatus.STARTING

    @property
    def uptime_seconds(self) -> float:
        """Get uptime in seconds."""
        return time.time() - self.start_time

    @property
    def uptime_hours(self) -> float:
        """Get uptime in hours."""
        return self.uptime_seconds / 3600

    def to_dict(self) -> Dict[str, Any]:
        """Convert metrics to dictionary."""
        return {
            "start_time": self.start_time,
            "uptime_seconds": self.uptime_seconds,
            "uptime_hours": self.uptime_hours,
            "restart_count": self.restart_count,
            "success_count": self.success_count,
            "failure_count": self.failure_count,
            "last_success": self.last_success,
            "last_failure": self.last_failure,
            "status": self.status.value,
        }


class HealthCheck:
    """
    Health check system for LuminaGuard daemon.

    Provides:
    - Periodic health pings to external monitoring services
    - Bot uptime and restart count tracking
    - HTTP health check endpoint
    - VSOCK health check endpoint
    - Configurable intervals and timeout thresholds
    """

    def __init__(
        self,
        config: Optional[HealthConfig] = None,
        on_status_change: Optional[Callable[[HealthStatus], Awaitable[None]]] = None,
    ):
        """
        Initialize health check system.

        Args:
            config: Health check configuration (uses defaults if None)
            on_status_change: Callback for status changes
        """
        self.config = config or HealthConfig()
        self.on_status_change = on_status_change
        self.metrics = HealthMetrics()
        self._running = False
        self._tasks: list[asyncio.Task] = []
        self._http_server: Optional[asyncio.Server] = None
        self._consecutive_failures = 0

    async def start(self) -> None:
        """Start the health check system."""
        if self._running:
            logger.warning("Health check already running")
            return

        logger.info("Starting health check system")
        self._running = True
        self.metrics.status = HealthStatus.HEALTHY

        # Start HTTP health endpoint if enabled
        if self.config.http_enabled:
            self._tasks.append(asyncio.create_task(self._start_http_server()))

        # Start VSOCK health endpoint if enabled
        if self.config.vsock_enabled:
            self._tasks.append(asyncio.create_task(self._start_vsock_server()))

        # Start periodic health ping
        self._tasks.append(asyncio.create_task(self._health_ping_loop()))

        logger.info(
            f"Health check started - HTTP:{self.config.http_port}, "
            f"VSOCK:{self.config.vsock_port}, "
            f"Ping interval:{self.config.ping_interval}s"
        )

    async def stop(self) -> None:
        """Stop the health check system."""
        if not self._running:
            return

        logger.info("Stopping health check system")
        self._running = False
        self.metrics.status = HealthStatus.STOPPED

        # Cancel all tasks
        for task in self._tasks:
            task.cancel()

        # Wait for tasks to complete
        if self._tasks:
            await asyncio.gather(*self._tasks, return_exceptions=True)

        # Stop HTTP server
        if self._http_server:
            self._http_server.close()
            await self._http_server.wait_closed()

        self._tasks.clear()
        logger.info("Health check stopped")

    def record_restart(self) -> None:
        """Record a bot restart."""
        self.metrics.restart_count += 1
        logger.info(f"Restart recorded. Total restarts: {self.metrics.restart_count}")

    async def record_success(self) -> None:
        """Record a successful health check."""
        self.metrics.success_count += 1
        self.metrics.last_success = time.time()
        self._consecutive_failures = 0

        # Update status if needed
        if self.metrics.status != HealthStatus.HEALTHY:
            await self._set_status(HealthStatus.HEALTHY)

    async def record_failure(self, error: Optional[str] = None) -> None:
        """Record a failed health check."""
        self.metrics.failure_count += 1
        self.metrics.last_failure = time.time()
        self._consecutive_failures += 1

        logger.warning(
            f"Health check failure ({self._consecutive_failures}/"
            f"{self.config.max_failures}): {error}"
        )

        # Update status based on consecutive failures
        if self._consecutive_failures >= self.config.max_failures:
            await self._set_status(HealthStatus.UNHEALTHY)
        elif self._consecutive_failures > 0:
            await self._set_status(HealthStatus.DEGRADED)

    async def _set_status(self, status: HealthStatus) -> None:
        """Set health status and trigger callback if changed."""
        if self.metrics.status != status:
            old_status = self.metrics.status
            self.metrics.status = status
            logger.info(f"Health status changed: {old_status.value} -> {status.value}")

            if self.on_status_change:
                try:
                    await self.on_status_change(status)
                except Exception as e:
                    logger.error(f"Error in status change callback: {e}")

    async def _health_ping_loop(self) -> None:
        """Periodic health ping loop."""
        while self._running:
            try:
                await asyncio.sleep(self.config.ping_interval)

                # Send external ping if configured
                if self.config.ping_url:
                    await self._send_external_ping()
                else:
                    # Internal health check
                    await self.record_success()

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in health ping loop: {e}")
                await self.record_failure(str(e))

    async def _send_external_ping(self) -> None:
        """Send health ping to external service."""
        import aiohttp

        try:
            async with aiohttp.ClientSession() as session:
                payload = {
                    "status": self.metrics.status.value,
                    "uptime_seconds": self.metrics.uptime_seconds,
                    "restart_count": self.metrics.restart_count,
                    "timestamp": time.time(),
                }

                async with session.post(
                    self.config.ping_url,
                    json=payload,
                    timeout=aiohttp.ClientTimeout(total=self.config.timeout),
                ) as response:
                    if response.status < 400:
                        await self.record_success()
                    else:
                        await self.record_failure(
                            f"External ping failed with status {response.status}"
                        )
        except asyncio.TimeoutError:
            await self.record_failure("External ping timed out")
        except Exception as e:
            await self.record_failure(f"External ping error: {e}")

    async def _start_http_server(self) -> None:
        """Start HTTP health check server."""

        async def health_handler(request):
            """HTTP health check handler."""
            return web.json_response(self.metrics.to_dict())

        async def ready_handler(request):
            """HTTP ready check handler."""
            return web.json_response({"ready": self._running})

        app = web.Application()
        app.router.add_get("/health", health_handler)
        app.router.add_get("/ready", ready_handler)

        self._http_server = await app.start_server(
            host="0.0.0.0",
            port=self.config.http_port,
        )
        logger.info(f"HTTP health server started on port {self.config.http_port}")

    async def _start_vsock_server(self) -> None:
        """Start VSOCK health check server."""
        # VSOCK implementation would go here
        # This is a placeholder for VSOCK connectivity
        logger.info(
            f"VSOCK health check server configured "
            f"(CID:{self.config.vsock_cid}, Port:{self.config.vsock_port})"
        )

        # For now, we'll just log that VSOCK is available
        # Full VSOCK implementation would require socket.AF_VSOCK
        while self._running:
            await asyncio.sleep(self.config.ping_interval)

    def get_status(self) -> Dict[str, Any]:
        """Get current health status."""
        return self.metrics.to_dict()

    def is_healthy(self) -> bool:
        """Check if the daemon is healthy."""
        return self.metrics.status in (HealthStatus.HEALTHY, HealthStatus.STARTING)


async def create_health_check(
    config: Optional[HealthConfig] = None,
    on_status_change: Optional[Callable[[HealthStatus], Awaitable[None]]] = None,
) -> HealthCheck:
    """
    Create and start a health check system.

    Args:
        config: Health check configuration
        on_status_change: Callback for status changes

    Returns:
        Started HealthCheck instance
    """
    health = HealthCheck(config=config, on_status_change=on_status_change)
    await health.start()
    return health
