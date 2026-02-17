"""
LuminaGuard Daemon Mode Module.

This module provides the core components for running LuminaGuard as a 24/7 daemon:
- Health check system (heartbeat, uptime tracking)
- Cron job scheduler
- Daemon tools (bash, grep, web, etc.)
- Daemon lifecycle management

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
"""

from .health import (
    HealthCheck,
    HealthConfig,
    HealthMetrics,
    HealthStatus,
    create_health_check,
)

from .scheduler import (
    JobScheduler,
    JobConfig,
    Job,
    JobExecution,
    JobStatus,
    JobType,
    create_job_scheduler,
)

from .tools import (
    DaemonTools,
    ToolConfig,
    ToolResult,
    ToolType,
    BashTool,
    GrepTool,
    WebTool,
    CurlTool,
    create_daemon_tools,
)

from .lifecycle import (
    LifecycleManager,
    LifecycleConfig,
    LifecycleMetrics,
    DaemonState,
    PIDFile,
    DaemonStateManager,
    create_lifecycle_manager,
)

__all__ = [
    # Health
    "HealthCheck",
    "HealthConfig",
    "HealthMetrics",
    "HealthStatus",
    "create_health_check",
    # Scheduler
    "JobScheduler",
    "JobConfig",
    "Job",
    "JobExecution",
    "JobStatus",
    "JobType",
    "create_job_scheduler",
    # Tools
    "DaemonTools",
    "ToolConfig",
    "ToolResult",
    "ToolType",
    "BashTool",
    "GrepTool",
    "WebTool",
    "CurlTool",
    "create_daemon_tools",
    # Lifecycle
    "LifecycleManager",
    "LifecycleConfig",
    "LifecycleMetrics",
    "DaemonState",
    "PIDFile",
    "DaemonStateManager",
    "create_lifecycle_manager",
]
