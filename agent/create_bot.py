#!/usr/bin/env python3
"""
LuminaGuard – Create a 24/7 Bot
================================

The fastest way to get a LuminaGuard bot running.

Usage
-----
  # Zero-config interactive REPL (auto-detects LLM from env vars)
  python create_bot.py

  # Named bot with a specific user
  python create_bot.py --name "MyBot" --username alice

  # One-shot: send a single message and print the reply
  python create_bot.py --message "Hello"

  # Check setup status
  python create_bot.py --status

Environment variables (set at least one to enable AI responses)
---------------------------------------------------------------
  OPENAI_API_KEY       OpenAI / GPT models
  ANTHROPIC_API_KEY    Anthropic / Claude models
  OLLAMA_HOST          Local Ollama server (e.g. http://localhost:11434)
  LLM_API_KEY          Generic fallback key

If none are set the bot still works and will tell you exactly what to do.
"""

from __future__ import annotations

import argparse
import logging
import os
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Ensure the agent root is on sys.path regardless of where this is invoked.
# ---------------------------------------------------------------------------
_HERE = Path(__file__).parent.resolve()
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))

from bot_factory import BotConfig, BotFactory, create_bot
from llm_client import NO_LLM_CONFIGURED_MESSAGE, is_llm_configured


def _print_banner(bot_name: str) -> None:
    width = 60
    print()
    print("=" * width)
    print(f"  🤖  LuminaGuard – {bot_name}")
    print("=" * width)


def _print_llm_status() -> None:
    """Print a human-friendly LLM configuration status."""
    if is_llm_configured():
        print("  ✅  LLM provider detected – AI responses enabled.")
    else:
        print(f"  ⚠️   {NO_LLM_CONFIGURED_MESSAGE}")
        print()
        print("  To enable AI responses, export one of:")
        print("    export OPENAI_API_KEY=sk-…")
        print("    export ANTHROPIC_API_KEY=sk-ant-…")
        print("    export OLLAMA_HOST=http://localhost:11434")
        print()
        print("  The bot will still respond – just without a real LLM.")


def _build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="create_bot",
        description="Create and run a LuminaGuard 24/7 bot.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--name", "-n",
        default="LuminaBot",
        metavar="NAME",
        help="Bot display name (default: LuminaBot)",
    )
    parser.add_argument(
        "--username", "-u",
        default="user",
        metavar="USER",
        help="Your username (default: user)",
    )
    parser.add_argument(
        "--use-case",
        default="general assistance",
        metavar="TEXT",
        help="Brief description of the bot's purpose",
    )
    parser.add_argument(
        "--config-dir",
        default=None,
        metavar="DIR",
        help="Directory to persist persona / profile data "
             "(default: ~/.luminaguard/bot)",
    )
    parser.add_argument(
        "--message", "-m",
        default=None,
        metavar="TEXT",
        help="Send a single message and print the reply, then exit",
    )
    parser.add_argument(
        "--status", "-s",
        action="store_true",
        help="Print setup status and exit",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Enable verbose logging",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _build_arg_parser()
    args = parser.parse_args(argv)

    # Configure logging
    log_level = logging.DEBUG if args.verbose else logging.WARNING
    logging.basicConfig(level=log_level, format="%(levelname)s %(message)s")

    # ------------------------------------------------------------------
    # --status: just print diagnostics and exit
    # ------------------------------------------------------------------
    if args.status:
        _print_banner(args.name)
        print()
        _print_llm_status()
        print()
        config_dir = Path(args.config_dir) if args.config_dir else Path.home() / ".luminaguard" / "bot"
        print(f"  Config dir : {config_dir}")
        print(f"  Username   : {args.username}")
        print(f"  Bot name   : {args.name}")
        print()
        return 0

    # ------------------------------------------------------------------
    # Build the bot
    # ------------------------------------------------------------------
    cfg = BotConfig(
        bot_name=args.name,
        username=args.username,
        use_case=args.use_case,
        config_dir=Path(args.config_dir) if args.config_dir else None,
    )

    bot = BotFactory.create(cfg)

    # ------------------------------------------------------------------
    # --message: one-shot mode
    # ------------------------------------------------------------------
    if args.message:
        reply = bot.chat(args.message)
        print(reply)
        return 0

    # ------------------------------------------------------------------
    # Interactive REPL
    # ------------------------------------------------------------------
    _print_banner(bot.persona.name)
    print()
    _print_llm_status()
    print()
    print(f"  Hello, {bot.profile.username}! I'm {bot.persona.name}.")
    print(f"  {bot.persona.description}")
    print()
    print("  Type 'quit' or press Ctrl-D to exit.")
    print("=" * 60)
    print()

    bot.run_repl()
    return 0


if __name__ == "__main__":
    sys.exit(main())
