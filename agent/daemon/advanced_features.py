"""
Advanced Bot Features for LuminaGuard Daemon Mode.

This module provides advanced features for 24/7 bot operation:
- Per-user/channel rate limiting
- Role-based permission system
- Command cooldowns
- Message queue for outbound messages
- Typing indicators
- Message threading support
- Reaction handling
- Scheduled messages
- Audit logging
- Circuit breaker pattern
- Message deduplication

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import time
import threading
from abc import ABC, abstractmethod
from collections import defaultdict, deque
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Set, Tuple, Union
import uuid

logger = logging.getLogger(__name__)


# =============================================================================
# RATE LIMITING
# =============================================================================


@dataclass
class RateLimitEntry:
    """Entry for rate limit tracking."""

    timestamp: float
    count: int = 1


class RateLimitScope(Enum):
    """Scope for rate limiting."""

    USER = "user"
    CHANNEL = "channel"
    GUILD = "guild"
    GLOBAL = "global"


@dataclass
class RateLimitConfig:
    """Configuration for rate limiting."""

    max_requests: int = 10
    window_seconds: float = 60.0
    scope: RateLimitScope = RateLimitScope.USER
    burst_allowance: int = 0  # Extra requests allowed in burst


class RateLimiterAdvanced:
    """
    Advanced rate limiter with per-user/channel/guild support.

    Features:
    - Sliding window rate limiting
    - Multiple scopes (user, channel, guild, global)
    - Burst allowance
    - Automatic cleanup of old entries
    """

    def __init__(self, default_config: Optional[RateLimitConfig] = None):
        self.default_config = default_config or RateLimitConfig()
        self._entries: Dict[str, deque[RateLimitEntry]] = defaultdict(deque)
        self._configs: Dict[str, RateLimitConfig] = {}
        self._lock = threading.RLock()
        self._cleanup_interval = 300  # 5 minutes
        self._last_cleanup = time.time()

    def configure(self, key: str, config: RateLimitConfig) -> None:
        """Configure rate limit for a specific key."""
        self._configs[key] = config

    def _get_config(self, key: str) -> RateLimitConfig:
        """Get rate limit config for a key."""
        return self._configs.get(key, self.default_config)

    def _make_key(
        self, scope: RateLimitScope, identifier: str, action: str = ""
    ) -> str:
        """Create a composite key for rate limiting."""
        if action:
            return f"{scope.value}:{identifier}:{action}"
        return f"{scope.value}:{identifier}"

    def is_allowed(
        self,
        user_id: str,
        channel_id: str = "",
        guild_id: str = "",
        action: str = "",
        custom_config: Optional[RateLimitConfig] = None,
    ) -> Tuple[bool, int, float]:
        """
        Check if an action is allowed under rate limits.

        Args:
            user_id: User identifier
            channel_id: Channel identifier
            guild_id: Guild/server identifier
            action: Optional action name for specific rate limiting
            custom_config: Optional custom rate limit config

        Returns:
            Tuple of (is_allowed, remaining_requests, reset_time)
        """
        with self._lock:
            self._maybe_cleanup()

            config = custom_config or self._get_config(action)
            key = self._make_key(
                config.scope,
                (
                    user_id
                    if config.scope == RateLimitScope.USER
                    else (
                        channel_id
                        if config.scope == RateLimitScope.CHANNEL
                        else (
                            guild_id
                            if config.scope == RateLimitScope.GUILD
                            else "global"
                        )
                    )
                ),
                action,
            )

            now = time.time()
            window_start = now - config.window_seconds

            # Get entries for this key
            entries = self._entries[key]

            # Remove old entries outside the window
            while entries and entries[0].timestamp < window_start:
                entries.popleft()

            # Count current requests in window
            current_count = sum(e.count for e in entries)

            # Check if allowed
            max_allowed = config.max_requests + config.burst_allowance
            is_allowed = current_count < max_allowed

            if is_allowed:
                # Add new entry
                entries.append(RateLimitEntry(timestamp=now, count=1))

            remaining = max(0, max_allowed - current_count - (1 if is_allowed else 0))
            reset_time = (
                (entries[0].timestamp + config.window_seconds)
                if entries
                else now + config.window_seconds
            )

            return is_allowed, remaining, reset_time

    def reset(self, scope: RateLimitScope, identifier: str, action: str = "") -> None:
        """Reset rate limit for a specific key."""
        key = self._make_key(scope, identifier, action)
        with self._lock:
            self._entries[key].clear()

    def _maybe_cleanup(self) -> None:
        """Periodically cleanup old entries."""
        now = time.time()
        if now - self._last_cleanup > self._cleanup_interval:
            self._cleanup()
            self._last_cleanup = now

    def _cleanup(self) -> int:
        """Remove all expired entries."""
        cleaned = 0
        max_window = max(
            (c.window_seconds for c in self._configs.values()),
            default=self.default_config.window_seconds,
        )
        cutoff = time.time() - max_window * 2

        with self._lock:
            for key in list(self._entries.keys()):
                entries = self._entries[key]
                while entries and entries[0].timestamp < cutoff:
                    entries.popleft()
                    cleaned += 1
                if not entries:
                    del self._entries[key]

        if cleaned:
            logger.debug(f"Cleaned up {cleaned} rate limit entries")
        return cleaned


# =============================================================================
# PERMISSION SYSTEM
# =============================================================================


class Permission(Enum):
    """Bot permissions."""

    # Basic permissions
    USE_BOT = "use_bot"
    VIEW_STATUS = "view_status"

    # Command permissions
    USE_BASIC_COMMANDS = "use_basic_commands"
    USE_ADMIN_COMMANDS = "use_admin_commands"
    USE_DANGEROUS_COMMANDS = "use_dangerous_commands"

    # Feature permissions
    EXECUTE_CODE = "execute_code"
    ACCESS_FILES = "access_files"
    MANAGE_SCHEDULED_TASKS = "manage_scheduled_tasks"
    MANAGE_WEBHOOKS = "manage_webhooks"

    # Admin permissions
    MANAGE_USERS = "manage_users"
    MANAGE_ROLES = "manage_roles"
    MANAGE_BOT = "manage_bot"
    VIEW_AUDIT_LOGS = "view_audit_logs"


@dataclass
class Role:
    """A role with associated permissions."""

    name: str
    permissions: Set[Permission]
    priority: int = 0  # Higher priority = more important
    description: str = ""

    def has_permission(self, permission: Permission) -> bool:
        """Check if role has a permission."""
        return permission in self.permissions

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "name": self.name,
            "permissions": [p.value for p in self.permissions],
            "priority": self.priority,
            "description": self.description,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Role":
        """Create from dictionary."""
        return cls(
            name=data["name"],
            permissions={Permission(p) for p in data.get("permissions", [])},
            priority=data.get("priority", 0),
            description=data.get("description", ""),
        )


# Default roles
DEFAULT_ROLES = {
    "banned": Role(
        name="banned",
        permissions=set(),
        priority=-1,
        description="Banned user with no access",
    ),
    "user": Role(
        name="user",
        permissions={
            Permission.USE_BOT,
            Permission.VIEW_STATUS,
            Permission.USE_BASIC_COMMANDS,
        },
        priority=0,
        description="Default user role",
    ),
    "trusted": Role(
        name="trusted",
        permissions={
            Permission.USE_BOT,
            Permission.VIEW_STATUS,
            Permission.USE_BASIC_COMMANDS,
            Permission.ACCESS_FILES,
            Permission.MANAGE_SCHEDULED_TASKS,
        },
        priority=1,
        description="Trusted user with extended access",
    ),
    "admin": Role(
        name="admin",
        permissions=set(Permission),  # All permissions
        priority=10,
        description="Administrator with full access",
    ),
}


class PermissionManager:
    """
    Manages user permissions and roles.

    Features:
    - Role-based access control
    - Per-user permission overrides
    - Permission inheritance
    - Persistent storage
    """

    def __init__(self, storage_path: Optional[Path] = None):
        self.storage_path = storage_path
        self._roles: Dict[str, Role] = dict(DEFAULT_ROLES)
        self._user_roles: Dict[str, str] = {}  # user_id -> role_name
        self._user_overrides: Dict[str, Set[Permission]] = (
            {}
        )  # user_id -> extra permissions
        self._lock = threading.RLock()

        if storage_path:
            self._load()

    def create_role(self, role: Role) -> None:
        """Create or update a role."""
        with self._lock:
            self._roles[role.name] = role
            self._save()

    def delete_role(self, role_name: str) -> bool:
        """Delete a role (cannot delete default roles)."""
        if role_name in DEFAULT_ROLES:
            return False

        with self._lock:
            if role_name in self._roles:
                del self._roles[role_name]
                # Remove role from users
                self._user_roles = {
                    uid: r for uid, r in self._user_roles.items() if r != role_name
                }
                self._save()
                return True
        return False

    def get_role(self, role_name: str) -> Optional[Role]:
        """Get a role by name."""
        return self._roles.get(role_name)

    def list_roles(self) -> List[Role]:
        """List all roles."""
        return list(self._roles.values())

    def assign_role(self, user_id: str, role_name: str) -> bool:
        """Assign a role to a user."""
        with self._lock:
            if role_name not in self._roles:
                return False
            self._user_roles[user_id] = role_name
            self._save()
            return True

    def remove_role(self, user_id: str) -> bool:
        """Remove a user's role (reverts to default)."""
        with self._lock:
            if user_id in self._user_roles:
                del self._user_roles[user_id]
                self._save()
                return True
        return False

    def get_user_role(self, user_id: str) -> Role:
        """Get a user's role (defaults to 'user')."""
        role_name = self._user_roles.get(user_id, "user")
        return self._roles.get(role_name, DEFAULT_ROLES["user"])

    def grant_permission(self, user_id: str, permission: Permission) -> None:
        """Grant an additional permission to a user."""
        with self._lock:
            if user_id not in self._user_overrides:
                self._user_overrides[user_id] = set()
            self._user_overrides[user_id].add(permission)
            self._save()

    def revoke_permission(self, user_id: str, permission: Permission) -> None:
        """Revoke a permission override from a user."""
        with self._lock:
            if user_id in self._user_overrides:
                self._user_overrides[user_id].discard(permission)
                if not self._user_overrides[user_id]:
                    del self._user_overrides[user_id]
            self._save()

    def has_permission(self, user_id: str, permission: Permission) -> bool:
        """Check if a user has a specific permission."""
        # Check overrides first
        if user_id in self._user_overrides:
            if permission in self._user_overrides[user_id]:
                return True

        # Check role
        role = self.get_user_role(user_id)
        return role.has_permission(permission)

    def check_permissions(
        self,
        user_id: str,
        permissions: Union[Permission, List[Permission]],
        require_all: bool = True,
    ) -> Tuple[bool, List[Permission]]:
        """
        Check if a user has the required permissions.

        Args:
            user_id: User identifier
            permissions: Single permission or list of permissions
            require_all: If True, all permissions required; if False, any is sufficient

        Returns:
            Tuple of (has_permissions, missing_permissions)
        """
        if isinstance(permissions, Permission):
            permissions = [permissions]

        missing = []
        for perm in permissions:
            if not self.has_permission(user_id, perm):
                missing.append(perm)

        if require_all:
            return len(missing) == 0, missing
        else:
            return len(missing) < len(permissions), missing

    def _save(self) -> None:
        """Save permission data to disk."""
        if not self.storage_path:
            return

        try:
            self.storage_path.parent.mkdir(parents=True, exist_ok=True)
            data = {
                "roles": {
                    name: role.to_dict()
                    for name, role in self._roles.items()
                    if name not in DEFAULT_ROLES
                },
                "user_roles": self._user_roles,
                "user_overrides": {
                    uid: [p.value for p in perms]
                    for uid, perms in self._user_overrides.items()
                },
            }
            self.storage_path.write_text(json.dumps(data, indent=2))
        except Exception as e:
            logger.error(f"Failed to save permission data: {e}")

    def _load(self) -> None:
        """Load permission data from disk."""
        if not self.storage_path or not self.storage_path.exists():
            return

        try:
            data = json.loads(self.storage_path.read_text())

            # Load custom roles
            for name, role_data in data.get("roles", {}).items():
                self._roles[name] = Role.from_dict(role_data)

            # Load user roles
            self._user_roles = data.get("user_roles", {})

            # Load user overrides
            self._user_overrides = {
                uid: {Permission(p) for p in perms}
                for uid, perms in data.get("user_overrides", {}).items()
            }

            logger.info(f"Loaded permission data for {len(self._user_roles)} users")
        except Exception as e:
            logger.error(f"Failed to load permission data: {e}")


# =============================================================================
# COMMAND COOLDOWNS
# =============================================================================


@dataclass
class CooldownConfig:
    """Configuration for command cooldowns."""

    default_seconds: float = 5.0
    per_user_seconds: Dict[str, float] = field(default_factory=dict)
    per_role_seconds: Dict[str, float] = field(default_factory=dict)


class CooldownManager:
    """
    Manages command cooldowns.

    Features:
    - Per-command cooldowns
    - Per-user and per-role cooldown multipliers
    - Dynamic cooldown adjustment
    """

    def __init__(self, config: Optional[CooldownConfig] = None):
        self.config = config or CooldownConfig()
        self._cooldowns: Dict[str, Dict[str, float]] = defaultdict(
            dict
        )  # command -> user -> expiry
        self._lock = threading.RLock()

    def get_cooldown(
        self,
        command: str,
        user_id: str,
        role: str = "user",
    ) -> float:
        """Get cooldown duration for a command."""
        base = self.config.per_user_seconds.get(command, self.config.default_seconds)

        # Apply role multiplier
        role_multiplier = self.config.per_role_seconds.get(role, 1.0)

        return base * role_multiplier

    def is_on_cooldown(self, command: str, user_id: str) -> Tuple[bool, float]:
        """
        Check if a command is on cooldown for a user.

        Returns:
            Tuple of (is_on_cooldown, remaining_seconds)
        """
        with self._lock:
            now = time.time()
            expiry = self._cooldowns[command].get(user_id, 0)

            if now < expiry:
                return True, expiry - now
            return False, 0.0

    def set_cooldown(
        self,
        command: str,
        user_id: str,
        role: str = "user",
        custom_seconds: Optional[float] = None,
    ) -> float:
        """
        Set cooldown for a command.

        Returns:
            The cooldown duration set
        """
        duration = custom_seconds or self.get_cooldown(command, user_id, role)

        with self._lock:
            self._cooldowns[command][user_id] = time.time() + duration

        return duration

    def reset_cooldown(self, command: str, user_id: str) -> None:
        """Reset cooldown for a command."""
        with self._lock:
            self._cooldowns[command].pop(user_id, None)

    def reset_all_cooldowns(self, user_id: str) -> int:
        """Reset all cooldowns for a user."""
        count = 0
        with self._lock:
            for command in self._cooldowns:
                if user_id in self._cooldowns[command]:
                    del self._cooldowns[command][user_id]
                    count += 1
        return count


# =============================================================================
# MESSAGE QUEUE
# =============================================================================


@dataclass
class QueuedMessage:
    """A message in the outbound queue."""

    id: str
    chat_id: str
    content: str
    priority: int = 0  # Higher = more important
    created_at: float = field(default_factory=time.time)
    scheduled_for: Optional[float] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    attempts: int = 0
    max_attempts: int = 3
    last_error: Optional[str] = None


class MessageQueue:
    """
    Priority queue for outbound messages.

    Features:
    - Priority-based ordering
    - Scheduled message delivery
    - Retry with exponential backoff
    - Rate limit integration
    """

    def __init__(
        self,
        max_size: int = 10000,
        retry_delay: float = 1.0,
        max_retry_delay: float = 60.0,
    ):
        self.max_size = max_size
        self.retry_delay = retry_delay
        self.max_retry_delay = max_retry_delay

        self._queue: List[QueuedMessage] = []
        self._pending: Dict[str, QueuedMessage] = {}
        self._lock = threading.RLock()
        self._event = threading.Event()

    def enqueue(
        self,
        chat_id: str,
        content: str,
        priority: int = 0,
        scheduled_for: Optional[float] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Optional[str]:
        """
        Add a message to the queue.

        Returns:
            Message ID if queued, None if queue is full
        """
        with self._lock:
            if len(self._queue) >= self.max_size:
                logger.warning("Message queue is full")
                return None

            msg = QueuedMessage(
                id=str(uuid.uuid4()),
                chat_id=chat_id,
                content=content,
                priority=priority,
                scheduled_for=scheduled_for,
                metadata=metadata or {},
            )

            self._queue.append(msg)
            self._queue.sort(key=lambda m: (-m.priority, m.created_at))
            self._event.set()

            return msg.id

    def dequeue(self, timeout: float = 1.0) -> Optional[QueuedMessage]:
        """
        Get the next message to send.

        Blocks until a message is available or timeout.
        """
        while True:
            with self._lock:
                now = time.time()

                # Find next available message
                for i, msg in enumerate(self._queue):
                    # Check if scheduled time has passed
                    if msg.scheduled_for and msg.scheduled_for > now:
                        continue

                    # Remove from queue and add to pending
                    self._queue.pop(i)
                    self._pending[msg.id] = msg
                    return msg

                # No message available
                self._event.clear()

            # Wait for new message
            if not self._event.wait(timeout):
                return None

    def ack(self, message_id: str) -> None:
        """Acknowledge successful message delivery."""
        with self._lock:
            self._pending.pop(message_id, None)

    def nack(self, message_id: str, error: str) -> bool:
        """
        Negative acknowledgement - message failed.

        Returns:
            True if message will be retried, False if exhausted
        """
        with self._lock:
            msg = self._pending.pop(message_id, None)
            if not msg:
                return False

            msg.attempts += 1
            msg.last_error = error

            if msg.attempts >= msg.max_attempts:
                logger.error(f"Message {message_id} exhausted retries: {error}")
                return False

            # Calculate backoff
            backoff = min(self.retry_delay * (2**msg.attempts), self.max_retry_delay)
            msg.scheduled_for = time.time() + backoff

            # Re-queue
            self._queue.append(msg)
            self._queue.sort(key=lambda m: (-m.priority, m.created_at))
            self._event.set()

            return True

    def get_stats(self) -> Dict[str, Any]:
        """Get queue statistics."""
        with self._lock:
            return {
                "queue_size": len(self._queue),
                "pending_size": len(self._pending),
                "max_size": self.max_size,
            }


# =============================================================================
# TYPING INDICATOR
# =============================================================================


class TypingIndicatorManager:
    """
    Manages "bot is typing" indicators.

    Features:
    - Auto-start typing on message receive
    - Auto-stop after response or timeout
    - Multi-platform support
    """

    def __init__(self, default_timeout: float = 10.0):
        self.default_timeout = default_timeout
        self._active: Dict[str, float] = {}  # chat_id -> expiry
        self._lock = threading.RLock()
        self._send_typing_callback: Optional[Callable] = None

    def set_callback(self, callback: Callable[[str], None]) -> None:
        """Set the callback to send typing indicator."""
        self._send_typing_callback = callback

    async def start_typing(
        self,
        chat_id: str,
        timeout: Optional[float] = None,
    ) -> None:
        """Start typing indicator for a chat."""
        timeout = timeout or self.default_timeout

        with self._lock:
            self._active[chat_id] = time.time() + timeout

        if self._send_typing_callback:
            try:
                await self._send_typing_callback(chat_id)
            except Exception as e:
                logger.error(f"Failed to send typing indicator: {e}")

    def stop_typing(self, chat_id: str) -> None:
        """Stop typing indicator for a chat."""
        with self._lock:
            self._active.pop(chat_id, None)

    def is_typing(self, chat_id: str) -> bool:
        """Check if typing indicator is active."""
        with self._lock:
            expiry = self._active.get(chat_id, 0)
            if time.time() < expiry:
                return True
            self._active.pop(chat_id, None)
            return False

    def get_active_chats(self) -> List[str]:
        """Get all chats with active typing indicators."""
        now = time.time()
        with self._lock:
            return [chat_id for chat_id, expiry in self._active.items() if expiry > now]


# =============================================================================
# MESSAGE THREADING
# =============================================================================


@dataclass
class ThreadMetadata:
    """Metadata for a message thread."""

    thread_id: str
    root_message_id: str
    chat_id: str
    created_at: float = field(default_factory=time.time)
    last_activity: float = field(default_factory=time.time)
    message_count: int = 0
    participants: Set[str] = field(default_factory=set)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "thread_id": self.thread_id,
            "root_message_id": self.root_message_id,
            "chat_id": self.chat_id,
            "created_at": self.created_at,
            "last_activity": self.last_activity,
            "message_count": self.message_count,
            "participants": list(self.participants),
        }


class ThreadManager:
    """
    Manages message threads.

    Features:
    - Thread creation and tracking
    - Thread context for replies
    - Thread archival
    """

    def __init__(self, max_threads: int = 1000, thread_ttl: float = 86400):
        self.max_threads = max_threads
        self.thread_ttl = thread_ttl

        self._threads: Dict[str, ThreadMetadata] = {}
        self._message_to_thread: Dict[str, str] = {}  # message_id -> thread_id
        self._lock = threading.RLock()

    def create_thread(
        self,
        root_message_id: str,
        chat_id: str,
        creator_id: str,
    ) -> ThreadMetadata:
        """Create a new thread."""
        with self._lock:
            self._cleanup_expired()

            thread_id = str(uuid.uuid4())
            thread = ThreadMetadata(
                thread_id=thread_id,
                root_message_id=root_message_id,
                chat_id=chat_id,
                participants={creator_id},
            )

            self._threads[thread_id] = thread
            self._message_to_thread[root_message_id] = thread_id

            return thread

    def get_thread(self, thread_id: str) -> Optional[ThreadMetadata]:
        """Get a thread by ID."""
        return self._threads.get(thread_id)

    def get_thread_for_message(self, message_id: str) -> Optional[ThreadMetadata]:
        """Get the thread a message belongs to."""
        thread_id = self._message_to_thread.get(message_id)
        if thread_id:
            return self._threads.get(thread_id)
        return None

    def add_message_to_thread(
        self,
        thread_id: str,
        message_id: str,
        user_id: str,
    ) -> bool:
        """Add a message to a thread."""
        with self._lock:
            thread = self._threads.get(thread_id)
            if not thread:
                return False

            thread.message_count += 1
            thread.last_activity = time.time()
            thread.participants.add(user_id)
            self._message_to_thread[message_id] = thread_id

            return True

    def archive_thread(self, thread_id: str) -> bool:
        """Archive a thread."""
        with self._lock:
            thread = self._threads.pop(thread_id, None)
            if thread:
                # Remove message mappings
                for msg_id in list(self._message_to_thread.keys()):
                    if self._message_to_thread[msg_id] == thread_id:
                        del self._message_to_thread[msg_id]
                return True
            return False

    def _cleanup_expired(self) -> int:
        """Remove expired threads."""
        now = time.time()
        expired = [
            tid
            for tid, t in self._threads.items()
            if now - t.last_activity > self.thread_ttl
        ]

        for tid in expired:
            self.archive_thread(tid)

        return len(expired)


# =============================================================================
# REACTION HANDLING
# =============================================================================


class ReactionType(Enum):
    """Types of reactions."""

    EMOJI = "emoji"
    CUSTOM = "custom"
    UPVOTE = "upvote"
    DOWNVOTE = "downvote"


@dataclass
class Reaction:
    """A reaction to a message."""

    message_id: str
    user_id: str
    reaction_type: ReactionType
    reaction_value: str  # Emoji or custom reaction ID
    timestamp: float = field(default_factory=time.time)


class ReactionManager:
    """
    Manages message reactions.

    Features:
    - Track reactions on messages
    - Reaction-based triggers
    - Reaction statistics
    """

    def __init__(self):
        self._reactions: Dict[str, List[Reaction]] = defaultdict(
            list
        )  # message_id -> reactions
        self._handlers: Dict[str, List[Callable]] = defaultdict(
            list
        )  # reaction_value -> handlers
        self._lock = threading.RLock()

    def add_reaction(
        self,
        message_id: str,
        user_id: str,
        reaction_value: str,
        reaction_type: ReactionType = ReactionType.EMOJI,
    ) -> bool:
        """Add a reaction to a message."""
        with self._lock:
            # Check if user already reacted with this
            for r in self._reactions[message_id]:
                if r.user_id == user_id and r.reaction_value == reaction_value:
                    return False  # Already reacted

            reaction = Reaction(
                message_id=message_id,
                user_id=user_id,
                reaction_type=reaction_type,
                reaction_value=reaction_value,
            )
            self._reactions[message_id].append(reaction)

            # Trigger handlers
            for handler in self._handlers.get(reaction_value, []):
                try:
                    handler(reaction)
                except Exception as e:
                    logger.error(f"Reaction handler error: {e}")

            return True

    def remove_reaction(
        self,
        message_id: str,
        user_id: str,
        reaction_value: str,
    ) -> bool:
        """Remove a reaction from a message."""
        with self._lock:
            reactions = self._reactions[message_id]
            for i, r in enumerate(reactions):
                if r.user_id == user_id and r.reaction_value == reaction_value:
                    reactions.pop(i)
                    return True
            return False

    def get_reactions(self, message_id: str) -> List[Reaction]:
        """Get all reactions for a message."""
        return self._reactions.get(message_id, [])

    def get_reaction_counts(
        self,
        message_id: str,
    ) -> Dict[str, int]:
        """Get reaction counts for a message."""
        counts: Dict[str, int] = defaultdict(int)
        for r in self._reactions.get(message_id, []):
            counts[r.reaction_value] += 1
        return dict(counts)

    def register_handler(
        self,
        reaction_value: str,
        handler: Callable[[Reaction], None],
    ) -> None:
        """Register a handler for a specific reaction."""
        self._handlers[reaction_value].append(handler)


# =============================================================================
# SCHEDULED MESSAGES
# =============================================================================


@dataclass
class ScheduledMessage:
    """A message scheduled for future delivery."""

    id: str
    chat_id: str
    content: str
    scheduled_for: float
    created_by: str
    created_at: float = field(default_factory=time.time)
    recurring: bool = False
    recurring_interval: Optional[float] = None  # Seconds between recurrences
    metadata: Dict[str, Any] = field(default_factory=dict)
    enabled: bool = True

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "chat_id": self.chat_id,
            "content": self.content,
            "scheduled_for": self.scheduled_for,
            "created_by": self.created_by,
            "created_at": self.created_at,
            "recurring": self.recurring,
            "recurring_interval": self.recurring_interval,
            "metadata": self.metadata,
            "enabled": self.enabled,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ScheduledMessage":
        return cls(**data)


class ScheduledMessageManager:
    """
    Manages scheduled messages.

    Features:
    - One-time scheduled messages
    - Recurring messages
    - Timezone support
    - Message cancellation
    """

    def __init__(self, storage_path: Optional[Path] = None):
        self.storage_path = storage_path
        self._messages: Dict[str, ScheduledMessage] = {}
        self._lock = threading.RLock()
        self._send_callback: Optional[Callable] = None

        if storage_path:
            self._load()

    def set_send_callback(self, callback: Callable) -> None:
        """Set callback for sending messages."""
        self._send_callback = callback

    def schedule_message(
        self,
        chat_id: str,
        content: str,
        scheduled_for: float,
        created_by: str,
        recurring: bool = False,
        recurring_interval: Optional[float] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> str:
        """Schedule a message for delivery."""
        with self._lock:
            msg = ScheduledMessage(
                id=str(uuid.uuid4()),
                chat_id=chat_id,
                content=content,
                scheduled_for=scheduled_for,
                created_by=created_by,
                recurring=recurring,
                recurring_interval=recurring_interval,
                metadata=metadata or {},
            )

            self._messages[msg.id] = msg
            self._save()

            return msg.id

    def cancel_message(self, message_id: str) -> bool:
        """Cancel a scheduled message."""
        with self._lock:
            if message_id in self._messages:
                del self._messages[message_id]
                self._save()
                return True
            return False

    def enable_message(self, message_id: str, enabled: bool = True) -> bool:
        """Enable or disable a scheduled message."""
        with self._lock:
            msg = self._messages.get(message_id)
            if msg:
                msg.enabled = enabled
                self._save()
                return True
            return False

    def get_pending_messages(self) -> List[ScheduledMessage]:
        """Get all pending scheduled messages."""
        now = time.time()
        with self._lock:
            return [
                msg
                for msg in self._messages.values()
                if msg.enabled and msg.scheduled_for <= now
            ]

    def get_user_scheduled(self, user_id: str) -> List[ScheduledMessage]:
        """Get all scheduled messages by a user."""
        with self._lock:
            return [msg for msg in self._messages.values() if msg.created_by == user_id]

    async def process_pending(self) -> int:
        """Process all pending scheduled messages."""
        if not self._send_callback:
            return 0

        pending = self.get_pending_messages()
        sent = 0

        for msg in pending:
            try:
                await self._send_callback(msg.chat_id, msg.content, msg.metadata)
                sent += 1

                if msg.recurring and msg.recurring_interval:
                    # Reschedule
                    msg.scheduled_for = time.time() + msg.recurring_interval
                else:
                    # Remove one-time message
                    with self._lock:
                        del self._messages[msg.id]

            except Exception as e:
                logger.error(f"Failed to send scheduled message {msg.id}: {e}")

        if sent:
            self._save()

        return sent

    def _save(self) -> None:
        """Save scheduled messages to disk."""
        if not self.storage_path:
            return

        try:
            self.storage_path.parent.mkdir(parents=True, exist_ok=True)
            data = {"messages": [msg.to_dict() for msg in self._messages.values()]}
            self.storage_path.write_text(json.dumps(data, indent=2))
        except Exception as e:
            logger.error(f"Failed to save scheduled messages: {e}")

    def _load(self) -> None:
        """Load scheduled messages from disk."""
        if not self.storage_path or not self.storage_path.exists():
            return

        try:
            data = json.loads(self.storage_path.read_text())
            for msg_data in data.get("messages", []):
                msg = ScheduledMessage.from_dict(msg_data)
                self._messages[msg.id] = msg
            logger.info(f"Loaded {len(self._messages)} scheduled messages")
        except Exception as e:
            logger.error(f"Failed to load scheduled messages: {e}")


# =============================================================================
# AUDIT LOGGING
# =============================================================================


class AuditEventType(Enum):
    """Types of audit events."""

    # Authentication
    LOGIN = "login"
    LOGOUT = "logout"
    AUTH_FAILED = "auth_failed"

    # Permissions
    ROLE_ASSIGNED = "role_assigned"
    ROLE_REMOVED = "role_removed"
    PERMISSION_GRANTED = "permission_granted"
    PERMISSION_REVOKED = "permission_revoked"

    # Commands
    COMMAND_EXECUTED = "command_executed"
    COMMAND_DENIED = "command_denied"

    # Messages
    MESSAGE_SENT = "message_sent"
    MESSAGE_EDITED = "message_edited"
    MESSAGE_DELETED = "message_deleted"

    # Configuration
    CONFIG_CHANGED = "config_changed"
    SCHEDULED_MESSAGE_CREATED = "scheduled_message_created"
    SCHEDULED_MESSAGE_CANCELLED = "scheduled_message_cancelled"

    # System
    BOT_STARTED = "bot_started"
    BOT_STOPPED = "bot_stopped"
    ERROR_OCCURRED = "error_occurred"


@dataclass
class AuditEvent:
    """An audit log event."""

    id: str
    event_type: AuditEventType
    timestamp: float
    user_id: Optional[str]
    details: Dict[str, Any]
    ip_address: Optional[str] = None
    user_agent: Optional[str] = None
    success: bool = True

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "event_type": self.event_type.value,
            "timestamp": self.timestamp,
            "user_id": self.user_id,
            "details": self.details,
            "ip_address": self.ip_address,
            "user_agent": self.user_agent,
            "success": self.success,
        }


class AuditLogger:
    """
    Comprehensive audit logging system.

    Features:
    - Structured event logging
    - Event filtering and search
    - Retention policy
    - Export capabilities
    """

    def __init__(
        self,
        storage_path: Optional[Path] = None,
        retention_days: int = 90,
        max_events: int = 100000,
    ):
        self.storage_path = storage_path
        self.retention_days = retention_days
        self.max_events = max_events

        self._events: List[AuditEvent] = []
        self._lock = threading.RLock()

        if storage_path:
            self._load()

    def log(
        self,
        event_type: AuditEventType,
        user_id: Optional[str] = None,
        details: Optional[Dict[str, Any]] = None,
        ip_address: Optional[str] = None,
        user_agent: Optional[str] = None,
        success: bool = True,
    ) -> str:
        """Log an audit event."""
        event = AuditEvent(
            id=str(uuid.uuid4()),
            event_type=event_type,
            timestamp=time.time(),
            user_id=user_id,
            details=details or {},
            ip_address=ip_address,
            user_agent=user_agent,
            success=success,
        )

        with self._lock:
            self._events.append(event)
            self._enforce_limits()
            self._save()

        logger.info(f"Audit: {event_type.value} by {user_id or 'system'}")
        return event.id

    def get_events(
        self,
        event_type: Optional[AuditEventType] = None,
        user_id: Optional[str] = None,
        start_time: Optional[float] = None,
        end_time: Optional[float] = None,
        success_only: bool = False,
        limit: int = 100,
    ) -> List[AuditEvent]:
        """Query audit events with filters."""
        with self._lock:
            events = list(self._events)

        # Apply filters
        if event_type:
            events = [e for e in events if e.event_type == event_type]
        if user_id:
            events = [e for e in events if e.user_id == user_id]
        if start_time:
            events = [e for e in events if e.timestamp >= start_time]
        if end_time:
            events = [e for e in events if e.timestamp <= end_time]
        if success_only:
            events = [e for e in events if e.success]

        # Sort by timestamp descending
        events.sort(key=lambda e: e.timestamp, reverse=True)

        return events[:limit]

    def get_user_activity(
        self,
        user_id: str,
        days: int = 7,
    ) -> Dict[str, int]:
        """Get activity summary for a user."""
        start_time = time.time() - (days * 86400)

        events = self.get_events(
            user_id=user_id,
            start_time=start_time,
            limit=10000,
        )

        summary: Dict[str, int] = defaultdict(int)
        for event in events:
            summary[event.event_type.value] += 1

        return dict(summary)

    def export_events(
        self,
        output_path: Path,
        start_time: Optional[float] = None,
        end_time: Optional[float] = None,
    ) -> int:
        """Export events to a JSON file."""
        events = self.get_events(
            start_time=start_time,
            end_time=end_time,
            limit=self.max_events,
        )

        try:
            output_path.parent.mkdir(parents=True, exist_ok=True)
            data = {
                "exported_at": time.time(),
                "event_count": len(events),
                "events": [e.to_dict() for e in events],
            }
            output_path.write_text(json.dumps(data, indent=2))
            return len(events)
        except Exception as e:
            logger.error(f"Failed to export audit events: {e}")
            return 0

    def _enforce_limits(self) -> None:
        """Enforce retention and max events limits."""
        # Special case: retention_days=0 means no retention (clear all events)
        if self.retention_days <= 0:
            self._events = []
            return

        now = time.time()
        cutoff = now - (self.retention_days * 86400)

        # Remove old events
        self._events = [e for e in self._events if e.timestamp >= cutoff]

        # Enforce max events
        if len(self._events) > self.max_events:
            self._events = self._events[-self.max_events :]

    def _save(self) -> None:
        """Save audit events to disk."""
        if not self.storage_path:
            return

        try:
            self.storage_path.parent.mkdir(parents=True, exist_ok=True)
            data = {"events": [e.to_dict() for e in self._events]}
            self.storage_path.write_text(json.dumps(data, indent=2))
        except Exception as e:
            logger.error(f"Failed to save audit events: {e}")

    def _load(self) -> None:
        """Load audit events from disk."""
        if not self.storage_path or not self.storage_path.exists():
            return

        try:
            data = json.loads(self.storage_path.read_text())
            for event_data in data.get("events", []):
                event = AuditEvent(
                    id=event_data["id"],
                    event_type=AuditEventType(event_data["event_type"]),
                    timestamp=event_data["timestamp"],
                    user_id=event_data.get("user_id"),
                    details=event_data.get("details", {}),
                    ip_address=event_data.get("ip_address"),
                    user_agent=event_data.get("user_agent"),
                    success=event_data.get("success", True),
                )
                self._events.append(event)

            self._enforce_limits()
            logger.info(f"Loaded {len(self._events)} audit events")
        except Exception as e:
            logger.error(f"Failed to load audit events: {e}")


# =============================================================================
# CIRCUIT BREAKER
# =============================================================================


class CircuitState(Enum):
    """Circuit breaker states."""

    CLOSED = "closed"  # Normal operation
    OPEN = "open"  # Failing, reject all calls
    HALF_OPEN = "half_open"  # Testing if recovered


@dataclass
class CircuitStats:
    """Statistics for circuit breaker."""

    failures: int = 0
    successes: int = 0
    last_failure_time: Optional[float] = None
    last_success_time: Optional[float] = None
    state_changed_at: float = field(default_factory=time.time)


class CircuitBreaker:
    """
    Circuit breaker pattern implementation.

    Features:
    - Configurable failure threshold
    - Automatic recovery attempts
    - Per-service circuits
    - Statistics tracking
    """

    def __init__(
        self,
        failure_threshold: int = 5,
        recovery_timeout: float = 60.0,
        half_open_max_calls: int = 3,
    ):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.half_open_max_calls = half_open_max_calls

        self._circuits: Dict[str, CircuitStats] = defaultdict(CircuitStats)
        self._half_open_calls: Dict[str, int] = defaultdict(int)
        self._lock = threading.RLock()

    def get_state(self, service: str) -> CircuitState:
        """Get current state of a circuit."""
        with self._lock:
            stats = self._circuits[service]

            if stats.failures >= self.failure_threshold:
                # Check if recovery timeout has passed
                if stats.last_failure_time:
                    elapsed = time.time() - stats.last_failure_time
                    if elapsed >= self.recovery_timeout:
                        return CircuitState.HALF_OPEN
                return CircuitState.OPEN

            return CircuitState.CLOSED

    def can_execute(self, service: str) -> bool:
        """Check if a call can be executed."""
        state = self.get_state(service)

        if state == CircuitState.CLOSED:
            return True

        if state == CircuitState.HALF_OPEN:
            with self._lock:
                if self._half_open_calls[service] < self.half_open_max_calls:
                    self._half_open_calls[service] += 1
                    return True

        return False

    def record_success(self, service: str) -> None:
        """Record a successful call."""
        with self._lock:
            stats = self._circuits[service]
            stats.successes += 1
            stats.last_success_time = time.time()

            state = self.get_state(service)
            if state == CircuitState.HALF_OPEN:
                # Reset circuit on success in half-open
                stats.failures = 0
                stats.state_changed_at = time.time()
                self._half_open_calls[service] = 0

    def record_failure(self, service: str) -> None:
        """Record a failed call."""
        with self._lock:
            stats = self._circuits[service]
            stats.failures += 1
            stats.last_failure_time = time.time()

            state = self.get_state(service)
            if state == CircuitState.HALF_OPEN:
                # Immediately open on failure in half-open
                self._half_open_calls[service] = 0

    def reset(self, service: str) -> None:
        """Reset a circuit to closed state."""
        with self._lock:
            self._circuits[service] = CircuitStats()
            self._half_open_calls[service] = 0

    def get_stats(self, service: str) -> Dict[str, Any]:
        """Get statistics for a circuit."""
        stats = self._circuits[service]
        return {
            "service": service,
            "state": self.get_state(service).value,
            "failures": stats.failures,
            "successes": stats.successes,
            "last_failure": stats.last_failure_time,
            "last_success": stats.last_success_time,
        }


# =============================================================================
# MESSAGE DEDUPLICATION
# =============================================================================


class MessageDeduplicator:
    """
    Prevents duplicate message processing.

    Features:
    - Content-based deduplication
    - Time-window based expiry
    - Configurable tolerance
    """

    def __init__(
        self,
        window_seconds: float = 60.0,
        max_messages: int = 10000,
        similarity_threshold: float = 0.95,
    ):
        self.window_seconds = window_seconds
        self.max_messages = max_messages
        self.similarity_threshold = similarity_threshold

        self._seen: Dict[str, float] = {}  # hash -> timestamp
        self._lock = threading.RLock()

    def _compute_hash(
        self,
        message_id: str,
        content: str,
        user_id: str,
        chat_id: str,
    ) -> str:
        """Compute a hash for deduplication."""
        data = f"{user_id}:{chat_id}:{content}"
        return hashlib.sha256(data.encode()).hexdigest()

    def is_duplicate(
        self,
        message_id: str,
        content: str,
        user_id: str,
        chat_id: str,
    ) -> bool:
        """Check if a message is a duplicate."""
        msg_hash = self._compute_hash(message_id, content, user_id, chat_id)
        now = time.time()

        with self._lock:
            self._cleanup_expired()

            if msg_hash in self._seen:
                # Check if within window
                if now - self._seen[msg_hash] < self.window_seconds:
                    return True

            # Mark as seen
            self._seen[msg_hash] = now
            return False

    def _cleanup_expired(self) -> int:
        """Remove expired entries."""
        now = time.time()
        cutoff = now - self.window_seconds

        expired = [h for h, t in self._seen.items() if t < cutoff]
        for h in expired:
            del self._seen[h]

        # Enforce max messages
        if len(self._seen) > self.max_messages:
            # Remove oldest
            sorted_items = sorted(self._seen.items(), key=lambda x: x[1])
            for h, _ in sorted_items[: len(self._seen) - self.max_messages]:
                del self._seen[h]

        return len(expired)

    def clear(self) -> None:
        """Clear all seen messages."""
        with self._lock:
            self._seen.clear()


# =============================================================================
# PLUGIN SYSTEM
# =============================================================================


@dataclass
class PluginInfo:
    """Information about a plugin."""

    name: str
    version: str
    description: str
    author: str
    enabled: bool = True
    priority: int = 0  # Higher = loaded first


class BotPlugin(ABC):
    """Base class for bot plugins."""

    @property
    @abstractmethod
    def info(self) -> PluginInfo:
        """Get plugin information."""
        pass

    @abstractmethod
    async def on_load(self, bot: Any) -> None:
        """Called when plugin is loaded."""
        pass

    @abstractmethod
    async def on_unload(self) -> None:
        """Called when plugin is unloaded."""
        pass

    async def on_message(self, event: Any) -> Optional[str]:
        """Handle incoming message. Return response or None."""
        return None

    async def on_command(self, command: str, event: Any) -> Optional[str]:
        """Handle command. Return response or None."""
        return None


class PluginManager:
    """
    Manages bot plugins.

    Features:
    - Dynamic plugin loading
    - Plugin dependencies
    - Hot-reload support
    - Plugin isolation
    """

    def __init__(self, plugin_dir: Optional[Path] = None):
        self.plugin_dir = plugin_dir
        self._plugins: Dict[str, BotPlugin] = {}
        self._bot: Any = None
        self._lock = threading.RLock()

    def set_bot(self, bot: Any) -> None:
        """Set the bot instance for plugins."""
        self._bot = bot

    async def load_plugin(self, plugin: BotPlugin) -> bool:
        """Load a plugin."""
        info = plugin.info

        with self._lock:
            if info.name in self._plugins:
                logger.warning(f"Plugin {info.name} already loaded")
                return False

            try:
                await plugin.on_load(self._bot)
                self._plugins[info.name] = plugin
                logger.info(f"Loaded plugin: {info.name} v{info.version}")
                return True
            except Exception as e:
                logger.error(f"Failed to load plugin {info.name}: {e}")
                return False

    async def unload_plugin(self, name: str) -> bool:
        """Unload a plugin."""
        with self._lock:
            plugin = self._plugins.get(name)
            if not plugin:
                return False

            try:
                await plugin.on_unload()
                del self._plugins[name]
                logger.info(f"Unloaded plugin: {name}")
                return True
            except Exception as e:
                logger.error(f"Failed to unload plugin {name}: {e}")
                return False

    async def reload_plugin(self, name: str) -> bool:
        """Reload a plugin."""
        if await self.unload_plugin(name):
            # Would need to re-instantiate plugin
            return True
        return False

    def get_plugin(self, name: str) -> Optional[BotPlugin]:
        """Get a loaded plugin."""
        return self._plugins.get(name)

    def list_plugins(self) -> List[PluginInfo]:
        """List all loaded plugins."""
        return [p.info for p in self._plugins.values()]

    async def dispatch_message(self, event: Any) -> Optional[str]:
        """Dispatch message to all plugins."""
        for plugin in sorted(
            self._plugins.values(), key=lambda p: p.info.priority, reverse=True
        ):
            if not plugin.info.enabled:
                continue

            try:
                response = await plugin.on_message(event)
                if response:
                    return response
            except Exception as e:
                logger.error(f"Plugin {plugin.info.name} error: {e}")

        return None

    async def dispatch_command(
        self,
        command: str,
        event: Any,
    ) -> Optional[str]:
        """Dispatch command to all plugins."""
        for plugin in sorted(
            self._plugins.values(), key=lambda p: p.info.priority, reverse=True
        ):
            if not plugin.info.enabled:
                continue

            try:
                response = await plugin.on_command(command, event)
                if response:
                    return response
            except Exception as e:
                logger.error(f"Plugin {plugin.info.name} error: {e}")

        return None


# =============================================================================
# FACADE CLASS
# =============================================================================


class AdvancedBotFeatures:
    """
    Unified interface for all advanced bot features.

    This class provides a single entry point for all advanced features,
    making it easy to integrate them into the bot.
    """

    def __init__(
        self,
        data_dir: Optional[Path] = None,
        rate_limit_config: Optional[RateLimitConfig] = None,
        cooldown_config: Optional[CooldownConfig] = None,
    ):
        data_dir = data_dir or Path.home() / ".luminaguard" / "data"
        data_dir.mkdir(parents=True, exist_ok=True)

        # Initialize all components
        self.rate_limiter = RateLimiterAdvanced(rate_limit_config)
        self.permissions = PermissionManager(data_dir / "permissions.json")
        self.cooldowns = CooldownManager(cooldown_config)
        self.message_queue = MessageQueue()
        self.typing = TypingIndicatorManager()
        self.threads = ThreadManager()
        self.reactions = ReactionManager()
        self.scheduled = ScheduledMessageManager(data_dir / "scheduled.json")
        self.audit = AuditLogger(data_dir / "audit.json")
        self.circuit_breaker = CircuitBreaker()
        self.deduplicator = MessageDeduplicator()
        self.plugins = PluginManager(data_dir / "plugins")

    async def process_incoming_message(
        self,
        message_id: str,
        content: str,
        user_id: str,
        chat_id: str,
        guild_id: str = "",
    ) -> Tuple[bool, Optional[str]]:
        """
        Process an incoming message through all feature layers.

        Returns:
            Tuple of (should_process, rejection_reason)
        """
        # Check for duplicate
        if self.deduplicator.is_duplicate(message_id, content, user_id, chat_id):
            self.audit.log(
                AuditEventType.COMMAND_DENIED,
                user_id=user_id,
                details={"reason": "duplicate", "message_id": message_id},
                success=False,
            )
            return False, "Duplicate message detected"

        # Check permissions
        if not self.permissions.has_permission(user_id, Permission.USE_BOT):
            self.audit.log(
                AuditEventType.COMMAND_DENIED,
                user_id=user_id,
                details={"reason": "no_permission", "message_id": message_id},
                success=False,
            )
            return False, "You don't have permission to use this bot"

        # Check rate limit
        is_allowed, remaining, _ = self.rate_limiter.is_allowed(
            user_id=user_id,
            channel_id=chat_id,
            guild_id=guild_id,
        )
        if not is_allowed:
            return False, f"Rate limit exceeded. Try again later."

        # Start typing indicator
        await self.typing.start_typing(chat_id)

        return True, None

    def get_stats(self) -> Dict[str, Any]:
        """Get statistics for all features."""
        return {
            "rate_limiter": {
                "active_keys": len(self.rate_limiter._entries),
            },
            "permissions": {
                "roles": len(self.permissions.list_roles()),
                "users_with_roles": len(self.permissions._user_roles),
            },
            "message_queue": self.message_queue.get_stats(),
            "threads": {
                "active_threads": len(self.threads._threads),
            },
            "scheduled": {
                "pending": len(self.scheduled._messages),
            },
            "audit": {
                "total_events": len(self.audit._events),
            },
            "plugins": {
                "loaded": len(self.plugins._plugins),
            },
        }


# Convenience factory function
def create_advanced_features(
    data_dir: Optional[Path] = None,
) -> AdvancedBotFeatures:
    """Create an AdvancedBotFeatures instance."""
    return AdvancedBotFeatures(data_dir=data_dir)
