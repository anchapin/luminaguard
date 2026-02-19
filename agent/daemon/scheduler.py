"""
Cron Job Scheduler for LuminaGuard Daemon Mode.

This module provides cron-style job scheduling for recurring tasks:
- Define jobs with cron expressions
- Support for one-time and recurring jobs
- Job persistence across bot restarts
- Job execution history and logging
- Job scheduling via configuration or API

Part of: luminaguard-0va.2 - Cron Job Scheduler
"""

import asyncio
import logging
import time
import json
import os
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, Callable, Awaitable, List
from enum import Enum
from datetime import datetime, timezone
from pathlib import Path
from croniter import croniter

logger = logging.getLogger(__name__)


class JobStatus(Enum):
    """Job execution status."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class JobType(Enum):
    """Job type."""

    RECURRING = "recurring"  # Cron-based recurring job
    ONE_TIME = "one_time"  # Single execution job


@dataclass
class JobConfig:
    """Configuration for job scheduler."""

    # Path to persist jobs (default: ~/.luminaguard/jobs.json)
    persistence_path: str = "~/.luminaguard/jobs.json"
    # Default timeout for job execution in seconds (default: 300)
    default_timeout: int = 300
    # Maximum concurrent jobs (default: 5)
    max_concurrent: int = 5
    # Enable job persistence (default: True)
    persistence_enabled: bool = True


@dataclass
class Job:
    """Job definition."""

    id: str
    name: str
    # Cron expression for recurring jobs, or ISO timestamp for one-time
    schedule: str
    job_type: JobType
    # Callable to execute (stored as string reference for persistence)
    handler: str
    # Arguments for the handler
    args: List[Any] = field(default_factory=list)
    kwargs: Dict[str, Any] = field(default_factory=dict)
    # Job metadata
    enabled: bool = True
    # Timezone for cron expressions (default: UTC)
    timezone: str = "UTC"
    # Timeout for this job (overrides default)
    timeout: Optional[int] = None
    # Next run time (calculated)
    next_run: Optional[float] = None
    # Last run time
    last_run: Optional[float] = None
    # Last result
    last_result: Optional[str] = None
    # Number of times run
    run_count: int = 0
    # Number of consecutive failures
    failure_count: int = 0

    def calculate_next_run(self) -> Optional[float]:
        """Calculate next run time based on schedule."""
        if self.job_type == JobType.RECURRING:
            try:
                cron = croniter(self.schedule, start_time=datetime.now(timezone.utc))
                return cron.get_next()
            except Exception as e:
                logger.error(f"Invalid cron expression '{self.schedule}': {e}")
                return None
        elif self.job_type == JobType.ONE_TIME:
            try:
                dt = datetime.fromisoformat(self.schedule.replace("Z", "+00:00"))
                return dt.timestamp()
            except Exception as e:
                logger.error(f"Invalid one-time schedule '{self.schedule}': {e}")
                return None
        return None

    def to_dict(self) -> Dict[str, Any]:
        """Convert job to dictionary."""
        return {
            "id": self.id,
            "name": self.name,
            "schedule": self.schedule,
            "job_type": self.job_type.value,
            "handler": self.handler,
            "args": self.args,
            "kwargs": self.kwargs,
            "enabled": self.enabled,
            "timezone": self.timezone,
            "timeout": self.timeout,
            "next_run": self.next_run,
            "last_run": self.last_run,
            "last_result": self.last_result,
            "run_count": self.run_count,
            "failure_count": self.failure_count,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Job":
        """Create job from dictionary."""
        return cls(
            id=data["id"],
            name=data["name"],
            schedule=data["schedule"],
            job_type=JobType(data["job_type"]),
            handler=data["handler"],
            args=data.get("args", []),
            kwargs=data.get("kwargs", {}),
            enabled=data.get("enabled", True),
            timezone=data.get("timezone", "UTC"),
            timeout=data.get("timeout"),
            next_run=data.get("next_run"),
            last_run=data.get("last_run"),
            last_result=data.get("last_result"),
            run_count=data.get("run_count", 0),
            failure_count=data.get("failure_count", 0),
        )


@dataclass
class JobExecution:
    """Job execution record."""

    job_id: str
    start_time: float
    end_time: Optional[float] = None
    status: JobStatus = JobStatus.PENDING
    result: Optional[str] = None
    error: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert execution to dictionary."""
        return {
            "job_id": self.job_id,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "status": self.status.value,
            "result": self.result,
            "error": self.error,
        }


class JobScheduler:
    """
    Cron job scheduler for LuminaGuard daemon.

    Provides:
    - Cron expression parsing and job scheduling
    - One-time and recurring job support
    - Job persistence across restarts
    - Execution history and logging
    - Configurable timeouts and concurrency
    """

    def __init__(
        self,
        config: Optional[JobConfig] = None,
        handlers: Optional[Dict[str, Callable[..., Awaitable[Any]]]] = None,
    ):
        """
        Initialize job scheduler.

        Args:
            config: Job scheduler configuration
            handlers: Dictionary of handler functions by name
        """
        self.config = config or JobConfig()
        self.handlers: Dict[str, Callable[..., Awaitable[Any]]] = handlers or {}
        self.jobs: Dict[str, Job] = {}
        self._running = False
        self._tasks: Dict[str, asyncio.Task] = {}
        self._execution_history: List[JobExecution] = []
        self._max_history = 100  # Keep last 100 executions

    def register_handler(
        self, name: str, handler: Callable[..., Awaitable[Any]]
    ) -> None:
        """
        Register a job handler.

        Args:
            name: Handler name (used in job definitions)
            handler: Async callable to execute
        """
        self.handlers[name] = handler
        logger.info(f"Registered job handler: {name}")

    def add_job(self, job: Job) -> None:
        """
        Add a job to the scheduler.

        Args:
            job: Job to add
        """
        # Calculate initial next_run
        job.next_run = job.calculate_next_run()
        self.jobs[job.id] = job
        logger.info(f"Added job: {job.name} (id={job.id}, next_run={job.next_run})")

        # Persist jobs
        if self.config.persistence_enabled:
            self._persist_jobs()

    def remove_job(self, job_id: str) -> bool:
        """
        Remove a job from the scheduler.

        Args:
            job_id: ID of job to remove

        Returns:
            True if job was removed, False if not found
        """
        if job_id in self.jobs:
            job = self.jobs[job_id]

            # Cancel running task if any
            if job_id in self._tasks:
                self._tasks[job_id].cancel()
                del self._tasks[job_id]

            del self.jobs[job_id]
            logger.info(f"Removed job: {job.name} (id={job_id})")

            # Persist jobs
            if self.config.persistence_enabled:
                self._persist_jobs()
            return True
        return False

    def get_job(self, job_id: str) -> Optional[Job]:
        """Get a job by ID."""
        return self.jobs.get(job_id)

    def list_jobs(self) -> List[Job]:
        """List all jobs."""
        return list(self.jobs.values())

    def get_next_runs(self, limit: int = 10) -> List[Dict[str, Any]]:
        """Get upcoming job runs."""
        upcoming = []
        for job in self.jobs.values():
            if job.enabled and job.next_run:
                upcoming.append(
                    {
                        "id": job.id,
                        "name": job.name,
                        "next_run": job.next_run,
                        "schedule": job.schedule,
                    }
                )

        # Sort by next_run
        upcoming.sort(key=lambda x: x["next_run"])
        return upcoming[:limit]

    async def start(self) -> None:
        """Start the job scheduler."""
        if self._running:
            logger.warning("Job scheduler already running")
            return

        logger.info("Starting job scheduler")
        self._running = True

        # Load persisted jobs
        if self.config.persistence_enabled:
            self._load_jobs()

        # Start scheduling loop
        asyncio.create_task(self._scheduler_loop())

        logger.info(f"Job scheduler started with {len(self.jobs)} jobs")

    async def stop(self) -> None:
        """Stop the job scheduler."""
        if not self._running:
            return

        logger.info("Stopping job scheduler")
        self._running = False

        # Cancel all running tasks
        for task in self._tasks.values():
            task.cancel()

        # Wait for tasks to complete
        if self._tasks:
            await asyncio.gather(*self._tasks.values(), return_exceptions=True)

        self._tasks.clear()

        # Persist jobs before stopping
        if self.config.persistence_enabled:
            self._persist_jobs()

        logger.info("Job scheduler stopped")

    async def _scheduler_loop(self) -> None:
        """Main scheduler loop."""
        check_interval = 1  # Check every second

        while self._running:
            try:
                await asyncio.sleep(check_interval)

                current_time = time.time()

                # Check each enabled job
                for job in self.jobs.values():
                    if not job.enabled:
                        continue

                    # Skip if job is currently running
                    if job.id in self._tasks:
                        continue

                    # Skip if max concurrent jobs reached
                    if len(self._tasks) >= self.config.max_concurrent:
                        break

                    # Check if it's time to run
                    if job.next_run and current_time >= job.next_run:
                        # Run the job
                        asyncio.create_task(self._run_job(job))

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in scheduler loop: {e}")

    async def _run_job(self, job: Job) -> None:
        """Run a job."""
        job_id = job.id
        logger.info(f"Starting job: {job.name} (id={job_id})")

        # Create execution record
        execution = JobExecution(
            job_id=job_id,
            start_time=time.time(),
            status=JobStatus.RUNNING,
        )
        self._execution_history.append(execution)

        # Trim history if needed
        if len(self._execution_history) > self._max_history:
            self._execution_history = self._execution_history[-self._max_history :]

        try:
            # Get handler
            handler = self.handlers.get(job.handler)
            if not handler:
                raise ValueError(f"Handler '{job.handler}' not found")

            # Get timeout
            timeout = job.timeout or self.config.default_timeout

            # Run handler with timeout
            result = await asyncio.wait_for(
                handler(*job.args, **job.kwargs),
                timeout=timeout,
            )

            # Record success
            execution.end_time = time.time()
            execution.status = JobStatus.COMPLETED
            execution.result = json.dumps(result) if result else None

            job.last_run = time.time()
            job.run_count += 1
            job.failure_count = 0
            job.last_result = execution.result

            logger.info(
                f"Job completed: {job.name} (id={job_id}) "
                f"in {execution.end_time - execution.start_time:.2f}s"
            )

        except asyncio.TimeoutError:
            execution.end_time = time.time()
            execution.status = JobStatus.FAILED
            execution.error = f"Job timed out after {timeout}s"

            job.failure_count += 1
            logger.error(f"Job timed out: {job.name} (id={job_id})")

        except Exception as e:
            execution.end_time = time.time()
            execution.status = JobStatus.FAILED
            execution.error = str(e)

            job.failure_count += 1
            logger.error(f"Job failed: {job.name} (id={job_id}): {e}")

        finally:
            # Remove from running tasks
            if job_id in self._tasks:
                del self._tasks[job_id]

        # This runs after try/except, not in finally
        # Calculate next run time
        if job.job_type == JobType.RECURRING:
            job.next_run = job.calculate_next_run()
            # Persist jobs
            if self.config.persistence_enabled:
                self._persist_jobs()
        else:
            # One-time job, remove after completion
            if job.last_run:
                # Store job name for logging before removal
                job_name = job.name
                self.remove_job(job_id)
                logger.info(f"One-time job completed and removed: {job_name}")

    def _get_persistence_path(self) -> Path:
        """Get the persistence file path."""
        path = os.path.expanduser(self.config.persistence_path)
        return Path(path).parent

    def _persist_jobs(self) -> None:
        """Persist jobs to disk."""
        try:
            path = Path(os.path.expanduser(self.config.persistence_path))
            path.parent.mkdir(parents=True, exist_ok=True)

            data = {
                "version": 1,
                "jobs": [job.to_dict() for job in self.jobs.values()],
            }

            with open(path, "w") as f:
                json.dump(data, f, indent=2)

            logger.debug(f"Persisted {len(self.jobs)} jobs to {path}")

        except Exception as e:
            logger.error(f"Failed to persist jobs: {e}")

    def _load_jobs(self) -> None:
        """Load jobs from disk."""
        try:
            path = Path(os.path.expanduser(self.config.persistence_path))

            if not path.exists():
                logger.debug("No persisted jobs found")
                return

            with open(path, "r") as f:
                data = json.load(f)

            # Load jobs
            for job_data in data.get("jobs", []):
                try:
                    job = Job.from_dict(job_data)
                    # Recalculate next_run if job is enabled
                    if job.enabled:
                        job.next_run = job.calculate_next_run()
                    self.jobs[job.id] = job
                except Exception as e:
                    logger.error(f"Failed to load job: {e}")

            logger.info(f"Loaded {len(self.jobs)} jobs from persistence")

        except Exception as e:
            logger.error(f"Failed to load jobs: {e}")

    def get_history(
        self, job_id: Optional[str] = None, limit: int = 50
    ) -> List[Dict[str, Any]]:
        """
        Get job execution history.

        Args:
            job_id: Filter by job ID (optional)
            limit: Maximum number of records to return

        Returns:
            List of execution records
        """
        history = self._execution_history

        if job_id:
            history = [e for e in history if e.job_id == job_id]

        # Return most recent first
        history = history[-limit:]
        history.reverse()

        return [e.to_dict() for e in history]


async def create_job_scheduler(
    config: Optional[JobConfig] = None,
    handlers: Optional[Dict[str, Callable[..., Awaitable[Any]]]] = None,
) -> JobScheduler:
    """
    Create and start a job scheduler.

    Args:
        config: Job scheduler configuration
        handlers: Dictionary of handler functions

    Returns:
        Started JobScheduler instance
    """
    scheduler = JobScheduler(config=config, handlers=handlers)
    await scheduler.start()
    return scheduler
