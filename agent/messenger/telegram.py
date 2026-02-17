"""
Telegram Connector for LuminaGuard Messenger Framework

This module provides Telegram-specific implementation of the MessengerConnector.
Supports bot mode with Telegram Bot API and webhooks for receiving events.
"""

from typing import Any, Optional
import asyncio
import aiohttp
import logging
import hashlib
import hmac
import json

from . import (
    BotEvent,
    EventType,
    Message,
    MessageType,
    MessengerConnector,
)

logger = logging.getLogger(__name__)


class TelegramConnector(MessengerConnector):
    """
    Telegram bot connector implementation.
    
    Supports:
    - Sending messages, images, files, videos
    - Receiving messages via webhooks or long polling
    - Inline keyboards and callback queries
    - Message editing and deletion
    - Chats and supergroups
    
    Configuration:
        - token: Telegram bot token
        - webhook_url: Webhook URL for receiving events (optional)
        - webhook_secret: Secret token for webhook verification
        - webhook_port: Port for webhook server (default: 8080)
        - poll_timeout: Long polling timeout in seconds (default: 60)
    
    Usage:
        config = {
            "token": "BOT_TOKEN",
            "webhook_url": "https://your-domain.com/webhook",
            "webhook_secret": "your_secret_token"
        }
        connector = TelegramConnector(config)
    """
    
    API_BASE = "https://api.telegram.org"
    
    def __init__(self, config: dict[str, Any]):
        super().__init__(config)
        self.token = config.get("token", "")
        self.webhook_url = config.get("webhook_url")
        self.webhook_secret = config.get("webhook_secret", "")
        self.webhook_port = config.get("webhook_port", 8080)
        self.poll_timeout = config.get("poll_timeout", 60)
        self._session: Optional[aiohttp.ClientSession] = None
        self._offset = 0
        self._polling_task: Optional[asyncio.Task] = None
        self._bot_info: Optional[dict] = None
        
    @property
    def platform_name(self) -> str:
        return "telegram"
    
    async def connect(self) -> bool:
        """
        Connect to Telegram using bot token.
        
        For webhook-based receiving, sets up the webhook server.
        For long polling, starts the polling loop.
        """
        if not self.token:
            logger.error("Telegram bot token is required")
            return False
        
        # Create HTTP session
        self._session = aiohttp.ClientSession()
        
        # Validate token by getting bot info
        try:
            self._bot_info = await self._call_api("getMe")
            logger.info(f"Logged in as Telegram bot: {self._bot_info.get('first_name')}")
        except Exception as e:
            logger.error(f"Failed to connect to Telegram: {e}")
            await self.disconnect()
            return False
        
        # Set webhook if configured
        if self.webhook_url:
            try:
                await self._set_webhook()
                await self.start_webhook_server(port=self.webhook_port)
            except Exception as e:
                logger.error(f"Failed to set webhook: {e}")
                await self.disconnect()
                return False
        else:
            # Start long polling
            self._polling_task = asyncio.create_task(self._polling_loop())
        
        self._running = True
        return True
    
    async def disconnect(self) -> None:
        """Disconnect from Telegram."""
        self._running = False
        
        # Stop polling
        if self._polling_task:
            self._polling_task.cancel()
            try:
                await self._polling_task
            except asyncio.CancelledError:
                pass
            self._polling_task = None
        
        # Remove webhook if set
        if self._session and self.webhook_url:
            try:
                await self._call_api("setWebhook", {"url": ""})
            except Exception:
                pass
        
        if self._session:
            await self._session.close()
            self._session = None
        
        logger.info("Disconnected from Telegram")
    
    async def _call_api(
        self,
        method: str,
        params: dict[str, Any] = None,
        post_data: dict[str, Any] = None
    ) -> Any:
        """
        Call Telegram Bot API method.
        
        Args:
            method: API method name.
            params: Query parameters.
            post_data: POST data (for multipart uploads).
            
        Returns:
            API response data.
        """
        if not self._session:
            raise RuntimeError("Not connected to Telegram")
        
        url = f"{self.API_BASE}/bot{self.token}/{method}"
        
        async with self._session.request(
            "POST" if post_data else "GET",
            url,
            params=params,
            json=post_data if post_data else (params if method in ["sendMessage", "editMessageText", "deleteMessage"] else None)
        ) as resp:
            data = await resp.json()
            
            if not data.get("ok"):
                error_code = data.get("error_code", 0)
                description = data.get("description", "Unknown error")
                raise RuntimeError(f"Telegram API error {error_code}: {description}")
            
            return data.get("result")
    
    async def _set_webhook(self) -> None:
        """Set the webhook URL with Telegram."""
        webhook_info = await self._call_api("getWebhookInfo")
        current_url = webhook_info.get("url", "")
        
        if current_url == self.webhook_url:
            logger.info("Webhook already set to the expected URL")
            return
        
        params = {"url": self.webhook_url}
        if self.webhook_secret:
            params["secret_token"] = self.webhook_secret
        
        await self._call_api("setWebhook", params)
        logger.info(f"Webhook set to {self.webhook_url}")
    
    async def _polling_loop(self) -> None:
        """Long polling loop for receiving updates."""
        logger.info("Starting long polling loop")
        
        while self._running:
            try:
                updates = await self._call_api(
                    "getUpdates",
                    {"offset": self._offset, "timeout": self.poll_timeout}
                )
                
                for update in updates:
                    self._offset = max(self._offset, update.get("update_id", 0) + 1)
                    await self._handle_update(update)
                    
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Polling error: {e}")
                await asyncio.sleep(5)  # Back off on error
        
        logger.info("Long polling loop stopped")
    
    async def _handle_update(self, update: dict[str, Any]) -> None:
        """Handle an incoming update from Telegram."""
        # Check for callback query (button click)
        if "callback_query" in update:
            await self._handle_callback_query(update["callback_query"])
            return
        
        # Check for messages
        message = update.get("message")
        if message:
            bot_event = await self._parse_message(message)
            if bot_event:
                await self._dispatch_event(bot_event)
    
    async def _handle_callback_query(self, callback_query: dict[str, Any]) -> None:
        """Handle a callback query (button click)."""
        message = callback_query.get("message", {})
        
        msg = Message(
            id=str(message.get("message_id", "")),
            chat_id=str(message.get("chat", {}).get("id", "")),
            sender_id=str(callback_query.get("from", {}).get("id", "")),
            sender_name=f"{callback_query.get('from', {}).get('first_name', '')} {callback_query.get('from', {}).get('last_name', '')}".strip(),
            content=callback_query.get("data", ""),  # Callback data is in content
            message_type=MessageType.BUTTON,
            timestamp=callback_query.get("message", {}).get("date", ""),
            metadata=callback_query,
        )
        
        raw_data = {
            "callback_id": callback_query.get("id", ""),
            "chat_instance": callback_query.get("chat_instance", ""),
        }
        
        event = BotEvent.from_message(EventType.BUTTON_CLICK, msg, raw_data)
        
        # Answer the callback query to remove loading state
        try:
            await self._call_api(
                "answerCallbackQuery",
                {"callback_query_id": callback_query.get("id", "")}
            )
        except Exception:
            pass
        
        await self._dispatch_event(event)
    
    async def _parse_message(self, message: dict[str, Any]) -> Optional[BotEvent]:
        """Parse a Telegram message into a BotEvent."""
        # Determine message type
        message_type = MessageType.TEXT
        content = message.get("text", message.get("caption", ""))
        
        if message.get("photo"):
            message_type = MessageType.IMAGE
        elif message.get("video"):
            message_type = MessageType.VIDEO
        elif message.get("audio"):
            message_type = MessageType.AUDIO
        elif message.get("document"):
            message_type = MessageType.FILE
        elif message.get("new_chat_members"):
            message_type = MessageType.TEXT
            content = "new_member"
        elif message.get("left_chat_member"):
            message_type = MessageType.TEXT
            content = "left_member"
        
        # Handle /commands
        if message.get("entities"):
            for entity in message["entities"]:
                if entity.get("type") == "bot_command":
                    message_type = MessageType.TEXT
                    break
        
        from datetime import datetime
        msg = Message(
            id=str(message.get("message_id", "")),
            chat_id=str(message.get("chat", {}).get("id", "")),
            sender_id=str(message.get("from", {}).get("id", "")),
            sender_name=f"{message.get('from', {}).get('first_name', '')} {message.get('from', {}).get('last_name', '')}".strip(),
            content=content,
            message_type=message_type,
            timestamp=message.get("date", 0),
            metadata=message,
        )
        
        return BotEvent.from_message(EventType.MESSAGE, msg, message)
    
    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: dict[str, Any] = None
    ) -> str:
        """
        Send a message to a Telegram chat.
        
        Args:
            chat_id: Chat ID to send to.
            content: Message content.
            message_type: Type of message.
            metadata: Additional Telegram-specific options (reply_markup, parse_mode, etc.)
            
        Returns:
            Message ID of sent message.
        """
        params = {
            "chat_id": chat_id,
            "text": content,
        }
        
        if metadata:
            if "parse_mode" in metadata:
                params["parse_mode"] = metadata["parse_mode"]
            if "reply_markup" in metadata:
                params["reply_markup"] = metadata["reply_markup"]
            if "reply_to_message_id" in metadata:
                params["reply_to_message_id"] = metadata["reply_to_message_id"]
            if "disable_web_page_preview" in metadata:
                params["disable_web_page_preview"] = metadata["disable_web_page_preview"]
        
        return await self._call_api("sendMessage", params)
    
    async def send_image(
        self,
        chat_id: str,
        image_url: str,
        caption: Optional[str] = None
    ) -> str:
        """
        Send an image to a Telegram chat.
        
        Args:
            chat_id: Chat ID to send to.
            image_url: URL or file_id of the image.
            caption: Optional caption.
            
        Returns:
            Message ID of sent message.
        """
        params = {
            "chat_id": chat_id,
            "photo": image_url,
        }
        
        if caption:
            params["caption"] = caption
        
        return await self._call_api("sendPhoto", params)
    
    async def send_file(
        self,
        chat_id: str,
        file_url: str,
        filename: Optional[str] = None
    ) -> str:
        """
        Send a file to a Telegram chat.
        
        Args:
            chat_id: Chat ID to send to.
            file_url: URL or file_id of the file.
            filename: Optional filename (for captions).
            
        Returns:
            Message ID of sent message.
        """
        params = {
            "chat_id": chat_id,
            "document": file_url,
        }
        
        if filename:
            params["caption"] = filename
        
        return await self._call_api("sendDocument", params)
    
    async def edit_message(
        self,
        chat_id: str,
        message_id: str,
        new_content: str
    ) -> bool:
        """
        Edit a Telegram message.
        
        Args:
            chat_id: Chat ID containing the message.
            message_id: ID of the message to edit.
            new_content: New message content.
            
        Returns:
            True if successful.
        """
        params = {
            "chat_id": chat_id,
            "message_id": int(message_id),
            "text": new_content,
        }
        
        await self._call_api("editMessageText", params)
        return True
    
    async def delete_message(
        self,
        chat_id: str,
        message_id: str
    ) -> bool:
        """
        Delete a Telegram message.
        
        Args:
            chat_id: Chat ID containing the message.
            message_id: ID of the message to delete.
            
        Returns:
            True if successful.
        """
        params = {
            "chat_id": chat_id,
            "message_id": int(message_id),
        }
        
        await self._call_api("deleteMessage", params)
        return True
    
    async def send_buttons(
        self,
        chat_id: str,
        content: str,
        buttons: list[dict[str, str]]
    ) -> str:
        """
        Send inline keyboard to a Telegram chat.
        
        Args:
            chat_id: Chat ID to send to.
            content: Text content above the keyboard.
            buttons: List of button definitions with 'id' and 'label'.
            
        Returns:
            Message ID of sent message.
        """
        # Convert buttons to inline keyboard format
        keyboard = {
            "inline_keyboard": [
                [
                    {
                        "text": btn.get("label", "Button"),
                        "callback_data": btn.get("id", btn.get("callback_id", ""))
                    }
                    for btn in buttons
                ]
            ]
        }
        
        params = {
            "chat_id": chat_id,
            "text": content,
            "reply_markup": json.dumps(keyboard),
        }
        
        return await self._call_api("sendMessage", params)
    
    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """
        Parse Telegram webhook data into a BotEvent.
        
        Handles:
        - Messages
        - Edited messages
        - Callback queries (button clicks)
        - Channel posts
        """
        # Handle callback query
        if "callback_query" in data:
            await self._handle_callback_query(data["callback_query"])
            return None
        
        # Handle message
        message = data.get("message")
        if message:
            return await self._parse_message(message)
        
        # Handle edited message
        edited_message = data.get("edited_message")
        if edited_message:
            return await self._parse_message(edited_message)
        
        # Handle channel post
        channel_post = data.get("channel_post")
        if channel_post:
            return await self._parse_message(channel_post)
        
        return None
    
    def _verify_webhook_secret(self, secret: str, data: str) -> bool:
        """
        Verify the secret token from a webhook request.
        
        Args:
            secret: The secret token from the request header.
            data: The request body.
            
        Returns:
            True if the secret is valid.
        """
        if not self.webhook_secret:
            return True
        
        expected = hmac.new(
            self.webhook_secret.encode(),
            data.encode(),
            hashlib.sha256
        ).hexdigest()
        
        return hmac.compare_digest(secret, expected)
    
    async def send_sticker(
        self,
        chat_id: str,
        sticker_id: str
    ) -> str:
        """
        Send a sticker to a Telegram chat.
        
        Args:
            chat_id: Chat ID to send to.
            sticker_id: File ID of the sticker.
            
        Returns:
            Message ID of sent message.
        """
        params = {
            "chat_id": chat_id,
            "sticker": sticker_id,
        }
        
        return await self._call_api("sendSticker", params)
    
    async def send_location(
        self,
        chat_id: str,
        latitude: float,
        longitude: float
    ) -> str:
        """
        Send a location to a Telegram chat.
        
        Args:
            chat_id: Chat ID to send to.
            latitude: Latitude coordinate.
            longitude: Longitude coordinate.
            
        Returns:
            Message ID of sent message.
        """
        params = {
            "chat_id": chat_id,
            "latitude": latitude,
            "longitude": longitude,
        }
        
        return await self._call_api("sendLocation", params)
    
    async def get_chat(self, chat_id: str) -> dict[str, Any]:
        """
        Get information about a chat.
        
        Args:
            chat_id: Chat ID to fetch.
            
        Returns:
            Chat data.
        """
        return await self._call_api("getChat", {"chat_id": chat_id})
    
    async def get_chat_member(
        self,
        chat_id: str,
        user_id: str
    ) -> dict[str, Any]:
        """
        Get information about a chat member.
        
        Args:
            chat_id: Chat ID.
            user_id: User ID.
            
        Returns:
            Chat member data.
        """
        return await self._call_api(
            "getChatMember",
            {"chat_id": chat_id, "user_id": user_id}
        )


# Convenience function to create a Telegram connector
def create_telegram_connector(
    token: str,
    webhook_url: Optional[str] = None,
    webhook_secret: Optional[str] = None,
    webhook_port: int = 8080
) -> TelegramConnector:
    """
    Create a Telegram connector with the given configuration.
    
    Args:
        token: Telegram bot token.
        webhook_url: Optional webhook URL for receiving events.
        webhook_secret: Optional secret token for webhook verification.
        webhook_port: Port for webhook server.
        
    Returns:
        Configured TelegramConnector instance.
    """
    config = {
        "token": token,
        "webhook_url": webhook_url,
        "webhook_secret": webhook_secret,
        "webhook_port": webhook_port,
    }
    return TelegramConnector(config)
