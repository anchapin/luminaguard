#!/usr/bin/env python3
"""
LuminaGuard Bot Server

Entry point for 24/7 bot operation with multiple messenger connectors.
Supports Discord, Telegram, and other messaging platforms.

Usage:
    # Run with config file
    python -m messenger.server --config config.json

    # Run with environment variables
    DISCORD_TOKEN=xxx TELEGRAM_TOKEN=yyy python -m messenger.server

    # Run with specific connectors
    python -m messenger.server --discord --telegram
"""

import argparse
import asyncio
import logging
import os
import signal
import sys
from pathlib import Path
from typing import Any, Optional

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from messenger import (
    BotEvent,
    MessengerBot,
    MessageRouter,
)
from messenger.discord import DiscordConnector
from messenger.telegram import TelegramConnector

# Configure logging
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


class LuminaGuardBotServer:
    """
    Main bot server class that manages all messenger connectors.

    Supports:
    - Multiple simultaneous connectors (Discord, Telegram)
    - Graceful shutdown
    - Health checks
    - Configuration from file or environment
    """

    def __init__(self, config: dict[str, Any]):
        self.config = config
        self.bot = MessengerBot()
        self._running = False
        self._shutdown_event = asyncio.Event()

    @classmethod
    def from_env(cls) -> "LuminaGuardBotServer":
        """Create bot server from environment variables."""
        config = {}

        # Discord
        if os.getenv("DISCORD_TOKEN"):
            config["discord"] = {
                "token": os.getenv("DISCORD_TOKEN"),
                "webhook_url": os.getenv("DISCORD_WEBHOOK_URL"),
                "webhook_port": int(os.getenv("DISCORD_WEBHOOK_PORT", "8080")),
            }

        # Telegram
        if os.getenv("TELEGRAM_TOKEN"):
            config["telegram"] = {
                "token": os.getenv("TELEGRAM_TOKEN"),
                "webhook_url": os.getenv("TELEGRAM_WEBHOOK_URL"),
                "webhook_secret": os.getenv("TELEGRAM_WEBHOOK_SECRET"),
                "webhook_port": int(os.getenv("TELEGRAM_WEBHOOK_PORT", "8081")),
            }

        # WhatsApp (future)
        if os.getenv("WHATSAPP_TOKEN"):
            config["whatsapp"] = {
                "token": os.getenv("WHATSAPP_TOKEN"),
                "phone_number_id": os.getenv("WHATSAPP_PHONE_ID"),
                "webhook_verify_token": os.getenv("WHATSAPP_VERIFY_TOKEN"),
            }

        return cls(config)

    @classmethod
    def from_file(cls, path: str) -> "LuminaGuardBotServer":
        """Create bot server from config file."""
        import json

        with open(path, "r") as f:
            config = json.load(f)

        return cls(config)

    def setup_handlers(self):
        """Set up message and command handlers."""

        @self.bot.on_message()
        async def handle_message(event: BotEvent) -> str:
            """Handle incoming messages."""
            if not event.message:
                return None

            content = event.message.content.lower()

            # Echo for testing
            return f"You said: {event.message.content}"

        @self.bot.command("help")
        async def cmd_help(event: BotEvent) -> str:
            """Help command."""
            return """Available commands:
/help - Show this help message
/status - Show bot status
/restart - Restart the bot
/info - Show bot information"""

        @self.bot.command("status")
        async def cmd_status(event: BotEvent) -> str:
            """Status command."""
            connector_count = len(self.bot.connectors)
            connected = sum(1 for c in self.bot.connectors if c.is_connected)

            return f"""LuminaGuard Bot Status:
- Active connectors: {connected}/{connector_count}
- Running: {self.bot.is_running}"""

        @self.bot.command("info")
        async def cmd_info(event: BotEvent) -> str:
            """Info command."""
            platforms = [c.platform_name for c in self.bot.connectors]
            return f"LuminaGuard Bot\nPlatforms: {', '.join(platforms)}"

        @self.bot.command("ping")
        async def cmd_ping(event: BotEvent) -> str:
            """Ping command."""
            return "pong"

    async def start(self):
        """Start the bot server."""
        logger.info("Starting LuminaGuard Bot Server...")

        # Set up handlers
        self.setup_handlers()

        # Add Discord connector if configured
        if "discord" in self.config:
            discord_config = self.config["discord"]
            if discord_config.get("token"):
                connector = DiscordConnector(discord_config)
                await self.bot.add_connector(connector)
                logger.info("Discord connector added")

        # Add Telegram connector if configured
        if "telegram" in self.config:
            telegram_config = self.config["telegram"]
            if telegram_config.get("token"):
                connector = TelegramConnector(telegram_config)
                await self.bot.add_connector(connector)
                logger.info("Telegram connector added")

        # Add WhatsApp connector if configured (future)
        if "whatsapp" in self.config:
            logger.warning("WhatsApp connector not yet implemented")

        if not self.bot.connectors:
            logger.error("No connectors configured!")
            return False

        # Start the bot
        try:
            await self.bot.start()
            self._running = True
            logger.info(
                f"LuminaGuard Bot started with {len(self.bot.connectors)} connectors"
            )

            # Wait for shutdown
            await self._shutdown_event.wait()

        except Exception as e:
            logger.error(f"Failed to start bot: {e}")
            return False

        return True

    async def stop(self):
        """Stop the bot server gracefully."""
        logger.info("Stopping LuminaGuard Bot Server...")
        self._running = False

        if self.bot.is_running:
            await self.bot.stop()

        self._shutdown_event.set()
        logger.info("LuminaGuard Bot Server stopped")

    def signal_handler(self, signum, frame):
        """Handle shutdown signals."""
        logger.info(f"Received signal {signum}, initiating shutdown...")
        asyncio.create_task(self.stop())


async def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="LuminaGuard Bot Server - 24/7 bot operation"
    )
    parser.add_argument(
        "--config",
        "-c",
        help="Path to configuration JSON file",
    )
    parser.add_argument(
        "--discord",
        action="store_true",
        help="Enable Discord connector (requires DISCORD_TOKEN env var)",
    )
    parser.add_argument(
        "--telegram",
        action="store_true",
        help="Enable Telegram connector (requires TELEGRAM_TOKEN env var)",
    )
    parser.add_argument(
        "--log-level",
        "-l",
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
        help="Set logging level",
    )
    parser.add_argument(
        "--port",
        "-p",
        type=int,
        default=8080,
        help="Default webhook server port",
    )

    args = parser.parse_args()

    # Set log level
    logging.getLogger().setLevel(getattr(logging, args.log_level))

    # Create bot server
    if args.config:
        server = LuminaGuardBotServer.from_file(args.config)
    else:
        server = LuminaGuardBotServer.from_env()

    # Set up signal handlers
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, lambda s=sig: asyncio.create_task(server.stop()))

    # Start server
    try:
        await server.start()
    except KeyboardInterrupt:
        await server.stop()
    except Exception as e:
        logger.error(f"Server error: {e}")
        await server.stop()
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
