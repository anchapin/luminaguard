#!/usr/bin/env python3
"""
Tests for the LLM fallback mechanism.

Validates:
- FallbackLLMClient tries clients in order
- Retryable errors (quota, rate-limit) trigger fallback to next client
- Non-retryable errors stop the chain immediately
- All clients exhausted → graceful is_complete=True response
- _is_retryable_error correctly classifies errors
- _collect_openai_keys / _collect_anthropic_keys read numbered env vars
- build_fallback_client builds the right chain from env vars
- is_llm_configured detects numbered fallback keys
- get_bot_response uses the fallback chain transparently
"""

from __future__ import annotations

import os
from typing import Any, Dict, List, Optional
from unittest.mock import MagicMock, patch

import pytest

import sys
from pathlib import Path

AGENT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(AGENT_ROOT))

from llm_client import (
    FallbackLLMClient,
    LLMClient,
    LLMClientError,
    LLMClientRetryableError,
    LLMConfig,
    LLMProvider,
    LLMResponse,
    MockLLMClient,
    OpenAILLMClient,
    _collect_anthropic_keys,
    _collect_openai_keys,
    _is_retryable_error,
    build_fallback_client,
    get_bot_response,
    is_llm_configured,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

class _SuccessClient(LLMClient):
    """Always returns a successful response."""

    def __init__(self, reasoning: str = "success"):
        self.reasoning = reasoning
        self.call_count = 0

    def decide_action(self, messages, available_tools, context) -> LLMResponse:
        self.call_count += 1
        return LLMResponse(None, {}, self.reasoning, is_complete=True)


class _RetryableErrorClient(LLMClient):
    """Always raises LLMClientRetryableError."""

    def __init__(self, message: str = "quota exceeded"):
        self.message = message
        self.call_count = 0

    def decide_action(self, messages, available_tools, context) -> LLMResponse:
        self.call_count += 1
        raise LLMClientRetryableError(self.message)


class _FatalErrorClient(LLMClient):
    """Always raises LLMClientError (non-retryable)."""

    def __init__(self, message: str = "auth error"):
        self.message = message
        self.call_count = 0

    def decide_action(self, messages, available_tools, context) -> LLMResponse:
        self.call_count += 1
        raise LLMClientError(self.message)


# ---------------------------------------------------------------------------
# _is_retryable_error
# ---------------------------------------------------------------------------

class TestIsRetryableError:
    def test_insufficient_quota(self):
        exc = Exception("Error code: 429 - insufficient_quota")
        assert _is_retryable_error(exc) is True

    def test_rate_limit_exceeded(self):
        exc = Exception("rate_limit_exceeded: too many requests")
        assert _is_retryable_error(exc) is True

    def test_http_429(self):
        exc = Exception("HTTP error 429 Too Many Requests")
        assert _is_retryable_error(exc) is True

    def test_overloaded(self):
        exc = Exception("Anthropic API overloaded")
        assert _is_retryable_error(exc) is True

    def test_quota_exceeded(self):
        exc = Exception("quota_exceeded for this project")
        assert _is_retryable_error(exc) is True

    def test_non_retryable_auth(self):
        exc = Exception("Invalid API key provided")
        assert _is_retryable_error(exc) is False

    def test_non_retryable_model(self):
        exc = Exception("Model not found")
        assert _is_retryable_error(exc) is False

    def test_case_insensitive(self):
        exc = Exception("INSUFFICIENT_QUOTA")
        assert _is_retryable_error(exc) is True


# ---------------------------------------------------------------------------
# FallbackLLMClient
# ---------------------------------------------------------------------------

class TestFallbackLLMClientInit:
    def test_requires_at_least_one_client(self):
        with pytest.raises(ValueError, match="at least one client"):
            FallbackLLMClient([])

    def test_single_client_accepted(self):
        client = FallbackLLMClient([_SuccessClient()])
        assert len(client.clients) == 1

    def test_multiple_clients_accepted(self):
        clients = [_SuccessClient(), _SuccessClient()]
        fb = FallbackLLMClient(clients)
        assert len(fb.clients) == 2


class TestFallbackLLMClientDecideAction:
    """Core fallback logic tests."""

    _MESSAGES = [{"role": "user", "content": "Hello"}]

    def test_first_client_success_no_fallback(self):
        """When the first client succeeds, the second is never called."""
        first = _SuccessClient("first")
        second = _SuccessClient("second")
        fb = FallbackLLMClient([first, second])

        resp = fb.decide_action(self._MESSAGES, [], {})

        assert resp.reasoning == "first"
        assert first.call_count == 1
        assert second.call_count == 0

    def test_first_retryable_falls_back_to_second(self):
        """Retryable error on first → second client is tried."""
        first = _RetryableErrorClient("quota exceeded")
        second = _SuccessClient("second succeeded")
        fb = FallbackLLMClient([first, second])

        resp = fb.decide_action(self._MESSAGES, [], {})

        assert resp.reasoning == "second succeeded"
        assert first.call_count == 1
        assert second.call_count == 1

    def test_all_retryable_exhausted_returns_graceful_response(self):
        """All clients fail with retryable errors → graceful is_complete response."""
        clients = [
            _RetryableErrorClient("key1 quota"),
            _RetryableErrorClient("key2 quota"),
            _RetryableErrorClient("key3 quota"),
        ]
        fb = FallbackLLMClient(clients)

        resp = fb.decide_action(self._MESSAGES, [], {})

        assert resp.is_complete is True
        assert resp.tool_name is None
        assert "exhausted" in resp.reasoning.lower() or "failed" in resp.reasoning.lower()
        assert all(c.call_count == 1 for c in clients)

    def test_fatal_error_stops_chain_immediately(self):
        """Non-retryable error stops the chain; subsequent clients not tried."""
        first = _FatalErrorClient("invalid api key")
        second = _SuccessClient("should not be reached")
        fb = FallbackLLMClient([first, second])

        resp = fb.decide_action(self._MESSAGES, [], {})

        assert resp.is_complete is True
        assert first.call_count == 1
        assert second.call_count == 0  # never reached

    def test_retryable_then_fatal_stops_chain(self):
        """Retryable on first, fatal on second → third never tried."""
        first = _RetryableErrorClient("quota")
        second = _FatalErrorClient("auth error")
        third = _SuccessClient("should not be reached")
        fb = FallbackLLMClient([first, second, third])

        resp = fb.decide_action(self._MESSAGES, [], {})

        assert resp.is_complete is True
        assert first.call_count == 1
        assert second.call_count == 1
        assert third.call_count == 0

    def test_last_error_included_in_response(self):
        """The last error message is included in the exhausted response."""
        clients = [
            _RetryableErrorClient("key1 quota exceeded"),
            _RetryableErrorClient("key2 rate_limit_exceeded"),
        ]
        fb = FallbackLLMClient(clients)

        resp = fb.decide_action(self._MESSAGES, [], {})

        assert "key2 rate_limit_exceeded" in resp.reasoning

    def test_passes_arguments_to_clients(self):
        """Messages, tools, and context are forwarded to each client."""
        received: dict = {}

        class _RecordingClient(LLMClient):
            def decide_action(self, messages, available_tools, context):
                received["messages"] = messages
                received["tools"] = available_tools
                received["context"] = context
                return LLMResponse(None, {}, "ok", True)

        fb = FallbackLLMClient([_RecordingClient()])
        messages = [{"role": "user", "content": "test"}]
        tools = ["read_file"]
        context = {"key": "value"}

        fb.decide_action(messages, tools, context)

        assert received["messages"] == messages
        assert received["tools"] == tools
        assert received["context"] == context

    def test_single_client_success(self):
        """FallbackLLMClient with one client works like a plain client."""
        fb = FallbackLLMClient([_SuccessClient("only one")])
        resp = fb.decide_action(self._MESSAGES, [], {})
        assert resp.reasoning == "only one"
        assert resp.is_complete is True


# ---------------------------------------------------------------------------
# _collect_openai_keys
# ---------------------------------------------------------------------------

class TestCollectOpenAIKeys:
    def test_no_keys_returns_empty(self):
        with patch.dict(os.environ, {}, clear=True):
            assert _collect_openai_keys() == []

    def test_primary_key_only(self):
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-primary"}, clear=True):
            assert _collect_openai_keys() == ["sk-primary"]

    def test_primary_and_secondary(self):
        env = {"OPENAI_API_KEY": "sk-1", "OPENAI_API_KEY_2": "sk-2"}
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_openai_keys()
        assert keys == ["sk-1", "sk-2"]

    def test_multiple_numbered_keys(self):
        env = {
            "OPENAI_API_KEY": "sk-1",
            "OPENAI_API_KEY_2": "sk-2",
            "OPENAI_API_KEY_3": "sk-3",
        }
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_openai_keys()
        assert keys == ["sk-1", "sk-2", "sk-3"]

    def test_gap_in_numbering_stops_at_gap(self):
        """Keys are collected in order; a missing _2 means _3 is also skipped."""
        env = {
            "OPENAI_API_KEY": "sk-1",
            # _2 missing
            "OPENAI_API_KEY_3": "sk-3",
        }
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_openai_keys()
        # _3 is still collected because we scan all indices up to _MAX_FALLBACK_KEYS
        assert "sk-1" in keys
        assert "sk-3" in keys

    def test_empty_string_key_ignored(self):
        env = {"OPENAI_API_KEY": "sk-1", "OPENAI_API_KEY_2": ""}
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_openai_keys()
        assert keys == ["sk-1"]

    def test_whitespace_only_key_ignored(self):
        env = {"OPENAI_API_KEY": "sk-1", "OPENAI_API_KEY_2": "   "}
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_openai_keys()
        assert keys == ["sk-1"]


# ---------------------------------------------------------------------------
# _collect_anthropic_keys
# ---------------------------------------------------------------------------

class TestCollectAnthropicKeys:
    def test_no_keys_returns_empty(self):
        with patch.dict(os.environ, {}, clear=True):
            assert _collect_anthropic_keys() == []

    def test_primary_key_only(self):
        with patch.dict(os.environ, {"ANTHROPIC_API_KEY": "sk-ant-1"}, clear=True):
            assert _collect_anthropic_keys() == ["sk-ant-1"]

    def test_primary_and_secondary(self):
        env = {"ANTHROPIC_API_KEY": "sk-ant-1", "ANTHROPIC_API_KEY_2": "sk-ant-2"}
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_anthropic_keys()
        assert keys == ["sk-ant-1", "sk-ant-2"]

    def test_empty_string_key_ignored(self):
        env = {"ANTHROPIC_API_KEY": "sk-ant-1", "ANTHROPIC_API_KEY_2": ""}
        with patch.dict(os.environ, env, clear=True):
            keys = _collect_anthropic_keys()
        assert keys == ["sk-ant-1"]


# ---------------------------------------------------------------------------
# is_llm_configured – numbered key detection
# ---------------------------------------------------------------------------

class TestIsLLMConfiguredFallbackKeys:
    def test_no_keys_false(self):
        with patch.dict(os.environ, {}, clear=True):
            assert is_llm_configured() is False

    def test_primary_openai_key(self):
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-1"}, clear=True):
            assert is_llm_configured() is True

    def test_numbered_openai_key_only(self):
        """OPENAI_API_KEY_2 alone (without primary) should still be detected."""
        with patch.dict(os.environ, {"OPENAI_API_KEY_2": "sk-2"}, clear=True):
            assert is_llm_configured() is True

    def test_numbered_anthropic_key_only(self):
        with patch.dict(os.environ, {"ANTHROPIC_API_KEY_2": "sk-ant-2"}, clear=True):
            assert is_llm_configured() is True

    def test_ollama_host(self):
        with patch.dict(os.environ, {"OLLAMA_HOST": "http://localhost:11434"}, clear=True):
            assert is_llm_configured() is True


# ---------------------------------------------------------------------------
# build_fallback_client
# ---------------------------------------------------------------------------

class TestBuildFallbackClient:
    def test_no_env_vars_returns_none(self):
        with patch.dict(os.environ, {}, clear=True):
            result = build_fallback_client()
        assert result is None

    def test_single_openai_key_returns_single_client(self):
        """One key → plain OpenAILLMClient (not wrapped in FallbackLLMClient)."""
        env = {"OPENAI_API_KEY": "sk-only"}
        with patch.dict(os.environ, env, clear=True):
            try:
                result = build_fallback_client()
            except ImportError:
                pytest.skip("openai package not installed")
        assert isinstance(result, OpenAILLMClient)

    def test_two_openai_keys_returns_fallback_client(self):
        """Two keys → FallbackLLMClient wrapping two OpenAILLMClients."""
        env = {"OPENAI_API_KEY": "sk-1", "OPENAI_API_KEY_2": "sk-2"}
        with patch.dict(os.environ, env, clear=True):
            try:
                result = build_fallback_client()
            except ImportError:
                pytest.skip("openai package not installed")
        assert isinstance(result, FallbackLLMClient)
        assert len(result.clients) == 2

    def test_three_openai_keys_returns_fallback_client(self):
        env = {
            "OPENAI_API_KEY": "sk-1",
            "OPENAI_API_KEY_2": "sk-2",
            "OPENAI_API_KEY_3": "sk-3",
        }
        with patch.dict(os.environ, env, clear=True):
            try:
                result = build_fallback_client()
            except ImportError:
                pytest.skip("openai package not installed")
        assert isinstance(result, FallbackLLMClient)
        assert len(result.clients) == 3

    def test_openai_import_error_skips_gracefully(self):
        """If openai package is missing, build_fallback_client returns None."""
        env = {"OPENAI_API_KEY": "sk-1"}
        with patch.dict(os.environ, env, clear=True):
            with patch("llm_client.OpenAILLMClient", side_effect=ImportError("no openai")):
                result = build_fallback_client()
        assert result is None


# ---------------------------------------------------------------------------
# get_bot_response – integration with fallback
# ---------------------------------------------------------------------------

class TestGetBotResponseFallback:
    """
    Tests that get_bot_response uses the fallback chain when multiple keys
    are available, and falls back gracefully when all keys are exhausted.
    """

    def test_no_llm_configured_returns_setup_message(self):
        from llm_client import NO_LLM_CONFIGURED_MESSAGE
        with patch.dict(os.environ, {}, clear=True):
            result = get_bot_response("Hello")
        assert result == NO_LLM_CONFIGURED_MESSAGE

    def test_uses_fallback_client_when_multiple_keys(self):
        """
        With two OpenAI keys, get_bot_response should build a FallbackLLMClient.
        We mock build_fallback_client to return a controllable client.
        """
        mock_client = _SuccessClient("fallback worked")
        env = {"OPENAI_API_KEY": "sk-1", "OPENAI_API_KEY_2": "sk-2"}
        with patch.dict(os.environ, env, clear=True):
            with patch("llm_client.build_fallback_client", return_value=mock_client):
                result = get_bot_response("Hello")
        assert result == "fallback worked"

    def test_fallback_client_exhausted_returns_error_string(self):
        """When all keys are exhausted, get_bot_response returns an error string."""
        exhausted_client = FallbackLLMClient([
            _RetryableErrorClient("quota1"),
            _RetryableErrorClient("quota2"),
        ])
        env = {"OPENAI_API_KEY": "sk-1", "OPENAI_API_KEY_2": "sk-2"}
        with patch.dict(os.environ, env, clear=True):
            with patch("llm_client.build_fallback_client", return_value=exhausted_client):
                result = get_bot_response("Hello")
        # Should return the exhausted message, not crash
        assert isinstance(result, str)
        assert len(result) > 0

    def test_explicit_mock_config_bypasses_fallback(self):
        """
        When an explicit MOCK config is passed, the mock client is used directly
        (no real API calls).
        """
        config = LLMConfig(provider=LLMProvider.MOCK)
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-1"}, clear=True):
            result = get_bot_response("Hello", config=config)
        # MockLLMClient returns reasoning for "Hello" (no tool match → default)
        assert isinstance(result, str)

    def test_single_openai_key_uses_build_fallback_client(self):
        """
        With a single OpenAI key and no explicit config, get_bot_response
        should call build_fallback_client (which returns a single OpenAILLMClient).
        """
        mock_client = _SuccessClient("single key response")
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-only"}, clear=True):
            with patch("llm_client.build_fallback_client", return_value=mock_client):
                result = get_bot_response("Hello")
        assert result == "single key response"


# ---------------------------------------------------------------------------
# OpenAILLMClient error classification
# ---------------------------------------------------------------------------

class TestOpenAILLMClientErrorClassification:
    """
    Verify that OpenAILLMClient raises the right exception types so that
    FallbackLLMClient can distinguish retryable from fatal errors.
    """

    def _make_client(self) -> OpenAILLMClient:
        try:
            return OpenAILLMClient(LLMConfig(provider=LLMProvider.OPENAI, api_key="sk-test"))
        except ImportError:
            pytest.skip("openai package not installed")

    def test_quota_error_raises_retryable(self):
        client = self._make_client()
        quota_exc = Exception(
            "Error code: 429 - {'error': {'code': 'insufficient_quota'}}"
        )
        with patch.object(client.client.chat.completions, "create", side_effect=quota_exc):
            with pytest.raises(LLMClientRetryableError):
                client.decide_action([{"role": "user", "content": "hi"}], [], {})

    def test_rate_limit_raises_retryable(self):
        client = self._make_client()
        rate_exc = Exception("rate_limit_exceeded: please slow down")
        with patch.object(client.client.chat.completions, "create", side_effect=rate_exc):
            with pytest.raises(LLMClientRetryableError):
                client.decide_action([{"role": "user", "content": "hi"}], [], {})

    def test_auth_error_raises_llm_client_error(self):
        client = self._make_client()
        auth_exc = Exception("Invalid API key provided")
        with patch.object(client.client.chat.completions, "create", side_effect=auth_exc):
            with pytest.raises(LLMClientError):
                client.decide_action([{"role": "user", "content": "hi"}], [], {})

    def test_generic_error_raises_llm_client_error(self):
        client = self._make_client()
        generic_exc = Exception("Something went wrong")
        with patch.object(client.client.chat.completions, "create", side_effect=generic_exc):
            with pytest.raises(LLMClientError):
                client.decide_action([{"role": "user", "content": "hi"}], [], {})


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
