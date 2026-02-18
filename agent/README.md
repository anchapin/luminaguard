# LuminaGuard Agent

The Python reasoning layer for LuminaGuard. Handles agent decision-making, LLM integration, messenger connectors, and the 24/7 bot runtime.

---

## 🚀 Quickstart — Create a 24/7 Bot

The fastest path to a running bot. No Firecracker, no KVM, no Rust build required.

### 1. Install dependencies

```bash
cd agent
python3 -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"
```

### 2. Run the bot

```bash
# Check your setup status
python create_bot.py --status

# Send a one-shot message
python create_bot.py --message "Hello"
# → Please setup environment variables for your LLM

# Start an interactive REPL
python create_bot.py
```

### 3. Enable AI responses

Set at least one LLM provider environment variable:

```bash
export OPENAI_API_KEY=sk-…          # OpenAI / GPT
export ANTHROPIC_API_KEY=sk-ant-…   # Anthropic / Claude
export OLLAMA_HOST=http://localhost:11434  # Local Ollama (free)
```

Then run again — the bot auto-detects your provider.

### 4. Use from Python

```python
from bot_factory import create_bot

# Zero-config
bot = create_bot()
print(bot.chat("Hello"))

# Custom bot
bot = create_bot(
    bot_name="MyBot",
    username="alice",
    use_case="monitoring",
)
bot.run_repl()          # interactive REPL
bot.status()            # diagnostics dict
```

---

## 📦 Key Modules

| File | Purpose |
|------|---------|
| [`bot_factory.py`](bot_factory.py) | **Start here.** `BotFactory`, `ReadyBot`, `BotConfig`, `create_bot()` |
| [`create_bot.py`](create_bot.py) | CLI script — `python create_bot.py --help` |
| [`llm_client.py`](llm_client.py) | LLM client factory, `get_bot_response()`, `is_llm_configured()` |
| [`loop.py`](loop.py) | Core agent reasoning loop (`run_loop`, `think`, `AgentState`) |
| [`daemon_config.py`](daemon_config.py) | Daemon configuration (`DaemonConfig`, `ConfigManager`) |
| [`daemon/persona.py`](daemon/persona.py) | Persona & onboarding (`PersonaManager`, `OnboardingFlow`) |
| [`messenger/`](messenger/) | Messenger connectors (Discord, Telegram, …) |
| [`mcp_client.py`](mcp_client.py) | MCP client for Rust orchestrator communication |

---

## 🤖 BotFactory API

### `create_bot()` — convenience function

```python
from bot_factory import create_bot

bot = create_bot(
    bot_name="LuminaBot",       # display name
    username="user",            # your username
    use_case="general",         # brief description
    config_dir=None,            # defaults to ~/.luminaguard/bot/
)
```

### `BotFactory.create(config)` — full control

```python
from bot_factory import BotFactory, BotConfig
from llm_client import LLMProvider

cfg = BotConfig(
    bot_name="MyBot",
    username="alice",
    llm_provider=LLMProvider.OPENAI,   # explicit override
    llm_api_key="sk-…",
    llm_model="gpt-4o",
    extra_handlers=[my_async_handler],  # custom handlers (run first)
)
bot = BotFactory.create(cfg)
```

### `ReadyBot` methods

```python
bot.chat("Hello")           # sync → str
await bot.achat("Hello")    # async → str
bot.run_repl()              # interactive terminal REPL
bot.status()                # → {"bot_name", "username", "llm_configured", "config_dir"}
```

### What `BotFactory.create()` does automatically

1. **Daemon config** — loads `DaemonConfig` with sensible defaults
2. **Persona** — loads from `~/.luminaguard/bot/` or creates a new one
3. **Onboarding profile** — loads or creates for the given username
4. **LLM client** — auto-detects from env vars (`OPENAI_API_KEY` → OpenAI, `ANTHROPIC_API_KEY` → Anthropic, `OLLAMA_HOST` → Ollama), falls back to mock
5. **Message router** — wires `MessageRouter` with an LLM-backed default handler

---

## 🔧 CLI Reference (`create_bot.py`)

```
python create_bot.py [OPTIONS]

Options:
  --name, -n NAME        Bot display name (default: LuminaBot)
  --username, -u USER    Your username (default: user)
  --use-case TEXT        Bot's purpose description
  --config-dir DIR       Persist persona/profile here (default: ~/.luminaguard/bot)
  --message, -m TEXT     One-shot: send TEXT and print reply, then exit
  --status, -s           Print LLM setup status and exit
  --verbose, -v          Enable verbose logging
```

**Examples:**

```bash
# Check what LLM is configured
python create_bot.py --status

# One-shot message (great for scripting / CI)
python create_bot.py --message "Hello"

# Named bot for a specific user
python create_bot.py --name "OpsBot" --username "ops-team" --use-case "infrastructure monitoring"

# Custom config directory
python create_bot.py --config-dir /etc/mybot --message "Hello"
```

---

## 🌐 Messenger Connectors

Connect the bot to Discord, Telegram, or other platforms via `messenger/server.py`:

```bash
# Discord
DISCORD_TOKEN=xxx python -m messenger.server --discord

# Telegram
TELEGRAM_TOKEN=yyy python -m messenger.server --telegram

# Both
DISCORD_TOKEN=xxx TELEGRAM_TOKEN=yyy python -m messenger.server
```

Or from a config file:

```bash
python -m messenger.server --config messenger.example.json
```

See [`messenger.example.json`](messenger.example.json) for the config format.

---

## 🧪 Running Tests

```bash
# All agent tests
python -m pytest tests/ -v

# Just the bot factory / setup tests
python -m pytest tests/test_bot_factory.py tests/test_247_bot_setup.py -v

# With coverage
python -m pytest tests/ --cov=. --cov-report=term-missing
```

---

## 📁 Directory Structure

```
agent/
├── bot_factory.py          ← Start here for 24/7 bot creation
├── create_bot.py           ← CLI entry point
├── llm_client.py           ← LLM client factory + get_bot_response()
├── loop.py                 ← Agent reasoning loop
├── daemon_config.py        ← Daemon configuration
├── mcp_client.py           ← MCP client
├── daemon/
│   ├── persona.py          ← Persona & onboarding
│   ├── config.py           ← Daemon config loader
│   └── ...
├── messenger/
│   ├── __init__.py         ← MessengerBot, MessageRouter, BotEvent
│   ├── server.py           ← 24/7 bot server (Discord, Telegram)
│   ├── discord.py          ← Discord connector
│   └── telegram.py         ← Telegram connector
├── examples/
│   └── mcp_filesystem_demo.py
└── tests/
    ├── test_247_bot_setup.py   ← Setup flow tests (29 tests)
    ├── test_bot_factory.py     ← BotFactory / ReadyBot tests (40 tests)
    └── ...
```

---

## 🔗 See Also

- [QUICKSTART.md](../QUICKSTART.md) — Get running in 5 minutes
- [INSTALL.md](../INSTALL.md) — Full installation guide
- [README.md](../README.md) — Project overview
- [MCP Protocol](https://modelcontextprotocol.io/)
