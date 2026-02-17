"""
External Integration API - Usage Examples

This module demonstrates how to use the external integration system
with RabbitMQ, Redis, and webhooks.

Examples:
    1. Using RabbitMQ message queue
    2. Using Redis list-based queue
    3. Using Redis pub/sub
    4. Using webhook receiver
    5. Using integration manager to coordinate all components
"""

import asyncio
import logging
from integration import (
    RabbitMQConsumer,
    RedisConsumer,
    WebhookReceiver,
    IntegrationManager,
    ExternalEvent,
    EventPriority,
)

logger = logging.getLogger(__name__)


# Example 1: RabbitMQ Consumer
async def example_rabbitmq():
    """Example: Using RabbitMQ as message queue"""
    
    consumer = RabbitMQConsumer(
        host="localhost",
        port=5672,
        username="guest",
        password="guest",
        queue_name="luminaguard.tasks",
        exchange_name="luminaguard.events",
    )
    
    # Register event handlers
    async def handle_task_created(event: ExternalEvent) -> bool:
        """Handle task.created events"""
        logger.info(f"Task created: {event.payload.get('task_id')}")
        return True
    
    async def handle_task_updated(event: ExternalEvent) -> bool:
        """Handle task.updated events"""
        logger.info(f"Task updated: {event.payload.get('task_id')}")
        return True
    
    consumer.register_handler("task.created", handle_task_created)
    consumer.register_handler("task.updated", handle_task_updated)
    
    # Start consuming
    await consumer.start()


# Example 2: Redis List Queue
async def example_redis_list():
    """Example: Using Redis list as message queue"""
    
    consumer = RedisConsumer(
        host="localhost",
        port=6379,
        queue_mode="list",
        queue_key="luminaguard:tasks",
    )
    
    # Register handlers
    async def handle_any_event(event: ExternalEvent) -> bool:
        """Handle all events"""
        logger.info(f"Event: {event.event_type} from {event.source}")
        return True
    
    consumer.register_handler("*", handle_any_event)
    
    # Start consuming
    await consumer.start()


# Example 3: Redis Pub/Sub
async def example_redis_pubsub():
    """Example: Using Redis pub/sub as message broker"""
    
    consumer = RedisConsumer(
        host="localhost",
        port=6379,
        queue_mode="pubsub",
        channel_pattern="luminaguard:events:*",
    )
    
    # Register handlers
    async def handle_event(event: ExternalEvent) -> bool:
        """Handle pub/sub events"""
        logger.info(f"Received event: {event.event_type}")
        return True
    
    consumer.register_handler("*", handle_event)
    
    # Start consuming
    await consumer.start()


# Example 4: Webhook Receiver
async def example_webhook():
    """Example: Using webhook receiver for HTTP callbacks"""
    
    receiver = WebhookReceiver(
        host="0.0.0.0",
        port=9000,
        path="/webhook",
        secret="your-webhook-secret",  # Optional HMAC signature verification
    )
    
    # Register handlers
    async def handle_webhook_event(event: ExternalEvent) -> bool:
        """Handle webhook events"""
        logger.info(f"Webhook received: {event.event_type}")
        return True
    
    receiver.register_handler("*", handle_webhook_event)
    
    # Start webhook server
    await receiver.start()
    
    # Keep running
    try:
        await asyncio.sleep(float('inf'))
    except KeyboardInterrupt:
        await receiver.stop()


# Example 5: Integration Manager
async def example_integration_manager():
    """Example: Coordinating multiple integrations"""
    
    manager = IntegrationManager()
    
    # Create consumers
    rabbitmq_consumer = RabbitMQConsumer(
        host="localhost",
        queue_name="luminaguard.tasks",
    )
    
    redis_consumer = RedisConsumer(
        host="localhost",
        queue_mode="list",
    )
    
    webhook_receiver = WebhookReceiver(
        host="0.0.0.0",
        port=9000,
        secret="webhook-secret",
    )
    
    # Add to manager
    manager.add_consumer("rabbitmq", rabbitmq_consumer)
    manager.add_consumer("redis", redis_consumer)
    manager.set_webhook_receiver(webhook_receiver)
    
    # Register global event handler (available to all integrations)
    async def global_handler(event: ExternalEvent) -> bool:
        """Global event handler"""
        logger.info(f"Global handler: {event.event_type}")
        
        # Process by event type
        if event.event_type == "task.created":
            logger.info(f"New task: {event.payload}")
        elif event.event_type == "task.completed":
            logger.info(f"Task completed: {event.payload}")
        
        return True
    
    manager.register_event_handler("*", global_handler)
    
    # Register specific handlers for specific consumers
    async def rabbitmq_specific(event: ExternalEvent) -> bool:
        """RabbitMQ-specific handler"""
        logger.info(f"RabbitMQ event: {event.event_type}")
        return True
    
    manager.register_event_handler(
        "task.created",
        rabbitmq_specific,
        consumer_name="rabbitmq"
    )
    
    # Start all integrations
    await manager.start()
    
    # Keep running
    try:
        await asyncio.sleep(float('inf'))
    except KeyboardInterrupt:
        await manager.stop()


# Example 6: Publishing Events
async def example_publishing():
    """Example: Publishing events from your application"""
    
    from datetime import datetime
    import uuid
    
    # Create an event
    event = ExternalEvent(
        event_id=str(uuid.uuid4()),
        event_type="task.created",
        source="api",
        payload={
            "task_id": "task-123",
            "title": "Example Task",
            "status": "pending",
        },
        priority=EventPriority.HIGH,
    )
    
    # For RabbitMQ - publish via AMQP
    # (In practice, you would use the RabbitMQ client directly)
    # amqp_message = json.dumps(event.to_dict())
    # await channel.basic_publish(amqp_message, exchange_name, routing_key)
    
    # For Redis - publish via list
    # redis.rpush("luminaguard:tasks", json.dumps(event.to_dict()))
    
    # For Redis pub/sub - publish to channel
    # redis.publish("luminaguard:events:task.created", json.dumps(event.to_dict()))
    
    logger.info(f"Would publish event: {event.to_dict()}")


# Example 7: Custom Event Handler with Error Handling
async def example_error_handling():
    """Example: Handling events with proper error handling"""
    
    consumer = RabbitMQConsumer()
    
    async def resilient_handler(event: ExternalEvent) -> bool:
        """Handler with error handling and logging"""
        try:
            logger.info(f"Processing event: {event.event_id}")
            
            # Do work here
            result = await process_event(event)
            
            if result:
                logger.info(f"Event {event.event_id} processed successfully")
                return True
            else:
                logger.warning(f"Event {event.event_id} processing failed")
                return False
                
        except Exception as e:
            logger.error(f"Error processing event {event.event_id}: {e}", exc_info=True)
            # Return False to trigger retry
            return False
    
    consumer.register_handler("*", resilient_handler)
    
    async def process_event(event: ExternalEvent) -> bool:
        """Simulate event processing"""
        await asyncio.sleep(0.1)
        return True
    
    await consumer.start()


# Example 8: Rate Limiting and Retry Configuration
async def example_rate_limiting():
    """Example: Configuring rate limiting and retry policies"""
    
    from daemon.integration import RateLimitConfig, RetryConfig
    
    consumer = RedisConsumer()
    
    # Configure rate limiting: 100 events per second, burst of 500
    consumer._rate_limiter.config = RateLimitConfig(
        max_events_per_second=100,
        max_burst_size=500,
        enabled=True,
    )
    
    # Configure retries: 5 attempts, exponential backoff from 1s to 300s
    consumer._retry_policy.config = RetryConfig(
        max_retries=5,
        initial_backoff_seconds=1.0,
        max_backoff_seconds=300.0,
        backoff_multiplier=2.0,
        enabled=True,
    )
    
    async def handler(event: ExternalEvent) -> bool:
        logger.info(f"Handling event with rate limit: {event.event_id}")
        return True
    
    consumer.register_handler("*", handler)
    await consumer.start()


# Example 9: Monitoring and Metrics
async def example_monitoring():
    """Example: Monitoring events and performance"""
    
    consumer = RabbitMQConsumer()
    
    # Track metrics
    metrics = {
        "events_received": 0,
        "events_processed": 0,
        "events_failed": 0,
        "total_processing_time": 0.0,
    }
    
    async def monitored_handler(event: ExternalEvent) -> bool:
        """Handler with monitoring"""
        import time
        
        metrics["events_received"] += 1
        start = time.time()
        
        try:
            # Process event
            result = await process_event(event)
            
            if result:
                metrics["events_processed"] += 1
            else:
                metrics["events_failed"] += 1
            
            metrics["total_processing_time"] += time.time() - start
            return result
            
        except Exception as e:
            metrics["events_failed"] += 1
            return False
    
    consumer.register_handler("*", monitored_handler)
    
    async def process_event(event: ExternalEvent) -> bool:
        await asyncio.sleep(0.01)
        return True
    
    # Log metrics periodically
    async def log_metrics():
        while True:
            await asyncio.sleep(60)
            logger.info(f"Metrics: {metrics}")
    
    asyncio.create_task(log_metrics())
    await consumer.start()


if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    )
    
    # Run examples (uncomment the one you want to run)
    # asyncio.run(example_rabbitmq())
    # asyncio.run(example_redis_list())
    # asyncio.run(example_redis_pubsub())
    # asyncio.run(example_webhook())
    # asyncio.run(example_integration_manager())
    # asyncio.run(example_publishing())
    # asyncio.run(example_error_handling())
    # asyncio.run(example_rate_limiting())
    # asyncio.run(example_monitoring())
    
    print("Select an example to run by uncommenting it in __main__")
