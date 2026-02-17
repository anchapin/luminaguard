"""
Unit tests for the Messenger Connector Framework
"""

import asyncio
from datetime import datetime
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from messenger import (
    BotEvent,
    EventType,
    Message,
    MessageType,
    MessengerBot,
    MessengerConnector,
    MessageRouter,
)
from messenger.discord import DiscordConnector
from messenger.telegram import TelegramConnector


class TestMessage:
    """Tests for the Message dataclass."""
    
    def test_message_creation(self):
        """Test creating a Message."""
        msg = Message(
            id="123",
            chat_id="456",
            sender_id="789",
            sender_name="Test User",
            content="Hello world",
            message_type=MessageType.TEXT,
            timestamp=datetime.utcnow(),
            metadata={}
        )
        
        assert msg.id == "123"
        assert msg.chat_id == "456"
        assert msg.content == "Hello world"
        assert msg.message_type == MessageType.TEXT
    
    def test_message_to_dict(self):
        """Test converting Message to dictionary."""
        msg = Message(
            id="123",
            chat_id="456",
            sender_id="789",
            sender_name="Test User",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=datetime(2026, 1, 1, 12, 0, 0),
            metadata={"key": "value"}
        )
        
        d = msg.to_dict()
        assert d["id"] == "123"
        assert d["content"] == "Hello"
        assert d["metadata"]["key"] == "value"


class TestBotEvent:
    """Tests for the BotEvent dataclass."""
    
    def test_bot_event_creation(self):
        """Test creating a BotEvent."""
        msg = Message(
            id="123",
            chat_id="456",
            sender_id="789",
            sender_name="Test User",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=datetime.utcnow(),
            metadata={}
        )
        
        event = BotEvent.from_message(EventType.MESSAGE, msg, {"raw": "data"})
        
        assert event.event_type == EventType.MESSAGE
        assert event.message is not None
        assert event.message.content == "Hello"


class TestMessageRouter:
    """Tests for the MessageRouter class."""
    
    @pytest.mark.asyncio
    async def test_command_handler(self):
        """Test registering and calling command handlers."""
        router = MessageRouter()
        
        @router.command("test")
        async def handle_test(event):
            return "Test response"
        
        # Create a mock event
        msg = Message(
            id="1",
            chat_id="1",
            sender_id="1",
            sender_name="User",
            content="/test",
            message_type=MessageType.TEXT,
            timestamp=datetime.utcnow(),
            metadata={}
        )
        event = BotEvent.from_message(EventType.COMMAND, msg)
        
        response = await router.route(event)
        assert response == "Test response"
    
    @pytest.mark.asyncio
    async def test_message_handler(self):
        """Test registering and calling message handlers."""
        router = MessageRouter()
        
        @router.message()
        async def handle_message(event):
            return f"Echo: {event.message.content}"
        
        # Create a mock event (not a command)
        msg = Message(
            id="1",
            chat_id="1",
            sender_id="1",
            sender_name="User",
            content="Hello world",
            message_type=MessageType.TEXT,
            timestamp=datetime.utcnow(),
            metadata={}
        )
        event = BotEvent.from_message(EventType.MESSAGE, msg)
        
        response = await router.route(event)
        assert response == "Echo: Hello world"
    
    @pytest.mark.asyncio
    async def test_no_matching_handler(self):
        """Test behavior when no handler matches."""
        router = MessageRouter()
        
        # No handlers registered
        
        msg = Message(
            id="1",
            chat_id="1",
            sender_id="1",
            sender_name="User",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=datetime.utcnow(),
            metadata={}
        )
        event = BotEvent.from_message(EventType.MESSAGE, msg)
        
        response = await router.route(event)
        assert response is None


class TestMessengerBot:
    """Tests for the MessengerBot class."""
    
    @pytest.mark.asyncio
    async def test_bot_creation(self):
        """Test creating a MessengerBot."""
        bot = MessengerBot()
        
        assert not bot.is_running
        assert len(bot.connectors) == 0
    
    @pytest.mark.asyncio
    async def test_add_connector(self):
        """Test adding a connector to the bot."""
        bot = MessengerBot()
        
        # Create a mock connector
        mock_connector = MagicMock(spec=MessengerConnector)
        mock_connector.platform_name = "test"
        mock_connector.is_connected = False
        
        await bot.add_connector(mock_connector)
        
        assert len(bot.connectors) == 1
    
    @pytest.mark.asyncio
    async def test_start_and_stop(self):
        """Test starting and stopping the bot."""
        bot = MessengerBot()
        
        # Create mock connectors
        mock_connector = MagicMock(spec=MessengerConnector)
        mock_connector.platform_name = "test"
        mock_connector.is_connected = False
        
        await bot.add_connector(mock_connector)
        
        # Mock connector's connect method
        mock_connector.connect = AsyncMock(return_value=True)
        mock_connector.disconnect = AsyncMock()
        
        await bot.start()
        
        assert bot.is_running
        mock_connector.connect.assert_called_once()
        
        await bot.stop()
        
        assert not bot.is_running
        mock_connector.disconnect.assert_called_once()


class TestDiscordConnector:
    """Tests for the DiscordConnector class."""
    
    @pytest.mark.asyncio
    async def test_connector_creation(self):
        """Test creating a DiscordConnector."""
        config = {
            "token": "test_token",
            "webhook_url": "https://example.com/webhook"
        }
        connector = DiscordConnector(config)
        
        assert connector.platform_name == "discord"
        assert connector.token == "test_token"
        assert connector.webhook_url == "https://example.com/webhook"
    
    @pytest.mark.asyncio
    async def test_send_message_without_connection(self):
        """Test sending a message without being connected raises error."""
        config = {"token": "test_token"}
        connector = DiscordConnector(config)
        
        with pytest.raises(RuntimeError, match="Not connected"):
            await connector.send_message("123", "Hello")


class TestTelegramConnector:
    """Tests for the TelegramConnector class."""
    
    @pytest.mark.asyncio
    async def test_connector_creation(self):
        """Test creating a TelegramConnector."""
        config = {
            "token": "test_token",
            "webhook_url": "https://example.com/webhook",
            "webhook_secret": "secret"
        }
        connector = TelegramConnector(config)
        
        assert connector.platform_name == "telegram"
        assert connector.token == "test_token"
        assert connector.webhook_url == "https://example.com/webhook"
        assert connector.webhook_secret == "secret"
    
    @pytest.mark.asyncio
    async def test_send_message_without_connection(self):
        """Test sending a message without being connected raises error."""
        config = {"token": "test_token"}
        connector = TelegramConnector(config)
        
        with pytest.raises(RuntimeError, match="Not connected"):
            await connector.send_message("123", "Hello")
    
    def test_verify_webhook_secret_not_required(self):
        """Test webhook secret not required when not set."""
        config = {"token": "test_token"}
        connector = TelegramConnector(config)
        
        # Should return True when no secret is configured
        assert connector._verify_webhook_secret("any", "data") is True


class TestMessengerConnector:
    """Tests for the MessengerConnector abstract class."""
    
    def test_cannot_instantiate_directly(self):
        """Test that MessengerConnector cannot be instantiated directly."""
        with pytest.raises(TypeError):
            MessengerConnector({})
    
    def test_abstract_methods(self):
        """Test that abstract methods must be implemented."""
        class IncompleteConnector(MessengerConnector):
            pass
        
        with pytest.raises(TypeError):
            IncompleteConnector({})
    
    def test_partial_implementation(self):
        """Test that partial implementation raises error."""
        class PartialConnector(MessengerConnector):
            @property
            def platform_name(self):
                return "test"
            
            async def connect(self):
                return True
        
        with pytest.raises(TypeError):
            PartialConnector({})


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
