#!/usr/bin/env python3
"""
LLM Client - Provider-agnostic interface for LLM calls
======================================================

This module provides a unified interface for calling different LLM providers
(currently Anthropic and OpenAI). It handles authentication via environment
variables and provides a consistent completion API.

Providers:
- Anthropic (Default): Uses `ANTHROPIC_API_KEY`
- OpenAI (Fallback): Uses `OPENAI_API_KEY`

Usage:
    >>> client = LlmClient()
    >>> response = client.complete(
    ...     messages=[{"role": "user", "content": "Hello"}],
    ...     system="You are a helpful assistant."
    ... )
    >>> print(response)
"""

import os
from typing import List, Dict, Optional, Union

try:
    from anthropic import Anthropic, AnthropicError
except ImportError:
    Anthropic = None  # type: ignore

try:
    from openai import OpenAI, OpenAIError
except ImportError:
    OpenAI = None  # type: ignore


class LlmError(Exception):
    """Generic error for LLM failures"""
    pass


class LlmClient:
    """
    Unified client for LLM providers.

    Tries to initialize Anthropic first, then OpenAI.
    Raises LlmError if no provider is available or configured.
    """

    def __init__(self, provider: Optional[str] = None):
        """
        Initialize the LLM client.

        Args:
            provider: Explicitly request 'anthropic' or 'openai'.
                      If None, tries Anthropic then OpenAI.
        """
        self.provider = provider
        self.client: Union[Anthropic, OpenAI, None] = None
        self.model_name: str = ""

        # Auto-detect or use requested provider
        if self.provider == "anthropic" or (not self.provider and os.environ.get("ANTHROPIC_API_KEY")):
            self._init_anthropic()
        elif self.provider == "openai" or (not self.provider and os.environ.get("OPENAI_API_KEY")):
            self._init_openai()
        else:
            raise LlmError(
                "No LLM provider configured. Please set ANTHROPIC_API_KEY or OPENAI_API_KEY."
            )

    def _init_anthropic(self):
        """Initialize Anthropic client"""
        if not Anthropic:
            raise LlmError("Anthropic package not installed. Run `pip install anthropic`.")

        api_key = os.environ.get("ANTHROPIC_API_KEY")
        if not api_key:
            raise LlmError("ANTHROPIC_API_KEY not found in environment.")

        try:
            self.client = Anthropic(api_key=api_key)
            self.provider = "anthropic"
            self.model_name = "claude-3-5-sonnet-20241022"  # Default to latest Sonnet
        except Exception as e:
            raise LlmError(f"Failed to initialize Anthropic client: {e}")

    def _init_openai(self):
        """Initialize OpenAI client"""
        if not OpenAI:
            raise LlmError("OpenAI package not installed. Run `pip install openai`.")

        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise LlmError("OPENAI_API_KEY not found in environment.")

        try:
            self.client = OpenAI(api_key=api_key)
            self.provider = "openai"
            self.model_name = "gpt-4-turbo-preview"  # Default to GPT-4 Turbo
        except Exception as e:
            raise LlmError(f"Failed to initialize OpenAI client: {e}")

    def complete(
        self,
        messages: List[Dict[str, str]],
        system: str = "",
        temperature: float = 0.0,
        max_tokens: int = 4096
    ) -> str:
        """
        Generate a completion from the LLM.

        Args:
            messages: List of message dicts (role, content)
            system: System prompt
            temperature: Sampling temperature (0.0 to 1.0)
            max_tokens: Maximum tokens to generate

        Returns:
            Generated text content

        Raises:
            LlmError: If the API call fails
        """
        if not self.client:
            raise LlmError("Client not initialized")

        try:
            if self.provider == "anthropic":
                return self._complete_anthropic(messages, system, temperature, max_tokens)
            elif self.provider == "openai":
                return self._complete_openai(messages, system, temperature, max_tokens)
            else:
                raise LlmError(f"Unknown provider: {self.provider}")
        except Exception as e:
            raise LlmError(f"LLM completion failed: {e}") from e

    def _complete_anthropic(
        self,
        messages: List[Dict[str, str]],
        system: str,
        temperature: float,
        max_tokens: int
    ) -> str:
        """Internal method for Anthropic completion"""
        # Anthropic doesn't support 'system' role in messages list, it's a separate param
        # Also ensure roles are correct (user, assistant)

        response = self.client.messages.create(
            model=self.model_name,
            max_tokens=max_tokens,
            temperature=temperature,
            system=system,
            messages=messages
        )

        if response.content and len(response.content) > 0:
            return response.content[0].text
        return ""

    def _complete_openai(
        self,
        messages: List[Dict[str, str]],
        system: str,
        temperature: float,
        max_tokens: int
    ) -> str:
        """Internal method for OpenAI completion"""
        # OpenAI puts system prompt in messages list
        full_messages = [{"role": "system", "content": system}] + messages

        response = self.client.chat.completions.create(
            model=self.model_name,
            messages=full_messages,
            temperature=temperature,
            max_tokens=max_tokens
        )

        return response.choices[0].message.content or ""

if __name__ == "__main__":
    # Simple test if run directly
    try:
        print(f"Initializing LLM client...")
        client = LlmClient()
        print(f"Provider: {client.provider}")
        print(f"Model: {client.model_name}")

        # Only run actual call if key is present (don't fail in CI/CD without keys)
        if (client.provider == "anthropic" and os.environ.get("ANTHROPIC_API_KEY")) or \
           (client.provider == "openai" and os.environ.get("OPENAI_API_KEY")):
            print("Sending test request...")
            response = client.complete(
                messages=[{"role": "user", "content": "Say hello!"}],
                system="You are a test bot."
            )
            print(f"Response: {response}")
        else:
            print("Skipping API call (no keys configured)")

    except Exception as e:
        print(f"Error: {e}")
