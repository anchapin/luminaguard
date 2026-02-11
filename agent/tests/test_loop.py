#!/usr/bin/env python3
"""
Tests for IronClaw Agent reasoning loop

This test suite validates:
- Core reasoning logic
- State management
- Tool execution
- Property-based tests (Hypothesis)
"""

import pytest
from hypothesis import given, strategies as st
from unittest.mock import MagicMock, Mock, patch

from loop import (
    AgentState,
    think,
    execute_tool,
    run_loop,
    ActionKind,
    ToolCall,
    determine_action_kind,
    construct_system_prompt,
    parse_response,
    present_diff_card,
)
from mcp_client import Tool, McpClient
from llm_client import LlmClient


class MockMcpClient:
    """Mock MCP client for testing"""

    def call_tool(self, name: str, arguments: dict) -> dict:
        """Mock tool call that returns success"""
        return {"result": f"Mock execution of {name}", "content": []}

    def list_tools(self):
        return [
            Tool(
                name="read_file",
                description="Read file",
                input_schema={"path": "string"},
            ),
            Tool(
                name="write_file",
                description="Write file",
                input_schema={"path": "string", "content": "string"},
            ),
        ]


class MockLlmClient:
    """Mock LLM client for testing"""

    def __init__(self, response=""):
        self.response = response

    def complete(self, messages, system="", temperature=0.0, max_tokens=4096):
        return self.response


class TestAgentState:
    """Tests for AgentState dataclass"""

    def test_state_initialization(self):
        """Test that state initializes correctly"""
        state = AgentState(messages=[], tools=[], context={})
        assert state.messages == []
        assert state.tools == []
        assert state.context == {}

    def test_add_message(self):
        """Test adding messages to state"""
        state = AgentState(messages=[], tools=[], context={})
        state.add_message("user", "Hello")
        assert len(state.messages) == 1
        assert state.messages[0]["role"] == "user"
        assert state.messages[0]["content"] == "Hello"

    def test_add_message_preserves_history(self):
        """Test that add_message preserves existing messages"""
        state = AgentState(messages=[], tools=[], context={})
        state.add_message("user", "First")
        state.add_message("assistant", "Second")
        assert len(state.messages) == 2
        assert state.messages[0]["content"] == "First"
        assert state.messages[1]["content"] == "Second"


class TestHelperFunctions:
    """Tests for helper functions"""

    def test_determine_action_kind(self):
        assert determine_action_kind("read_file") == ActionKind.GREEN
        assert determine_action_kind("write_file") == ActionKind.RED
        assert determine_action_kind("list_files") == ActionKind.GREEN
        assert determine_action_kind("delete_file") == ActionKind.RED
        assert determine_action_kind("unknown_tool") == ActionKind.RED

    def test_construct_system_prompt(self):
        tools = [
            Tool(
                name="read_file",
                description="Read file",
                input_schema={"path": "string"},
            ),
            Tool(
                name="write_file",
                description="Write file",
                input_schema={"path": "string", "content": "string"},
            ),
        ]
        prompt = construct_system_prompt(tools)
        assert "<tool_definition>" in prompt
        assert "<name>read_file</name>" in prompt
        assert "write_file" in prompt

    def test_parse_response_with_call(self):
        response = """
        <thought>I should read the file.</thought>
        <function_calls>
        <function_call name="read_file">
        <arg name="path">/tmp/test.txt</arg>
        </function_call>
        </function_calls>
        """
        thought, call = parse_response(response)
        assert thought == "I should read the file."
        assert call.name == "read_file"
        assert call.arguments["path"] == "/tmp/test.txt"
        assert call.action_kind == ActionKind.GREEN

    def test_parse_response_without_call(self):
        response = """<thought>Just thinking.</thought>"""
        thought, call = parse_response(response)
        assert thought == "Just thinking."
        assert call is None

    def test_present_diff_card(self):
        """Test present_diff_card format"""
        call = ToolCall(
            name="delete_file",
            arguments={"path": "file.txt"},
            action_kind=ActionKind.RED,
        )
        card = present_diff_card(call)
        assert "delete_file" in card
        assert "Approve?" in card


class TestThink:
    """Tests for the think() function"""

    def test_think_returns_tool_call(self):
        """Test that think() returns ToolCall when LLM requests one"""
        state = AgentState(messages=[], tools=[], context={})
        llm_response = """
        <thought>Thinking...</thought>
        <function_calls>
        <function_call name="read_file">
        <arg name="path">test.txt</arg>
        </function_call>
        </function_calls>
        """
        mock_llm = MockLlmClient(response=llm_response)

        result = think(state, mock_llm)
        assert isinstance(result, ToolCall)
        assert result.name == "read_file"
        # Verify state updated with thought
        assert state.messages[-1]["role"] == "assistant"
        assert state.messages[-1]["content"] == llm_response

    def test_think_returns_none_when_task_complete(self):
        """Test think() returns None when no tool call"""
        state = AgentState(messages=[], tools=[], context={})
        llm_response = """<thought>Task complete.</thought>"""
        mock_llm = MockLlmClient(response=llm_response)

        result = think(state, mock_llm)
        assert result is None
        # Verify state updated with thought
        assert state.messages[-1]["role"] == "assistant"
        assert state.messages[-1]["content"] == llm_response

    def test_think_without_llm_client(self):
        """Test think() returns None without LLM client"""
        state = AgentState(messages=[], tools=[], context={})
        result = think(state, None)
        assert result is None


class TestExecuteTool:
    """Tests for the execute_tool() function"""

    def test_execute_tool_returns_dict(self):
        """Test that execute_tool returns a dict"""
        call = ToolCall(
            name="test_tool", arguments={"arg1": "value1"}, action_kind=ActionKind.GREEN
        )
        mock_client = MockMcpClient()
        result = execute_tool(call, mock_client)
        assert isinstance(result, dict)

    def test_execute_tool_has_status(self):
        """Test that execute_tool result has status key"""
        call = ToolCall(name="test_tool", arguments={}, action_kind=ActionKind.GREEN)
        mock_client = MockMcpClient()
        result = execute_tool(call, mock_client)
        assert "status" in result


class TestRunLoop:
    """Tests for the run_loop() function"""

    def test_run_loop_returns_state(self):
        """Test that run_loop returns an AgentState"""
        # Mock think to return None immediately to avoid infinite loop or LLM calls
        with patch("loop.think", return_value=None):
            state = run_loop("Test task", tools=[])
            assert isinstance(state, AgentState)

    def test_run_loop_initializes_with_user_message(self):
        """Test that run_loop adds user message to state"""
        task = "Test task"
        with patch("loop.think", return_value=None):
            state = run_loop(task, tools=[])
            assert len(state.messages) >= 1
            assert state.messages[0]["role"] == "user"
            assert state.messages[0]["content"] == task

    def test_run_loop_includes_tools_in_state(self):
        """Test that run_loop includes tools in state"""
        tools = [Tool(name="test", description="desc", input_schema={})]
        with patch("loop.think", return_value=None):
            state = run_loop("Test task", tools=tools)
            assert state.tools == tools

    def test_run_loop_fetches_tools_from_mcp(self):
        """Test run_loop uses MCP client to fetch tools"""
        mock_mcp = MockMcpClient()
        with patch("loop.think", return_value=None):
            state = run_loop("Test task", mcp_client=mock_mcp)
            assert len(state.tools) == 2
            assert state.tools[0].name == "read_file"


class TestToolCall:
    """Tests for ToolCall dataclass"""

    def test_tool_call_creation(self):
        """Test creating a ToolCall"""
        call = ToolCall(
            name="test_tool", arguments={"arg": "value"}, action_kind=ActionKind.GREEN
        )
        assert call.name == "test_tool"
        assert call.arguments == {"arg": "value"}
        assert call.action_kind == ActionKind.GREEN


# Property-based tests using Hypothesis


class TestPropertyBased:
    """Property-based tests for core functions"""

    @given(st.text())
    def test_state_handles_various_message_content(self, content):
        """Property test: AgentState should handle any content string"""
        state = AgentState(messages=[], tools=[], context={})
        state.add_message("user", content)
        assert state.messages[0]["content"] == content

    @given(st.lists(st.text()))
    def test_state_handles_various_message_lists(self, messages):
        """Property test: AgentState should handle any list of messages"""
        state = AgentState(
            messages=[{"role": "user", "content": m} for m in messages],
            tools=[],
            context={},
        )
        # Should not crash
        assert isinstance(state.messages, list)
        assert len(state.messages) == len(messages)
