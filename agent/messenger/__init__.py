"""
Messenger Connector Framework for LuminaGuard

This module provides a unified interface for connecting to various messaging platforms
(Discord, Telegram, WhatsApp) to enable 24/7 bot operation.

Architecture:
- Base MessengerConnector class defines the interface
- Platform-specific implementations handle protocol details
- Message routing and handling is abstracted
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Any, Callable, Optional
import asyncio
import json
import logging

logger = logging.getLogger(__name__)


class MessageType(Enum):
    """Types of messages supported by the messenger framework."""
    TEXT = "text"
    IMAGE = "image"
    FILE = "file"
    AUDIO = "audio"
    VIDEO = "video"
    BUTTON = "button"
    INTERACTIVE = "interactive"


class EventType(Enum):
    """Events that can be received from messaging platforms."""
    MESSAGE = "message"
    MESSAGE_EDITED = "message_edited"
    MESSAGE_DELETED = "message_deleted"
    BUTTON_CLICK = "button_click"
    COMMAND = "command"
    callback = "callback"
    JOIN = "join"
    LEAVE = "leave"
    READY = "ready"
    ERROR = "error"


@dataclass
class Message:
    """Represents a message received from or sent to a messaging platform."""
    id: str
    chat_id: str
    sender_id: str
    sender_name: str
    content: str
    message_type: MessageType
    timestamp: datetime
    metadata: dict[str, Any]
    
    def to_dict(self) -> dict[str, Any]:
        """Convert message to dictionary."""
        return {
            "id": self.id,
            "chat_id": self.chat_id,
            "sender_id": self.sender_id,
            "sender_name": self.sender_name,
            "content": self.content,
            "message_type": self.message_type.value,
            "timestamp": self.timestamp.isoformat(),
            "metadata": self.metadata,
        }


@dataclass
class BotEvent:
    """Represents an event received from a messaging platform."""
    event_type: EventType
    message: Optional[Message]
    raw_data: dict[str, Any]
    timestamp: datetime
    
    @classmethod
    def from_message(cls, event_type: EventType, message: Message, raw_data: dict[str, Any] = None) -> "BotEvent":
        """Create an event from a message."""
        return cls(
            event_type=event_type,
            message=message,
            raw_data=raw_data or {},
            timestamp=datetime.utcnow(),
        )


class MessengerConnector(ABC):
    """
    Abstract base class for messenger platform connectors.
    
    All platform-specific implementations must inherit from this class
    and implement the required abstract methods.
    
    Usage:
        class MyConnector(MessengerConnector):
            async def connect(self): ...
            async def disconnect(self): ...
            async def send_message(self, chat_id: str, content: str): ...
            async def send_image(self, chat_id: str, image_url: str): ...
    """
    
    def __init__(self, config: dict[str, Any]):
        """
        Initialize the connector with platform-specific configuration.
        
        Args:
            config: Dictionary containing platform-specific configuration.
                   Common keys:
                   - token: API token/bot token
                   - api_key: Alternative API key
                   - webhook_url: Webhook URL for receiving events
                   - port: Webhook server port
        """
        self.config = config
        self._running = False
        self._message_handler: Optional[Callable[[BotEvent], Any]] = None
        self._event_handler: Optional[Callable[[BotEvent], Any]] = None
        self._webhook_server: Optional[asyncio.Server] = None
        
    @property
    @abstractmethod
    def platform_name(self) -> str:
        """Return the name of the platform."""
        pass
    
    @property
    def is_connected(self) -> bool:
        """Check if the connector is currently connected."""
        return self._running
    
    @abstractmethod
    async def connect(self) -> bool:
        """
        Establish connection to the messaging platform.
        
        Returns:
            True if connection successful, False otherwise.
        """
        pass
    
    @abstractmethod
    async def disconnect(self) -> None:
        """Disconnect from the messaging platform."""
        pass
    
    @abstractmethod
    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: dict[str, Any] = None
    ) -> str:
        """
        Send a message to a chat.
        
        Args:
            chat_id: The ID of the chat to send to.
            content: The message content.
            message_type: Type of message to send.
            metadata: Additional metadata for the message.
            
        Returns:
            The ID of the sent message.
        """
        pass
    
    @abstractmethod
    async def send_image(
        self,
        chat_id: str,
        image_url: str,
        caption: Optional[str] = None
    ) -> str:
        """
        Send an image to a chat.
        
        Args:
            chat_id: The ID of the chat to send to.
            image_url: URL of the image to send.
            caption: Optional caption for the image.
            
        Returns:
            The ID of the sent message.
        """
        pass
    
    @abstractmethod
    async def send_file(
        self,
        chat_id: str,
        file_url: str,
        filename: Optional[str] = None
    ) -> str:
        """
        Send a file to a chat.
        
        Args:
            chat_id: The ID of the chat to send to.
            file_url: URL of the file to send.
            filename: Optional filename.
            
        Returns:
            The ID of the sent message.
        """
        pass
    
    @abstractmethod
    async def edit_message(
        self,
        chat_id: str,
        message_id: str,
        new_content: str
    ) -> bool:
        """
        Edit a previously sent message.
        
        Args:
            chat_id: The ID of the chat containing the message.
            message_id: The ID of the message to edit.
            new_content: The new content for the message.
            
        Returns:
            True if successful, False otherwise.
        """
        pass
    
    @abstractmethod
    async def delete_message(
        self,
        chat_id: str,
        message_id: str
    ) -> bool:
        """
        Delete a message.
        
        Args:
            chat_id: The ID of the chat containing the message.
            message_id: The ID of the message to delete.
            
        Returns:
            True if successful, False otherwise.
        """
        pass
    
    @abstractmethod
    async def send_buttons(
        self,
        chat_id: str,
        content: str,
        buttons: list[dict[str, str]]
    ) -> str:
        """
        Send interactive buttons to a chat.
        
        Args:
            chat_id: The ID of the chat to send to.
            content: Text content above the buttons.
            buttons: List of button definitions, each with 'id' and 'label'.
            
        Returns:
            The ID of the sent message.
        """
        pass
    
    def set_message_handler(self, handler: Callable[[BotEvent], Any]) -> None:
        """
        Set the handler for incoming messages.
        
        Args:
            handler: Async function that handles BotEvent.
        """
        self._message_handler = handler
    
    def set_event_handler(self, handler: Callable[[BotEvent], Any]) -> None:
        """
        Set the handler for platform events.
        
        Args:
            handler: Async function that handles BotEvent.
        """
        self._event_handler = handler
    
    async def _dispatch_event(self, event: BotEvent) -> None:
        """Dispatch an event to the appropriate handler."""
        if self._event_handler and event.event_type != EventType.MESSAGE:
            try:
                await self._event_handler(event)
            except Exception as e:
                logger.error(f"Error in event handler: {e}")
        
        if self._message_handler and event.message:
            try:
                await self._message_handler(event)
            except Exception as e:
                logger.error(f"Error in message handler: {e}")
    
    async def start_webhook_server(
        self,
        host: str = "0.0.0.0",
        port: int = 8080
    ) -> None:
        """
        Start a webhook server to receive events.
        
        This is a default implementation that can be overridden
        by platform-specific implementations.
        
        Args:
            host: Host to bind to.
            port: Port to listen on.
        """
        from aiohttp import web
        
        async def webhook_handler(request):
            """Handle incoming webhook requests."""
            try:
                data = await request.json()
                event = await self._parse_webhook_data(data)
                if event:
                    await self._dispatch_event(event)
                return web.Response(status=200)
            except Exception as e:
                logger.error(f"Webhook error: {e}")
                return web.Response(status=500, text=str(e))
        
        app = web.Application()
        app.router.add_post("/webhook", webhook_handler)
        
        self._webhook_server = await app.make_handler(host=host, port=port)
        logger.info(f"Webhook server started on {host}:{port}")
    
    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """
        Parse webhook data into a BotEvent.
        
        Override this in platform-specific implementations.
        """
        return None
    
    def __repr__(self) -> str:
        return f"<{self.__class__.__name__} platform={self.platform_name} connected={self.is_connected}>"


class MessageRouter:
    """
    Routes messages to appropriate handlers based on content/commands.
    
    Usage:
        router = MessageRouter()
        @router.command("help")
        async def handle_help(event: BotEvent):
            return "Available commands: help, status, restart"
        
        @router.message()
        async def handle_message(event: BotEvent):
            return "I received your message!"
    """
    
    def __init__(self):
        self._command_handlers: dict[str, Callable] = {}
        self._message_handlers: list[Callable] = []
        self._callback_handlers: dict[str, Callable] = {}
    
    def command(self, command: str) -> Callable:
        """Decorator to register a command handler."""
        def decorator(func: Callable) -> Callable:
            self._command_handlers[command.lower()] = func
            return func
        return decorator
    
    def message(self) -> Callable:
        """Decorator to register a default message handler."""
        def decorator(func: Callable) -> Callable:
            self._message_handlers.append(func)
            return func
        return decorator
    
    def callback(self, callback_id: str) -> Callable:
        """Decorator to register a callback handler."""
        def decorator(func: Callable) -> Callable:
            self._callback_handlers[callback_id] = func
            return func
        return decorator
    
    async def route(self, event: BotEvent) -> Optional[str]:
        """
        Route an event to the appropriate handler.
        
        Returns:
            Response string, or None if no handler matched.
        """
        # Handle commands
        if event.message and event.message.content.startswith("/"):
            parts = event.message.content[1:].split(maxsplit=1)
            command = parts[0].lower()
            if command in self._command_handlers:
                handler = self._command_handlers[command]
                return await handler(event)
        
        # Handle callbacks
        if event.event_type == EventType.BUTTON_CLICK:
            callback_id = event.raw_data.get("callback_id")
            if callback_id and callback_id in self._callback_handlers:
                handler = self._callback_handlers[callback_id]
                return await handler(event)
        
        # Handle default messages
        for handler in self._message_handlers:
            result = await handler(event)
            if result:
                return result
        
        return None


class MessengerBot:
    """
    Main bot class that manages messenger connectors.
    
    Usage:
        bot = MessengerBot()
        
        @bot.on_message()
        async def handle_message(event):
            return f"You said: {event.message.content}"
        
        await bot.add_connector(DiscordConnector(config))
        await bot.start()
    """
    
    def __init__(self):
        self._connectors: list[MessengerConnector] = []
        self._router = MessageRouter()
        self._running = False
    
    def on_message(self) -> Callable:
        """Decorator to register a message handler."""
        return self._router.message()
    
    def command(self, command: str) -> Callable:
        """Decorator to register a command handler."""
        return self._router.command(command)
    
    def callback(self, callback_id: str) -> Callable:
        """Decorator to register a callback handler."""
        return self._router.callback(callback_id)
    
    async def add_connector(self, connector: MessengerConnector) -> None:
        """Add a messenger connector to the bot."""
        connector.set_message_handler(self._handle_message)
        connector.set_event_handler(self._handle_event)
        self._connectors.append(connector)
    
    async def _handle_message(self, event: BotEvent) -> None:
        """Handle incoming messages."""
        response = await self._router.route(event)
        if response and event.message:
            # Find the connector that received this message
            for connector in self._connectors:
                if connector.is_connected:
                    await connector.send_message(
                        event.message.chat_id,
                        response
                    )
    
    async def _handle_event(self, event: BotEvent) -> None:
        """Handle non-message events."""
        logger.info(f"Event received: {event.event_type}")
    
    async def start(self) -> None:
        """Start all connectors."""
        self._running = True
        for connector in self._connectors:
            await connector.connect()
        logger.info(f"Messenger bot started with {len(self._connectors)} connectors")
    
    async def stop(self) -> None:
        """Stop all connectors."""
        self._running = False
        for connector in self._connectors:
            await connector.disconnect()
        logger.info("Messenger bot stopped")
    
    @property
    def connectors(self) -> list[MessengerConnector]:
        """Get list of active connectors."""
        return self._connectors
    
    @property
    def is_running(self) -> bool:
        """Check if the bot is running."""
        return self._running
