"""
Structured Logging Configuration for LuminaGuard Agent

This module provides centralized logging configuration using structlog for
structured, parseable logging output.

Configuration:
- LUMINAGUARD_LOG_LEVEL: trace, debug, info, warn, error (default: info)
- LUMINAGUARD_LOG_FORMAT: json, text (default: text)
- LUMINAGUARD_LOG_FILE: Optional file path for log output
"""

import os
import sys
from typing import Any, Optional

import structlog
from structlog.types import EventDict


def add_log_level(
    logger: Any, method_name: str, event_dict: EventDict
) -> EventDict:
    """Add log level to event dict (already included by structlog)."""
    return event_dict


def add_task_context(
    logger: Any, method_name: str, event_dict: EventDict
) -> EventDict:
    """Add task_id and session_id context if available."""
    # This can be extended to pull from context vars
    return event_dict


def setup_logging(
    log_level: Optional[str] = None,
    log_format: Optional[str] = None,
    log_file: Optional[str] = None,
    verbose: bool = False,
) -> None:
    """
    Configure structured logging for the agent.

    Args:
        log_level: Log level (trace, debug, info, warn, error)
        log_format: Log format (json, text)
        log_file: Optional file path for log output
        verbose: Enable verbose (debug) logging
    """
    # Determine log level from parameters, environment, or default
    level = (log_level or
             os.getenv("LUMINAGUARD_LOG_LEVEL", "info")).lower()
    if verbose:
        level = "debug"

    # Map string levels to logging module constants
    level_map = {
        "trace": 5,
        "debug": 10,
        "info": 20,
        "warn": 30,
        "error": 40,
        "critical": 50,
    }
    numeric_level = level_map.get(level, 20)

    # Determine log format
    format_type = (log_format or
                  os.getenv("LUMINAGUARD_LOG_FORMAT", "text")).lower()

    # Configure processors (middleware for log events)
    shared_processors = [
        structlog.contextvars.merge_contextvars,
        structlog.processors.add_log_level,
        structlog.processors.StackInfoRenderer(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.UnicodeDecoder(),
    ]

    if format_type == "json":
        # JSON format for production/machine parsing
        structlog.configure(
            processors=shared_processors + [
                structlog.processors.JSONRenderer()
            ],
            wrapper_class=structlog.make_filtering_bound_logger(numeric_level),
            context_class=dict,
            logger_factory=structlog.PrintLoggerFactory(),
            cache_logger_on_first_use=True,
        )
    else:
        # Human-readable text format for development
        structlog.configure(
            processors=shared_processors + [
                structlog.dev.ConsoleRenderer(colors=True),
            ],
            wrapper_class=structlog.make_filtering_bound_logger(numeric_level),
            context_class=dict,
            logger_factory=structlog.PrintLoggerFactory(),
            cache_logger_on_first_use=True,
        )

    # Configure file output if specified
    if log_file:
        try:
            file_handler = structlog.WriteLoggerFactory(
                open(log_file, "a")
            )
            structlog.configure(
                processors=shared_processors + [
                    structlog.processors.JSONRenderer()
                ],
                wrapper_class=structlog.make_filtering_bound_logger(numeric_level),
                context_class=dict,
                logger_factory=file_handler,
                cache_logger_on_first_use=True,
            )
        except IOError as e:
            # Fall back to stdout if file cannot be opened
            print(f"Warning: Could not open log file {log_file}: {e}")


def get_logger(name: str = "lumina-agent") -> structlog.BoundLogger:
    """
    Get a configured logger instance.

    Args:
        name: Logger name (default: "lumina-agent")

    Returns:
        Configured structlog logger
    """
    return structlog.get_logger(name)


def bind_context(**kwargs: Any) -> None:
    """
    Bind context variables to all subsequent log entries.

    Args:
        **kwargs: Context key-value pairs
    """
    structlog.contextvars.bind_contextvars(**kwargs)


def clear_context() -> None:
    """Clear all bound context variables."""
    structlog.contextvars.clear_contextvars()


# Convenience function to get logger and bind common context
def init_agent_logger(
    task_id: Optional[str] = None,
    session_id: Optional[str] = None,
    verbose: bool = False,
) -> structlog.BoundLogger:
    """
    Initialize and return an agent logger with context.

    Args:
        task_id: Optional task identifier
        session_id: Optional session identifier
        verbose: Enable verbose logging

    Returns:
        Configured logger with bound context
    """
    # Setup logging
    setup_logging(verbose=verbose)

    # Get logger
    logger = get_logger("lumina-agent")

    # Bind context if provided
    if task_id or session_id:
        bind_context(task_id=task_id, session_id=session_id)

    return logger


# Auto-configure on import for convenience
# This can be disabled if manual control is preferred
if os.getenv("LUMINAGUARD_AUTO_LOGGING", "false").lower() == "true":
    setup_logging()
