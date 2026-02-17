"""
Slack Connector for LuminaGuard Messenger Framework

This module provides Slack-specific implementation of the MessengerConnector.
Supports bot mode with Slack API and event subscriptions.
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


class SlackConnector(MessengerConnector):
    """
    Slack bot connector implementation.

    Supports:
    - Sending messages, images, files
    - Receiving messages via webhooks or socket mode
    - Reactions and message threads
    - Message editing and deletion
    - Interactive components (buttons, select menus)

    Configuration:
        - bot_token: Slack bot token (xoxb-...)
        - webhook_url: Webhook URL for receiving events (optional)
        - app_token: Slack app token for socket mode (optional)
        - signing_secret: Secret for webhook verification

    Usage:
        config = {
            "bot_token": "xoxb-...",
            "signing_secret": "your-signing-secret",
        }
        connector = SlackConnector(config)
    """

    API_BASE = "https://slack.com/api"

    def __init__(self, config: dict[str, Any]):
        super().__init__(config)
        self.bot_token = config.get("bot_token", "")
        self.signing_secret = config.get("signing_secret", "")
        self.webhook_url = config.get("webhook_url")
        self.app_token = config.get("app_token")
        self._session: Optional[aiohttp.ClientSession] = None
        self._bot_user_id: Optional[str] = None
        self._socket_task: Optional[asyncio.Task] = None

    @property
    def platform_name(self) -> str:
        return "slack"

    async def connect(self) -> bool:
        """
        Connect to Slack using bot token.

        Supports:
        - Webhook-based event receiving
        - Socket Mode for real-time events
        """
        if not self.bot_token:
            logger.error("Slack bot token is required")
            return False

        # Create HTTP session
        self._session = aiohttp.ClientSession(
            headers={
                "Authorization": f"Bearer {self.bot_token}",
                "Content-Type": "application/json",
            }
        )

        # Validate token by getting bot info
        try:
            async with self._session.get(f"{self.API_BASE}/auth.test") as resp:
                if resp.status != 200:
                    logger.error(f"Invalid Slack token: {resp.status}")
                    await self.disconnect()
                    return False

                data = await resp.json()
                if not data.get("ok"):
                    logger.error(f"Auth failed: {data.get('error')}")
                    await self.disconnect()
                    return False

                self._bot_user_id = data.get("user_id")
                logger.info(f"Logged in as Slack bot: {data.get('user')}")
        except Exception as e:
            logger.error(f"Failed to connect to Slack: {e}")
            await self.disconnect()
            return False

        # Start webhook server if configured
        if self.webhook_url:
            await self.start_webhook_server(port=self.config.get("webhook_port", 8080))

        # Start socket mode if app token available
        if self.app_token:
            self._socket_task = asyncio.create_task(self._socket_mode_loop())

        self._running = True
        return True

    async def disconnect(self) -> None:
        """Disconnect from Slack"""
        self._running = False

        if self._socket_task:
            self._socket_task.cancel()
            try:
                await self._socket_task
            except asyncio.CancelledError:
                pass

        if self._session:
            await self._session.close()

    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: Optional[dict[str, Any]] = None,
    ) -> str:
        """Send a message to a Slack channel or user"""
        try:
            payload = {
                "channel": chat_id,
                "text": content,
            }

            # Add threading info if available
            if metadata and "thread_ts" in metadata:
                payload["thread_ts"] = metadata["thread_ts"]

            async with self._session.post(f"{self.API_BASE}/chat.postMessage", json=payload) as resp:
                data = await resp.json()
                if data.get("ok"):
                    return data.get("ts", "")
                else:
                    logger.error(f"Failed to send message: {data.get('error')}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending message: {e}")
            return ""

    async def send_image(
        self, chat_id: str, image_url: str, caption: Optional[str] = None
    ) -> str:
        """Send an image to a Slack channel"""
        try:
            blocks = [
                {
                    "type": "image",
                    "image_url": image_url,
                    "alt_text": caption or "Image",
                }
            ]

            if caption:
                blocks.append({
                    "type": "context",
                    "elements": [{
                        "type": "mrkdwn",
                        "text": caption,
                    }],
                })

            payload = {
                "channel": chat_id,
                "blocks": blocks,
            }

            async with self._session.post(f"{self.API_BASE}/chat.postMessage", json=payload) as resp:
                data = await resp.json()
                if data.get("ok"):
                    return data.get("ts", "")
                else:
                    logger.error(f"Failed to send image: {data.get('error')}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending image: {e}")
            return ""

    async def send_file(
        self, chat_id: str, file_url: str, filename: Optional[str] = None
    ) -> str:
        """Send a file to a Slack channel"""
        try:
            payload = {
                "channel": chat_id,
                "file": file_url,
                "title": filename or "File",
            }

            async with self._session.post(f"{self.API_BASE}/files.upload", json=payload) as resp:
                data = await resp.json()
                if data.get("ok"):
                    return data.get("file", {}).get("id", "")
                else:
                    logger.error(f"Failed to send file: {data.get('error')}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending file: {e}")
            return ""

    async def edit_message(
        self, chat_id: str, message_id: str, new_content: str
    ) -> bool:
        """Edit a previously sent message"""
        try:
            payload = {
                "channel": chat_id,
                "ts": message_id,
                "text": new_content,
            }

            async with self._session.post(f"{self.API_BASE}/chat.update", json=payload) as resp:
                data = await resp.json()
                return data.get("ok", False)
        except Exception as e:
            logger.error(f"Error editing message: {e}")
            return False

    async def delete_message(self, chat_id: str, message_id: str) -> bool:
        """Delete a message"""
        try:
            payload = {
                "channel": chat_id,
                "ts": message_id,
            }

            async with self._session.post(f"{self.API_BASE}/chat.delete", json=payload) as resp:
                data = await resp.json()
                return data.get("ok", False)
        except Exception as e:
            logger.error(f"Error deleting message: {e}")
            return False

    async def send_buttons(
        self, chat_id: str, content: str, buttons: list[dict[str, str]]
    ) -> str:
        """Send interactive buttons to a Slack channel"""
        try:
            elements = []
            for button in buttons:
                elements.append({
                    "type": "button",
                    "text": {
                        "type": "plain_text",
                        "text": button.get("label", "Button"),
                    },
                    "value": button.get("id", ""),
                    "action_id": f"button_{button.get('id', '')}",
                })

            payload = {
                "channel": chat_id,
                "blocks": [
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": content,
                        },
                    },
                    {
                        "type": "actions",
                        "elements": elements,
                    },
                ],
            }

            async with self._session.post(f"{self.API_BASE}/chat.postMessage", json=payload) as resp:
                data = await resp.json()
                if data.get("ok"):
                    return data.get("ts", "")
                else:
                    logger.error(f"Failed to send buttons: {data.get('error')}")
                    return ""
        except Exception as e:
            logger.error(f"Error sending buttons: {e}")
            return ""

    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """Parse webhook data into a BotEvent"""
        # Handle URL verification
        if data.get("type") == "url_verification":
            return None

        # Handle message events
        if data.get("type") == "event_callback":
            event_data = data.get("event", {})
            event_type = event_data.get("type")

            if event_type == "message" and "text" in event_data:
                # Ignore bot's own messages
                if event_data.get("user") == self._bot_user_id:
                    return None

                message = Message(
                    id=event_data.get("ts", ""),
                    chat_id=event_data.get("channel", ""),
                    sender_id=event_data.get("user", ""),
                    sender_name=event_data.get("username", "Unknown"),
                    content=event_data.get("text", ""),
                    message_type=MessageType.TEXT,
                    timestamp=None,
                    metadata={
                        "thread_ts": event_data.get("thread_ts"),
                        "subtype": event_data.get("subtype"),
                    },
                )

                return BotEvent.from_message(EventType.MESSAGE, message, event_data)

        return None

    async def _socket_mode_loop(self) -> None:
        """Handle Slack Socket Mode for real-time events"""
        try:
            import slack_sdk
            from slack_sdk.socket_mode import SocketModeClient
            from slack_sdk.socket_mode.request import SocketModeRequest
            from slack_sdk.socket_mode.response import SocketModeResponse

            client = SocketModeClient(
                app_token=self.app_token,
                trace_enabled=False,
            )

            def socket_mode_message_handler(
                client: SocketModeClient,
                req: SocketModeRequest,
            ) -> None:
                if req.type == "events_api":
                    # Handle event
                    asyncio.create_task(self._handle_socket_event(req.payload))
                    # Acknowledge receipt
                    response = SocketModeResponse(envelope_id=req.envelope_id)
                    client.send_socket_mode_response(response)

            client.socket_mode_request_listeners.append(socket_mode_message_handler)

            await client.connect_async()
            client.close()

        except Exception as e:
            logger.error(f"Socket Mode error: {e}")

    async def _handle_socket_event(self, payload: dict[str, Any]) -> None:
        """Handle a Socket Mode event"""
        event = payload.get("event", {})
        if event.get("type") == "message" and "text" in event:
            if event.get("user") != self._bot_user_id:
                message = Message(
                    id=event.get("ts", ""),
                    chat_id=event.get("channel", ""),
                    sender_id=event.get("user", ""),
                    sender_name=event.get("username", "Unknown"),
                    content=event.get("text", ""),
                    message_type=MessageType.TEXT,
                    timestamp=None,
                    metadata={"socket_mode": True},
                )
                bot_event = BotEvent.from_message(EventType.MESSAGE, message, event)
                await self._dispatch_event(bot_event)
