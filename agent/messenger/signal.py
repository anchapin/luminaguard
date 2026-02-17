"""
Signal Connector for LuminaGuard Messenger Framework

This module provides Signal messaging integration via the Signal REST API.
Supports bot mode with Signal via a Signal REST server.
"""

from typing import Any, Optional
import asyncio
import aiohttp
import logging
import json

from . import (
    BotEvent,
    EventType,
    Message,
    MessageType,
    MessengerConnector,
)

logger = logging.getLogger(__name__)


class SignalConnector(MessengerConnector):
    """
    Signal messenger connector implementation.

    Supports:
    - Sending messages, images, files, videos
    - Receiving messages via webhooks
    - Reactions and message reactions
    - Message editing (Signal doesn't support edit natively)
    - Group messaging

    Configuration:
        - signal_server_url: URL of Signal REST API server
        - api_key: API key for Signal server authentication
        - phone_number: Phone number registered with Signal (sender identity)
        - webhook_url: Webhook URL for receiving events
        - webhook_secret: Secret for webhook verification

    Setup:
        Requires a Signal REST API server running (e.g., signal-cli with REST mode):
        $ signal-cli --config ~/.local/share/signal-cli daemon --use-native-libsignal

    Usage:
        config = {
            "signal_server_url": "http://localhost:8080",
            "phone_number": "+1234567890",
            "api_key": "your-api-key",
        }
        connector = SignalConnector(config)
    """

    def __init__(self, config: dict[str, Any]):
        super().__init__(config)
        self.signal_server_url = config.get("signal_server_url", "http://localhost:8080")
        self.api_key = config.get("api_key", "")
        self.phone_number = config.get("phone_number", "")
        self.webhook_url = config.get("webhook_url")
        self.webhook_secret = config.get("webhook_secret", "")
        self._session: Optional[aiohttp.ClientSession] = None

    @property
    def platform_name(self) -> str:
        return "signal"

    async def connect(self) -> bool:
        """
        Connect to Signal REST API server.
        """
        if not self.phone_number or not self.signal_server_url:
            logger.error("Signal phone_number and signal_server_url are required")
            return False

        # Create HTTP session
        self._session = aiohttp.ClientSession()

        # Validate connection by getting account info
        try:
            async with self._session.get(
                f"{self.signal_server_url}/api/v1/profiles/{self.phone_number}",
                headers={"Authorization": f"Bearer {self.api_key}"},
            ) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    logger.info(f"Connected to Signal server for: {self.phone_number}")
                else:
                    logger.error(f"Failed to connect to Signal server: {resp.status}")
                    await self.disconnect()
                    return False
        except Exception as e:
            logger.error(f"Failed to connect to Signal: {e}")
            await self.disconnect()
            return False

        # Start webhook server if configured
        if self.webhook_url:
            await self.start_webhook_server(port=self.config.get("webhook_port", 8080))

        self._running = True
        return True

    async def disconnect(self) -> None:
        """Disconnect from Signal"""
        self._running = False

        if self._session:
            await self._session.close()

    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: Optional[dict[str, Any]] = None,
    ) -> str:
        """Send a message via Signal"""
        try:
            payload = {
                "recipients": [chat_id] if not self._is_group_id(chat_id) else None,
                "groupId": chat_id if self._is_group_id(chat_id) else None,
                "message": content,
            }

            # Remove None values
            payload = {k: v for k, v in payload.items() if v is not None}

            async with self._session.post(
                f"{self.signal_server_url}/api/v1/send",
                json=payload,
                headers={"Authorization": f"Bearer {self.api_key}"},
            ) as resp:
                if resp.status in (200, 201):
                    data = await resp.json()
                    return data.get("timestamp", "")
                else:
                    logger.error(f"Failed to send Signal message: {resp.status}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending Signal message: {e}")
            return ""

    async def send_image(
        self, chat_id: str, image_url: str, caption: Optional[str] = None
    ) -> str:
        """Send an image via Signal"""
        try:
            payload = {
                "recipients": [chat_id] if not self._is_group_id(chat_id) else None,
                "groupId": chat_id if self._is_group_id(chat_id) else None,
                "attachments": [{"uri": image_url}],
                "message": caption or "",
            }

            payload = {k: v for k, v in payload.items() if v is not None}

            async with self._session.post(
                f"{self.signal_server_url}/api/v1/send",
                json=payload,
                headers={"Authorization": f"Bearer {self.api_key}"},
            ) as resp:
                if resp.status in (200, 201):
                    data = await resp.json()
                    return data.get("timestamp", "")
                else:
                    logger.error(f"Failed to send Signal image: {resp.status}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending Signal image: {e}")
            return ""

    async def send_file(
        self, chat_id: str, file_url: str, filename: Optional[str] = None
    ) -> str:
        """Send a file via Signal"""
        try:
            payload = {
                "recipients": [chat_id] if not self._is_group_id(chat_id) else None,
                "groupId": chat_id if self._is_group_id(chat_id) else None,
                "attachments": [{"uri": file_url}],
                "message": f"File: {filename}" if filename else "File",
            }

            payload = {k: v for k, v in payload.items() if v is not None}

            async with self._session.post(
                f"{self.signal_server_url}/api/v1/send",
                json=payload,
                headers={"Authorization": f"Bearer {self.api_key}"},
            ) as resp:
                if resp.status in (200, 201):
                    data = await resp.json()
                    return data.get("timestamp", "")
                else:
                    logger.error(f"Failed to send Signal file: {resp.status}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending Signal file: {e}")
            return ""

    async def edit_message(
        self, chat_id: str, message_id: str, new_content: str
    ) -> bool:
        """Edit a message (Signal doesn't support native edit, resend instead)"""
        logger.warning("Signal doesn't support message editing, resending instead")
        result = await self.send_message(chat_id, new_content)
        return bool(result)

    async def delete_message(self, chat_id: str, message_id: str) -> bool:
        """Delete a message (Signal doesn't support deletion via API)"""
        logger.warning("Signal doesn't support message deletion via API")
        return False

    async def send_buttons(
        self, chat_id: str, content: str, buttons: list[dict[str, str]]
    ) -> str:
        """Send interactive buttons via Signal"""
        try:
            # Signal doesn't have native buttons, so we'll send buttons as text options
            message = content + "\n\nOptions:\n"
            for button in buttons:
                message += f"- {button.get('label', 'Button')}\n"

            return await self.send_message(chat_id, message)
        except Exception as e:
            logger.error(f"Error sending Signal buttons: {e}")
            return ""

    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """Parse webhook data into a BotEvent"""
        try:
            # Handle Signal envelope format
            if "envelope" in data:
                envelope = data["envelope"]
                source_number = envelope.get("source")
                timestamp = envelope.get("timestamp")

                # Handle text message
                if "dataMessage" in envelope:
                    data_msg = envelope["dataMessage"]
                    if "body" in data_msg:
                        message = Message(
                            id=str(timestamp),
                            chat_id=data_msg.get("groupInfo", {}).get("groupId") or source_number,
                            sender_id=source_number,
                            sender_name=source_number,
                            content=data_msg.get("body", ""),
                            message_type=MessageType.TEXT,
                            timestamp=None,
                            metadata={
                                "timestamp": timestamp,
                                "is_group": "groupInfo" in data_msg,
                            },
                        )

                        return BotEvent.from_message(EventType.MESSAGE, message, data)

        except Exception as e:
            logger.error(f"Error parsing Signal webhook: {e}")

        return None

    def _is_group_id(self, chat_id: str) -> bool:
        """Check if chat_id is a group ID"""
        # Group IDs in Signal are typically different format than phone numbers
        return not chat_id.startswith("+") and len(chat_id) > 15


class SignalGroupConnector(MessengerConnector):
    """
    Enhanced Signal connector with group management support.
    """

    def __init__(self, config: dict[str, Any]):
        self._base_connector = SignalConnector(config)
        super().__init__(config)

    @property
    def platform_name(self) -> str:
        return "signal"

    async def connect(self) -> bool:
        return await self._base_connector.connect()

    async def disconnect(self) -> None:
        await self._base_connector.disconnect()

    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: Optional[dict[str, Any]] = None,
    ) -> str:
        return await self._base_connector.send_message(chat_id, content, message_type, metadata)

    async def send_image(
        self, chat_id: str, image_url: str, caption: Optional[str] = None
    ) -> str:
        return await self._base_connector.send_image(chat_id, image_url, caption)

    async def send_file(
        self, chat_id: str, file_url: str, filename: Optional[str] = None
    ) -> str:
        return await self._base_connector.send_file(chat_id, file_url, filename)

    async def edit_message(
        self, chat_id: str, message_id: str, new_content: str
    ) -> bool:
        return await self._base_connector.edit_message(chat_id, message_id, new_content)

    async def delete_message(self, chat_id: str, message_id: str) -> bool:
        return await self._base_connector.delete_message(chat_id, message_id)

    async def send_buttons(
        self, chat_id: str, content: str, buttons: list[dict[str, str]]
    ) -> str:
        return await self._base_connector.send_buttons(chat_id, content, buttons)

    async def create_group(self, members: list[str], group_name: str) -> Optional[str]:
        """Create a Signal group"""
        try:
            payload = {
                "members": members,
                "name": group_name,
            }

            async with self._base_connector._session.post(
                f"{self._base_connector.signal_server_url}/api/v1/groups",
                json=payload,
                headers={"Authorization": f"Bearer {self._base_connector.api_key}"},
            ) as resp:
                if resp.status in (200, 201):
                    data = await resp.json()
                    return data.get("groupId")
                else:
                    logger.error(f"Failed to create Signal group: {resp.status}")
                    return None
        except Exception as e:
            logger.error(f"Error creating Signal group: {e}")
            return None

    async def add_group_members(self, group_id: str, members: list[str]) -> bool:
        """Add members to a Signal group"""
        try:
            payload = {"members": members}

            async with self._base_connector._session.patch(
                f"{self._base_connector.signal_server_url}/api/v1/groups/{group_id}",
                json=payload,
                headers={"Authorization": f"Bearer {self._base_connector.api_key}"},
            ) as resp:
                return resp.status in (200, 204)
        except Exception as e:
            logger.error(f"Error adding Signal group members: {e}")
            return False

    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        return await self._base_connector._parse_webhook_data(data)
