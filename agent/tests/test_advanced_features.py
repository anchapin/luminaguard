"""
Tests for Advanced Bot Features.

This module tests all the advanced features added to LuminaGuard daemon mode:
- Rate limiting
- Permission system
- Command cooldowns
- Message queue
- Typing indicators
- Message threading
- Reaction handling
- Scheduled messages
- Audit logging
- Circuit breaker
- Message deduplication
- Plugin system
"""

import asyncio
import json
import time
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from daemon.advanced_features import (
    # Rate Limiting
    RateLimiterAdvanced,
    RateLimitConfig,
    RateLimitScope,
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

# =============================================================================
# RATE LIMITER TESTS
# =============================================================================


class TestRateLimiterAdvanced:
    """Tests for advanced rate limiter."""

    def test_default_config(self):
        """Test default configuration."""
        limiter = RateLimiterAdvanced()
        assert limiter.default_config.max_requests == 10
        assert limiter.default_config.window_seconds == 60.0

    def test_is_allowed_within_limit(self):
        """Test that requests within limit are allowed."""
        limiter = RateLimiterAdvanced(
            RateLimitConfig(max_requests=5, window_seconds=60)
        )

        for i in range(5):
            is_allowed, remaining, _ = limiter.is_allowed(user_id="user1")
            assert is_allowed, f"Request {i+1} should be allowed"

        # 6th request should be denied
        is_allowed, remaining, _ = limiter.is_allowed(user_id="user1")
        assert not is_allowed

    def test_different_users_independent(self):
        """Test that different users have independent rate limits."""
        limiter = RateLimiterAdvanced(
            RateLimitConfig(max_requests=2, window_seconds=60)
        )

        # User 1 uses their quota
        limiter.is_allowed(user_id="user1")
        limiter.is_allowed(user_id="user1")

        # User 1 should be rate limited
        is_allowed, _, _ = limiter.is_allowed(user_id="user1")
        assert not is_allowed

        # User 2 should still be allowed
        is_allowed, _, _ = limiter.is_allowed(user_id="user2")
        assert is_allowed

    def test_channel_scope(self):
        """Test channel-scoped rate limiting."""
        config = RateLimitConfig(max_requests=2, scope=RateLimitScope.CHANNEL)
        limiter = RateLimiterAdvanced(config)

        # Two users in same channel
        limiter.is_allowed(user_id="user1", channel_id="channel1")
        limiter.is_allowed(user_id="user2", channel_id="channel1")

        # Third request in same channel should be denied
        is_allowed, _, _ = limiter.is_allowed(user_id="user3", channel_id="channel1")
        assert not is_allowed

        # Different channel should be allowed
        is_allowed, _, _ = limiter.is_allowed(user_id="user1", channel_id="channel2")
        assert is_allowed

    def test_burst_allowance(self):
        """Test burst allowance."""
        config = RateLimitConfig(max_requests=2, burst_allowance=2)
        limiter = RateLimiterAdvanced(config)

        # Should allow 4 requests (2 normal + 2 burst)
        for _ in range(4):
            is_allowed, _, _ = limiter.is_allowed(user_id="user1")
            assert is_allowed

        # 5th should be denied
        is_allowed, _, _ = limiter.is_allowed(user_id="user1")
        assert not is_allowed

    def test_reset(self):
        """Test rate limit reset."""
        limiter = RateLimiterAdvanced(RateLimitConfig(max_requests=1))

        limiter.is_allowed(user_id="user1")
        is_allowed, _, _ = limiter.is_allowed(user_id="user1")
        assert not is_allowed

        limiter.reset(RateLimitScope.USER, "user1")
        is_allowed, _, _ = limiter.is_allowed(user_id="user1")
        assert is_allowed


# =============================================================================
# PERMISSION SYSTEM TESTS
# =============================================================================


class TestPermissionManager:
    """Tests for permission management."""

    def test_default_roles_exist(self):
        """Test that default roles are created."""
        manager = PermissionManager()

        assert "banned" in manager._roles
        assert "user" in manager._roles
        assert "trusted" in manager._roles
        assert "admin" in manager._roles

    def test_user_has_default_role(self):
        """Test that users get default role."""
        manager = PermissionManager()

        role = manager.get_user_role("unknown_user")
        assert role.name == "user"

    def test_assign_role(self):
        """Test assigning a role to a user."""
        manager = PermissionManager()

        assert manager.assign_role("user1", "admin")
        role = manager.get_user_role("user1")
        assert role.name == "admin"

    def test_assign_invalid_role(self):
        """Test assigning an invalid role."""
        manager = PermissionManager()

        assert not manager.assign_role("user1", "nonexistent")

    def test_has_permission(self):
        """Test permission checking."""
        manager = PermissionManager()

        # Default user should have USE_BOT
        assert manager.has_permission("user1", Permission.USE_BOT)

        # Default user should not have MANAGE_USERS
        assert not manager.has_permission("user1", Permission.MANAGE_USERS)

        # Admin should have all permissions
        manager.assign_role("admin1", "admin")
        assert manager.has_permission("admin1", Permission.MANAGE_USERS)

    def test_grant_permission_override(self):
        """Test granting additional permission to user."""
        manager = PermissionManager()

        # User doesn't have MANAGE_USERS by default
        assert not manager.has_permission("user1", Permission.MANAGE_USERS)

        # Grant the permission
        manager.grant_permission("user1", Permission.MANAGE_USERS)

        # Now user should have it
        assert manager.has_permission("user1", Permission.MANAGE_USERS)

    def test_revoke_permission_override(self):
        """Test revoking permission override."""
        manager = PermissionManager()

        manager.grant_permission("user1", Permission.MANAGE_USERS)
        assert manager.has_permission("user1", Permission.MANAGE_USERS)

        manager.revoke_permission("user1", Permission.MANAGE_USERS)
        assert not manager.has_permission("user1", Permission.MANAGE_USERS)

    def test_create_custom_role(self):
        """Test creating a custom role."""
        manager = PermissionManager()

        role = Role(
            name="moderator",
            permissions={Permission.USE_BOT, Permission.MANAGE_USERS},
            priority=5,
        )
        manager.create_role(role)

        assert manager.get_role("moderator") is not None
        manager.assign_role("user1", "moderator")
        assert manager.has_permission("user1", Permission.MANAGE_USERS)

    def test_persistence(self):
        """Test permission data persistence."""
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "permissions.json"

            # Create and save
            manager1 = PermissionManager(path)
            manager1.assign_role("user1", "admin")
            manager1.grant_permission("user2", Permission.EXECUTE_CODE)

            # Load in new instance
            manager2 = PermissionManager(path)
            assert manager2.get_user_role("user1").name == "admin"
            assert manager2.has_permission("user2", Permission.EXECUTE_CODE)


# =============================================================================
# COOLDOWN TESTS
# =============================================================================


class TestCooldownManager:
    """Tests for command cooldowns."""

    def test_default_cooldown(self):
        """Test default cooldown configuration."""
        manager = CooldownManager()

        assert manager.config.default_seconds == 5.0

    def test_set_and_check_cooldown(self):
        """Test setting and checking cooldowns."""
        manager = CooldownManager(CooldownConfig(default_seconds=1.0))

        # Initially not on cooldown
        is_on, remaining = manager.is_on_cooldown("test_command", "user1")
        assert not is_on

        # Set cooldown
        manager.set_cooldown("test_command", "user1")

        # Now on cooldown
        is_on, remaining = manager.is_on_cooldown("test_command", "user1")
        assert is_on
        assert remaining > 0

    def test_cooldown_expiry(self):
        """Test that cooldowns expire."""
        manager = CooldownManager(CooldownConfig(default_seconds=0.1))

        manager.set_cooldown("test_command", "user1")

        # Wait for cooldown to expire
        time.sleep(0.2)

        is_on, _ = manager.is_on_cooldown("test_command", "user1")
        assert not is_on

    def test_reset_cooldown(self):
        """Test resetting cooldowns."""
        manager = CooldownManager()

        manager.set_cooldown("test_command", "user1")
        manager.reset_cooldown("test_command", "user1")

        is_on, _ = manager.is_on_cooldown("test_command", "user1")
        assert not is_on

    def test_per_command_cooldowns(self):
        """Test different cooldowns for different commands."""
        config = CooldownConfig(per_user_seconds={"cmd1": 1.0, "cmd2": 2.0})
        manager = CooldownManager(config)

        assert manager.get_cooldown("cmd1", "user1") == 1.0
        assert manager.get_cooldown("cmd2", "user1") == 2.0
        assert manager.get_cooldown("cmd3", "user1") == 5.0  # default


# =============================================================================
# MESSAGE QUEUE TESTS
# =============================================================================


class TestMessageQueue:
    """Tests for message queue."""

    def test_enqueue_dequeue(self):
        """Test basic enqueue and dequeue."""
        queue = MessageQueue()

        msg_id = queue.enqueue(chat_id="chat1", content="Hello")
        assert msg_id is not None

        msg = queue.dequeue(timeout=0.1)
        assert msg is not None
        assert msg.content == "Hello"
        assert msg.chat_id == "chat1"

    def test_priority_ordering(self):
        """Test that higher priority messages are sent first."""
        queue = MessageQueue()

        queue.enqueue(chat_id="chat1", content="low", priority=1)
        queue.enqueue(chat_id="chat1", content="high", priority=10)
        queue.enqueue(chat_id="chat1", content="medium", priority=5)

        msg1 = queue.dequeue(timeout=0.1)
        assert msg1.content == "high"

        msg2 = queue.dequeue(timeout=0.1)
        assert msg2.content == "medium"

        msg3 = queue.dequeue(timeout=0.1)
        assert msg3.content == "low"

    def test_max_size(self):
        """Test queue size limit."""
        queue = MessageQueue(max_size=2)

        assert queue.enqueue(chat_id="c1", content="m1") is not None
        assert queue.enqueue(chat_id="c1", content="m2") is not None
        assert queue.enqueue(chat_id="c1", content="m3") is None  # Should fail

    def test_ack_nack(self):
        """Test acknowledgement and negative acknowledgement."""
        queue = MessageQueue(retry_delay=0.01, max_retry_delay=0.1)

        queue.enqueue(chat_id="chat1", content="test")
        msg = queue.dequeue(timeout=0.1)

        # NACK should requeue
        assert queue.nack(msg.id, "Test error")

        # Wait for backoff
        time.sleep(0.05)

        # Message should be back in queue
        msg2 = queue.dequeue(timeout=0.1)
        assert msg2 is not None
        assert msg2.id == msg.id
        assert msg2.attempts == 1

        # ACK should remove
        queue.ack(msg2.id)
        assert msg2.id not in queue._pending

    def test_max_retries(self):
        """Test that messages are dropped after max retries."""
        queue = MessageQueue(retry_delay=0.01, max_retry_delay=0.1)

        queue.enqueue(chat_id="chat1", content="test")
        msg = queue.dequeue(timeout=0.1)
        original_id = msg.id

        # Exhaust retries (max_attempts = 3)
        # First nack - attempts becomes 1, requeued
        queue.nack(msg.id, "Error 1")
        time.sleep(0.05)
        msg = queue.dequeue(timeout=0.1)
        assert msg is not None
        assert msg.id == original_id
        assert msg.attempts == 1

        # Second nack - attempts becomes 2, requeued
        queue.nack(msg.id, "Error 2")
        time.sleep(0.05)
        msg = queue.dequeue(timeout=0.1)
        assert msg is not None
        assert msg.id == original_id
        assert msg.attempts == 2

        # Third nack - attempts becomes 3, NOT requeued (exhausted)
        result = queue.nack(msg.id, "Error 3")
        assert not result  # Should return False - not requeued

        # No more messages in queue
        msg = queue.dequeue(timeout=0.1)
        assert msg is None


# =============================================================================
# TYPING INDICATOR TESTS
# =============================================================================


class TestTypingIndicatorManager:
    """Tests for typing indicator management."""

    def test_start_stop_typing(self):
        """Test starting and stopping typing indicator."""
        manager = TypingIndicatorManager(default_timeout=5.0)

        assert not manager.is_typing("chat1")

        # Start typing (sync wrapper for async)
        asyncio.run(manager.start_typing("chat1"))

        assert manager.is_typing("chat1")

        manager.stop_typing("chat1")
        assert not manager.is_typing("chat1")

    def test_typing_expiry(self):
        """Test that typing indicator expires."""
        manager = TypingIndicatorManager(default_timeout=0.1)

        asyncio.run(manager.start_typing("chat1"))
        assert manager.is_typing("chat1")

        time.sleep(0.2)
        assert not manager.is_typing("chat1")

    def test_active_chats(self):
        """Test getting all active typing chats."""
        manager = TypingIndicatorManager(default_timeout=5.0)

        asyncio.run(manager.start_typing("chat1"))
        asyncio.run(manager.start_typing("chat2"))

        active = manager.get_active_chats()
        assert "chat1" in active
        assert "chat2" in active


# =============================================================================
# THREAD MANAGER TESTS
# =============================================================================


class TestThreadManager:
    """Tests for message threading."""

    def test_create_thread(self):
        """Test creating a thread."""
        manager = ThreadManager()

        thread = manager.create_thread(
            root_message_id="msg1",
            chat_id="chat1",
            creator_id="user1",
        )

        assert thread.thread_id is not None
        assert thread.root_message_id == "msg1"
        assert "user1" in thread.participants

    def test_get_thread(self):
        """Test retrieving a thread."""
        manager = ThreadManager()

        created = manager.create_thread("msg1", "chat1", "user1")
        retrieved = manager.get_thread(created.thread_id)

        assert retrieved is not None
        assert retrieved.thread_id == created.thread_id

    def test_add_message_to_thread(self):
        """Test adding messages to a thread."""
        manager = ThreadManager()

        thread = manager.create_thread("msg1", "chat1", "user1")

        manager.add_message_to_thread(thread.thread_id, "msg2", "user2")

        assert thread.message_count == 1
        assert "user2" in thread.participants

    def test_archive_thread(self):
        """Test archiving a thread."""
        manager = ThreadManager()

        thread = manager.create_thread("msg1", "chat1", "user1")
        manager.archive_thread(thread.thread_id)

        assert manager.get_thread(thread.thread_id) is None


# =============================================================================
# REACTION MANAGER TESTS
# =============================================================================


class TestReactionManager:
    """Tests for reaction handling."""

    def test_add_reaction(self):
        """Test adding a reaction."""
        manager = ReactionManager()

        result = manager.add_reaction(
            message_id="msg1",
            user_id="user1",
            reaction_value="👍",
        )

        assert result
        reactions = manager.get_reactions("msg1")
        assert len(reactions) == 1

    def test_duplicate_reaction(self):
        """Test that duplicate reactions are prevented."""
        manager = ReactionManager()

        manager.add_reaction("msg1", "user1", "👍")
        result = manager.add_reaction("msg1", "user1", "👍")

        assert not result
        assert len(manager.get_reactions("msg1")) == 1

    def test_remove_reaction(self):
        """Test removing a reaction."""
        manager = ReactionManager()

        manager.add_reaction("msg1", "user1", "👍")
        result = manager.remove_reaction("msg1", "user1", "👍")

        assert result
        assert len(manager.get_reactions("msg1")) == 0

    def test_reaction_counts(self):
        """Test getting reaction counts."""
        manager = ReactionManager()

        manager.add_reaction("msg1", "user1", "👍")
        manager.add_reaction("msg1", "user2", "👍")
        manager.add_reaction("msg1", "user3", "❤️")

        counts = manager.get_reaction_counts("msg1")
        assert counts["👍"] == 2
        assert counts["❤️"] == 1

    def test_reaction_handler(self):
        """Test reaction event handlers."""
        manager = ReactionManager()

        handler_called = []

        def handler(reaction):
            handler_called.append(reaction)

        manager.register_handler("👍", handler)
        manager.add_reaction("msg1", "user1", "👍")

        assert len(handler_called) == 1


# =============================================================================
# SCHEDULED MESSAGE TESTS
# =============================================================================


class TestScheduledMessageManager:
    """Tests for scheduled messages."""

    def test_schedule_message(self):
        """Test scheduling a message."""
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "scheduled.json"
            manager = ScheduledMessageManager(path)

            scheduled_time = time.time() + 3600
            msg_id = manager.schedule_message(
                chat_id="chat1",
                content="Scheduled message",
                scheduled_for=scheduled_time,
                created_by="user1",
            )

            assert msg_id is not None
            assert len(manager._messages) == 1

    def test_cancel_message(self):
        """Test cancelling a scheduled message."""
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "scheduled.json"
            manager = ScheduledMessageManager(path)

            msg_id = manager.schedule_message(
                chat_id="chat1",
                content="Test",
                scheduled_for=time.time() + 3600,
                created_by="user1",
            )

            assert manager.cancel_message(msg_id)
            assert len(manager._messages) == 0

    def test_get_pending_messages(self):
        """Test getting pending messages."""
        manager = ScheduledMessageManager()

        # Schedule in the past
        manager.schedule_message(
            chat_id="chat1",
            content="Past",
            scheduled_for=time.time() - 1,
            created_by="user1",
        )

        # Schedule in the future
        manager.schedule_message(
            chat_id="chat1",
            content="Future",
            scheduled_for=time.time() + 3600,
            created_by="user1",
        )

        pending = manager.get_pending_messages()
        assert len(pending) == 1
        assert pending[0].content == "Past"

    def test_recurring_message(self):
        """Test recurring messages."""
        manager = ScheduledMessageManager()

        msg_id = manager.schedule_message(
            chat_id="chat1",
            content="Recurring",
            scheduled_for=time.time() - 1,
            created_by="user1",
            recurring=True,
            recurring_interval=3600,
        )

        msg = manager._messages[msg_id]
        assert msg.recurring
        assert msg.recurring_interval == 3600


# =============================================================================
# AUDIT LOGGER TESTS
# =============================================================================


class TestAuditLogger:
    """Tests for audit logging."""

    def test_log_event(self):
        """Test logging an audit event."""
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "audit.json"
            logger = AuditLogger(path)

            event_id = logger.log(
                event_type=AuditEventType.COMMAND_EXECUTED,
                user_id="user1",
                details={"command": "help"},
            )

            assert event_id is not None
            assert len(logger._events) == 1

    def test_get_events(self):
        """Test querying events."""
        logger = AuditLogger()

        logger.log(AuditEventType.COMMAND_EXECUTED, user_id="user1")
        logger.log(AuditEventType.COMMAND_DENIED, user_id="user2")
        logger.log(AuditEventType.COMMAND_EXECUTED, user_id="user1")

        events = logger.get_events(user_id="user1")
        assert len(events) == 2

        events = logger.get_events(event_type=AuditEventType.COMMAND_DENIED)
        assert len(events) == 1

    def test_user_activity(self):
        """Test getting user activity summary."""
        logger = AuditLogger()

        logger.log(AuditEventType.COMMAND_EXECUTED, user_id="user1")
        logger.log(AuditEventType.COMMAND_EXECUTED, user_id="user1")
        logger.log(AuditEventType.MESSAGE_SENT, user_id="user1")

        activity = logger.get_user_activity("user1")
        assert activity["command_executed"] == 2
        assert activity["message_sent"] == 1

    def test_retention(self):
        """Test event retention."""
        logger = AuditLogger(retention_days=0)  # 0 days = immediate expiry

        logger.log(AuditEventType.COMMAND_EXECUTED, user_id="user1")

        # Force cleanup by calling _enforce_limits
        # Events with retention_days=0 should be expired immediately
        logger._enforce_limits()

        # Events should be cleaned up
        assert len(logger._events) == 0


# =============================================================================
# CIRCUIT BREAKER TESTS
# =============================================================================


class TestCircuitBreaker:
    """Tests for circuit breaker pattern."""

    def test_closed_state(self):
        """Test normal operation (closed state)."""
        cb = CircuitBreaker(failure_threshold=3)

        assert cb.get_state("service1") == CircuitState.CLOSED
        assert cb.can_execute("service1")

    def test_open_after_failures(self):
        """Test circuit opens after threshold failures."""
        cb = CircuitBreaker(failure_threshold=3)

        for _ in range(3):
            cb.record_failure("service1")

        assert cb.get_state("service1") == CircuitState.OPEN
        assert not cb.can_execute("service1")

    def test_half_open_recovery(self):
        """Test recovery through half-open state."""
        cb = CircuitBreaker(failure_threshold=2, recovery_timeout=0.1)

        # Open the circuit
        cb.record_failure("service1")
        cb.record_failure("service1")
        assert cb.get_state("service1") == CircuitState.OPEN

        # Wait for recovery timeout
        time.sleep(0.2)

        # Should be half-open
        assert cb.get_state("service1") == CircuitState.HALF_OPEN
        assert cb.can_execute("service1")

        # Success should close the circuit
        cb.record_success("service1")
        assert cb.get_state("service1") == CircuitState.CLOSED

    def test_reset(self):
        """Test manual circuit reset."""
        cb = CircuitBreaker(failure_threshold=2)

        cb.record_failure("service1")
        cb.record_failure("service1")
        assert cb.get_state("service1") == CircuitState.OPEN

        cb.reset("service1")
        assert cb.get_state("service1") == CircuitState.CLOSED


# =============================================================================
# MESSAGE DEDUPLICATION TESTS
# =============================================================================


class TestMessageDeduplicator:
    """Tests for message deduplication."""

    def test_not_duplicate_initially(self):
        """Test that first message is not a duplicate."""
        dedup = MessageDeduplicator()

        assert not dedup.is_duplicate(
            message_id="msg1",
            content="Hello",
            user_id="user1",
            chat_id="chat1",
        )

    def test_detects_duplicate(self):
        """Test that duplicates are detected."""
        dedup = MessageDeduplicator(window_seconds=60)

        dedup.is_duplicate("msg1", "Hello", "user1", "chat1")

        # Same content from same user in same chat
        assert dedup.is_duplicate("msg2", "Hello", "user1", "chat1")

    def test_different_users_not_duplicate(self):
        """Test that different users don't trigger duplicates."""
        dedup = MessageDeduplicator()

        dedup.is_duplicate("msg1", "Hello", "user1", "chat1")

        assert not dedup.is_duplicate("msg2", "Hello", "user2", "chat1")

    def test_window_expiry(self):
        """Test that deduplication window expires."""
        dedup = MessageDeduplicator(window_seconds=0.1)

        dedup.is_duplicate("msg1", "Hello", "user1", "chat1")

        time.sleep(0.2)

        assert not dedup.is_duplicate("msg2", "Hello", "user1", "chat1")


# =============================================================================
# PLUGIN SYSTEM TESTS
# =============================================================================


class TestPluginManager:
    """Tests for plugin system."""

    def test_load_plugin(self):
        """Test loading a plugin."""
        manager = PluginManager()

        class TestPlugin(BotPlugin):
            @property
            def info(self):
                return PluginInfo(
                    name="test",
                    version="1.0",
                    description="Test plugin",
                    author="Test",
                )

            async def on_load(self, bot):
                pass

            async def on_unload(self):
                pass

        plugin = TestPlugin()
        result = asyncio.run(manager.load_plugin(plugin))

        assert result
        assert "test" in manager._plugins

    def test_unload_plugin(self):
        """Test unloading a plugin."""
        manager = PluginManager()

        class TestPlugin(BotPlugin):
            @property
            def info(self):
                return PluginInfo(name="test", version="1.0", description="", author="")

            async def on_load(self, bot):
                pass

            async def on_unload(self):
                pass

        plugin = TestPlugin()
        asyncio.run(manager.load_plugin(plugin))

        result = asyncio.run(manager.unload_plugin("test"))
        assert result
        assert "test" not in manager._plugins

    def test_dispatch_message(self):
        """Test message dispatching to plugins."""
        manager = PluginManager()

        class TestPlugin(BotPlugin):
            @property
            def info(self):
                return PluginInfo(name="test", version="1.0", description="", author="")

            async def on_load(self, bot):
                pass

            async def on_unload(self):
                pass

            async def on_message(self, event):
                return "Plugin response"

        plugin = TestPlugin()
        asyncio.run(manager.load_plugin(plugin))

        response = asyncio.run(manager.dispatch_message({"test": "data"}))
        assert response == "Plugin response"


# =============================================================================
# FACADE TESTS
# =============================================================================


class TestAdvancedBotFeatures:
    """Tests for the unified facade."""

    def test_initialization(self):
        """Test that all components are initialized."""
        with tempfile.TemporaryDirectory() as tmpdir:
            features = AdvancedBotFeatures(data_dir=Path(tmpdir))

            assert features.rate_limiter is not None
            assert features.permissions is not None
            assert features.cooldowns is not None
            assert features.message_queue is not None
            assert features.typing is not None
            assert features.threads is not None
            assert features.reactions is not None
            assert features.scheduled is not None
            assert features.audit is not None
            assert features.circuit_breaker is not None
            assert features.deduplicator is not None
            assert features.plugins is not None

    def test_process_incoming_message_allowed(self):
        """Test that valid messages are allowed."""
        with tempfile.TemporaryDirectory() as tmpdir:
            features = AdvancedBotFeatures(data_dir=Path(tmpdir))

            should_process, reason = asyncio.run(
                features.process_incoming_message(
                    message_id="msg1",
                    content="Hello",
                    user_id="user1",
                    chat_id="chat1",
                )
            )

            assert should_process
            assert reason is None

    def test_process_incoming_message_duplicate(self):
        """Test that duplicate messages are rejected."""
        with tempfile.TemporaryDirectory() as tmpdir:
            features = AdvancedBotFeatures(data_dir=Path(tmpdir))

            # First message
            asyncio.run(
                features.process_incoming_message("msg1", "Hello", "user1", "chat1")
            )

            # Duplicate
            should_process, reason = asyncio.run(
                features.process_incoming_message(
                    message_id="msg2",
                    content="Hello",
                    user_id="user1",
                    chat_id="chat1",
                )
            )

            assert not should_process
            assert "duplicate" in reason.lower()

    def test_get_stats(self):
        """Test getting statistics."""
        with tempfile.TemporaryDirectory() as tmpdir:
            features = AdvancedBotFeatures(data_dir=Path(tmpdir))

            stats = features.get_stats()

            assert "rate_limiter" in stats
            assert "permissions" in stats
            assert "message_queue" in stats
            assert "threads" in stats
            assert "scheduled" in stats
            assert "audit" in stats
            assert "plugins" in stats


# =============================================================================
# FACTORY FUNCTION TESTS
# =============================================================================


class TestFactoryFunctions:
    """Tests for factory functions."""

    def test_create_advanced_features(self):
        """Test create_advanced_features factory."""
        features = create_advanced_features()
        assert isinstance(features, AdvancedBotFeatures)

    def test_create_advanced_features_with_dir(self):
        """Test factory with custom directory."""
        with tempfile.TemporaryDirectory() as tmpdir:
            features = create_advanced_features(Path(tmpdir))
            assert (
                features.permissions.storage_path == Path(tmpdir) / "permissions.json"
            )


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
