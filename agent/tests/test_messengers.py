"""Tests for multi-channel messenger implementations"""

import pytest
import asyncio
from datetime import datetime, timezone
from messenger import (
    Message,
    MessageType,
    EventType,
    BotEvent,
    MessengerBot,
    MessageRouter,
)
from messenger.slack import SlackConnector
from messenger.signal import SignalConnector, SignalGroupConnector


class TestSlackConnector:
    """Test SlackConnector"""

    def test_slack_creation(self):
        """Test creating a Slack connector"""
        config = {
            "bot_token": "xoxb-test-token",
            "signing_secret": "test-secret",
        }
        connector = SlackConnector(config)

        assert connector.platform_name == "slack"
        assert connector.bot_token == "xoxb-test-token"
        assert not connector.is_connected

    @pytest.mark.asyncio
    async def test_slack_send_message_format(self):
        """Test Slack message format"""
        config = {"bot_token": "xoxb-test"}
        connector = SlackConnector(config)

        # This would normally call the API
        # Just test that the method signature is correct
        assert hasattr(connector, "send_message")
        assert hasattr(connector, "send_image")
        assert hasattr(connector, "send_file")

    @pytest.mark.asyncio
    async def test_slack_button_format(self):
        """Test Slack button format"""
        config = {"bot_token": "xoxb-test"}
        connector = SlackConnector(config)

        buttons = [
            {"id": "yes", "label": "Yes"},
            {"id": "no", "label": "No"},
        ]

        # Test that method exists
        assert hasattr(connector, "send_buttons")


class TestSignalConnector:
    """Test SignalConnector"""

    def test_signal_creation(self):
        """Test creating a Signal connector"""
        config = {
            "signal_server_url": "http://localhost:8080",
            "phone_number": "+1234567890",
            "api_key": "test-api-key",
        }
        connector = SignalConnector(config)

        assert connector.platform_name == "signal"
        assert connector.phone_number == "+1234567890"
        assert not connector.is_connected

    def test_signal_group_id_detection(self):
        """Test Signal group ID detection"""
        config = {
            "signal_server_url": "http://localhost:8080",
            "phone_number": "+1234567890",
            "api_key": "test-api-key",
        }
        connector = SignalConnector(config)

        # Phone number should not be detected as group
        assert not connector._is_group_id("+1234567890")

        # Long hash-like ID should be detected as group
        assert connector._is_group_id("1234567890abcdef1234567890abcdef")

    @pytest.mark.asyncio
    async def test_signal_send_message_format(self):
        """Test Signal message format"""
        config = {
            "signal_server_url": "http://localhost:8080",
            "phone_number": "+1234567890",
            "api_key": "test-api-key",
        }
        connector = SignalConnector(config)

        assert hasattr(connector, "send_message")
        assert hasattr(connector, "send_image")
        assert hasattr(connector, "send_file")


class TestSignalGroupConnector:
    """Test SignalGroupConnector with group management"""

    def test_signal_group_connector_creation(self):
        """Test creating a Signal group connector"""
        config = {
            "signal_server_url": "http://localhost:8080",
            "phone_number": "+1234567890",
            "api_key": "test-api-key",
        }
        connector = SignalGroupConnector(config)

        assert connector.platform_name == "signal"
        assert hasattr(connector, "create_group")
        assert hasattr(connector, "add_group_members")


class TestMessengerBot:
    """Test MessengerBot with multiple connectors"""

    @pytest.mark.asyncio
    async def test_bot_creation(self):
        """Test creating a messenger bot"""
        bot = MessengerBot()

        assert not bot.is_running
        assert len(bot.connectors) == 0

    @pytest.mark.asyncio
    async def test_bot_add_connector(self):
        """Test adding connectors to bot"""
        bot = MessengerBot()

        slack_config = {"bot_token": "xoxb-test"}
        slack_connector = SlackConnector(slack_config)

        await bot.add_connector(slack_connector)

        assert len(bot.connectors) == 1
        assert bot.connectors[0].platform_name == "slack"

    @pytest.mark.asyncio
    async def test_bot_multi_connector(self):
        """Test bot with multiple connectors"""
        bot = MessengerBot()

        slack_config = {"bot_token": "xoxb-test"}
        slack_connector = SlackConnector(slack_config)

        signal_config = {
            "signal_server_url": "http://localhost:8080",
            "phone_number": "+1234567890",
            "api_key": "test-api-key",
        }
        signal_connector = SignalConnector(signal_config)

        await bot.add_connector(slack_connector)
        await bot.add_connector(signal_connector)

        assert len(bot.connectors) == 2
        platforms = {c.platform_name for c in bot.connectors}
        assert platforms == {"slack", "signal"}

    @pytest.mark.asyncio
    async def test_bot_message_handler(self):
        """Test bot message handler registration"""
        bot = MessengerBot()

        @bot.on_message()
        async def handle_message(event):
            return f"Echo: {event.message.content}"

        # Router should have message handler
        assert len(bot._router._message_handlers) > 0


class TestMessageRouter:
    """Test MessageRouter"""

    def test_router_creation(self):
        """Test creating a message router"""
        router = MessageRouter()

        assert len(router._command_handlers) == 0
        assert len(router._message_handlers) == 0
        assert len(router._callback_handlers) == 0

    def test_router_command_registration(self):
        """Test registering command handlers"""
        router = MessageRouter()

        @router.command("help")
        async def handle_help(event):
            return "Available commands..."

        assert "help" in router._command_handlers

    def test_router_callback_registration(self):
        """Test registering callback handlers"""
        router = MessageRouter()

        @router.callback("button_yes")
        async def handle_yes(event):
            return "You clicked yes"

        assert "button_yes" in router._callback_handlers

    @pytest.mark.asyncio
    async def test_router_command_routing(self):
        """Test command routing"""
        router = MessageRouter()

        @router.command("hello")
        async def handle_hello(event):
            return "Hello there!"

        message = Message(
            id="1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User",
            content="/hello",
            message_type=MessageType.TEXT,
            timestamp=datetime.now(timezone.utc),
            metadata={},
        )

        event = BotEvent.from_message(EventType.MESSAGE, message)
        result = await router.route(event)

        assert result == "Hello there!"

    @pytest.mark.asyncio
    async def test_router_message_routing(self):
        """Test default message routing"""
        router = MessageRouter()

        @router.message()
        async def handle_message(event):
            return f"Got: {event.message.content}"

        message = Message(
            id="1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User",
            content="Hello bot",
            message_type=MessageType.TEXT,
            timestamp=datetime.now(timezone.utc),
            metadata={},
        )

        event = BotEvent.from_message(EventType.MESSAGE, message)
        result = await router.route(event)

        assert result == "Got: Hello bot"

    @pytest.mark.asyncio
    async def test_router_no_match(self):
        """Test router when no handler matches"""
        router = MessageRouter()

        message = Message(
            id="1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User",
            content="/unknown",
            message_type=MessageType.TEXT,
            timestamp=datetime.now(timezone.utc),
            metadata={},
        )

        event = BotEvent.from_message(EventType.MESSAGE, message)
        result = await router.route(event)

        assert result is None


class TestMessage:
    """Test Message dataclass"""

    def test_message_creation(self):
        """Test creating a message"""
        now = datetime.now(timezone.utc)
        message = Message(
            id="msg1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User One",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=now,
            metadata={"platform": "slack"},
        )

        assert message.id == "msg1"
        assert message.content == "Hello"
        assert message.sender_name == "User One"

    def test_message_to_dict(self):
        """Test converting message to dict"""
        now = datetime.now(timezone.utc)
        message = Message(
            id="msg1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=now,
            metadata={},
        )

        data = message.to_dict()

        assert data["id"] == "msg1"
        assert data["content"] == "Hello"
        assert data["message_type"] == "text"


class TestBotEvent:
    """Test BotEvent"""

    def test_bot_event_creation(self):
        """Test creating a bot event"""
        now = datetime.now(timezone.utc)
        message = Message(
            id="msg1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=now,
            metadata={},
        )

        event = BotEvent(
            event_type=EventType.MESSAGE,
            message=message,
            raw_data={},
            timestamp=now,
        )

        assert event.event_type == EventType.MESSAGE
        assert event.message == message

    def test_bot_event_from_message(self):
        """Test creating event from message"""
        now = datetime.now(timezone.utc)
        message = Message(
            id="msg1",
            chat_id="ch1",
            sender_id="user1",
            sender_name="User",
            content="Hello",
            message_type=MessageType.TEXT,
            timestamp=now,
            metadata={},
        )

        event = BotEvent.from_message(EventType.MESSAGE, message)

        assert event.event_type == EventType.MESSAGE
        assert event.message == message


if __name__ == "__main__":
    pytest.main([__file__])
