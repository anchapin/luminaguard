#!/usr/bin/env python3
"""
Tests for LuminaGuard Agent reasoning loop

This test suite validates:
- Core reasoning logic
- State management
- Tool execution
- Property-based tests (Hypothesis)
"""

import pytest
from hypothesis import given, strategies as st
from unittest.mock import MagicMock, Mock

from loop import AgentState, think, execute_tool, run_loop, ActionKind, ToolCall


class MockMcpClient:
    """Mock MCP client for testing"""

    def call_tool(self, name: str, arguments: dict) -> dict:
        """Mock tool call that returns success"""
        return {"result": f"Mock execution of {name}", "content": []}


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


class TestThink:
    """Tests for the think() function"""

    def test_think_returns_optional_tool_call(self):
        """Test that think() returns None or ToolCall"""
        state = AgentState(messages=[], tools=[], context={})
        result = think(state)
        assert result is None or isinstance(result, ToolCall)

    def test_think_with_empty_state(self):
        """Test think() with empty state"""
        state = AgentState(messages=[], tools=[], context={})
        result = think(state)
        # Currently returns None (placeholder)
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

    def test_execute_tool_with_green_action(self):
        """Test execute_tool with green action"""
        call = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/file.txt"},
            action_kind=ActionKind.GREEN,
        )
        mock_client = MockMcpClient()
        result = execute_tool(call, mock_client)
        assert result["status"] == "ok"

    def test_execute_tool_with_red_action(self):
        """Test execute_tool with red action"""
        call = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/file.txt"},
            action_kind=ActionKind.RED,
        )
        mock_client = MockMcpClient()
        result = execute_tool(call, mock_client)
        assert result["status"] == "ok"


class TestRunLoop:
    """Tests for the run_loop() function"""

    def test_run_loop_returns_state(self):
        """Test that run_loop returns an AgentState"""
        state = run_loop("Test task", ["tool1", "tool2"])
        assert isinstance(state, AgentState)

    def test_run_loop_initializes_with_user_message(self):
        """Test that run_loop adds user message to state"""
        task = "Test task"
        state = run_loop(task, [])
        assert len(state.messages) >= 1
        assert state.messages[0]["role"] == "user"
        assert state.messages[0]["content"] == task

    def test_run_loop_includes_tools_in_state(self):
        """Test that run_loop includes tools in state"""
        tools = ["read_file", "write_file", "search"]
        state = run_loop("Test task", tools)
        assert state.tools == tools


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

    def test_tool_call_with_green_action(self):
        """Test ToolCall with green action"""
        call = ToolCall(name="read_file", arguments={}, action_kind=ActionKind.GREEN)
        assert call.action_kind == ActionKind.GREEN

    def test_tool_call_with_red_action(self):
        """Test ToolCall with red action"""
        call = ToolCall(name="delete_file", arguments={}, action_kind=ActionKind.RED)
        assert call.action_kind == ActionKind.RED


class TestActionKind:
    """Tests for ActionKind enum"""

    def test_green_action_value(self):
        """Test GREEN action value"""
        assert ActionKind.GREEN.value == "green"

    def test_red_action_value(self):
        """Test RED action value"""
        assert ActionKind.RED.value == "red"


# Property-based tests using Hypothesis


class TestPropertyBased:
    """Property-based tests for core functions"""

    @given(st.text())
    def test_think_handles_various_tasks(self, task):
        """Property test: think() should handle any task string"""
        state = AgentState(
            messages=[{"role": "user", "content": task}], tools=[], context={}
        )
        result = think(state)
        # Should not crash, should return None or ToolCall
        assert result is None or isinstance(result, ToolCall)

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

    @given(st.dictionaries(st.text(), st.text()))
    def test_state_handles_various_contexts(self, context):
        """Property test: AgentState should handle any context dict"""
        state = AgentState(messages=[], tools=[], context=context)
        # Should not crash
        assert isinstance(state.context, dict)

    @given(st.lists(st.text()))
    def test_run_loop_with_various_tools(self, tools):
        """Property test: run_loop should handle any list of tools"""
        state = run_loop("Test task", tools)
        assert state.tools == tools
