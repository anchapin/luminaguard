#!/usr/bin/env python3
"""
Additional tests for loop.py to improve coverage
"""

import pytest
import sys
import os
from unittest.mock import patch, MagicMock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from loop import (
    ToolCall,
    ActionKind,
    ExecutionMode,
    Style,
    AgentState,
    determine_action_kind,
    present_diff_card,
    think,
    execute_tool,
    get_execution_mode,
    run_loop,
    _get_risk_display,
    GREEN_KEYWORDS,
    RED_KEYWORDS,
)


class TestLoopExecutionMode:
    """Tests for ExecutionMode enum"""

    def test_execution_mode_values(self):
        """Test ExecutionMode enum values"""
        assert ExecutionMode.HOST.value == "host"
        assert ExecutionMode.VM.value == "vm"


class TestStyle:
    """Tests for Style class"""

    def test_style_bold(self):
        """Test Style.bold method"""
        result = Style.bold("test")
        assert "test" in result

    def test_style_cyan(self):
        """Test Style.cyan method"""
        result = Style.cyan("test")
        assert "test" in result


class TestAgentState:
    """Tests for AgentState class"""

    def test_agent_state_init(self):
        """Test AgentState initialization"""
        state = AgentState(
            messages=[{"role": "user", "content": "hello"}],
            tools=["read_file", "write_file"],
            context={"mode": "host"}
        )
        
        assert len(state.messages) == 1
        assert len(state.tools) == 2
        assert state.context["mode"] == "host"

    def test_agent_state_add_message(self):
        """Test adding messages to AgentState"""
        state = AgentState(messages=[], tools=[], context={})
        
        state.add_message("user", "Hello")
        state.add_message("assistant", "Hi there")
        
        assert len(state.messages) == 2
        assert state.messages[0]["role"] == "user"
        assert state.messages[1]["content"] == "Hi there"


class TestDetermineActionKind:
    """Tests for determine_action_kind function"""

    def test_determine_action_kind_read(self):
        """Test action kind for read operations"""
        assert determine_action_kind("read file") == ActionKind.GREEN

    def test_determine_action_kind_list(self):
        """Test action kind for list operations"""
        assert determine_action_kind("list files") == ActionKind.GREEN

    def test_determine_action_kind_search(self):
        """Test action kind for search operations"""
        assert determine_action_kind("search logs") == ActionKind.GREEN

    def test_determine_action_kind_check(self):
        """Test action kind for check operations"""
        assert determine_action_kind("check status") == ActionKind.GREEN

    def test_determine_action_kind_get(self):
        """Test action kind for get operations"""
        assert determine_action_kind("get info") == ActionKind.GREEN

    def test_determine_action_kind_show(self):
        """Test action kind for show operations"""
        assert determine_action_kind("show config") == ActionKind.GREEN

    def test_determine_action_kind_view(self):
        """Test action kind for view operations"""
        assert determine_action_kind("view file") == ActionKind.GREEN

    def test_determine_action_kind_find(self):
        """Test action kind for find operations"""
        assert determine_action_kind("find file") == ActionKind.GREEN

    def test_determine_action_kind_delete(self):
        """Test action kind for delete operations"""
        assert determine_action_kind("delete file") == ActionKind.RED

    def test_determine_action_kind_remove(self):
        """Test action kind for remove operations"""
        assert determine_action_kind("remove file") == ActionKind.RED

    def test_determine_action_kind_write(self):
        """Test action kind for write operations"""
        assert determine_action_kind("write file") == ActionKind.RED

    def test_determine_action_kind_edit(self):
        """Test action kind for edit operations"""
        assert determine_action_kind("edit file") == ActionKind.RED

    def test_determine_action_kind_modify(self):
        """Test action kind for modify operations"""
        assert determine_action_kind("modify config") == ActionKind.RED

    def test_determine_action_kind_create(self):
        """Test action kind for create operations"""
        assert determine_action_kind("create file") == ActionKind.RED

    def test_determine_action_kind_update(self):
        """Test action kind for update operations"""
        assert determine_action_kind("update config") == ActionKind.RED

    def test_determine_action_kind_change(self):
        """Test action kind for change operations"""
        assert determine_action_kind("change password") == ActionKind.RED

    def test_determine_action_kind_send(self):
        """Test action kind for send operations"""
        assert determine_action_kind("send email") == ActionKind.RED

    def test_determine_action_kind_post(self):
        """Test action kind for post operations"""
        assert determine_action_kind("post data") == ActionKind.RED

    def test_determine_action_kind_transfer(self):
        """Test action kind for transfer operations"""
        assert determine_action_kind("transfer funds") == ActionKind.RED

    def test_determine_action_kind_execute(self):
        """Test action kind for execute operations"""
        assert determine_action_kind("execute command") == ActionKind.RED

    def test_determine_action_kind_run(self):
        """Test action kind for run operations"""
        assert determine_action_kind("run script") == ActionKind.RED

    def test_determine_action_kind_deploy(self):
        """Test action kind for deploy operations"""
        assert determine_action_kind("deploy app") == ActionKind.RED

    def test_determine_action_kind_install(self):
        """Test action kind for install operations"""
        assert determine_action_kind("install package") == ActionKind.RED

    def test_determine_action_kind_uninstall(self):
        """Test action kind for uninstall operations"""
        assert determine_action_kind("uninstall app") == ActionKind.RED

    def test_determine_action_kind_commit(self):
        """Test action kind for commit operations"""
        assert determine_action_kind("commit changes") == ActionKind.RED

    def test_determine_action_kind_push(self):
        """Test action kind for push operations"""
        assert determine_action_kind("push to remote") == ActionKind.RED

    def test_determine_action_kind_publish(self):
        """Test action kind for publish operations"""
        assert determine_action_kind("publish package") == ActionKind.RED

    def test_determine_action_kind_mixed_keywords(self):
        """Test action kind when both green and red keywords present"""
        # Red should take precedence
        result = determine_action_kind("read and delete file")
        assert result == ActionKind.RED


class TestThink:
    """Tests for think function"""

    def test_think_with_no_messages(self):
        """Test think with no messages in state"""
        state = AgentState(
            messages=[],
            tools=[],
            context={}
        )
        
        result = think(state)
        # With no messages, returns None due to ImportError fallback
        assert result is None


class TestExecuteTool:
    """Tests for execute_tool function"""

    def test_execute_tool_success(self):
        """Test execute_tool with successful result"""
        mock_client = MagicMock()
        mock_client.call_tool.return_value = {"status": "ok", "data": "test"}

        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN
        )

        result = execute_tool(action, mock_client)

        assert result["status"] == "ok"
        assert result["action_kind"] == "green"

    def test_execute_tool_exception(self):
        """Test execute_tool with exception"""
        mock_client = MagicMock()
        mock_client.call_tool.side_effect = RuntimeError("Tool failed")

        action = ToolCall(
            name="write_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )

        result = execute_tool(action, mock_client)

        assert result["status"] == "error"
        assert "Tool failed" in result["error"]


class TestGetExecutionMode:
    """Tests for get_execution_mode function"""

    def test_get_execution_mode_host(self):
        """Test default execution mode is host"""
        with patch.dict(os.environ, {"LUMINAGUARD_MODE": "host"}):
            result = get_execution_mode()
            assert result == ExecutionMode.HOST

    def test_get_execution_mode_vm(self):
        """Test VM execution mode"""
        with patch.dict(os.environ, {"LUMINAGUARD_MODE": "vm"}):
            result = get_execution_mode()
            assert result == ExecutionMode.VM

    def test_get_execution_mode_invalid(self):
        """Test invalid mode defaults to host"""
        with patch.dict(os.environ, {"LUMINAGUARD_MODE": "invalid"}):
            result = get_execution_mode()
            assert result == ExecutionMode.HOST

    def test_get_execution_mode_not_set(self):
        """Test when mode is not set"""
        # Remove the env var if set
        original = os.environ.pop("LUMINAGUARD_MODE", None)
        try:
            result = get_execution_mode()
            assert result == ExecutionMode.HOST
        finally:
            if original is not None:
                os.environ["LUMINAGUARD_MODE"] = original


class TestGetRiskDisplay:
    """Tests for _get_risk_display function"""

    def test_get_risk_display_green(self):
        """Test risk display for green action"""
        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN
        )

        result = _get_risk_display(action)
        assert "GREEN" in result

    def test_get_risk_display_delete(self):
        """Test risk display for delete action"""
        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )

        result = _get_risk_display(action)
        assert "CRITICAL" in result

    def test_get_risk_display_remove(self):
        """Test risk display for remove action"""
        action = ToolCall(
            name="remove_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )

        result = _get_risk_display(action)
        assert "CRITICAL" in result

    def test_get_risk_display_write(self):
        """Test risk display for write action"""
        action = ToolCall(
            name="write_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )

        result = _get_risk_display(action)
        assert "HIGH" in result

    def test_get_risk_display_edit(self):
        """Test risk display for edit action"""
        action = ToolCall(
            name="edit_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )

        result = _get_risk_display(action)
        assert "HIGH" in result

    def test_get_risk_display_default(self):
        """Test risk display for default case"""
        action = ToolCall(
            name="some_action",
            arguments={},
            action_kind=ActionKind.RED
        )

        result = _get_risk_display(action)
        assert "MEDIUM" in result


class TestToolCall:
    """Tests for ToolCall dataclass"""

    def test_tool_call_creation(self):
        """Test creating a ToolCall"""
        action = ToolCall(
            name="write_file",
            arguments={"path": "/tmp/test.txt", "content": "hello"},
            action_kind=ActionKind.RED
        )

        assert action.name == "write_file"
        assert action.arguments["path"] == "/tmp/test.txt"
        assert action.action_kind == ActionKind.RED


class TestGreenKeywords:
    """Tests for GREEN_KEYWORDS list"""

    def test_green_keywords_contain_common(self):
        """Test that GREEN_KEYWORDS contains common safe keywords"""
        common_green = ["read", "list", "search", "check", "get", "show", "view", "find"]
        for keyword in common_green:
            assert keyword in GREEN_KEYWORDS


class TestRedKeywords:
    """Tests for RED_KEYWORDS list"""

    def test_red_keywords_contain_destructive(self):
        """Test that RED_KEYWORDS contains common destructive keywords"""
        common_red = ["delete", "remove", "write", "edit", "execute", "run", "send"]
        for keyword in common_red:
            assert keyword in RED_KEYWORDS
