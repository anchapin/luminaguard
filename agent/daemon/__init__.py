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
    DaemonLifecycle,
    LifecycleConfig,
    RestartPolicy,
    PIDFileManager,
    GracefulShutdown,
    SystemdManager,
    DaemonState,
    ShutdownReason,
    create_daemon_lifecycle,
    run_daemon,
)

from .config import (
    DaemonConfig,
    ConfigLoader,
    HotReloadConfig,
    ConfigFormat,
    ConfigWatcher,
    HealthConfigData,
    SchedulerConfigData,
    LoggingConfigData,
    LifecycleConfigData,
    MessengerConfigData,
    StateConfigData,
    create_config_loader,
    create_hot_reload_config,
    load_config,
)

from .daemon_logging import (
    DaemonLogger,
    DaemonMetrics,
    LogLevel,
    LogFormat,
    LogRotationConfig,
    MetricsConfig,
    LogForwarder,
    create_logger,
)

from .state import (
    PersistentStateManager,
    ConversationHistory,
    ConversationMessage,
    TaskState,
    StateSnapshot,
    StateEncryption,
    StateStorage,
    TaskQueue,
    StateType,
    create_state_manager,
)

from .advanced_features import (
    # Rate Limiting
    RateLimiterAdvanced,
    RateLimitConfig,
    RateLimitScope,
    RateLimitEntry,
    # Permissions
    PermissionManager,
    Permission,
    Role,
    DEFAULT_ROLES,
    # Cooldowns
    CooldownManager,
    CooldownConfig,
    # Message Queue
    MessageQueue,
    QueuedMessage,
    # Typing Indicator
    TypingIndicatorManager,
    # Threading
    ThreadManager,
    ThreadMetadata,
    # Reactions
    ReactionManager,
    Reaction,
    ReactionType,
    # Scheduled Messages
    ScheduledMessageManager,
    ScheduledMessage,
    # Audit Logging
    AuditLogger,
    AuditEvent,
    AuditEventType,
    # Circuit Breaker
    CircuitBreaker,
    CircuitState,
    CircuitStats,
    # Deduplication
    MessageDeduplicator,
    # Plugin System
    PluginManager,
    BotPlugin,
    PluginInfo,
    # Facade
    AdvancedBotFeatures,
    create_advanced_features,
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
    "DaemonLifecycle",
    "LifecycleConfig",
    "RestartPolicy",
    "PIDFileManager",
    "GracefulShutdown",
    "SystemdManager",
    "DaemonState",
    "ShutdownReason",
    "create_daemon_lifecycle",
    "run_daemon",
    # Config
    "DaemonConfig",
    "ConfigLoader",
    "HotReloadConfig",
    "ConfigFormat",
    "ConfigWatcher",
    "HealthConfigData",
    "SchedulerConfigData",
    "LoggingConfigData",
    "LifecycleConfigData",
    "MessengerConfigData",
    "StateConfigData",
    "create_config_loader",
    "create_hot_reload_config",
    "load_config",
    # Logging
    "DaemonLogger",
    "DaemonMetrics",
    "LogLevel",
    "LogFormat",
    "LogRotationConfig",
    "MetricsConfig",
    "LogForwarder",
    "create_logger",
    # State
    "PersistentStateManager",
    "ConversationHistory",
    "ConversationMessage",
    "TaskState",
    "StateSnapshot",
    "StateEncryption",
    "StateStorage",
    "TaskQueue",
    "StateType",
    "create_state_manager",
    # Advanced Features - Rate Limiting
    "RateLimiterAdvanced",
    "RateLimitConfig",
    "RateLimitScope",
    "RateLimitEntry",
    # Advanced Features - Permissions
    "PermissionManager",
    "Permission",
    "Role",
    "DEFAULT_ROLES",
    # Advanced Features - Cooldowns
    "CooldownManager",
    "CooldownConfig",
    # Advanced Features - Message Queue
    "MessageQueue",
    "QueuedMessage",
    # Advanced Features - Typing Indicator
    "TypingIndicatorManager",
    # Advanced Features - Threading
    "ThreadManager",
    "ThreadMetadata",
    # Advanced Features - Reactions
    "ReactionManager",
    "Reaction",
    "ReactionType",
    # Advanced Features - Scheduled Messages
    "ScheduledMessageManager",
    "ScheduledMessage",
    # Advanced Features - Audit Logging
    "AuditLogger",
    "AuditEvent",
    "AuditEventType",
    # Advanced Features - Circuit Breaker
    "CircuitBreaker",
    "CircuitState",
    "CircuitStats",
    # Advanced Features - Deduplication
    "MessageDeduplicator",
    # Advanced Features - Plugin System
    "PluginManager",
    "BotPlugin",
    "PluginInfo",
    # Advanced Features - Facade
    "AdvancedBotFeatures",
    "create_advanced_features",
]
