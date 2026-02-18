#!/usr/bin/env python3
"""
Test: 24/7 Bot Setup Flow

This test walks through the complete setup steps for creating a LuminaGuard
24/7 bot and verifies that:
  1. Daemon configuration is created
  2. Persona / onboarding profile is initialised
  3. An LLM client is created
  4. The bot is assembled (MessengerBot + MessageRouter)
  5. Sending "Hello" returns "Please setup environment variables for your LLM"
     when no LLM provider environment variables are present.

The test is intentionally self-contained and does not require any external
services, API keys, or running infrastructure.
"""

from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path
from datetime import datetime, timezone
from unittest.mock import patch, MagicMock
import asyncio

import pytest

# ---------------------------------------------------------------------------
# Path setup – allow imports from the agent root
# ---------------------------------------------------------------------------
AGENT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(AGENT_ROOT))

# ---------------------------------------------------------------------------
# Imports from the LuminaGuard agent package
# ---------------------------------------------------------------------------
from daemon_config import DaemonConfig, ConfigManager, ConfigLoader
from daemon.persona import PersonaConfig, OnboardingProfile, PersonaManager, OnboardingFlow
from llm_client import (
    LLMConfig,
    LLMProvider,
    MockLLMClient,
    create_llm_client,
    is_llm_configured,
    get_bot_response,
    NO_LLM_CONFIGURED_MESSAGE,
)
from messenger import (
    BotEvent,
    EventType,
    Message,
    MessageType,
    MessengerBot,
    MessageRouter,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_hello_event() -> BotEvent:
    """Create a BotEvent that represents a user sending 'Hello'."""
    msg = Message(
        id="msg-001",
        chat_id="chat-001",
        sender_id="user-001",
        sender_name="TestUser",
        content="Hello",
        message_type=MessageType.TEXT,
        timestamp=datetime.now(timezone.utc),
        metadata={},
    )
    return BotEvent.from_message(EventType.MESSAGE, msg)


# ---------------------------------------------------------------------------
# Step 1 – Daemon configuration
# ---------------------------------------------------------------------------

class TestStep1DaemonConfig:
    """Step 1: Create and validate daemon configuration."""

    def test_default_daemon_config_is_created(self):
        """A DaemonConfig can be instantiated with sensible defaults."""
        config = DaemonConfig()
        assert config.name == "luminaguard"
        assert config.execution_mode in ("host", "vm")
        assert config.port > 0

    def test_config_loader_returns_daemon_config(self):
        """ConfigLoader.load() returns a DaemonConfig even without a file."""
        loader = ConfigLoader(config_path=None)
        config = loader.load()
        assert isinstance(config, DaemonConfig)

    def test_config_manager_wraps_loader(self):
        """ConfigManager provides a high-level interface over ConfigLoader."""
        manager = ConfigManager(config_path=None)
        assert isinstance(manager.config, DaemonConfig)

    def test_config_manager_get_port(self):
        """ConfigManager.get() can retrieve nested values."""
        manager = ConfigManager(config_path=None)
        port = manager.get("port")
        assert isinstance(port, int)
        assert 0 < port < 65536

    def test_config_export_is_dict(self):
        """ConfigManager.export() returns a plain dictionary."""
        manager = ConfigManager(config_path=None)
        exported = manager.export()
        assert isinstance(exported, dict)
        assert "name" in exported


# ---------------------------------------------------------------------------
# Step 2 – Persona / onboarding
# ---------------------------------------------------------------------------

class TestStep2PersonaSetup:
    """Step 2: Initialise bot persona and onboarding profile."""

    def test_persona_config_defaults(self):
        """PersonaConfig can be created with minimal arguments."""
        persona = PersonaConfig(
            name="LuminaBot",
            description="A helpful 24/7 assistant",
        )
        assert persona.name == "LuminaBot"
        assert persona.temperature == 0.7  # default

    def test_onboarding_profile_defaults(self):
        """OnboardingProfile can be created with just a username."""
        profile = OnboardingProfile(username="testuser")
        assert profile.username == "testuser"
        assert profile.expertise_level == "intermediate"

    def test_persona_manager_save_and_load(self, tmp_path):
        """PersonaManager persists and reloads a persona correctly."""
        manager = PersonaManager(config_dir=tmp_path)
        persona = PersonaConfig(
            name="TestBot",
            description="Bot for testing",
            behavior_traits={"verbosity": "concise"},
        )
        assert manager.save_persona(persona)

        loaded = manager.load_persona()
        assert loaded is not None
        assert loaded.name == "TestBot"
        assert loaded.behavior_traits["verbosity"] == "concise"

    def test_onboarding_profile_save_and_load(self, tmp_path):
        """PersonaManager persists and reloads an onboarding profile."""
        manager = PersonaManager(config_dir=tmp_path)
        profile = OnboardingProfile(
            username="alice",
            use_case="automation",
            expertise_level="expert",
        )
        assert manager.save_onboarding_profile(profile)

        loaded = manager.load_onboarding_profile()
        assert loaded is not None
        assert loaded.username == "alice"
        assert loaded.expertise_level == "expert"

    def test_onboarding_flow_primes_context(self, tmp_path):
        """OnboardingFlow.prime_context() returns a context dict."""
        manager = PersonaManager(config_dir=tmp_path)
        flow = OnboardingFlow(persona_manager=manager)

        profile = OnboardingProfile(username="bob", use_case="monitoring")
        persona = PersonaConfig(
            name="BobBot",
            description="Monitoring bot",
            behavior_traits={"responsiveness": "immediate"},
        )

        context = flow.prime_context(profile, persona)
        assert "user" in context
        assert context["user"]["username"] == "bob"
        assert "bot" in context
        assert context["bot"]["name"] == "BobBot"
        assert "system_prompt" in context


# ---------------------------------------------------------------------------
# Step 3 – LLM client creation
# ---------------------------------------------------------------------------

class TestStep3LLMClientCreation:
    """Step 3: Create an LLM client for the bot."""

    def test_create_mock_llm_client_by_default(self):
        """create_llm_client() returns a MockLLMClient when no config given."""
        client = create_llm_client()
        assert isinstance(client, MockLLMClient)

    def test_create_mock_llm_client_explicitly(self):
        """create_llm_client() with MOCK provider returns MockLLMClient."""
        config = LLMConfig(provider=LLMProvider.MOCK)
        client = create_llm_client(config)
        assert isinstance(client, MockLLMClient)

    def test_llm_config_defaults(self):
        """LLMConfig has sensible defaults for a 24/7 bot."""
        config = LLMConfig()
        assert config.provider == LLMProvider.MOCK
        assert config.temperature == 0.0  # deterministic
        assert config.max_tokens > 0

    def test_is_llm_configured_false_without_env_vars(self):
        """is_llm_configured() returns False when no LLM env vars are set."""
        env_vars_to_clear = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "OLLAMA_HOST",
            "LUMINAGUARD_LLM_API_KEY",
            "LLM_API_KEY",
        ]
        clean_env = {k: v for k, v in os.environ.items() if k not in env_vars_to_clear}
        with patch.dict(os.environ, clean_env, clear=True):
            assert is_llm_configured() is False

    def test_is_llm_configured_true_with_openai_key(self):
        """is_llm_configured() returns True when OPENAI_API_KEY is set."""
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test-key"}):
            assert is_llm_configured() is True

    def test_is_llm_configured_true_with_anthropic_key(self):
        """is_llm_configured() returns True when ANTHROPIC_API_KEY is set."""
        with patch.dict(os.environ, {"ANTHROPIC_API_KEY": "sk-ant-test"}):
            assert is_llm_configured() is True


# ---------------------------------------------------------------------------
# Step 4 – Bot assembly (MessengerBot + MessageRouter)
# ---------------------------------------------------------------------------

class TestStep4BotAssembly:
    """Step 4: Assemble the 24/7 bot using MessengerBot and MessageRouter."""

    def test_messenger_bot_can_be_instantiated(self):
        """MessengerBot can be created without any connectors."""
        bot = MessengerBot()
        assert not bot.is_running
        assert bot.connectors == []

    def test_message_router_can_be_instantiated(self):
        """MessageRouter can be created and used standalone."""
        router = MessageRouter()
        assert router is not None

    def test_message_router_registers_command_handler(self):
        """MessageRouter registers command handlers via decorator."""
        router = MessageRouter()

        @router.command("hello")
        async def handle_hello(event: BotEvent) -> str:
            return "Hi there!"

        assert "hello" in router._command_handlers

    def test_message_router_registers_message_handler(self):
        """MessageRouter registers default message handlers via decorator."""
        router = MessageRouter()

        @router.message()
        async def handle_message(event: BotEvent) -> str:
            return "Got your message"

        assert len(router._message_handlers) == 1

    @pytest.mark.asyncio
    async def test_message_router_routes_command(self):
        """MessageRouter routes /hello command to the correct handler."""
        router = MessageRouter()

        @router.command("hello")
        async def handle_hello(event: BotEvent) -> str:
            return "Hi there!"

        msg = Message(
            id="1",
            chat_id="c1",
            sender_id="u1",
            sender_name="User",
            content="/hello",
            message_type=MessageType.TEXT,
            timestamp=datetime.now(timezone.utc),
            metadata={},
        )
        event = BotEvent.from_message(EventType.COMMAND, msg)
        response = await router.route(event)
        assert response == "Hi there!"

    @pytest.mark.asyncio
    async def test_message_router_routes_plain_message(self):
        """MessageRouter routes a plain message to the default handler."""
        router = MessageRouter()

        @router.message()
        async def handle_message(event: BotEvent) -> str:
            return f"Echo: {event.message.content}"

        event = _make_hello_event()
        response = await router.route(event)
        assert response == "Echo: Hello"


# ---------------------------------------------------------------------------
# Step 5 – The core test: send "Hello", expect the setup message
# ---------------------------------------------------------------------------

class TestStep5BotRespondsToHello:
    """
    Step 5: The 24/7 bot is created and prompted with 'Hello'.

    When no LLM environment variables are configured the bot must respond
    with the canonical setup message:
        'Please setup environment variables for your LLM'
    """

    def _clear_llm_env(self) -> dict:
        """Return a copy of os.environ with all LLM env vars removed."""
        llm_vars = {
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "OLLAMA_HOST",
            "LUMINAGUARD_LLM_API_KEY",
            "LLM_API_KEY",
        }
        return {k: v for k, v in os.environ.items() if k not in llm_vars}

    # ------------------------------------------------------------------
    # 5a. get_bot_response() – the low-level helper
    # ------------------------------------------------------------------

    def test_get_bot_response_returns_setup_message_without_env_vars(self):
        """
        get_bot_response('Hello') returns the LLM setup message when no
        LLM provider environment variables are configured.
        """
        with patch.dict(os.environ, self._clear_llm_env(), clear=True):
            response = get_bot_response("Hello")
        assert response == NO_LLM_CONFIGURED_MESSAGE
        assert response == "Please setup environment variables for your LLM"

    def test_no_llm_configured_message_constant(self):
        """The constant matches the expected human-readable string."""
        assert NO_LLM_CONFIGURED_MESSAGE == "Please setup environment variables for your LLM"

    # ------------------------------------------------------------------
    # 5b. Full setup flow → bot → "Hello" → setup message
    # ------------------------------------------------------------------

    def test_full_setup_flow_hello_returns_setup_message(self, tmp_path):
        """
        Walk through every setup step and verify the bot's response to
        'Hello' is 'Please setup environment variables for your LLM'.

        Setup steps:
          1. Create daemon config
          2. Initialise persona and onboarding profile
          3. Create LLM client
          4. Assemble the bot (MessageRouter with LLM-backed handler)
          5. Send 'Hello' and assert the response
        """
        # ---- Step 1: Daemon config ----------------------------------------
        config_manager = ConfigManager(config_path=None)
        daemon_config = config_manager.config
        assert isinstance(daemon_config, DaemonConfig)

        # ---- Step 2: Persona / onboarding -----------------------------------
        persona_manager = PersonaManager(config_dir=tmp_path)
        persona = PersonaConfig(
            name="LuminaBot",
            description="24/7 assistant bot",
            behavior_traits={"responsiveness": "immediate"},
        )
        profile = OnboardingProfile(
            username="testuser",
            use_case="general assistance",
        )
        persona_manager.save_persona(persona)
        persona_manager.save_onboarding_profile(profile)

        loaded_persona = persona_manager.load_persona()
        loaded_profile = persona_manager.load_onboarding_profile()
        assert loaded_persona is not None
        assert loaded_profile is not None

        # ---- Step 3: LLM client ---------------------------------------------
        llm_config = LLMConfig(provider=LLMProvider.MOCK)
        llm_client = create_llm_client(llm_config)
        assert isinstance(llm_client, MockLLMClient)

        # ---- Step 4: Bot assembly -------------------------------------------
        router = MessageRouter()

        @router.message()
        async def handle_message(event: BotEvent) -> str:
            """Route all messages through the LLM-aware bot response."""
            prompt = event.message.content if event.message else ""
            return get_bot_response(prompt)

        # ---- Step 5: Send 'Hello' and assert response -----------------------
        async def _run():
            return await router.route(_make_hello_event())

        with patch.dict(os.environ, self._clear_llm_env(), clear=True):
            response = asyncio.run(_run())

        assert response == "Please setup environment variables for your LLM"

    @pytest.mark.asyncio
    async def test_full_setup_flow_hello_async(self, tmp_path):
        """
        Async variant of the full setup flow test.

        Walks through every setup step and verifies the bot's response to
        'Hello' is 'Please setup environment variables for your LLM'.
        """
        # ---- Step 1: Daemon config ----------------------------------------
        config_manager = ConfigManager(config_path=None)
        assert isinstance(config_manager.config, DaemonConfig)

        # ---- Step 2: Persona / onboarding -----------------------------------
        persona_manager = PersonaManager(config_dir=tmp_path)
        persona = PersonaConfig(
            name="LuminaBot",
            description="24/7 assistant bot",
        )
        profile = OnboardingProfile(username="asyncuser")
        persona_manager.save_persona(persona)
        persona_manager.save_onboarding_profile(profile)

        # ---- Step 3: LLM client ---------------------------------------------
        llm_client = create_llm_client(LLMConfig(provider=LLMProvider.MOCK))
        assert isinstance(llm_client, MockLLMClient)

        # ---- Step 4: Bot assembly -------------------------------------------
        router = MessageRouter()

        @router.message()
        async def handle_message(event: BotEvent) -> str:
            prompt = event.message.content if event.message else ""
            return get_bot_response(prompt)

        # ---- Step 5: Send 'Hello' and assert response -----------------------
        clean_env = {
            k: v for k, v in os.environ.items()
            if k not in {
                "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OLLAMA_HOST",
                "LUMINAGUARD_LLM_API_KEY", "LLM_API_KEY",
            }
        }
        with patch.dict(os.environ, clean_env, clear=True):
            event = _make_hello_event()
            response = await router.route(event)

        assert response == "Please setup environment variables for your LLM"

    def test_bot_response_changes_when_llm_is_configured(self):
        """
        When an LLM env var IS set, get_bot_response() does NOT return the
        setup message (it falls through to the actual LLM client).
        """
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-fake-key-for-test"}):
            # is_llm_configured() should now return True
            assert is_llm_configured() is True
            # The response will come from the MockLLMClient (since we pass
            # provider=MOCK explicitly), not the setup message.
            response = get_bot_response(
                "Hello",
                config=LLMConfig(provider=LLMProvider.MOCK),
            )
        # With a mock client and "Hello" (no tool keywords), the mock returns
        # its default reasoning – NOT the setup message.
        assert response != NO_LLM_CONFIGURED_MESSAGE


# ---------------------------------------------------------------------------
# Step 6 – Regression: the message is stable across multiple calls
# ---------------------------------------------------------------------------

class TestStep6Regression:
    """Regression tests to ensure the setup message is stable."""

    def test_setup_message_is_idempotent(self):
        """Calling get_bot_response('Hello') multiple times gives same result."""
        clean_env = {
            k: v for k, v in os.environ.items()
            if k not in {
                "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OLLAMA_HOST",
                "LUMINAGUARD_LLM_API_KEY", "LLM_API_KEY",
            }
        }
        with patch.dict(os.environ, clean_env, clear=True):
            responses = [get_bot_response("Hello") for _ in range(5)]

        assert all(r == "Please setup environment variables for your LLM" for r in responses)

    def test_setup_message_for_various_prompts(self):
        """Without LLM env vars, any prompt returns the setup message."""
        prompts = ["Hello", "Hi", "What can you do?", "Help me", ""]
        clean_env = {
            k: v for k, v in os.environ.items()
            if k not in {
                "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OLLAMA_HOST",
                "LUMINAGUARD_LLM_API_KEY", "LLM_API_KEY",
            }
        }
        with patch.dict(os.environ, clean_env, clear=True):
            for prompt in prompts:
                response = get_bot_response(prompt)
                assert response == "Please setup environment variables for your LLM", (
                    f"Unexpected response for prompt {prompt!r}: {response!r}"
                )


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
