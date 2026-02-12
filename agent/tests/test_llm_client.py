#!/usr/bin/env python3
"""
Tests for LLM Client
"""

import os
import pytest
from unittest.mock import MagicMock, patch
from llm_client import LlmClient, LlmError


class TestLlmClient:

    @patch.dict(os.environ, {"ANTHROPIC_API_KEY": "sk-test-anthropic"})
    @patch("llm_client.Anthropic")
    def test_init_anthropic_default(self, mock_anthropic):
        """Test initialization with Anthropic as default"""
        client = LlmClient()
        assert client.provider == "anthropic"
        mock_anthropic.assert_called_once()

    @patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test-openai"}, clear=True)
    @patch("llm_client.OpenAI")
    @patch("llm_client.Anthropic", None)  # Simulate Anthropic missing/failing
    def test_init_openai_fallback(self, mock_openai):
        """Test initialization with OpenAI fallback"""
        # Ensure Anthropic is not set in env to force fallback logic if we were relying on that,
        # but here we mock the class to be None.

        # However, my code checks environment variables to decide.
        # If ANTHROPIC_API_KEY is missing, it checks OPENAI_API_KEY.

        client = LlmClient()
        assert client.provider == "openai"
        mock_openai.assert_called_once()

    @patch.dict(os.environ, {}, clear=True)
    def test_init_no_keys(self):
        """Test initialization with no keys raises error"""
        with pytest.raises(LlmError, match="No LLM provider configured"):
            LlmClient()

    @patch.dict(os.environ, {"ANTHROPIC_API_KEY": "sk-test"})
    @patch("llm_client.Anthropic")
    def test_complete_anthropic(self, mock_anthropic_cls):
        """Test completion with Anthropic"""
        mock_instance = mock_anthropic_cls.return_value
        mock_response = MagicMock()
        mock_response.content = [MagicMock(text="Hello from Claude")]
        mock_instance.messages.create.return_value = mock_response

        client = LlmClient()
        response = client.complete(
            messages=[{"role": "user", "content": "Hi"}], system="System prompt"
        )

        assert response == "Hello from Claude"
        mock_instance.messages.create.assert_called_once()
        call_kwargs = mock_instance.messages.create.call_args.kwargs
        assert call_kwargs["system"] == "System prompt"
        assert call_kwargs["messages"] == [{"role": "user", "content": "Hi"}]

    @patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test"}, clear=True)
    @patch("llm_client.OpenAI")
    @patch("llm_client.Anthropic", None)
    def test_complete_openai(self, mock_openai_cls):
        """Test completion with OpenAI"""
        mock_instance = mock_openai_cls.return_value
        mock_response = MagicMock()
        mock_response.choices = [MagicMock(message=MagicMock(content="Hello from GPT"))]
        mock_instance.chat.completions.create.return_value = mock_response

        client = LlmClient()
        response = client.complete(
            messages=[{"role": "user", "content": "Hi"}], system="System prompt"
        )

        assert response == "Hello from GPT"
        mock_instance.chat.completions.create.assert_called_once()
        call_kwargs = mock_instance.chat.completions.create.call_args.kwargs
        # OpenAI puts system in messages
        assert call_kwargs["messages"][0] == {
            "role": "system",
            "content": "System prompt",
        }
        assert call_kwargs["messages"][1] == {"role": "user", "content": "Hi"}
