# External Integration API - Message Queue Integration

This module provides external integration capabilities for LuminaGuard daemon mode:

- **RabbitMQ (AMQP)**: Message queue consumer for async task triggering
- **Redis**: Support for both list-based and pub/sub message queues
- **Webhooks**: HTTP callback receiver for external integrations
- **Event-Driven Processing**: Dispatch events to registered handlers
- **Rate Limiting**: Token bucket rate limiter for controlled event processing
- **Retry Policies**: Automatic retry with exponential backoff

## Components

### Core Classes

#### ExternalEvent
Represents an event coming from an external source.

```python
from daemon.integration import ExternalEvent, EventPriority

event = ExternalEvent(
    event_id="evt-123",
    event_type="task.created",
    source="webhook",
    payload={"task_id": "123", "status": "pending"},
    priority=EventPriority.HIGH,  # NORMAL, HIGH, CRITICAL
)
```

#### RateLimiter
Token bucket rate limiter for controlling event processing rate.

```python
from daemon.integration import RateLimiter, RateLimitConfig

config = RateLimitConfig(
    max_events_per_second=100,
    max_burst_size=500,
    enabled=True,
)
limiter = RateLimiter(config)

if limiter.is_allowed(priority):
    # Process event
    pass
```

#### RetryPolicy
Handles retry logic with exponential backoff.

```python
from daemon.integration import RetryPolicy, RetryConfig

config = RetryConfig(
    max_retries=3,
    initial_backoff_seconds=1.0,
    max_backoff_seconds=60.0,
    backoff_multiplier=2.0,
    enabled=True,
)
policy = RetryPolicy(config)

if policy.should_retry(event):
    backoff_time = policy.get_backoff_time(event)
    await asyncio.sleep(backoff_time)
    # Retry event
```

### Message Queue Consumers

#### RabbitMQConsumer
AMQP message queue consumer using RabbitMQ.

```python
from daemon.integration import RabbitMQConsumer

consumer = RabbitMQConsumer(
    host="localhost",
    port=5672,
    username="guest",
    password="guest",
    vhost="/",
    queue_name="luminaguard.tasks",
    exchange_name="luminaguard.events",
)

# Register event handlers
async def handle_task_created(event):
    print(f"Task created: {event.payload}")
    return True

consumer.register_handler("task.created", handle_task_created)

# Start consuming
await consumer.start()
```

**Dependencies**: `pip install aio-pika`

**Features**:
- AMQP protocol support
- Durable exchanges and queues
- Topic-based routing
- Automatic reconnection
- Graceful shutdown

#### RedisConsumer
Redis message queue consumer with list and pub/sub support.

```python
from daemon.integration import RedisConsumer

# List-based queue
consumer = RedisConsumer(
    host="localhost",
    port=6379,
    queue_mode="list",  # or "pubsub"
    queue_key="luminaguard:tasks",
)

# Register handlers
async def handle_event(event):
    print(f"Event: {event.event_type}")
    return True

consumer.register_handler("*", handle_event)

# Start consuming
await consumer.start()
```

**Dependencies**: `pip install redis`

**Features**:
- List-based queue with blocking pop
- Pub/sub pattern matching
- Automatic reconnection
- Configurable timeouts

### Webhook Receiver

```python
from daemon.integration import WebhookReceiver

receiver = WebhookReceiver(
    host="0.0.0.0",
    port=9000,
    path="/webhook",
    secret="webhook-secret",  # Optional HMAC signature verification
)

# Register handlers
async def handle_webhook(event):
    print(f"Webhook event: {event.event_type}")
    return True

receiver.register_handler("*", handle_webhook)

# Start webhook server
await receiver.start()
```

**Features**:
- HTTP endpoint for receiving webhooks
- HMAC-SHA256 signature verification
- Rate limiting per source
- JSON payload support

### Integration Manager

Coordinate multiple integrations:

```python
from daemon.integration import IntegrationManager

manager = IntegrationManager()

# Add consumers
manager.add_consumer("rabbitmq", RabbitMQConsumer())
manager.add_consumer("redis", RedisConsumer())
manager.set_webhook_receiver(WebhookReceiver())

# Register global handler
async def global_handler(event):
    print(f"Global: {event.event_type}")
    return True

manager.register_event_handler("*", global_handler)

# Register consumer-specific handler
async def rabbitmq_handler(event):
    print(f"RabbitMQ: {event.event_type}")
    return True

manager.register_event_handler(
    "task.created",
    rabbitmq_handler,
    consumer_name="rabbitmq"
)

# Start all integrations
await manager.start()

# Stop all integrations
await manager.stop()
```

## Configuration

### Daemon Config Integration

Update `agent/daemon/config.py` with integration settings:

```yaml
integration:
  enabled: true
  
  # Webhook
  webhook_enabled: true
  webhook_host: "0.0.0.0"
  webhook_port: 9000
  webhook_path: "/webhook"
  webhook_secret: "your-secret"
  
  # RabbitMQ
  rabbitmq_enabled: true
  rabbitmq_host: "localhost"
  rabbitmq_port: 5672
  rabbitmq_username: "guest"
  rabbitmq_password: "guest"
  rabbitmq_queue: "luminaguard.tasks"
  rabbitmq_exchange: "luminaguard.events"
  
  # Redis
  redis_enabled: true
  redis_host: "localhost"
  redis_port: 6379
  redis_db: 0
  redis_mode: "list"  # or "pubsub"
  redis_queue_key: "luminaguard:tasks"
  
  # Rate Limiting
  rate_limit_enabled: true
  rate_limit_max_per_second: 100
  rate_limit_burst_size: 500
  
  # Retry Policy
  retry_enabled: true
  retry_max_attempts: 3
  retry_initial_backoff: 1.0
  retry_max_backoff: 60.0
  retry_backoff_multiplier: 2.0
```

## Event Structure

### Creating Events

```python
from daemon.integration import ExternalEvent, EventPriority
from datetime import datetime
import uuid

event = ExternalEvent(
    event_id=str(uuid.uuid4()),
    event_type="task.created",
    source="webhook",
    payload={
        "task_id": "task-123",
        "title": "Example Task",
        "description": "Task description",
        "priority": "high",
    },
    priority=EventPriority.HIGH,
    metadata={
        "source_ip": "192.168.1.1",
        "user_id": "user-456",
    },
)
```

### Event Serialization

```python
# Serialize to dict/JSON
event_dict = event.to_dict()
import json
event_json = json.dumps(event_dict)

# Deserialize from dict/JSON
event_data = json.loads(event_json)
event = ExternalEvent.from_dict(event_data)
```

## Event Handlers

### Synchronous Handlers

```python
def sync_handler(event: ExternalEvent) -> bool:
    """Synchronous event handler"""
    print(f"Handling: {event.event_id}")
    return True

consumer.register_handler("task.created", sync_handler)
```

### Asynchronous Handlers

```python
async def async_handler(event: ExternalEvent) -> bool:
    """Asynchronous event handler"""
    print(f"Handling: {event.event_id}")
    await some_async_operation()
    return True

consumer.register_handler("task.created", async_handler)
```

### Wildcard Handlers

```python
# Handle all events
async def wildcard_handler(event: ExternalEvent) -> bool:
    print(f"Event: {event.event_type}")
    return True

consumer.register_handler("*", wildcard_handler)
```

### Handler Error Handling

Handlers should return `True` on success, `False` to trigger retry:

```python
async def robust_handler(event: ExternalEvent) -> bool:
    try:
        # Process event
        result = await process(event)
        return True
    except Exception as e:
        logger.error(f"Error: {e}")
        # Return False to trigger retry
        return False
```

## Rate Limiting

### Token Bucket Algorithm

- **Tokens**: Resources available for processing events
- **Refill Rate**: Events per second (max_events_per_second)
- **Burst Size**: Maximum tokens available (max_burst_size)
- **Priority Levels**: Critical uses fewer tokens than normal

```python
config = RateLimitConfig(
    max_events_per_second=100,  # Refill 100 tokens/second
    max_burst_size=500,         # Maximum 500 tokens
    enabled=True,
)

limiter = RateLimiter(config)

# Check if allowed (non-blocking)
if limiter.is_allowed(EventPriority.NORMAL):
    # Process event
    pass

# Wait for allowance (blocking)
await limiter.wait_for_allowance(EventPriority.HIGH)
```

### Priority-based Rate Limiting

Higher priority events consume fewer tokens:

- `CRITICAL`: 1 token
- `HIGH`: 2 tokens
- `NORMAL`: 3 tokens
- `LOW`: 4 tokens

## Retry Policy

### Exponential Backoff

```python
config = RetryConfig(
    max_retries=3,
    initial_backoff_seconds=1.0,
    max_backoff_seconds=60.0,
    backoff_multiplier=2.0,
    enabled=True,
)

# Retry timeline:
# Attempt 1: Fails
# Attempt 2: Wait 1s, retry
# Attempt 3: Wait 2s, retry
# Attempt 4: Wait 4s, retry
# After 3 retries: Give up
```

### Manual Retry

```python
if not await consumer.dispatch_event(event):
    if consumer._retry_policy.should_retry(event):
        event.retries += 1
        backoff = consumer._retry_policy.get_backoff_time(event)
        await asyncio.sleep(backoff)
        # Retry
        await consumer.dispatch_event(event)
```

## Publishing Events

### To RabbitMQ

```python
import json
import aio_pika

connection = await aio_pika.connect_robust("amqp://guest:guest@localhost/")
channel = await connection.channel()
exchange = await channel.declare_exchange("luminaguard.events", aio_pika.ExchangeType.TOPIC)

message = aio_pika.Message(
    body=json.dumps(event.to_dict()).encode(),
    delivery_mode=aio_pika.DeliveryMode.PERSISTENT,
)

await exchange.publish(message, routing_key="events.task.created")
```

### To Redis List

```python
import redis
import json

redis_client = redis.Redis(host="localhost")
redis_client.rpush(
    "luminaguard:tasks",
    json.dumps(event.to_dict())
)
```

### To Redis Pub/Sub

```python
redis_client.publish(
    "luminaguard:events:task.created",
    json.dumps(event.to_dict())
)
```

### To Webhook

```python
import aiohttp
import json
import hmac
import hashlib

async with aiohttp.ClientSession() as session:
    payload = json.dumps(event.to_dict()).encode()
    
    # Generate signature
    signature = hmac.new(
        b"webhook-secret",
        payload,
        hashlib.sha256
    ).hexdigest()
    
    headers = {"X-Luminaguard-Signature": signature}
    
    async with session.post(
        "https://external-service/webhook",
        data=payload,
        headers=headers,
    ) as resp:
        result = await resp.json()
```

## Testing

Run the test suite:

```bash
python -m pytest tests/test_daemon_integration.py -v
```

Test coverage includes:

- Event serialization/deserialization
- Rate limiting with different priorities
- Retry policies and backoff calculation
- Consumer creation and lifecycle
- Event handler registration and dispatching
- Webhook signature verification
- Integration manager coordination

## Monitoring and Logging

All components use Python's standard logging:

```python
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)
```

Monitor key events:

- Consumer connection/disconnection
- Event dispatch and handler execution
- Rate limit hits
- Retry attempts
- Webhook requests

## Performance Considerations

1. **Rate Limiting**: Adjust `max_events_per_second` and `max_burst_size` based on your system capacity
2. **Retry Policy**: Balance between resilience and latency (don't retry too many times)
3. **Handler Execution**: Keep handlers fast; use async for I/O-bound operations
4. **Memory**: Configure appropriate queue sizes in RabbitMQ/Redis to prevent memory issues
5. **Concurrency**: Adjust `max_concurrent_jobs` in scheduler config

## Troubleshooting

### RabbitMQ Connection Issues

```python
# Verify connection
connection_string = "amqp://user:pass@host:port/vhost"
# Test with: python -c "import aio_pika; asyncio.run(aio_pika.connect_robust(...))"
```

### Redis Connection Issues

```python
# Verify connection
import redis
redis_client = redis.Redis(host="localhost", port=6379)
redis_client.ping()  # Should return True
```

### Webhook Signature Verification

```python
# Debug signature:
import hmac
import hashlib

payload = b"..."
secret = b"webhook-secret"
signature = hmac.new(secret, payload, hashlib.sha256).hexdigest()
print(f"Expected: {signature}")
```

## Examples

See `integration_example.py` for complete working examples:

1. RabbitMQ consumer
2. Redis list queue
3. Redis pub/sub
4. Webhook receiver
5. Integration manager
6. Publishing events
7. Error handling
8. Rate limiting configuration
9. Monitoring and metrics

## API Reference

See docstrings in `integration.py` for complete API documentation.

## Integration with Daemon

To integrate with the main daemon:

```python
from daemon.integration import IntegrationManager, RabbitMQConsumer, RedisConsumer, WebhookReceiver
from daemon.config import load_config

# Load config
config = load_config("config.yaml")

# Create manager
manager = IntegrationManager()

# Add consumers based on config
if config.integration.rabbitmq_enabled:
    rabbitmq = RabbitMQConsumer(
        host=config.integration.rabbitmq_host,
        port=config.integration.rabbitmq_port,
        username=config.integration.rabbitmq_username,
        password=config.integration.rabbitmq_password,
        queue_name=config.integration.rabbitmq_queue,
    )
    manager.add_consumer("rabbitmq", rabbitmq)

if config.integration.redis_enabled:
    redis = RedisConsumer(
        host=config.integration.redis_host,
        port=config.integration.redis_port,
        queue_mode=config.integration.redis_mode,
    )
    manager.add_consumer("redis", redis)

if config.integration.webhook_enabled:
    webhook = WebhookReceiver(
        host=config.integration.webhook_host,
        port=config.integration.webhook_port,
        secret=config.integration.webhook_secret,
    )
    manager.set_webhook_receiver(webhook)

# Register your event handlers
async def handle_event(event):
    # Your event processing logic
    return True

manager.register_event_handler("*", handle_event)

# Start in daemon lifecycle
await manager.start()
```

## Related Issues

- **#446**: Daemon Logging and Monitoring - ✅ Implemented
- **#447**: Persistent State Management - ✅ Implemented
- **#449**: Multi-Channel Messenger Support - ✅ Implemented
- **#448**: External Integration API - ✅ This implementation
