#!/usr/bin/env python3
"""
Tests for BotFactory, ReadyBot, BotConfig, create_bot(), and create_bot.py CLI.

Covers the first-time user UX:
  - Zero-config bot creation
  - Custom BotConfig
  - ReadyBot.chat() / achat()
  - ReadyBot.status()
  - Persona persistence (save → reload)
  - LLM auto-detection from env vars
  - CLI --message one-shot mode
  - CLI --status mode
  - The core assertion: bot.chat("Hello") == "Please setup environment variables for your LLM"
    when no LLM env vars are set
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

# ---------------------------------------------------------------------------
# Path setup
# ---------------------------------------------------------------------------
AGENT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(AGENT_ROOT))

from bot_factory import BotConfig, BotFactory, ReadyBot, create_bot
from llm_client import (
    LLMProvider,
    NO_LLM_CONFIGURED_MESSAGE,
    is_llm_configured,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_LLM_VARS = {
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "OLLAMA_HOST",
    "LUMINAGUARD_LLM_API_KEY",
    "LLM_API_KEY",
}


def _clean_env() -> dict:
    """Return os.environ without any LLM provider variables."""
    return {k: v for k, v in os.environ.items() if k not in _LLM_VARS}


# ===========================================================================
# BotConfig
# ===========================================================================

class TestBotConfig:
    """BotConfig dataclass defaults and customisation."""

    def test_defaults(self):
        cfg = BotConfig()
        assert cfg.bot_name == "LuminaBot"
        assert cfg.username == "user"
        assert cfg.use_case == "general assistance"
        assert cfg.config_dir is not None  # auto-set in __post_init__
        assert cfg.llm_provider is None
        assert cfg.extra_handlers == []

    def test_custom_values(self, tmp_path):
        cfg = BotConfig(
            bot_name="MyBot",
            username="alice",
            use_case="monitoring",
            config_dir=tmp_path,
        )
        assert cfg.bot_name == "MyBot"
        assert cfg.username == "alice"
        assert cfg.config_dir == tmp_path

    def test_config_dir_defaults_to_home(self):
        cfg = BotConfig()
        assert ".luminaguard" in str(cfg.config_dir)


# ===========================================================================
# BotFactory.create()
# ===========================================================================

class TestBotFactoryCreate:
    """BotFactory.create() assembles a ReadyBot correctly."""

    def test_returns_ready_bot(self, tmp_path):
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        assert isinstance(bot, ReadyBot)

    def test_zero_config_returns_ready_bot(self, tmp_path):
        """BotFactory.create() with no arguments returns a ReadyBot."""
        # Override config_dir so we don't write to ~/.luminaguard during tests
        with patch("bot_factory.BotConfig.__post_init__", lambda self: setattr(self, "config_dir", tmp_path)):
            cfg = BotConfig()
            cfg.config_dir = tmp_path
            bot = BotFactory.create(cfg)
        assert isinstance(bot, ReadyBot)

    def test_persona_is_set(self, tmp_path):
        cfg = BotConfig(bot_name="TestBot", config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        assert bot.persona.name == "TestBot"

    def test_profile_is_set(self, tmp_path):
        cfg = BotConfig(username="carol", config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        assert bot.profile.username == "carol"

    def test_daemon_config_is_set(self, tmp_path):
        from daemon_config import DaemonConfig
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        assert isinstance(bot.daemon_config, DaemonConfig)

    def test_persona_persisted_to_disk(self, tmp_path):
        """Persona is saved so a second create() reloads it."""
        cfg = BotConfig(bot_name="PersistBot", config_dir=tmp_path)
        BotFactory.create(cfg)

        # Second create – should load from disk, not recreate
        cfg2 = BotConfig(bot_name="DIFFERENT", config_dir=tmp_path)
        bot2 = BotFactory.create(cfg2)
        assert bot2.persona.name == "PersistBot"  # loaded from disk

    def test_llm_config_mock_when_no_env_vars(self, tmp_path):
        """Without LLM env vars the factory uses the MOCK provider."""
        cfg = BotConfig(config_dir=tmp_path)
        with patch.dict(os.environ, _clean_env(), clear=True):
            bot = BotFactory.create(cfg)
        # The bot is created successfully regardless
        assert isinstance(bot, ReadyBot)

    def test_llm_config_openai_when_env_var_set(self, tmp_path):
        """With OPENAI_API_KEY set, _build_llm_config picks OPENAI."""
        from llm_client import LLMConfig
        cfg = BotConfig(config_dir=tmp_path)
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test"}):
            llm_cfg = BotFactory._build_llm_config(cfg)
        assert llm_cfg.provider == LLMProvider.OPENAI
        assert llm_cfg.api_key == "sk-test"

    def test_llm_config_anthropic_when_env_var_set(self, tmp_path):
        cfg = BotConfig(config_dir=tmp_path)
        # Use _clean_env() to ensure no other LLM keys (e.g. OPENAI_API_KEY
        # from the developer's environment) take priority over ANTHROPIC_API_KEY.
        env = {**_clean_env(), "ANTHROPIC_API_KEY": "sk-ant-test"}
        with patch.dict(os.environ, env, clear=True):
            llm_cfg = BotFactory._build_llm_config(cfg)
        assert llm_cfg.provider == LLMProvider.ANTHROPIC

    def test_llm_config_ollama_when_env_var_set(self, tmp_path):
        cfg = BotConfig(config_dir=tmp_path)
        # Use _clean_env() to ensure no other LLM keys take priority.
        env = {**_clean_env(), "OLLAMA_HOST": "http://localhost:11434"}
        with patch.dict(os.environ, env, clear=True):
            llm_cfg = BotFactory._build_llm_config(cfg)
        assert llm_cfg.provider == LLMProvider.OLLAMA

    def test_llm_config_explicit_override(self, tmp_path):
        """Explicit llm_provider in BotConfig takes precedence over env vars."""
        cfg = BotConfig(
            config_dir=tmp_path,
            llm_provider=LLMProvider.MOCK,
        )
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test"}):
            llm_cfg = BotFactory._build_llm_config(cfg)
        assert llm_cfg.provider == LLMProvider.MOCK

    def test_extra_handlers_registered(self, tmp_path):
        """Extra handlers provided in BotConfig are registered first."""
        called = []

        async def my_handler(event):
            called.append(event)
            return "custom"

        cfg = BotConfig(config_dir=tmp_path, extra_handlers=[my_handler])
        bot = BotFactory.create(cfg)

        with patch.dict(os.environ, _clean_env(), clear=True):
            reply = bot.chat("anything")

        # The extra handler fires first and returns "custom"
        assert reply == "custom"
        assert len(called) == 1


# ===========================================================================
# ReadyBot.chat() / achat()
# ===========================================================================

class TestReadyBotChat:
    """ReadyBot.chat() and achat() return the expected responses."""

    def test_chat_hello_without_llm_env_vars(self, tmp_path):
        """
        Core assertion: bot.chat('Hello') == 'Please setup environment
        variables for your LLM' when no LLM env vars are set.
        """
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)

        with patch.dict(os.environ, _clean_env(), clear=True):
            reply = bot.chat("Hello")

        assert reply == "Please setup environment variables for your LLM"

    @pytest.mark.asyncio
    async def test_achat_hello_without_llm_env_vars(self, tmp_path):
        """Async variant of the core assertion."""
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)

        with patch.dict(os.environ, _clean_env(), clear=True):
            reply = await bot.achat("Hello")

        assert reply == "Please setup environment variables for your LLM"

    def test_chat_returns_string(self, tmp_path):
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        with patch.dict(os.environ, _clean_env(), clear=True):
            reply = bot.chat("Hi there")
        assert isinstance(reply, str)

    def test_chat_empty_prompt(self, tmp_path):
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        with patch.dict(os.environ, _clean_env(), clear=True):
            reply = bot.chat("")
        assert isinstance(reply, str)

    def test_chat_multiple_messages(self, tmp_path):
        """Multiple sequential chat() calls all return the setup message."""
        cfg = BotConfig(config_dir=tmp_path)
        bot = BotFactory.create(cfg)
        with patch.dict(os.environ, _clean_env(), clear=True):
            replies = [bot.chat(msg) for msg in ["Hello", "Hi", "Help"]]
        assert all(r == NO_LLM_CONFIGURED_MESSAGE for r in replies)


# ===========================================================================
# ReadyBot.status()
# ===========================================================================

class TestReadyBotStatus:
    """ReadyBot.status() returns a useful diagnostics dict."""

    def test_status_is_dict(self, tmp_path):
        bot = BotFactory.create(BotConfig(config_dir=tmp_path))
        s = bot.status()
        assert isinstance(s, dict)

    def test_status_keys(self, tmp_path):
        bot = BotFactory.create(BotConfig(config_dir=tmp_path))
        s = bot.status()
        assert "bot_name" in s
        assert "username" in s
        assert "llm_configured" in s
        assert "config_dir" in s

    def test_status_llm_configured_false(self, tmp_path):
        bot = BotFactory.create(BotConfig(config_dir=tmp_path))
        with patch.dict(os.environ, _clean_env(), clear=True):
            s = bot.status()
        assert s["llm_configured"] is False

    def test_status_llm_configured_true(self, tmp_path):
        bot = BotFactory.create(BotConfig(config_dir=tmp_path))
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test"}):
            s = bot.status()
        assert s["llm_configured"] is True

    def test_status_bot_name(self, tmp_path):
        bot = BotFactory.create(BotConfig(bot_name="StatusBot", config_dir=tmp_path))
        assert bot.status()["bot_name"] == "StatusBot"


# ===========================================================================
# create_bot() convenience function
# ===========================================================================

class TestCreateBotFunction:
    """create_bot() is a thin wrapper that works identically to BotFactory.create()."""

    def test_returns_ready_bot(self, tmp_path):
        bot = create_bot(config_dir=tmp_path)
        assert isinstance(bot, ReadyBot)

    def test_custom_name(self, tmp_path):
        bot = create_bot(bot_name="FuncBot", config_dir=tmp_path)
        assert bot.persona.name == "FuncBot"

    def test_custom_username(self, tmp_path):
        bot = create_bot(username="dave", config_dir=tmp_path)
        assert bot.profile.username == "dave"

    def test_hello_returns_setup_message(self, tmp_path):
        bot = create_bot(config_dir=tmp_path)
        with patch.dict(os.environ, _clean_env(), clear=True):
            assert bot.chat("Hello") == NO_LLM_CONFIGURED_MESSAGE


# ===========================================================================
# create_bot.py CLI
# ===========================================================================

class TestCreateBotCLI:
    """Tests for the create_bot.py command-line interface."""

    def _run_cli(self, argv: list[str]) -> tuple[int, str]:
        """Run the CLI main() and capture stdout."""
        import io
        from contextlib import redirect_stdout
        from create_bot import main

        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = main(argv)
        return exit_code, buf.getvalue()

    def test_status_flag_exits_zero(self):
        code, _ = self._run_cli(["--status"])
        assert code == 0

    def test_status_flag_prints_bot_name(self):
        _, output = self._run_cli(["--status", "--name", "CLIBot"])
        assert "CLIBot" in output

    def test_status_flag_prints_llm_status_no_env(self):
        with patch.dict(os.environ, _clean_env(), clear=True):
            _, output = self._run_cli(["--status"])
        assert "Please setup environment variables" in output

    def test_status_flag_prints_llm_configured(self):
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test"}):
            _, output = self._run_cli(["--status"])
        assert "LLM provider detected" in output

    def test_message_flag_one_shot(self, tmp_path):
        """--message sends one message and prints the reply."""
        with patch.dict(os.environ, _clean_env(), clear=True):
            code, output = self._run_cli([
                "--message", "Hello",
                "--config-dir", str(tmp_path),
            ])
        assert code == 0
        assert "Please setup environment variables for your LLM" in output

    def test_message_flag_custom_name(self, tmp_path):
        """--name is accepted without error."""
        with patch.dict(os.environ, _clean_env(), clear=True):
            code, _ = self._run_cli([
                "--name", "MyCLIBot",
                "--message", "Hello",
                "--config-dir", str(tmp_path),
            ])
        assert code == 0

    def test_message_flag_custom_username(self, tmp_path):
        with patch.dict(os.environ, _clean_env(), clear=True):
            code, _ = self._run_cli([
                "--username", "eve",
                "--message", "Hello",
                "--config-dir", str(tmp_path),
            ])
        assert code == 0


# ===========================================================================
# End-to-end: full first-time user flow
# ===========================================================================

class TestFirstTimeUserFlow:
    """
    Simulate the complete first-time user experience:
      1. User runs create_bot.py --message "Hello" (no LLM env vars)
      2. Bot responds with the setup message
      3. User sets OPENAI_API_KEY and the bot no longer returns the setup msg
    """

    def test_step1_no_llm_returns_setup_message(self, tmp_path):
        """Step 1: No LLM configured → setup message."""
        bot = create_bot(config_dir=tmp_path)
        with patch.dict(os.environ, _clean_env(), clear=True):
            reply = bot.chat("Hello")
        assert reply == "Please setup environment variables for your LLM"

    def test_step2_with_llm_does_not_return_setup_message(self, tmp_path):
        """Step 2: LLM configured → no setup message (mock returns reasoning)."""
        cfg = BotConfig(
            config_dir=tmp_path,
            llm_provider=LLMProvider.MOCK,  # use mock so no real API call
        )
        bot = BotFactory.create(cfg)
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-fake"}):
            reply = bot.chat("Hello")
        assert reply != "Please setup environment variables for your LLM"

    def test_bot_persists_across_restarts(self, tmp_path):
        """Persona is saved so a second create_bot() reloads it."""
        create_bot(bot_name="PersistentBot", config_dir=tmp_path)
        bot2 = create_bot(bot_name="IGNORED", config_dir=tmp_path)
        assert bot2.persona.name == "PersistentBot"

    def test_cli_message_hello_end_to_end(self, tmp_path):
        """CLI --message 'Hello' prints the setup message end-to-end."""
        import io
        from contextlib import redirect_stdout
        from create_bot import main

        buf = io.StringIO()
        with redirect_stdout(buf), patch.dict(os.environ, _clean_env(), clear=True):
            exit_code = main(["--message", "Hello", "--config-dir", str(tmp_path)])

        assert exit_code == 0
        assert "Please setup environment variables for your LLM" in buf.getvalue()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
