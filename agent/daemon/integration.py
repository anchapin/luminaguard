"""
External Integration API Module - Message Queue Integration.

This module provides message queue integration for external services:
- RabbitMQ (AMQP) consumer for async task triggering
- Redis pub/sub and list-based queue support
- Event-driven task execution
- Rate limiting and retry policies
- Webhook receiver for HTTP callbacks

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
Issue: #448 - External Integration API (webhooks, message queues)
"""

from __future__ import annotations

import asyncio
import inspect
import json
import logging
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from typing import Any, Callable, Dict, List, Optional, Tuple
from enum import Enum
from collections import deque
import hashlib
import hmac

logger = logging.getLogger(__name__)


class MessageQueueType(Enum):
    """Supported message queue types"""
    RABBITMQ = "rabbitmq"
    REDIS = "redis"


class EventPriority(Enum):
    """Event priority levels for rate limiting and processing"""
    LOW = 0
    NORMAL = 1
    HIGH = 2
    CRITICAL = 3


@dataclass
class RateLimitConfig:
    """Rate limiting configuration"""
    max_events_per_second: int = 10
    max_burst_size: int = 50
    enabled: bool = True


@dataclass
class RetryConfig:
    """Retry policy configuration"""
    max_retries: int = 3
    initial_backoff_seconds: float = 1.0
    max_backoff_seconds: float = 60.0
    backoff_multiplier: float = 2.0
    enabled: bool = True


@dataclass
class ExternalEvent:
    """External event data structure"""
    event_id: str
    event_type: str
    source: str
    payload: Dict[str, Any]
    timestamp: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    priority: EventPriority = EventPriority.NORMAL
    retries: int = 0
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert event to dictionary for serialization"""
        return {
            "event_id": self.event_id,
            "event_type": self.event_type,
            "source": self.source,
            "payload": self.payload,
            "timestamp": self.timestamp.isoformat(),
            "priority": self.priority.name,
            "retries": self.retries,
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> ExternalEvent:
        """Create event from dictionary"""
        return cls(
            event_id=data.get("event_id"),
            event_type=data.get("event_type"),
            source=data.get("source"),
            payload=data.get("payload", {}),
            timestamp=datetime.fromisoformat(data.get("timestamp", datetime.now(timezone.utc).isoformat())),
            priority=EventPriority[data.get("priority", "NORMAL")],
            retries=data.get("retries", 0),
            metadata=data.get("metadata", {}),
        )


class RateLimiter:
    """Token bucket rate limiter for event processing"""

    def __init__(self, config: RateLimitConfig):
        self.config = config
        self.tokens = float(config.max_burst_size)
        self.last_refill = time.time()

    def _refill_tokens(self):
        """Refill tokens based on elapsed time"""
        now = time.time()
        elapsed = now - self.last_refill
        tokens_to_add = elapsed * self.config.max_events_per_second
        self.tokens = min(
            float(self.config.max_burst_size),
            self.tokens + tokens_to_add,
        )
        self.last_refill = now

    def is_allowed(self, priority: EventPriority = EventPriority.NORMAL) -> bool:
        """Check if an event with given priority is allowed"""
        if not self.config.enabled:
            return True

        self._refill_tokens()

        # Higher priority events consume fewer tokens
        tokens_needed = max(1.0, float(4 - priority.value))

        if self.tokens >= tokens_needed:
            self.tokens -= tokens_needed
            return True

        return False

    async def wait_for_allowance(self, priority: EventPriority = EventPriority.NORMAL):
        """Async wait until event is allowed"""
        while not self.is_allowed(priority):
            await asyncio.sleep(0.1)


class RetryPolicy:
    """Handles retry logic with exponential backoff"""

    def __init__(self, config: RetryConfig):
        self.config = config

    def should_retry(self, event: ExternalEvent) -> bool:
        """Determine if event should be retried"""
        if not self.config.enabled:
            return False

        return event.retries < self.config.max_retries

    def get_backoff_time(self, event: ExternalEvent) -> float:
        """Calculate backoff time for retry"""
        backoff = self.config.initial_backoff_seconds * (
            self.config.backoff_multiplier ** event.retries
        )
        return min(backoff, self.config.max_backoff_seconds)


class MessageQueueConsumer(ABC):
    """Abstract base class for message queue consumers"""

    def __init__(self, name: str):
        self.name = name
        self._running = False
        self._event_handlers: Dict[str, List[Callable]] = {}
        self._rate_limiter = RateLimiter(RateLimitConfig())
        self._retry_policy = RetryPolicy(RetryConfig())

    @abstractmethod
    async def connect(self) -> bool:
        """Connect to the message queue"""
        pass

    @abstractmethod
    async def disconnect(self) -> None:
        """Disconnect from the message queue"""
        pass

    @abstractmethod
    async def consume_messages(self) -> None:
        """Main message consumption loop"""
        pass

    def register_handler(
        self,
        event_type: str,
        handler: Callable[[ExternalEvent], Any],
    ) -> None:
        """Register a handler for a specific event type"""
        if event_type not in self._event_handlers:
            self._event_handlers[event_type] = []
        self._event_handlers[event_type].append(handler)

    def unregister_handler(
        self,
        event_type: str,
        handler: Callable[[ExternalEvent], Any],
    ) -> None:
        """Unregister a handler"""
        if event_type in self._event_handlers:
            self._event_handlers[event_type].remove(handler)

    async def dispatch_event(self, event: ExternalEvent) -> bool:
        """Dispatch event to registered handlers"""
        await self._rate_limiter.wait_for_allowance(event.priority)

        handlers = self._event_handlers.get(event.event_type, [])
        handlers += self._event_handlers.get("*", [])  # Wildcard handlers

        if not handlers:
            logger.warning(f"No handlers registered for event type: {event.event_type}")
            return False

        try:
            results = []
            for handler in handlers:
                try:
                    if inspect.iscoroutinefunction(handler):
                        result = await handler(event)
                    else:
                        result = handler(event)
                    results.append(result)
                except Exception as e:
                    logger.error(
                        f"Error in handler for event {event.event_id}: {e}",
                        exc_info=True,
                    )
                    if self._retry_policy.should_retry(event):
                        return False  # Signal for retry
                    return False

            return all(results) if results else True

        except Exception as e:
            logger.error(f"Error dispatching event {event.event_id}: {e}", exc_info=True)
            return False

    async def start(self) -> None:
        """Start consuming messages"""
        if self._running:
            logger.warning(f"{self.name} is already running")
            return

        logger.info(f"Starting {self.name}...")
        self._running = True

        if not await self.connect():
            logger.error(f"Failed to connect {self.name}")
            self._running = False
            return

        try:
            await self.consume_messages()
        except asyncio.CancelledError:
            logger.info(f"{self.name} was cancelled")
        except Exception as e:
            logger.error(f"Error in {self.name}: {e}", exc_info=True)
        finally:
            self._running = False
            await self.disconnect()
            logger.info(f"{self.name} stopped")

    async def stop(self) -> None:
        """Stop consuming messages"""
        self._running = False


class RabbitMQConsumer(MessageQueueConsumer):
    """RabbitMQ AMQP message queue consumer"""

    def __init__(
        self,
        host: str = "localhost",
        port: int = 5672,
        username: str = "guest",
        password: str = "guest",
        vhost: str = "/",
        queue_name: str = "luminaguard.tasks",
        exchange_name: str = "luminaguard.events",
        exchange_type: str = "topic",
    ):
        super().__init__("RabbitMQConsumer")
        self.host = host
        self.port = port
        self.username = username
        self.password = password
        self.vhost = vhost
        self.queue_name = queue_name
        self.exchange_name = exchange_name
        self.exchange_type = exchange_type
        self._connection = None
        self._channel = None

    async def connect(self) -> bool:
        """Connect to RabbitMQ"""
        try:
            import aio_pika

            connection_string = (
                f"amqp://{self.username}:{self.password}@{self.host}:{self.port}/{self.vhost}"
            )

            self._connection = await aio_pika.connect_robust(connection_string)
            self._channel = await self._connection.channel()

            # Declare exchange and queue
            exchange = await self._channel.declare_exchange(
                self.exchange_name,
                aio_pika.ExchangeType.TOPIC,
                durable=True,
            )

            queue = await self._channel.declare_queue(
                self.queue_name,
                durable=True,
            )

            # Bind queue to exchange
            await queue.bind(exchange, routing_key="events.*")

            logger.info(f"Connected to RabbitMQ at {self.host}:{self.port}")
            return True

        except ImportError:
            logger.error("aio_pika not installed. Install with: pip install aio-pika")
            return False
        except Exception as e:
            logger.error(f"Failed to connect to RabbitMQ: {e}")
            return False

    async def disconnect(self) -> None:
        """Disconnect from RabbitMQ"""
        if self._connection:
            await self._connection.close()
            self._connection = None
            self._channel = None

    async def consume_messages(self) -> None:
        """Consume messages from RabbitMQ"""
        if not self._channel:
            logger.error("Not connected to RabbitMQ")
            return

        try:
            queue = await self._channel.get_queue(self.queue_name)

            async with queue.iterator() as queue_iter:
                async for message in queue_iter:
                    if not self._running:
                        break

                    async with message.process():
                        try:
                            event_data = json.loads(message.body.decode())
                            event = ExternalEvent.from_dict(event_data)

                            success = await self.dispatch_event(event)

                            if not success and self._retry_policy.should_retry(event):
                                event.retries += 1
                                # Republish for retry
                                await self._republish_event(event)

                        except Exception as e:
                            logger.error(f"Error processing message: {e}", exc_info=True)

        except Exception as e:
            logger.error(f"Error consuming messages: {e}", exc_info=True)

    async def _republish_event(self, event: ExternalEvent) -> None:
        """Republish event for retry"""
        try:
            backoff = self._retry_policy.get_backoff_time(event)
            exchange = await self._channel.get_exchange(self.exchange_name)

            await asyncio.sleep(backoff)

            message = aio_pika.Message(
                body=json.dumps(event.to_dict()).encode(),
                delivery_mode=aio_pika.DeliveryMode.PERSISTENT,
            )

            await exchange.publish(message, routing_key=f"events.{event.event_type}")

            logger.info(f"Republished event {event.event_id} for retry")

        except Exception as e:
            logger.error(f"Failed to republish event: {e}")


class RedisConsumer(MessageQueueConsumer):
    """Redis message queue consumer (pub/sub and list-based)"""

    def __init__(
        self,
        host: str = "localhost",
        port: int = 6379,
        db: int = 0,
        password: Optional[str] = None,
        queue_mode: str = "list",  # "list" or "pubsub"
        queue_key: str = "luminaguard:tasks",
        channel_pattern: str = "luminaguard:events:*",
    ):
        super().__init__("RedisConsumer")
        self.host = host
        self.port = port
        self.db = db
        self.password = password
        self.queue_mode = queue_mode
        self.queue_key = queue_key
        self.channel_pattern = channel_pattern
        self._redis = None

    async def connect(self) -> bool:
        """Connect to Redis"""
        try:
            import redis.asyncio as redis

            self._redis = await redis.from_url(
                f"redis://{self.host}:{self.port}/{self.db}",
                password=self.password,
                decode_responses=True,
            )

            # Test connection
            await self._redis.ping()

            logger.info(f"Connected to Redis at {self.host}:{self.port}")
            return True

        except ImportError:
            logger.error("redis not installed. Install with: pip install redis")
            return False
        except Exception as e:
            logger.error(f"Failed to connect to Redis: {e}")
            return False

    async def disconnect(self) -> None:
        """Disconnect from Redis"""
        if self._redis:
            await self._redis.close()
            self._redis = None

    async def consume_messages(self) -> None:
        """Consume messages from Redis"""
        if not self._redis:
            logger.error("Not connected to Redis")
            return

        if self.queue_mode == "list":
            await self._consume_list_queue()
        elif self.queue_mode == "pubsub":
            await self._consume_pubsub()
        else:
            logger.error(f"Unknown queue mode: {self.queue_mode}")

    async def _consume_list_queue(self) -> None:
        """Consume messages from Redis list"""
        try:
            while self._running:
                try:
                    # BLPOP with timeout
                    result = await asyncio.wait_for(
                        self._redis.blpop(self.queue_key, timeout=5),
                        timeout=6.0,
                    )

                    if not result:
                        continue

                    _, message_data = result

                    try:
                        event_data = json.loads(message_data)
                        event = ExternalEvent.from_dict(event_data)

                        success = await self.dispatch_event(event)

                        if not success and self._retry_policy.should_retry(event):
                            event.retries += 1
                            await self._republish_event(event)

                    except Exception as e:
                        logger.error(f"Error processing message: {e}", exc_info=True)

                except asyncio.TimeoutError:
                    continue

        except Exception as e:
            logger.error(f"Error consuming list queue: {e}", exc_info=True)

    async def _consume_pubsub(self) -> None:
        """Consume messages from Redis pub/sub"""
        try:
            pubsub = self._redis.pubsub()
            await pubsub.psubscribe(self.channel_pattern)

            async for message in pubsub.listen():
                if not self._running:
                    break

                if message["type"] == "pmessage":
                    try:
                        event_data = json.loads(message["data"])
                        event = ExternalEvent.from_dict(event_data)

                        success = await self.dispatch_event(event)

                        if not success and self._retry_policy.should_retry(event):
                            event.retries += 1
                            await self._republish_event(event)

                    except Exception as e:
                        logger.error(f"Error processing message: {e}", exc_info=True)

            await pubsub.close()

        except Exception as e:
            logger.error(f"Error consuming pub/sub: {e}", exc_info=True)

    async def _republish_event(self, event: ExternalEvent) -> None:
        """Republish event for retry"""
        try:
            backoff = self._retry_policy.get_backoff_time(event)
            await asyncio.sleep(backoff)

            if self.queue_mode == "list":
                await self._redis.rpush(self.queue_key, json.dumps(event.to_dict()))
            elif self.queue_mode == "pubsub":
                await self._redis.publish(
                    f"luminaguard:events:{event.event_type}",
                    json.dumps(event.to_dict()),
                )

            logger.info(f"Republished event {event.event_id} for retry")

        except Exception as e:
            logger.error(f"Failed to republish event: {e}")


class WebhookReceiver:
    """HTTP webhook receiver for external integrations"""

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 9000,
        path: str = "/webhook",
        secret: Optional[str] = None,
    ):
        self.host = host
        self.port = port
        self.path = path
        self.secret = secret
        self._running = False
        self._event_handlers: Dict[str, List[Callable]] = {}
        self._server = None
        self._rate_limiter = RateLimiter(RateLimitConfig())

    def register_handler(
        self,
        event_type: str,
        handler: Callable[[ExternalEvent], Any],
    ) -> None:
        """Register a handler for webhook events"""
        if event_type not in self._event_handlers:
            self._event_handlers[event_type] = []
        self._event_handlers[event_type].append(handler)

    def _verify_signature(self, payload: bytes, signature: str) -> bool:
        """Verify webhook signature using HMAC-SHA256"""
        if not self.secret:
            return True  # No signature verification if no secret

        expected_signature = hmac.new(
            self.secret.encode(),
            payload,
            hashlib.sha256,
        ).hexdigest()

        return hmac.compare_digest(signature, expected_signature)

    async def handle_webhook(self, request) -> Tuple[int, Dict[str, Any]]:
        """Handle incoming webhook request"""
        try:
            # Verify signature
            signature = request.headers.get("X-Luminaguard-Signature")
            payload = await request.read()

            if not self._verify_signature(payload, signature):
                logger.warning("Invalid webhook signature")
                return 401, {"error": "Invalid signature"}

            # Parse event
            event_data = json.loads(payload.decode())
            event = ExternalEvent.from_dict(event_data)

            # Check rate limit
            if not self._rate_limiter.is_allowed(event.priority):
                logger.warning(f"Rate limit exceeded for event {event.event_id}")
                return 429, {"error": "Rate limit exceeded"}

            # Dispatch to handlers
            handlers = self._event_handlers.get(event.event_type, [])
            handlers += self._event_handlers.get("*", [])

            if not handlers:
                logger.warning(f"No handlers for event type: {event.event_type}")
                return 404, {"error": "No handlers registered"}

            results = []
            for handler in handlers:
                try:
                    if inspect.iscoroutinefunction(handler):
                        result = await handler(event)
                    else:
                        result = handler(event)
                    results.append(result)
                except Exception as e:
                    logger.error(f"Handler error for event {event.event_id}: {e}")
                    return 500, {"error": "Handler error"}

            return 200, {"success": True, "event_id": event.event_id}

        except json.JSONDecodeError:
            logger.error("Invalid JSON in webhook request")
            return 400, {"error": "Invalid JSON"}
        except Exception as e:
            logger.error(f"Error processing webhook: {e}", exc_info=True)
            return 500, {"error": "Internal error"}

    async def start(self) -> None:
        """Start webhook receiver"""
        try:
            from aiohttp import web

            app = web.Application()
            app.router.add_post(self.path, self._aiohttp_handler)

            runner = web.AppRunner(app)
            await runner.setup()

            site = web.TCPSite(runner, self.host, self.port)
            await site.start()

            self._running = True
            logger.info(f"Webhook receiver started on {self.host}:{self.port}{self.path}")

        except ImportError:
            logger.error("aiohttp not installed. Install with: pip install aiohttp")
        except Exception as e:
            logger.error(f"Failed to start webhook receiver: {e}")

    async def stop(self) -> None:
        """Stop webhook receiver"""
        self._running = False

    async def _aiohttp_handler(self, request) -> Any:
        """aiohttp request handler"""
        status, response = await self.handle_webhook(request)
        return aiohttp(status=status, json=response)


class IntegrationManager:
    """Manages external integrations (webhooks, message queues)"""

    def __init__(self):
        self._consumers: Dict[str, MessageQueueConsumer] = {}
        self._webhook_receiver: Optional[WebhookReceiver] = None
        self._tasks: List[asyncio.Task] = []

    def add_consumer(self, name: str, consumer: MessageQueueConsumer) -> None:
        """Add a message queue consumer"""
        self._consumers[name] = consumer
        logger.info(f"Added consumer: {name}")

    def remove_consumer(self, name: str) -> None:
        """Remove a message queue consumer"""
        if name in self._consumers:
            del self._consumers[name]
            logger.info(f"Removed consumer: {name}")

    def set_webhook_receiver(self, receiver: WebhookReceiver) -> None:
        """Set the webhook receiver"""
        self._webhook_receiver = receiver

    def register_event_handler(
        self,
        event_type: str,
        handler: Callable[[ExternalEvent], Any],
        consumer_name: Optional[str] = None,
    ) -> None:
        """Register a handler for an event type"""
        if consumer_name:
            if consumer_name in self._consumers:
                self._consumers[consumer_name].register_handler(event_type, handler)
        else:
            # Register with all consumers
            for consumer in self._consumers.values():
                consumer.register_handler(event_type, handler)

        # Register with webhook receiver
        if self._webhook_receiver:
            self._webhook_receiver.register_handler(event_type, handler)

    async def start(self) -> None:
        """Start all integrations"""
        logger.info("Starting external integrations...")

        # Start consumers
        for name, consumer in self._consumers.items():
            task = asyncio.create_task(consumer.start())
            self._tasks.append(task)

        # Start webhook receiver
        if self._webhook_receiver:
            await self._webhook_receiver.start()

        logger.info("External integrations started")

    async def stop(self) -> None:
        """Stop all integrations"""
        logger.info("Stopping external integrations...")

        # Stop webhook receiver
        if self._webhook_receiver:
            await self._webhook_receiver.stop()

        # Stop consumers
        for consumer in self._consumers.values():
            await consumer.stop()

        # Cancel tasks
        for task in self._tasks:
            if not task.done():
                task.cancel()

        # Wait for tasks
        if self._tasks:
            await asyncio.gather(*self._tasks, return_exceptions=True)

        self._tasks.clear()

        logger.info("External integrations stopped")


def create_integration_manager() -> IntegrationManager:
    """Factory function to create an IntegrationManager"""
    return IntegrationManager()
