"""
WhatsApp Business API Connector for LuminaGuard Messenger Framework

This module provides WhatsApp Business API implementation of the MessengerConnector.
Supports the WhatsApp Business Cloud API (Meta).
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


class WhatsAppConnector(MessengerConnector):
    """
    WhatsApp Business API connector implementation.

    Supports:
    - Sending text messages, images, documents, audio, video
    - Receiving messages via webhooks
    - Interactive buttons and lists
    - Message templates
    - Read receipts and typing indicators

    Configuration:
        - phone_number_id: WhatsApp Business Phone Number ID
        - access_token: Meta access token for the app
        - business_account_id: WhatsApp Business Account ID (optional)
        - webhook_verify_token: Token for webhook verification
        - webhook_port: Port for webhook server (default: 8080)
        - app_secret: Meta App Secret for webhook verification

    Usage:
        config = {
            "phone_number_id": "PHONE_NUMBER_ID",
            "access_token": "ACCESS_TOKEN",
            "webhook_verify_token": "your_verify_token",
            "app_secret": "APP_SECRET"
        }
        connector = WhatsAppConnector(config)
    """

    API_BASE = "https://graph.facebook.com/v18.0"

    def __init__(self, config: dict[str, Any]):
        super().__init__(config)
        self.phone_number_id = config.get("phone_number_id", "")
        self.access_token = config.get("access_token", "")
        self.business_account_id = config.get("business_account_id", "")
        self.webhook_verify_token = config.get("webhook_verify_token", "")
        self.webhook_port = config.get("webhook_port", 8080)
        self.app_secret = config.get("app_secret", "")
        self._session: Optional[aiohttp.ClientSession] = None
        self._webhook_server = None
        self._message_ids: dict[str, str] = {}  # Track message IDs for replies

    @property
    def platform_name(self) -> str:
        return "whatsapp"

    async def connect(self) -> bool:
        """
        Connect to WhatsApp Business API.
        
        Validates the access token and starts webhook server if configured.
        """
        if not self.phone_number_id or not self.access_token:
            logger.error("WhatsApp phone_number_id and access_token are required")
            return False

        # Create HTTP session
        self._session = aiohttp.ClientSession()

        # Validate credentials by fetching phone number info
        try:
            url = f"{self.API_BASE}/{self.phone_number_id}"
            headers = {"Authorization": f"Bearer {self.access_token}"}
            async with self._session.get(url, headers=headers) as resp:
                if resp.status != 200:
                    logger.error(f"Failed to validate WhatsApp credentials: {resp.status}")
                    await self.disconnect()
                    return False
                data = await resp.json()
                logger.info(f"Connected to WhatsApp Business: {data.get('verified_name', 'Unknown')}")
        except Exception as e:
            logger.error(f"Failed to connect to WhatsApp: {e}")
            await self.disconnect()
            return False

        # Start webhook server
        try:
            await self.start_webhook_server(port=self.webhook_port)
        except Exception as e:
            logger.warning(f"Failed to start webhook server: {e}")

        self._running = True
        return True

    async def disconnect(self) -> None:
        """Disconnect from WhatsApp."""
        self._running = False

        if self._webhook_server:
            self._webhook_server.close()
            self._webhook_server = None

        if self._session:
            await self._session.close()
            self._session = None

        logger.info("Disconnected from WhatsApp")

    async def _call_api(
        self,
        method: str,
        endpoint: str,
        data: Optional[dict[str, Any]] = None,
    ) -> Any:
        """
        Call WhatsApp Business API.

        Args:
            method: HTTP method (GET, POST, DELETE)
            endpoint: API endpoint path
            data: Request body data

        Returns:
            API response data
        """
        if not self._session:
            raise RuntimeError("Not connected to WhatsApp")

        url = f"{self.API_BASE}/{endpoint}"
        headers = {
            "Authorization": f"Bearer {self.access_token}",
            "Content-Type": "application/json",
        }

        if method == "GET":
            async with self._session.get(url, headers=headers) as resp:
                return await self._handle_response(resp)
        elif method == "POST":
            async with self._session.post(url, headers=headers, json=data) as resp:
                return await self._handle_response(resp)
        elif method == "DELETE":
            async with self._session.delete(url, headers=headers) as resp:
                return await self._handle_response(resp)
        else:
            raise ValueError(f"Unsupported HTTP method: {method}")

    async def _handle_response(self, resp: aiohttp.ClientResponse) -> dict[str, Any]:
        """Handle WhatsApp API response."""
        data = await resp.json()

        if resp.status not in (200, 201):
            error = data.get("error", {})
            raise RuntimeError(
                f"WhatsApp API error {resp.status}: {error.get('message', 'Unknown error')}"
            )

        return data

    async def _handle_webhook(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """
        Handle incoming webhook data from WhatsApp.
        
        Handles:
        - Text messages
        - Image, audio, video, document messages
        - Button replies
        - List selections
        - Message acknowledgments
        """
        entry = data.get("entry", [])
        if not entry:
            return None

        changes = entry[0].get("changes", [])
        if not changes:
            return None

        # Handle different webhook entry types
        for change in changes:
            # Standard messages
            if "messages" in change.get("value", {}):
                messages = change["value"]["messages"]
                for msg in messages:
                    return await self._parse_message(msg, change["value"])
            
            # Status updates (acknowledgments)
            if "statuses" in change.get("value", {}):
                statuses = change["value"]["statuses"]
                for status in statuses:
                    event = await self._handle_status(status)
                    if event:
                        return event

        return None

    async def _parse_message(
        self, 
        message: dict[str, Any], 
        value: dict[str, Any]
    ) -> Optional[BotEvent]:
        """Parse a WhatsApp message into a BotEvent."""
        from_id = message.get("from", "")
        msg_id = message.get("id", "")
        timestamp = message.get("timestamp", "")

        # Store message ID for reference
        self._message_ids[msg_id] = from_id

        # Determine message type and content
        message_type = MessageType.TEXT
        content = ""
        metadata = message

        if message.get("type") == "text":
            content = message.get("text", {}).get("body", "")
        elif message.get("type") == "image":
            message_type = MessageType.IMAGE
            image = message.get("image", {})
            content = image.get("caption", "[Image]")
            metadata = image
        elif message.get("type") == "audio":
            message_type = MessageType.AUDIO
            content = "[Audio]"
            metadata = message.get("audio", {})
        elif message.get("type") == "video":
            message_type = MessageType.VIDEO
            video = message.get("video", {})
            content = video.get("caption", "[Video]")
            metadata = video
        elif message.get("type") == "document":
            message_type = MessageType.FILE
            doc = message.get("document", {})
            content = doc.get("caption", doc.get("filename", "[Document]"))
            metadata = doc
        elif message.get("type") == "button":
            # Interactive button reply
            content = message.get("button", {}).get("text", "")
            message_type = MessageType.BUTTON
        elif message.get("type") == "interactive":
            # List or button block response
            interactive = message.get("interactive", {})
            if interactive.get("type") == "list_reply":
                content = interactive.get("list_reply", {}).get("title", "")
                message_type = MessageType.BUTTON
            elif interactive.get("type") == "button_reply":
                content = interactive.get("button_reply", {}).get("title", "")
                message_type = MessageType.BUTTON
        elif message.get("type") == "reaction":
            content = "[Reaction]"
        elif message.get("type") == "order":
            content = "[Order]"
        elif message.get("type") == "location":
            content = "[Location]"

        # Get sender info from metadata
        profile = value.get("metadata", {}).get("phone_number_id", "")

        msg = Message(
            id=msg_id,
            chat_id=from_id,
            sender_id=from_id,
            sender_name=profile,  # WhatsApp doesn't provide sender name by default
            content=content,
            message_type=message_type,
            timestamp=timestamp,
            metadata=metadata,
        )

        return BotEvent.from_message(EventType.MESSAGE, msg, message)

    async def _handle_status(self, status: dict[str, Any]) -> Optional[BotEvent]:
        """Handle message status updates (read, delivered, sent)."""
        status_type = status.get("status", "")
        
        # Map WhatsApp status to event type
        if status_type == "read":
            event_type = EventType.MESSAGE_READ
        elif status_type == "delivered":
            event_type = EventType.MESSAGE_DELIVERED
        elif status_type == "sent":
            event_type = EventType.MESSAGE_SENT
        else:
            return None

        msg = Message(
            id=status.get("id", ""),
            chat_id=status.get("recipient_id", ""),
            sender_id="",
            sender_name="",
            content=status_type,
            message_type=MessageType.TEXT,
            timestamp=status.get("timestamp", ""),
            metadata=status,
        )

        return BotEvent.from_message(event_type, msg, status)

    def _verify_webhook_signature(
        self, 
        signature: str, 
        body: str
    ) -> bool:
        """
        Verify the webhook signature from Meta.

        Args:
            signature: X-HubSpot-256 header (or X-Meta-Signature)
            body: Raw request body

        Returns:
            True if signature is valid
        """
        if not self.app_secret:
            return True

        expected = hmac.new(
            self.app_secret.encode(),
            body.encode(),
            hashlib.sha256
        ).hexdigest()

        return hmac.compare_digest(signature, expected)

    def _verify_webhook_token(self, token: str) -> bool:
        """
        Verify the webhook verify token.

        Args:
            token: Token from the verification request

        Returns:
            True if token matches
        """
        return hmac.compare_digest(token, self.webhook_verify_token)

    async def send_message(
        self,
        chat_id: str,
        content: str,
        message_type: MessageType = MessageType.TEXT,
        metadata: Optional[dict[str, Any]] = None,
    ) -> str:
        """
        Send a message to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number (with country code, no +)
            content: Message content
            message_type: Type of message
            metadata: Additional options (reply_to, preview_url, etc.)

        Returns:
            Message ID of sent message
        """
        payload: dict[str, Any] = {
            "messaging_product": "whatsapp",
            "to": chat_id,
        }

        if message_type == MessageType.TEXT:
            payload["text"] = {"body": content}
            if metadata and metadata.get("preview_url"):
                payload["text"]["preview_url"] = True
        else:
            # Default to text for unknown types
            payload["text"] = {"body": content}

        # Add reply to if specified
        if metadata and metadata.get("reply_to"):
            payload["context"] = {"message_id": metadata["reply_to"]}

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        msg_id = response.get("messages", [{}])[0].get("id", "")
        return msg_id

    async def send_image(
        self,
        chat_id: str,
        image_url: str,
        caption: Optional[str] = None,
    ) -> str:
        """
        Send an image to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            image_url: URL of the image
            caption: Optional caption

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "image",
            "image": {"link": image_url},
        }

        if caption:
            payload["image"]["caption"] = caption

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def send_file(
        self,
        chat_id: str,
        file_url: str,
        filename: Optional[str] = None,
    ) -> str:
        """
        Send a file to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            file_url: URL of the file
            filename: Optional filename for the file

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "document",
            "document": {"link": file_url},
        }

        if filename:
            payload["document"]["filename"] = filename

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def send_audio(
        self,
        chat_id: str,
        audio_url: str,
    ) -> str:
        """
        Send an audio file to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            audio_url: URL of the audio

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "audio",
            "audio": {"link": audio_url},
        }

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def send_video(
        self,
        chat_id: str,
        video_url: str,
        caption: Optional[str] = None,
    ) -> str:
        """
        Send a video to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            video_url: URL of the video
            caption: Optional caption

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "video",
            "video": {"link": video_url},
        }

        if caption:
            payload["video"]["caption"] = caption

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def send_buttons(
        self,
        chat_id: str,
        content: str,
        buttons: list[dict[str, str]],
    ) -> str:
        """
        Send interactive buttons to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            content: Text content above buttons
            buttons: List of button definitions with 'id' and 'label'

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "interactive",
            "interactive": {
                "type": "button",
                "body": {"text": content},
                "action": {
                    "buttons": [
                        {
                            "type": "reply",
                            "reply": {
                                "id": btn.get("id", btn.get("callback_id", "")),
                                "title": btn.get("label", "Button")[:20],  # Max 20 chars
                            }
                        }
                        for btn in buttons[:3]  # Max 3 buttons
                    ]
                },
            },
        }

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def send_list(
        self,
        chat_id: str,
        content: str,
        button_text: str,
        sections: list[dict[str, Any]],
    ) -> str:
        """
        Send a list message to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            content: Text content above the list
            button_text: Text on the button to open the list
            sections: List sections with items

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "interactive",
            "interactive": {
                "type": "list",
                "body": {"text": content},
                "action": {
                    "button": button_text[:20],  # Max 20 chars
                    "sections": sections,
                },
            },
        }

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def send_template(
        self,
        chat_id: str,
        template_name: str,
        language: str = "en_US",
        components: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        """
        Send a message template to a WhatsApp user.

        Args:
            chat_id: WhatsApp user phone number
            template_name: Name of the template
            language: Template language code (default: en_US)
            components: Template components (header, body, buttons)

        Returns:
            Message ID of sent message
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "type": "template",
            "template": {
                "name": template_name,
                "language": {"code": language},
            },
        }

        if components:
            payload["template"]["components"] = components

        response = await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return response.get("messages", [{}])[0].get("id", "")

    async def mark_read(self, message_id: str) -> bool:
        """
        Mark a message as read.

        Args:
            message_id: ID of the message to mark as read

        Returns:
            True if successful
        """
        payload = {
            "messaging_product": "whatsapp",
            "status": "read",
            "message_id": message_id,
        }

        await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return True

    async def send_typing(self, chat_id: str, typing: bool = True) -> bool:
        """
        Send typing indicator to a chat.

        Args:
            chat_id: WhatsApp user phone number
            typing: True for typing on, False for typing off

        Returns:
            True if successful
        """
        payload = {
            "messaging_product": "whatsapp",
            "to": chat_id,
            "typing": "typing_on" if typing else "typing_off",
        }

        await self._call_api(
            "POST",
            f"{self.phone_number_id}/messages",
            payload,
        )

        return True

    async def _parse_webhook_data(self, data: dict[str, Any]) -> Optional[BotEvent]:
        """Parse webhook data into a BotEvent."""
        return await self._handle_webhook(data)

    async def start_webhook_server(self, port: int = 8080) -> None:
        """Start a webhook server to receive WhatsApp events."""
        from aiohttp import web

        async def webhook_handler(request):
            # Verify webhook on initial setup
            mode = request.query.get("hub.mode")
            token = request.query.get("hub.verify_token")
            challenge = request.query.get("hub.challenge")

            if mode == "subscribe":
                if self._verify_webhook_token(token):
                    return web.Response(text=challenge)
                return web.Response(status=403, text="Invalid verify token")

            # Handle webhook events
            try:
                data = await request.json()
            except Exception:
                return web.Response(status=400, text="Invalid JSON")

            # Verify signature if app_secret is configured
            signature = request.headers.get("X-HubSpot-256") or request.headers.get(
                "X-Meta-Signature", ""
            )
            if signature and self.app_secret:
                body = await request.text()
                if not self._verify_webhook_signature(signature, body):
                    return web.Response(status=403, text="Invalid signature")

            # Process the webhook event
            event = await self._parse_webhook_data(data)
            if event:
                await self._dispatch_event(event)

            return web.Response(text="OK")

        self._webhook_server = web.Application()
        self._webhook_server.router.add_post("/webhook", webhook_handler)
        
        runner = web.AppRunner(self._webhook_server)
        await runner.setup()
        site = web.TCPSite(runner, "0.0.0.0", port)
        await site.start()
        
        logger.info(f"WhatsApp webhook server started on port {port}")


# Convenience function to create a WhatsApp connector
def create_whatsapp_connector(
    phone_number_id: str,
    access_token: str,
    webhook_verify_token: Optional[str] = None,
    app_secret: Optional[str] = None,
    webhook_port: int = 8080,
) -> WhatsAppConnector:
    """
    Create a WhatsApp connector with the given configuration.

    Args:
        phone_number_id: WhatsApp Business Phone Number ID
        access_token: Meta access token
        webhook_verify_token: Token for webhook verification
        app_secret: Meta App Secret for signature verification
        webhook_port: Port for webhook server

    Returns:
        Configured WhatsAppConnector instance
    """
    config = {
        "phone_number_id": phone_number_id,
        "access_token": access_token,
        "webhook_verify_token": webhook_verify_token,
        "app_secret": app_secret,
        "webhook_port": webhook_port,
    }
    return WhatsAppConnector(config)
