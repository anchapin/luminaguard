#!/usr/bin/env python3
"""
LLM Client for LuminaGuard Agent Reasoning

This module provides LLM integration for agent decision-making.
Supports multiple LLM providers through a unified interface.

Key Features:
- Multi-turn reasoning support
- Tool selection based on context (not keywords)
- Deterministic outputs via temperature=0
- Configurable provider selection
- Mock LLM for testing
- Fallback mechanism: tries multiple API keys/clients when one fails
"""

from __future__ import annotations

import json
import logging
from abc import ABC, abstractmethod
from typing import Any, Dict, List, Optional
from dataclasses import dataclass, field
from enum import Enum

logger = logging.getLogger(__name__)

from loop import ToolCall, ActionKind


class LLMProvider(Enum):
    """Supported LLM providers"""

    MOCK = "mock"  # For testing
    OPENAI = "openai"  # GPT models
    ANTHROPIC = "anthropic"  # Claude models
    OLLAMA = "ollama"  # Local models


@dataclass
class LLMConfig:
    """Configuration for LLM client"""

    provider: LLMProvider = LLMProvider.MOCK
    model: str = "mock-model"
    api_key: Optional[str] = None
    base_url: Optional[str] = None
    temperature: float = 0.0  # Deterministic
    max_tokens: int = 1000
    timeout: int = 30


class LLMResponse:
    """Response from LLM"""

    def __init__(
        self,
        tool_name: Optional[str],
        arguments: Dict[str, Any],
        reasoning: str,
        is_complete: bool,
    ):
        self.tool_name = tool_name
        self.arguments = arguments
        self.reasoning = reasoning
        self.is_complete = is_complete


class LLMClient(ABC):
    """Abstract base class for LLM clients"""

    @abstractmethod
    def decide_action(
        self,
        messages: List[Dict[str, Any]],
        available_tools: List[str],
        context: Dict[str, Any],
    ) -> LLMResponse:
        """
        Decide next action based on conversation state.

        Args:
            messages: Conversation history
            available_tools: List of available tool names
            context: Additional context (files, environment, etc.)

        Returns:
            LLMResponse with tool selection and reasoning
        """
        pass


class MockLLMClient(LLMClient):
    """
    Mock LLM client for testing.

    Provides deterministic, testable behavior without requiring
    actual LLM API calls.

    Uses pattern matching on messages to select tools.
    """

    # Mock reasoning responses for different scenarios
    REASONING_TEMPLATES = {
        "read": "User wants to read a file. I'll use read_file tool.",
        "write": "User wants to write content. I'll use write_file tool.",
        "search": "User wants to search for content. I'll use search tool.",
        "list": "User wants to list files. I'll use list_directory tool.",
        "delete": "User wants to delete a file. This requires approval.",
        "edit": "User wants to edit a file. This requires approval.",
        "default": "I don't need to take any action. Task is complete.",
    }

    # Tool mappings based on patterns
    TOOL_PATTERNS = [
        ("read_file", ["read", "view", "show", "display", "cat"]),
        ("write_file", ["write", "create", "save"]),
        ("search", ["search", "find", "grep", "locate"]),
        ("list_directory", ["list", "ls", "dir"]),
        ("delete_file", ["delete", "remove", "rm"]),
        ("edit_file", ["edit", "modify", "update"]),
    ]

    def __init__(self, config: Optional[LLMConfig] = None):
        self.config = config or LLMConfig()
        self.call_count = 0

    def decide_action(
        self,
        messages: List[Dict[str, Any]],
        available_tools: List[str],
        context: Dict[str, Any],
    ) -> LLMResponse:
        """
        Mock LLM decision-making.

        Analyzes the last user message to decide on an action.

        Multi-turn reasoning:
        - After tool execution, checks if task is complete
        - Can request multiple tools in sequence
        """
        self.call_count += 1

        if not messages:
            return LLMResponse(None, {}, "No messages provided", True)

        # Check if we already have tool responses (multi-turn)
        tool_msgs = [m for m in messages if m["role"] == "tool"]

        # Get the last user message
        user_msgs = [m for m in messages if m["role"] == "user"]

        if not user_msgs:
            # No user message, task complete
            if tool_msgs:
                return LLMResponse(
                    None,
                    {},
                    "Task completed after tool execution",
                    True,
                )
            return LLMResponse(None, {}, "No user message found", True)

        # For multi-turn: check if we've already executed a tool
        # If tool_msgs exist and we've seen the last user message before,
        # this is a continuation after tool execution
        if tool_msgs and len(tool_msgs) >= self.call_count - 1:
            # After tool execution, assume task is complete
            # (simplified for mock - real LLM would analyze the tool result)
            return LLMResponse(
                None,
                {},
                "Task completed after tool execution",
                True,
            )

        last_msg = user_msgs[-1]["content"].lower()

        # Find matching tool
        tool_name = None
        reasoning = self.REASONING_TEMPLATES["default"]

        for tool, patterns in self.TOOL_PATTERNS:
            if any(pattern in last_msg for pattern in patterns):
                if tool in available_tools:
                    tool_name = tool
                    reasoning = self.REASONING_TEMPLATES.get(
                        tool.split("_")[0], reasoning
                    )
                    break

        if tool_name:
            # Generate mock arguments based on tool
            arguments = self._generate_arguments(tool_name, last_msg)
            return LLMResponse(
                tool_name,
                arguments,
                reasoning,
                False,  # Not complete, need to execute tool
            )

        # No tool needed, task complete
        return LLMResponse(None, {}, reasoning, True)

    def _generate_arguments(self, tool_name: str, message: str) -> Dict[str, Any]:
        """Generate mock arguments based on tool type."""
        if tool_name == "read_file":
            return {"path": "/tmp/test.txt"}
        elif tool_name == "write_file":
            return {"path": "/tmp/test.txt", "content": "Test content"}
        elif tool_name == "search":
            return {"query": "test"}
        elif tool_name == "list_directory":
            return {"path": "/tmp"}
        elif tool_name == "delete_file":
            return {"path": "/tmp/test.txt"}
        elif tool_name == "edit_file":
            return {"path": "/tmp/test.txt", "content": "Updated content"}
        else:
            return {}


# Error codes that indicate a key is exhausted/rate-limited and a fallback
# should be attempted.  These are checked against the string representation
# of the exception so we don't need to import provider-specific exception
# types at module level.
_RETRYABLE_ERROR_CODES = frozenset(
    [
        "insufficient_quota",   # OpenAI: quota exceeded
        "rate_limit_exceeded",  # OpenAI / Anthropic: rate limit
        "429",                  # HTTP 429 Too Many Requests
        "overloaded",           # Anthropic: server overloaded
        "quota_exceeded",       # Generic quota error
    ]
)


def _is_retryable_error(exc: Exception) -> bool:
    """Return True if *exc* is a transient/quota error worth retrying."""
    exc_str = str(exc).lower()
    return any(code in exc_str for code in _RETRYABLE_ERROR_CODES)


class LLMClientError(Exception):
    """Raised by LLM clients when a call fails and should not be retried."""


class LLMClientRetryableError(Exception):
    """
    Raised by LLM clients when a call fails due to quota/rate-limiting.

    The :class:`FallbackLLMClient` catches this and tries the next client.
    """


class OpenAILLMClient(LLMClient):
    """
    OpenAI GPT client for production use.

    Requires OPENAI_API_KEY environment variable.
    """

    def __init__(self, config: LLMConfig):
        self.config = config
        try:
            import openai

            self.client = openai.OpenAI(api_key=config.api_key)
            self.openai = openai
        except ImportError:
            raise ImportError(
                "openai package not installed. "
                "Install with: pip install openai"
            )

    def decide_action(
        self,
        messages: List[Dict[str, Any]],
        available_tools: List[str],
        context: Dict[str, Any],
    ) -> LLMResponse:
        """
        Use OpenAI API to decide next action.

        Uses function calling to request structured tool selection.

        Raises:
            LLMClientRetryableError: on quota / rate-limit errors so that a
                :class:`FallbackLLMClient` can try the next key.
            LLMClientError: on non-retryable API errors.
        """
        system_prompt = self._build_system_prompt(available_tools, context)

        # Convert messages to OpenAI format
        openai_messages = [{"role": "system", "content": system_prompt}]
        for msg in messages:
            if msg["role"] in ["user", "assistant"]:
                openai_messages.append({"role": msg["role"], "content": msg["content"]})

        # Define tools for function calling
        tools = self._build_tool_definitions(available_tools)

        try:
            response = self.client.chat.completions.create(
                model=self.config.model,
                messages=openai_messages,
                tools=tools,
                tool_choice="auto",
                temperature=self.config.temperature,
                max_tokens=self.config.max_tokens,
                timeout=self.config.timeout,
            )

            message = response.choices[0].message

            # Check if tool call was made
            if message.tool_calls and len(message.tool_calls) > 0:
                tool_call = message.tool_calls[0]
                function_name = tool_call.function.name
                arguments = json.loads(tool_call.function.arguments)

                return LLMResponse(
                    tool_name=function_name,
                    arguments=arguments,
                    reasoning=f"Selected {function_name} based on context",
                    is_complete=False,
                )
            else:
                # No tool call, task complete
                return LLMResponse(
                    None,
                    {},
                    message.content or "No action needed",
                    is_complete=True,
                )

        except Exception as e:
            if _is_retryable_error(e):
                raise LLMClientRetryableError(str(e)) from e
            raise LLMClientError(str(e)) from e

    def _build_system_prompt(
        self, available_tools: List[str], context: Dict[str, Any]
    ) -> str:
        """Build system prompt for LLM."""
        tools_str = "\n  - ".join(available_tools)
        context_str = json.dumps(context, indent=2) if context else "{}"

        return f"""You are LuminaGuard, an AI assistant that helps with tasks using tools.

Available tools:
  - {tools_str}

Context:
{context_str}

Your role:
1. Analyze the user's request
2. Select the appropriate tool if needed
3. Extract required arguments from the request
4. If no tool is needed, respond with a helpful message

Return "NO_TOOL" if the task is complete or doesn't require a tool.
"""

    def _build_tool_definitions(self, available_tools: List[str]) -> List[Dict[str, Any]]:
        """Build OpenAI function calling definitions."""
        # Simplified tool definitions - can be expanded
        tools = [
            {
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read the contents of a file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file to read",
                            }
                        },
                        "required": ["path"],
                    },
                },
            },
            {
                "type": "function",
                "function": {
                    "name": "write_file",
                    "description": "Write content to a file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file",
                            },
                            "content": {
                                "type": "string",
                                "description": "Content to write",
                            },
                        },
                        "required": ["path", "content"],
                    },
                },
            },
            {
                "type": "function",
                "function": {
                    "name": "search",
                    "description": "Search for text in files",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Search query",
                            }
                        },
                        "required": ["query"],
                    },
                },
            },
            {
                "type": "function",
                "function": {
                    "name": "list_directory",
                    "description": "List files in a directory",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Directory path",
                            }
                        },
                        "required": ["path"],
                    },
                },
            },
        ]

        # Filter to only available tools
        return [
            t
            for t in tools
            if t["function"]["name"] in available_tools
        ]


class FallbackLLMClient(LLMClient):
    """
    A composite LLM client that tries multiple underlying clients in order.

    When a client raises :class:`LLMClientRetryableError` (quota exceeded,
    rate-limited, etc.) the next client in the chain is tried automatically.
    If all clients are exhausted the last error is surfaced as a graceful
    ``is_complete=True`` response so the bot keeps running.

    Example – two OpenAI keys with an Anthropic key as final fallback::

        from llm_client import FallbackLLMClient, OpenAILLMClient, LLMConfig, LLMProvider

        clients = [
            OpenAILLMClient(LLMConfig(provider=LLMProvider.OPENAI, api_key="sk-key1")),
            OpenAILLMClient(LLMConfig(provider=LLMProvider.OPENAI, api_key="sk-key2")),
        ]
        client = FallbackLLMClient(clients)
        response = client.decide_action(messages, tools, context)
    """

    def __init__(self, clients: List[LLMClient]):
        if not clients:
            raise ValueError("FallbackLLMClient requires at least one client")
        self.clients = clients

    def decide_action(
        self,
        messages: List[Dict[str, Any]],
        available_tools: List[str],
        context: Dict[str, Any],
    ) -> LLMResponse:
        """
        Try each client in order, falling back on retryable errors.

        Returns the first successful response.  If all clients fail with
        retryable errors the last error message is returned as a completed
        (non-crashing) response.
        """
        last_error: Optional[str] = None

        for idx, client in enumerate(self.clients):
            try:
                return client.decide_action(messages, available_tools, context)
            except LLMClientRetryableError as exc:
                last_error = str(exc)
                logger.warning(
                    "LLM client %d/%d failed with retryable error (%s); "
                    "trying next key.",
                    idx + 1,
                    len(self.clients),
                    last_error,
                )
            except LLMClientError as exc:
                # Non-retryable – surface immediately
                last_error = str(exc)
                logger.error("LLM client %d/%d failed: %s", idx + 1, len(self.clients), last_error)
                break

        # All clients exhausted or non-retryable error encountered
        return LLMResponse(
            None,
            {},
            f"All LLM API keys exhausted or failed. Last error: {last_error}",
            is_complete=True,
        )


def create_llm_client(config: Optional[LLMConfig] = None) -> LLMClient:
    """
    Factory function to create LLM client based on configuration.

    Args:
        config: LLMConfig instance (uses defaults if None)

    Returns:
        LLMClient instance
    """
    if config is None:
        config = LLMConfig()

    if config.provider == LLMProvider.MOCK:
        return MockLLMClient(config)
    elif config.provider == LLMProvider.OPENAI:
        return OpenAILLMClient(config)
    # Add other providers here as needed
    else:
        raise ValueError(f"Unsupported LLM provider: {config.provider}")


# ---------------------------------------------------------------------------
# Environment variable names for supported LLM providers.
#
# Fallback keys follow the pattern  <BASE_VAR>_2, <BASE_VAR>_3, …
# e.g.  OPENAI_API_KEY, OPENAI_API_KEY_2, OPENAI_API_KEY_3
# ---------------------------------------------------------------------------

# Primary env-var names (one per provider)
_LLM_ENV_VARS = [
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "OLLAMA_HOST",
    "LUMINAGUARD_LLM_API_KEY",
    "LLM_API_KEY",
]

# Provider-specific base names that support numbered fallback keys
_PROVIDER_KEY_BASES: Dict[str, LLMProvider] = {
    "OPENAI_API_KEY": LLMProvider.OPENAI,
    "ANTHROPIC_API_KEY": LLMProvider.ANTHROPIC,
}

NO_LLM_CONFIGURED_MESSAGE = "Please setup environment variables for your LLM"

# Maximum number of numbered fallback keys to look for (e.g. _2 … _N)
_MAX_FALLBACK_KEYS = 10


def _collect_openai_keys() -> List[str]:
    """
    Return all OpenAI API keys found in the environment.

    Checks ``OPENAI_API_KEY`` first, then ``OPENAI_API_KEY_2`` …
    ``OPENAI_API_KEY_<_MAX_FALLBACK_KEYS>``.
    """
    import os

    keys: List[str] = []
    primary = os.environ.get("OPENAI_API_KEY", "").strip()
    if primary:
        keys.append(primary)
    for i in range(2, _MAX_FALLBACK_KEYS + 1):
        key = os.environ.get(f"OPENAI_API_KEY_{i}", "").strip()
        if key:
            keys.append(key)
    return keys


def _collect_anthropic_keys() -> List[str]:
    """
    Return all Anthropic API keys found in the environment.

    Checks ``ANTHROPIC_API_KEY`` first, then ``ANTHROPIC_API_KEY_2`` …
    ``ANTHROPIC_API_KEY_<_MAX_FALLBACK_KEYS>``.
    """
    import os

    keys: List[str] = []
    primary = os.environ.get("ANTHROPIC_API_KEY", "").strip()
    if primary:
        keys.append(primary)
    for i in range(2, _MAX_FALLBACK_KEYS + 1):
        key = os.environ.get(f"ANTHROPIC_API_KEY_{i}", "").strip()
        if key:
            keys.append(key)
    return keys


def build_fallback_client(base_config: Optional[LLMConfig] = None) -> Optional[LLMClient]:
    """
    Build a :class:`FallbackLLMClient` from all API keys found in the
    environment, or return a single client when only one key is available.

    Key discovery order (highest priority first):
      1. OpenAI keys  – ``OPENAI_API_KEY``, ``OPENAI_API_KEY_2``, …
      2. Anthropic keys – ``ANTHROPIC_API_KEY``, ``ANTHROPIC_API_KEY_2``, …
      3. Ollama host  – ``OLLAMA_HOST``

    Returns ``None`` when no keys are configured.

    Args:
        base_config: Optional base :class:`LLMConfig` used to inherit
            ``model``, ``temperature``, ``max_tokens``, and ``timeout``
            settings.  Provider and ``api_key`` are overridden per key.

    Returns:
        A :class:`FallbackLLMClient` (multiple keys), a single
        :class:`LLMClient`, or ``None`` if nothing is configured.
    """
    import os

    base = base_config or LLMConfig()
    clients: List[LLMClient] = []

    # --- OpenAI keys ---------------------------------------------------------
    for key in _collect_openai_keys():
        cfg = LLMConfig(
            provider=LLMProvider.OPENAI,
            api_key=key,
            model=base.model if base.provider == LLMProvider.OPENAI else "gpt-4o-mini",
            temperature=base.temperature,
            max_tokens=base.max_tokens,
            timeout=base.timeout,
        )
        try:
            clients.append(OpenAILLMClient(cfg))
        except ImportError:
            logger.warning("openai package not installed; skipping OpenAI keys.")
            break

    # --- Anthropic keys ------------------------------------------------------
    for key in _collect_anthropic_keys():
        cfg = LLMConfig(
            provider=LLMProvider.ANTHROPIC,
            api_key=key,
            model=base.model if base.provider == LLMProvider.ANTHROPIC else "claude-3-haiku-20240307",
            temperature=base.temperature,
            max_tokens=base.max_tokens,
            timeout=base.timeout,
        )
        # Anthropic client not yet implemented; skip gracefully
        logger.debug("Anthropic client not yet implemented; skipping key.")
        break  # remove this break when AnthropicLLMClient is added

    # --- Ollama --------------------------------------------------------------
    ollama_host = os.environ.get("OLLAMA_HOST", "").strip()
    if ollama_host:
        cfg = LLMConfig(
            provider=LLMProvider.OLLAMA,
            base_url=ollama_host,
            model=base.model if base.provider == LLMProvider.OLLAMA else "llama3",
            temperature=base.temperature,
            max_tokens=base.max_tokens,
            timeout=base.timeout,
        )
        # Ollama client not yet implemented; skip gracefully
        logger.debug("Ollama client not yet implemented; skipping.")

    if not clients:
        return None
    if len(clients) == 1:
        return clients[0]
    return FallbackLLMClient(clients)


def is_llm_configured() -> bool:
    """
    Check whether any LLM provider environment variables are set.

    Also checks numbered fallback keys (e.g. ``OPENAI_API_KEY_2``).

    Returns:
        True if at least one LLM provider env var is configured.
    """
    import os

    if any(os.environ.get(var) for var in _LLM_ENV_VARS):
        return True
    # Check numbered fallback keys
    for base in _PROVIDER_KEY_BASES:
        for i in range(2, _MAX_FALLBACK_KEYS + 1):
            if os.environ.get(f"{base}_{i}"):
                return True
    return False


def get_bot_response(prompt: str, config: Optional[LLMConfig] = None) -> str:
    """
    Get a response from the 24/7 bot for a given prompt.

    If no LLM environment variables are configured, returns a setup
    instruction message instead of attempting an LLM call.

    When multiple API keys are available (e.g. ``OPENAI_API_KEY`` and
    ``OPENAI_API_KEY_2``) a :class:`FallbackLLMClient` is used so that
    quota/rate-limit errors on one key automatically fall back to the next.

    Args:
        prompt: The user's input prompt.
        config: Optional LLMConfig. If None, auto-detects from environment.

    Returns:
        Bot response string.
    """
    if not is_llm_configured():
        return NO_LLM_CONFIGURED_MESSAGE

    # Try to build a fallback-aware client from all available env keys.
    # Fall back to the legacy single-config path when a specific config is
    # provided (e.g. from BotFactory with an explicit provider override).
    if config is None or config.provider == LLMProvider.MOCK:
        fallback_client = build_fallback_client(config)
        if fallback_client is not None:
            messages = [{"role": "user", "content": prompt}]
            try:
                response = fallback_client.decide_action(messages, [], {})
                return response.reasoning or NO_LLM_CONFIGURED_MESSAGE
            except (LLMClientError, LLMClientRetryableError) as exc:
                return f"LLM error: {exc}"

    # Explicit single-provider config path (e.g. MOCK for tests, or a
    # BotFactory override with a specific provider).
    client = create_llm_client(config)
    messages = [{"role": "user", "content": prompt}]
    try:
        response = client.decide_action(messages, [], {})
        return response.reasoning or NO_LLM_CONFIGURED_MESSAGE
    except (LLMClientError, LLMClientRetryableError) as exc:
        return f"LLM error: {exc}"
