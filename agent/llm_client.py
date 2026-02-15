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
"""

from __future__ import annotations

import json
from abc import ABC, abstractmethod
from typing import Any, Dict, List, Optional
from dataclasses import dataclass
from enum import Enum

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
            # Fall back to complete on error
            return LLMResponse(
                None,
                {},
                f"LLM error: {str(e)}. Task stopped.",
                is_complete=True,
            )

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
