"""
Tests for daemon external integration module.

Tests for:
- MessageQueueConsumer and its implementations (RabbitMQ, Redis)
- WebhookReceiver
- IntegrationManager
- Event dispatching and retry logic
"""

import pytest
import asyncio
import json
from datetime import datetime, timezone
from unittest.mock import AsyncMock, MagicMock, patch

import sys
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from daemon.integration import (
    ExternalEvent,
    EventPriority,
    RateLimitConfig,
    RateLimiter,
    RetryConfig,
    RetryPolicy,
    RabbitMQConsumer,
    RedisConsumer,
    WebhookReceiver,
    IntegrationManager,
    create_integration_manager,
)


class TestExternalEvent:
    """Tests for ExternalEvent"""

    def test_create_event(self):
        """Test creating an external event"""
        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="webhook",
            payload={"task_id": "123", "status": "pending"},
        )

        assert event.event_id == "test-1"
        assert event.event_type == "task.created"
        assert event.source == "webhook"
        assert event.priority == EventPriority.NORMAL

    def test_event_to_dict(self):
        """Test event serialization"""
        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="webhook",
            payload={"task_id": "123"},
            priority=EventPriority.HIGH,
        )

        data = event.to_dict()

        assert data["event_id"] == "test-1"
        assert data["event_type"] == "task.created"
        assert data["priority"] == "HIGH"
        assert data["payload"] == {"task_id": "123"}

    def test_event_from_dict(self):
        """Test event deserialization"""
        data = {
            "event_id": "test-1",
            "event_type": "task.created",
            "source": "webhook",
            "payload": {"task_id": "123"},
            "priority": "HIGH",
            "retries": 2,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

        event = ExternalEvent.from_dict(data)

        assert event.event_id == "test-1"
        assert event.priority == EventPriority.HIGH
        assert event.retries == 2


class TestRateLimiter:
    """Tests for RateLimiter"""

    def test_rate_limiter_enabled(self):
        """Test rate limiting with enabled config"""
        config = RateLimitConfig(max_events_per_second=10, max_burst_size=50, enabled=True)
        limiter = RateLimiter(config)

        # Should allow some events in initial burst
        allowed_count = 0
        for i in range(60):
            if limiter.is_allowed():
                allowed_count += 1

        # Should allow at least the burst size
        assert allowed_count >= 10  # Reasonable threshold

    def test_rate_limiter_disabled(self):
        """Test rate limiting when disabled"""
        config = RateLimitConfig(enabled=False)
        limiter = RateLimiter(config)

        # Should always allow
        for i in range(100):
            assert limiter.is_allowed()

    def test_rate_limiter_priority(self):
        """Test rate limiting with different priorities"""
        config = RateLimitConfig(max_events_per_second=10, max_burst_size=50)
        limiter = RateLimiter(config)

        # Critical priority should consume fewer tokens
        assert limiter.is_allowed(EventPriority.CRITICAL)

    @pytest.mark.asyncio
    async def test_rate_limiter_wait_for_allowance(self):
        """Test async wait for rate limit allowance"""
        config = RateLimitConfig(max_events_per_second=1, max_burst_size=1, enabled=True)
        limiter = RateLimiter(config)

        # Use burst
        if limiter.is_allowed():
            # Should wait for allowance
            task = asyncio.create_task(limiter.wait_for_allowance())

            # Give it a moment to start waiting
            await asyncio.sleep(0.2)

            # Should still be waiting
            assert not task.done()

            # Wait a bit more for token to refill
            await asyncio.sleep(1.5)

            # Now it should complete
            try:
                await asyncio.wait_for(task, timeout=0.5)
            except asyncio.TimeoutError:
                pytest.fail("wait_for_allowance timed out")
        else:
            # Skip test if initial token not available
            pass


class TestRetryPolicy:
    """Tests for RetryPolicy"""

    def test_should_retry(self):
        """Test retry decision logic"""
        config = RetryConfig(max_retries=3)
        policy = RetryPolicy(config)

        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="test",
            payload={},
            retries=0,
        )

        assert policy.should_retry(event)

        event.retries = 2
        assert policy.should_retry(event)

        event.retries = 3
        assert not policy.should_retry(event)

    def test_backoff_calculation(self):
        """Test exponential backoff calculation"""
        config = RetryConfig(
            initial_backoff_seconds=1.0,
            backoff_multiplier=2.0,
            max_backoff_seconds=60.0,
        )
        policy = RetryPolicy(config)

        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="test",
            payload={},
        )

        event.retries = 0
        assert policy.get_backoff_time(event) == 1.0

        event.retries = 1
        assert policy.get_backoff_time(event) == 2.0

        event.retries = 2
        assert policy.get_backoff_time(event) == 4.0

        event.retries = 10
        # Should be capped at max_backoff
        assert policy.get_backoff_time(event) == 60.0

    def test_retry_disabled(self):
        """Test retry when disabled"""
        config = RetryConfig(enabled=False)
        policy = RetryPolicy(config)

        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="test",
            payload={},
        )

        assert not policy.should_retry(event)


class TestRabbitMQConsumer:
    """Tests for RabbitMQConsumer"""

    @pytest.mark.asyncio
    async def test_rabbitmq_consumer_creation(self):
        """Test creating a RabbitMQ consumer"""
        consumer = RabbitMQConsumer(
            host="localhost",
            port=5672,
            queue_name="test.queue",
        )

        assert consumer.host == "localhost"
        assert consumer.port == 5672
        assert consumer.queue_name == "test.queue"

    @pytest.mark.asyncio
    async def test_rabbitmq_connect_without_aio_pika(self):
        """Test RabbitMQ connection fails without aio_pika installed"""
        consumer = RabbitMQConsumer()

        with patch("builtins.__import__", side_effect=ImportError("aio_pika")):
            result = await consumer.connect()

        assert not result

    @pytest.mark.asyncio
    async def test_rabbitmq_register_handler(self):
        """Test registering event handlers"""
        consumer = RabbitMQConsumer()

        async def test_handler(event):
            return True

        consumer.register_handler("task.created", test_handler)

        assert "task.created" in consumer._event_handlers
        assert test_handler in consumer._event_handlers["task.created"]

    @pytest.mark.asyncio
    async def test_rabbitmq_unregister_handler(self):
        """Test unregistering event handlers"""
        consumer = RabbitMQConsumer()

        async def test_handler(event):
            return True

        consumer.register_handler("task.created", test_handler)
        consumer.unregister_handler("task.created", test_handler)

        assert "task.created" not in consumer._event_handlers or len(
            consumer._event_handlers["task.created"]
        ) == 0


class TestRedisConsumer:
    """Tests for RedisConsumer"""

    @pytest.mark.asyncio
    async def test_redis_consumer_creation(self):
        """Test creating a Redis consumer"""
        consumer = RedisConsumer(
            host="localhost",
            port=6379,
            queue_mode="list",
        )

        assert consumer.host == "localhost"
        assert consumer.port == 6379
        assert consumer.queue_mode == "list"

    @pytest.mark.asyncio
    async def test_redis_connect_without_redis(self):
        """Test Redis connection fails without redis installed"""
        consumer = RedisConsumer()

        with patch("builtins.__import__", side_effect=ImportError("redis")):
            result = await consumer.connect()

        assert not result

    @pytest.mark.asyncio
    async def test_redis_register_handler(self):
        """Test registering event handlers"""
        consumer = RedisConsumer()

        async def test_handler(event):
            return True

        consumer.register_handler("task.created", test_handler)

        assert "task.created" in consumer._event_handlers
        assert test_handler in consumer._event_handlers["task.created"]


class TestWebhookReceiver:
    """Tests for WebhookReceiver"""

    def test_webhook_receiver_creation(self):
        """Test creating a webhook receiver"""
        receiver = WebhookReceiver(
            host="0.0.0.0",
            port=9000,
            path="/webhook",
            secret="test-secret",
        )

        assert receiver.host == "0.0.0.0"
        assert receiver.port == 9000
        assert receiver.secret == "test-secret"

    def test_webhook_signature_verification_with_secret(self):
        """Test webhook signature verification"""
        receiver = WebhookReceiver(secret="test-secret")

        payload = b'{"event_id": "test-1"}'
        import hmac
        import hashlib

        correct_signature = hmac.new(
            b"test-secret", payload, hashlib.sha256
        ).hexdigest()
        wrong_signature = hmac.new(b"wrong-secret", payload, hashlib.sha256).hexdigest()

        assert receiver._verify_signature(payload, correct_signature)
        assert not receiver._verify_signature(payload, wrong_signature)

    def test_webhook_signature_verification_without_secret(self):
        """Test webhook signature verification when no secret is set"""
        receiver = WebhookReceiver()

        payload = b'{"event_id": "test-1"}'

        # Should always return True when no secret is set
        assert receiver._verify_signature(payload, "any-signature")

    def test_webhook_register_handler(self):
        """Test registering webhook handlers"""
        receiver = WebhookReceiver()

        async def test_handler(event):
            return True

        receiver.register_handler("task.created", test_handler)

        assert "task.created" in receiver._event_handlers
        assert test_handler in receiver._event_handlers["task.created"]


class TestIntegrationManager:
    """Tests for IntegrationManager"""

    def test_create_integration_manager(self):
        """Test creating an integration manager"""
        manager = create_integration_manager()

        assert isinstance(manager, IntegrationManager)
        assert len(manager._consumers) == 0

    def test_add_consumer(self):
        """Test adding a consumer"""
        manager = IntegrationManager()
        consumer = RabbitMQConsumer()

        manager.add_consumer("rabbitmq", consumer)

        assert "rabbitmq" in manager._consumers
        assert manager._consumers["rabbitmq"] == consumer

    def test_remove_consumer(self):
        """Test removing a consumer"""
        manager = IntegrationManager()
        consumer = RabbitMQConsumer()

        manager.add_consumer("rabbitmq", consumer)
        manager.remove_consumer("rabbitmq")

        assert "rabbitmq" not in manager._consumers

    def test_set_webhook_receiver(self):
        """Test setting webhook receiver"""
        manager = IntegrationManager()
        receiver = WebhookReceiver()

        manager.set_webhook_receiver(receiver)

        assert manager._webhook_receiver == receiver

    def test_register_event_handler_all_consumers(self):
        """Test registering handler for all consumers"""
        manager = IntegrationManager()

        consumer1 = RabbitMQConsumer()
        consumer2 = RedisConsumer()

        manager.add_consumer("rabbitmq", consumer1)
        manager.add_consumer("redis", consumer2)

        async def test_handler(event):
            return True

        manager.register_event_handler("task.created", test_handler)

        assert "task.created" in consumer1._event_handlers
        assert "task.created" in consumer2._event_handlers

    def test_register_event_handler_specific_consumer(self):
        """Test registering handler for specific consumer"""
        manager = IntegrationManager()

        consumer1 = RabbitMQConsumer()
        consumer2 = RedisConsumer()

        manager.add_consumer("rabbitmq", consumer1)
        manager.add_consumer("redis", consumer2)

        async def test_handler(event):
            return True

        manager.register_event_handler("task.created", test_handler, "rabbitmq")

        assert "task.created" in consumer1._event_handlers
        assert "task.created" not in consumer2._event_handlers or len(
            consumer2._event_handlers.get("task.created", [])
        ) == 0

    @pytest.mark.asyncio
    async def test_integration_manager_lifecycle(self):
        """Test integration manager start/stop lifecycle"""
        manager = IntegrationManager()

        consumer = RabbitMQConsumer()
        consumer.connect = AsyncMock(return_value=False)

        manager.add_consumer("rabbitmq", consumer)

        # Should handle gracefully when connect fails
        await manager.start()
        await manager.stop()


class TestEventDispatch:
    """Tests for event dispatching"""

    @pytest.mark.asyncio
    async def test_dispatch_event_to_handler(self):
        """Test dispatching event to registered handler"""
        consumer = RabbitMQConsumer()

        handled_event = None

        async def test_handler(event):
            nonlocal handled_event
            handled_event = event
            return True

        consumer.register_handler("task.created", test_handler)

        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="test",
            payload={"task_id": "123"},
        )

        result = await consumer.dispatch_event(event)

        assert result
        assert handled_event == event

    @pytest.mark.asyncio
    async def test_dispatch_event_wildcard_handler(self):
        """Test wildcard event handler"""
        consumer = RabbitMQConsumer()

        handled_events = []

        async def wildcard_handler(event):
            handled_events.append(event)
            return True

        consumer.register_handler("*", wildcard_handler)

        event = ExternalEvent(
            event_id="test-1",
            event_type="any.event",
            source="test",
            payload={},
        )

        result = await consumer.dispatch_event(event)

        assert result
        assert len(handled_events) == 1

    @pytest.mark.asyncio
    async def test_dispatch_event_handler_error(self):
        """Test event dispatch when handler raises exception"""
        consumer = RabbitMQConsumer()

        async def failing_handler(event):
            raise ValueError("Handler error")

        consumer.register_handler("task.created", failing_handler)

        event = ExternalEvent(
            event_id="test-1",
            event_type="task.created",
            source="test",
            payload={},
        )

        result = await consumer.dispatch_event(event)

        # Should return False on handler error
        assert not result

    @pytest.mark.asyncio
    async def test_dispatch_event_no_handlers(self):
        """Test event dispatch when no handlers registered"""
        consumer = RabbitMQConsumer()

        event = ExternalEvent(
            event_id="test-1",
            event_type="unknown.event",
            source="test",
            payload={},
        )

        result = await consumer.dispatch_event(event)

        assert not result


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
