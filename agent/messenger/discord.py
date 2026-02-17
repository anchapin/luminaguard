"""
Discord Connector for LuminaGuard Messenger Framework

This module provides Discord-specific implementation of the MessengerConnector.
Supports bot mode with Discord's HTTP API and webhooks for receiving events.
"""

from typing import Any, Optional
import asyncio
import aiohttp
import logging

from . import (
    BotEvent,
    EventType,
    Message,
    MessageType,
    MessengerConnector,
)

logger = logging.getLogger(__name__)


class DiscordConnector(MessengerConnector):
    """
    Discord bot connector implementation.
    
    Supports:
    - Sending messages, images, files
    - Receiving messages via webhooks
    - Button interactions
    - Message editing and deletion
    
    Configuration:
        - token: Discord bot token
        - webhook_url: Webhook URL for receiving events (optional)
        - webhook_port: Port for webhook server (default: 8080)
        - intents: Gateway intents to subscribe to
    
    Usage:
        config = {
            "token": "Bot TOKEN",
            "webhook_url": "https://your-domain.com/webhook"
        }
        connector = DiscordConnector(config)
    """
    
    API_BASE = "https://discord.com/api/v10"
    
    def __init__(self, config: dict[str, Any]):
        super().__init__(config)
        self.token = config.get("token", "")
        self.webhook_url = config.get("webhook_url")
        self.webhook_port = config.get("webhook_port", 8080)
        self.intents = config.get("intents", 513)  # Default: GUILDS + GUILD_MESSAGES
        self._session: Optional[aiohttp.ClientSession] = None
        self._websocket = None
        self._sequence: Optional[int] = None
        self._session_id: Optional[str] = None
        
    @property
    def platform_name(self) -> str:
        return "discord"
    
    async def connect(self) -> bool:
        """
        Connect to Discord using bot token.
        
        For webhook-based receiving, sets up the webhook server.
        For gateway-based receiving, establishes websocket connection.
        """
        if not self.token:
            logger.error("Discord bot token is required")
            return False
        
        # Create HTTP session
        self._session = aiohttp.ClientSession(
            headers={
                "Authorization": f"Bot {self.token}",
                "Content-Type": "application/json",
            }
        )
        
        # Validate token by getting bot info
        try:
            async with self._session.get(f"{self.API_BASE}/users/@me") as resp:
                if resp.status != 200:
                    logger.error(f"Invalid Discord token: {resp.status}")
                    await self.disconnect()
                    return False
                bot_info = await resp.json()
                logger.info(f"Logged in as Discord bot: {bot_info.get('username')}")
        except Exception as e:
            logger.error(f"Failed to connect to Discord: {e}")
            await self.disconnect()
            return False
        
        # Start webhook server if configured
        if self.webhook_url:
            await self.start_webhook_server(port=self.webhook_port)
        
        self._running = True
        return True
    
    async def disconnect(self) -> None:
        """Disconnect from Discord."""
        self._running = False
        
        if self._websocket:
            try:
                await self._websocket.close()
            except Exception:
                pass
            self._websocket = None
        
        if self._session:
            await self._session.close()
            self._session = None
        
        logger.info("Disconnected from Discord")
    
    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: dict[str, Any] = None
    ) -> str:
        """
        Send a message to a Discord channel.
        
        Args:
            chat_id: Channel ID to send to.
            content: Message content.
            message_type: Type of message (TEXT, BUTTON, etc.)
            metadata: Additional Discord-specific options (embeds, components, etc.)
            
        Returns:
            Message ID of sent message.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        payload = {
            "content": content,
        }
        
        # Add embeds if provided
        if metadata and "embeds" in metadata:
            payload["embeds"] = metadata["embeds"]
        
        # Add components (buttons) if provided
        if metadata and "components" in metadata:
            payload["components"] = metadata["components"]
        
        # Add reply if provided
        if metadata and "message_reference" in metadata:
            payload["message_reference"] = metadata["message_reference"]
        
        url = f"{self.API_BASE}/channels/{chat_id}/messages"
        
        async with self._session.post(url, json=payload) as resp:
            if resp.status not in (200, 201):
                error = await resp.text()
                raise RuntimeError(f"Failed to send message: {resp.status} - {error}")
            
            data = await resp.json()
            return data["id"]
    
    async def send_image(
        self,
        chat_id: str,
        image_url: str,
        caption: Optional[str] = None
    ) -> str:
        """
        Send an image to a Discord channel.
        
        Args:
            chat_id: Channel ID to send to.
            image_url: URL of the image.
            caption: Optional caption/alt text.
            
        Returns:
            Message ID of sent message.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        payload = {
            "embeds": [
                {
                    "image": {"url": image_url},
                    "description": caption or "",
                }
            ]
        }
        
        url = f"{self.API_BASE}/channels/{chat_id}/messages"
        
        async with self._session.post(url, json=payload) as resp:
            if resp.status not in (200, 201):
                error = await resp.text()
                raise RuntimeError(f"Failed to send image: {resp.status} - {error}")
            
            data = await resp.json()
            return data["id"]
    
    async def send_file(
        self,
        chat_id: str,
        file_url: str,
        filename: Optional[str] = None
    ) -> str:
        """
        Send a file to a Discord channel.
        
        Args:
            chat_id: Channel ID to send to.
            file_url: URL of the file.
            filename: Optional filename.
            
        Returns:
            Message ID of sent message.
        """
        # Discord requires uploading files directly, not via URL
        # This is a simplified implementation
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        # For URL-based files, we send an embed with the file link
        payload = {
            "content": file_url,
        }
        
        if filename:
            payload["content"] = f"**{filename}**: {file_url}"
        
        url = f"{self.API_BASE}/channels/{chat_id}/messages"
        
        async with self._session.post(url, json=payload) as resp:
            if resp.status not in (200, 201):
                error = await resp.text()
                raise RuntimeError(f"Failed to send file: {resp.status} - {error}")
            
            data = await resp.json()
            return data["id"]
    
    async def edit_message(
        self,
        chat_id: str,
        message_id: str,
        new_content: str
    ) -> bool:
        """
        Edit a Discord message.
        
        Args:
            chat_id: Channel ID containing the message.
            message_id: ID of the message to edit.
            new_content: New message content.
            
        Returns:
            True if successful.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        payload = {"content": new_content}
        
        url = f"{self.API_BASE}/channels/{chat_id}/messages/{message_id}"
        
        async with self._session.patch(url, json=payload) as resp:
            if resp.status != 200:
                error = await resp.text()
                raise RuntimeError(f"Failed to edit message: {resp.status} - {error}")
            
            return True
    
    async def delete_message(
        self,
        chat_id: str,
        message_id: str
    ) -> bool:
        """
        Delete a Discord message.
        
        Args:
            chat_id: Channel ID containing the message.
            message_id: ID of the message to delete.
            
        Returns:
            True if successful.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        url = f"{self.API_BASE}/channels/{chat_id}/messages/{message_id}"
        
        async with self._session.delete(url) as resp:
            if resp.status not in (200, 204):
                error = await resp.text()
                raise RuntimeError(f"Failed to delete message: {resp.status} - {error}")
            
            return True
    
    async def send_buttons(
        self,
        chat_id: str,
        content: str,
        buttons: list[dict[str, str]]
    ) -> str:
        """
        Send interactive buttons to a Discord channel.
        
        Args:
            chat_id: Channel ID to send to.
            content: Text content above buttons.
            buttons: List of button definitions with 'id' and 'label'.
            
        Returns:
            Message ID of sent message.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        # Convert buttons to Discord component format
        components = [
            {
                "type": 1,  # Action Row
                "components": [
                    {
                        "type": 2,  # Button
                        "style": 1,  # Primary
                        "label": btn.get("label", "Button"),
                        "custom_id": btn.get("id", btn.get("callback_id", "")),
                    }
                    for btn in buttons
                ]
            }
        ]
        
        payload = {
            "content": content,
            "components": components,
        }
        
        url = f"{self.API_BASE}/channels/{chat_id}/messages"
        
        async with self._session.post(url, json=payload) as resp:
            if resp.status not in (200, 201):
                error = await resp.text()
                raise RuntimeError(f"Failed to send buttons: {resp.status} - {error}")
            
            data = await resp.json()
            return data["id"]
    
    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """
        Parse Discord webhook data into a BotEvent.
        
        Handles:
        - Message created
        - Message updated
        - Message deleted
        - Button interactions (component interactions)
        """
        # Handle interaction callbacks (button clicks, etc.)
        if data.get("type") == 3:  # INTERACTION_CALLBACK
            return await self._handle_interaction(data)
        
        # Handle message events
        message_data = data.get("message", {})
        if not message_data:
            return None
        
        # Create Message object
        message = Message(
            id=message_data.get("id", ""),
            chat_id=data.get("channel_id", ""),
            sender_id=message_data.get("author", {}).get("id", ""),
            sender_name=message_data.get("author", {}).get("username", "Unknown"),
            content=message_data.get("content", ""),
            message_type=MessageType.TEXT,
            timestamp=message_data.get("timestamp", ""),
            metadata=data,
        )
        
        # Determine event type
        event_type = EventType.MESSAGE
        
        if data.get("t") == "MESSAGE_UPDATE":
            event_type = EventType.MESSAGE_EDITED
        elif data.get("t") == "MESSAGE_DELETE":
            event_type = EventType.MESSAGE_DELETED
        
        return BotEvent.from_message(event_type, message, data)
    
    async def _handle_interaction(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """Handle button/interaction callbacks from Discord."""
        # Get the message and user from the interaction
        message_data = data.get("message", {})
        user_data = data.get("user", data.get("member", {}))
        
        message = Message(
            id=message_data.get("id", ""),
            chat_id=message_data.get("channel_id", ""),
            sender_id=user_data.get("id", ""),
            sender_name=user_data.get("username", "Unknown"),
            content="",  # Button clicks don't have content
            message_type=MessageType.BUTTON,
            timestamp=message_data.get("timestamp", ""),
            metadata=data,
        )
        
        # Extract custom_id from the interaction
        raw_data = {
            "callback_id": data.get("data", {}).get("custom_id", ""),
            "interaction_id": data.get("id", ""),
        }
        
        return BotEvent.from_message(EventType.BUTTON_CLICK, message, raw_data)
    
    async def send_followup(
        self,
        interaction_token: str,
        content: str
    ) -> str:
        """
        Send a followup message to an interaction.
        
        Used for responding to button clicks and other interactions.
        
        Args:
            interaction_token: Token from the interaction.
            content: Message content to send.
            
        Returns:
            Message ID of sent message.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        application_id = self.config.get("application_id", "")
        
        payload = {"content": content}
        
        url = f"{self.API_BASE}/webhooks/{application_id}/{interaction_token}"
        
        async with self._session.post(url, json=payload) as resp:
            if resp.status not in (200, 201):
                error = await resp.text()
                raise RuntimeError(f"Failed to send followup: {resp.status} - {error}")
            
            data = await resp.json()
            return data["id"]
    
    async def create_dm(self, user_id: str) -> str:
        """
        Create a DM channel with a user.
        
        Args:
            user_id: User ID to create DM with.
            
        Returns:
            Channel ID of the DM.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        payload = {"recipients": [user_id]}
        
        url = f"{self.API_BASE}/users/@me/channels"
        
        async with self._session.post(url, json=payload) as resp:
            if resp.status not in (200, 201):
                error = await resp.text()
                raise RuntimeError(f"Failed to create DM: {resp.status} - {error}")
            
            data = await resp.json()
            return data["id"]
    
    async def get_channel(self, channel_id: str) -> dict[str, Any]:
        """
        Get information about a channel.
        
        Args:
            channel_id: Channel ID to fetch.
            
        Returns:
            Channel data.
        """
        if not self._session:
            raise RuntimeError("Not connected to Discord")
        
        url = f"{self.API_BASE}/channels/{channel_id}"
        
        async with self._session.get(url) as resp:
            if resp.status != 200:
                raise RuntimeError(f"Failed to get channel: {resp.status}")
            
            return await resp.json()


# Convenience function to create a Discord connector
def create_discord_connector(
    token: str,
    webhook_url: Optional[str] = None,
    webhook_port: int = 8080
) -> DiscordConnector:
    """
    Create a Discord connector with the given configuration.
    
    Args:
        token: Discord bot token.
        webhook_url: Optional webhook URL for receiving events.
        webhook_port: Port for webhook server.
        
    Returns:
        Configured DiscordConnector instance.
    """
    config = {
        "token": token,
        "webhook_url": webhook_url,
        "webhook_port": webhook_port,
    }
    return DiscordConnector(config)
