#!/usr/bin/env python3
"""
LuminaGuard Bot Factory
=======================

One-stop helper for creating a 24/7 LuminaGuard bot.

A first-time user only needs to call:

    from bot_factory import BotFactory
    bot = BotFactory.create()
    bot.chat("Hello")

Or from the command line:

    python create_bot.py

The factory handles every setup step automatically:
  1. Daemon configuration (with sensible defaults)
  2. Persona / onboarding profile (auto-generated or loaded from disk)
  3. LLM client (auto-detected from environment variables)
  4. Message router wired to the LLM

Environment variables for LLM providers (set at least one):
  OPENAI_API_KEY       – OpenAI / GPT models
  ANTHROPIC_API_KEY    – Anthropic / Claude models
  OLLAMA_HOST          – Local Ollama server (e.g. http://localhost:11434)
  LLM_API_KEY          – Generic fallback key

If none are set the bot still works but responds with a friendly setup
prompt instead of calling an LLM.
"""

from __future__ import annotations

import asyncio
import logging
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Optional

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Resolve the agent root so imports work whether this file is run directly
# or imported from a test.
# ---------------------------------------------------------------------------
_AGENT_ROOT = Path(__file__).parent
if str(_AGENT_ROOT) not in sys.path:
    sys.path.insert(0, str(_AGENT_ROOT))

# ---------------------------------------------------------------------------
# Auto-load .env file so users don't have to manually `source .env`.
# Searches for .env in:
#   1. The agent/ directory (agent/.env)
#   2. The project root (one level up from agent/)
# This means `cp .env.example .env` works from either location.
# ---------------------------------------------------------------------------
def _load_dotenv() -> None:
    """Load .env file if python-dotenv is available."""
    try:
        from dotenv import load_dotenv
    except ImportError:
        return  # python-dotenv not installed; rely on env vars being set manually

    # Check agent/ directory first, then project root
    candidates = [
        _AGENT_ROOT / ".env",
        _AGENT_ROOT.parent / ".env",
    ]
    for dotenv_path in candidates:
        if dotenv_path.is_file():
            load_dotenv(dotenv_path=dotenv_path, override=False)
            logger.debug("Loaded .env from %s", dotenv_path)
            break


_load_dotenv()

from daemon_config import DaemonConfig, ConfigManager
from daemon.persona import (
    OnboardingFlow,
    OnboardingProfile,
    PersonaConfig,
    PersonaManager,
)
from llm_client import (
    LLMConfig,
    LLMProvider,
    MockLLMClient,
    NO_LLM_CONFIGURED_MESSAGE,
    create_llm_client,
    get_bot_response,
    is_llm_configured,
)
from messenger import (
    BotEvent,
    EventType,
    Message,
    MessageRouter,
    MessageType,
    MessengerBot,
)


# ---------------------------------------------------------------------------
# BotConfig – thin wrapper that collects all user-facing options
# ---------------------------------------------------------------------------

@dataclass
class BotConfig:
    """
    High-level configuration for the 24/7 bot.

    All fields have sensible defaults so a first-time user can call
    ``BotFactory.create()`` with zero arguments.
    """

    # Bot identity
    bot_name: str = "LuminaBot"
    bot_description: str = "Your 24/7 LuminaGuard assistant"

    # User identity (used for onboarding profile)
    username: str = "user"
    use_case: str = "general assistance"

    # Where to persist persona / onboarding data
    config_dir: Optional[Path] = None

    # LLM provider override (auto-detected from env vars if None)
    llm_provider: Optional[LLMProvider] = None
    llm_api_key: Optional[str] = None
    llm_model: Optional[str] = None

    # Extra message handlers registered by the caller
    extra_handlers: list[Callable] = field(default_factory=list)

    def __post_init__(self):
        if self.config_dir is None:
            self.config_dir = Path.home() / ".luminaguard" / "bot"


# ---------------------------------------------------------------------------
# ReadyBot – the assembled bot returned to the caller
# ---------------------------------------------------------------------------

class ReadyBot:
    """
    A fully assembled 24/7 bot ready to receive messages.

    Usage::

        bot = BotFactory.create()

        # Synchronous one-shot
        reply = bot.chat("Hello")
        print(reply)

        # Async one-shot
        reply = await bot.achat("Hello")

        # Run an interactive REPL
        bot.run_repl()
    """

    def __init__(
        self,
        router: MessageRouter,
        daemon_config: DaemonConfig,
        persona: PersonaConfig,
        profile: OnboardingProfile,
        bot_config: BotConfig,
    ):
        self._router = router
        self.daemon_config = daemon_config
        self.persona = persona
        self.profile = profile
        self.bot_config = bot_config

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def chat(self, prompt: str) -> str:
        """
        Send a message to the bot and return its response (synchronous).

        Args:
            prompt: The user's message.

        Returns:
            The bot's response string.
        """
        return asyncio.run(self.achat(prompt))

    async def achat(self, prompt: str) -> str:
        """
        Send a message to the bot and return its response (async).

        Args:
            prompt: The user's message.

        Returns:
            The bot's response string.
        """
        from datetime import datetime, timezone

        msg = Message(
            id="msg-0",
            chat_id="cli",
            sender_id=self.profile.username,
            sender_name=self.profile.username,
            content=prompt,
            message_type=MessageType.TEXT,
            timestamp=datetime.now(timezone.utc),
            metadata={},
        )
        event = BotEvent.from_message(EventType.MESSAGE, msg)
        response = await self._router.route(event)
        return response or ""

    def run_repl(self) -> None:
        """
        Start an interactive read-eval-print loop in the terminal.

        Type 'quit', 'exit', or press Ctrl-D to stop.
        """
        print(f"\n{'='*60}")
        print(f"  {self.persona.name}")
        print(f"  {self.persona.description}")
        if not is_llm_configured():
            print()
            print(f"  ⚠️  {NO_LLM_CONFIGURED_MESSAGE}")
            print("  Set OPENAI_API_KEY (or another LLM env var) to enable AI.")
        print(f"{'='*60}")
        print("  Type 'quit' or press Ctrl-D to exit.\n")

        while True:
            try:
                user_input = input("You: ").strip()
            except (EOFError, KeyboardInterrupt):
                print("\nGoodbye!")
                break

            if not user_input:
                continue
            if user_input.lower() in ("quit", "exit", "bye"):
                print("Goodbye!")
                break

            reply = self.chat(user_input)
            print(f"Bot: {reply}\n")

    # ------------------------------------------------------------------
    # Diagnostics
    # ------------------------------------------------------------------

    def status(self) -> dict[str, Any]:
        """Return a status dictionary for diagnostics / health checks."""
        return {
            "bot_name": self.persona.name,
            "username": self.profile.username,
            "llm_configured": is_llm_configured(),
            "config_dir": str(self.bot_config.config_dir),
        }


# ---------------------------------------------------------------------------
# BotFactory – the single entry point
# ---------------------------------------------------------------------------

class BotFactory:
    """
    Factory that assembles a 24/7 LuminaGuard bot in one call.

    Typical usage::

        # Zero-config – everything auto-detected
        bot = BotFactory.create()
        print(bot.chat("Hello"))

        # Custom config
        from bot_factory import BotFactory, BotConfig
        cfg = BotConfig(bot_name="MyBot", username="alice")
        bot = BotFactory.create(cfg)
        bot.run_repl()
    """

    @classmethod
    def create(cls, config: Optional[BotConfig] = None) -> ReadyBot:
        """
        Create and return a fully assembled ReadyBot.

        Steps performed automatically:
          1. Load / create daemon configuration
          2. Load / create persona and onboarding profile
          3. Detect LLM provider from environment variables
          4. Wire message router with LLM-backed handler
          5. Return a ReadyBot ready to receive messages

        Args:
            config: Optional BotConfig. Uses all defaults if None.

        Returns:
            A ReadyBot instance.
        """
        cfg = config or BotConfig()

        logger.info("🤖 Creating 24/7 LuminaGuard bot…")

        # ---- Step 1: Daemon config ----------------------------------------
        daemon_config = cls._setup_daemon_config()
        logger.info("✅ Step 1/4 – Daemon config loaded")

        # ---- Step 2: Persona / onboarding -----------------------------------
        persona, profile = cls._setup_persona(cfg)
        logger.info("✅ Step 2/4 – Persona '%s' ready", persona.name)

        # ---- Step 3: LLM client ---------------------------------------------
        llm_config = cls._build_llm_config(cfg)
        logger.info(
            "✅ Step 3/4 – LLM client: %s (%s)",
            llm_config.provider.value,
            "configured" if is_llm_configured() else "⚠️  no env vars set",
        )

        # ---- Step 4: Message router -----------------------------------------
        router = cls._build_router(cfg, llm_config)
        logger.info("✅ Step 4/4 – Message router assembled")

        logger.info("🚀 Bot ready! Call bot.chat('Hello') to start.")

        return ReadyBot(
            router=router,
            daemon_config=daemon_config,
            persona=persona,
            profile=profile,
            bot_config=cfg,
        )

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _setup_daemon_config() -> DaemonConfig:
        """Load daemon configuration (uses defaults if no file found)."""
        manager = ConfigManager(config_path=None)
        return manager.config

    @staticmethod
    def _setup_persona(cfg: BotConfig) -> tuple[PersonaConfig, OnboardingProfile]:
        """Load or create persona and onboarding profile."""
        cfg.config_dir.mkdir(parents=True, exist_ok=True)
        manager = PersonaManager(config_dir=cfg.config_dir)

        # Try to load existing profile / persona
        persona = manager.load_persona()
        profile = manager.load_onboarding_profile()

        if persona is None:
            persona = PersonaConfig(
                name=cfg.bot_name,
                description=cfg.bot_description,
                behavior_traits={
                    "responsiveness": "immediate",
                    "verbosity": "concise",
                    "error_handling": "graceful",
                },
            )
            manager.save_persona(persona)

        if profile is None:
            profile = OnboardingProfile(
                username=cfg.username,
                use_case=cfg.use_case,
            )
            manager.save_onboarding_profile(profile)

        return persona, profile

    @staticmethod
    def _build_llm_config(cfg: BotConfig) -> LLMConfig:
        """
        Build an LLMConfig by auto-detecting the provider from env vars,
        or falling back to MOCK if nothing is configured.
        """
        # Explicit override from BotConfig
        if cfg.llm_provider is not None:
            return LLMConfig(
                provider=cfg.llm_provider,
                api_key=cfg.llm_api_key,
                model=cfg.llm_model or "mock-model",
            )

        # Auto-detect from environment
        if os.environ.get("OPENAI_API_KEY"):
            return LLMConfig(
                provider=LLMProvider.OPENAI,
                api_key=os.environ["OPENAI_API_KEY"],
                model=cfg.llm_model or "gpt-4o-mini",
            )
        if os.environ.get("ANTHROPIC_API_KEY"):
            return LLMConfig(
                provider=LLMProvider.ANTHROPIC,
                api_key=os.environ["ANTHROPIC_API_KEY"],
                model=cfg.llm_model or "claude-3-haiku-20240307",
            )
        if os.environ.get("OLLAMA_HOST"):
            return LLMConfig(
                provider=LLMProvider.OLLAMA,
                base_url=os.environ["OLLAMA_HOST"],
                model=cfg.llm_model or "llama3",
            )

        # Fallback – mock client (always works, returns setup message for real prompts)
        return LLMConfig(provider=LLMProvider.MOCK)

    @staticmethod
    def _build_router(cfg: BotConfig, llm_config: LLMConfig) -> MessageRouter:
        """Wire a MessageRouter with an LLM-backed default handler."""
        router = MessageRouter()

        # Register any extra handlers provided by the caller first
        for handler in cfg.extra_handlers:
            router.message()(handler)

        # Default handler: route through get_bot_response()
        @router.message()
        async def _llm_handler(event: BotEvent) -> str:
            prompt = event.message.content if event.message else ""
            return get_bot_response(prompt, config=llm_config)

        return router


# ---------------------------------------------------------------------------
# Convenience top-level function
# ---------------------------------------------------------------------------

def create_bot(
    bot_name: str = "LuminaBot",
    username: str = "user",
    use_case: str = "general assistance",
    config_dir: Optional[Path] = None,
    **kwargs: Any,
) -> ReadyBot:
    """
    Convenience wrapper around ``BotFactory.create()``.

    Accepts the most common options as keyword arguments so callers don't
    need to import ``BotConfig`` for simple use-cases.

    Example::

        from bot_factory import create_bot
        bot = create_bot(bot_name="MyBot", username="alice")
        print(bot.chat("Hello"))

    Args:
        bot_name:   Display name for the bot persona.
        username:   The user's name (stored in onboarding profile).
        use_case:   Brief description of what the bot will be used for.
        config_dir: Directory to persist persona / profile data.
        **kwargs:   Any additional BotConfig fields.

    Returns:
        A ReadyBot instance.
    """
    cfg = BotConfig(
        bot_name=bot_name,
        username=username,
        use_case=use_case,
        config_dir=config_dir,
        **kwargs,
    )
    return BotFactory.create(cfg)
