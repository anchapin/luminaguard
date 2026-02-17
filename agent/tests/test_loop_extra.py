
#!/usr/bin/env python3
"""
Additional tests for loop.py to improve coverage
"""

import pytest
from unittest.mock import MagicMock, patch

# Test ExecutionMode
def test_execution_mode_host_value():
    from loop import ExecutionMode
    assert ExecutionMode.HOST.value == "host"

def test_execution_mode_vm_value():
    from loop import ExecutionMode
    assert ExecutionMode.VM.value == "vm"

def test_execution_mode_enum_values():
    from loop import ExecutionMode
    modes = [e.value for e in ExecutionMode]
    assert "host" in modes
    assert "vm" in modes


# Test Session
def test_session_creation():
    from loop import Session, AgentState
    import time
    
    session = Session(
        session_id="test-123",
        created_at=time.time(),
        last_activity=time.time(),
        state=AgentState(messages=[], tools=[], context={}),
        metadata={"key": "value"}
    )
    
    assert session.session_id == "test-123"
    assert session.metadata["key"] == "value"

def test_session_is_expired():
    from loop import Session, AgentState
    import time
    
    session = Session(
        session_id="test-123",
        created_at=time.time() - 7200,
        last_activity=time.time() - 7200,
        state=AgentState(messages=[], tools=[], context={}),
        metadata={}
    )
    
    assert session.is_expired(ttl_seconds=3600) is True

def test_session_not_expired():
    from loop import Session, AgentState
    import time
    
    session = Session(
        session_id="test-123",
        created_at=time.time(),
        last_activity=time.time(),
        state=AgentState(messages=[], tools=[], context={}),
        metadata={}
    )
    
    assert session.is_expired(ttl_seconds=3600) is False

def test_session_update_activity():
    from loop import Session, AgentState
    import time
    
    old_time = time.time() - 100
    session = Session(
        session_id="test-123",
        created_at=old_time,
        last_activity=old_time,
        state=AgentState(messages=[], tools=[], context={}),
        metadata={}
    )
    
    session.update_activity()
    
    assert session.last_activity > old_time


# Test SessionManager
def test_session_manager_creation():
    from loop import SessionManager
    
    manager = SessionManager(ttl_seconds=1800)
    assert manager.ttl_seconds == 1800
    assert len(manager.sessions) == 0

def test_create_session():
    from loop import SessionManager
    
    manager = SessionManager()
    session = manager.create_session("session-1", ["read_file", "write_file"])
    
    assert session.session_id == "session-1"
    assert "read_file" in session.state.tools

def test_get_session_exists():
    from loop import SessionManager
    
    manager = SessionManager()
    created = manager.create_session("session-1", ["tool1"])
    retrieved = manager.get_session("session-1")
    
    assert retrieved is not None
    assert retrieved.session_id == "session-1"

def test_get_session_not_exists():
    from loop import SessionManager
    
    manager = SessionManager()
    result = manager.get_session("non-existent")
    
    assert result is None

def test_remove_session():
    from loop import SessionManager
    
    manager = SessionManager()
    manager.create_session("session-1", ["tool1"])
    
    manager.remove_session("session-1")
    
    assert manager.get_session("session-1") is None

def test_remove_nonexistent_session():
    from loop import SessionManager
    
    manager = SessionManager()
    
    manager.remove_session("non-existent")

def test_cleanup_expired():
    from loop import SessionManager
    import time
    
    manager = SessionManager(ttl_seconds=1)
    
    manager.create_session("session-1", ["tool1"])
    manager.create_session("session-2", ["tool2"])
    
    time.sleep(1.5)
    
    cleaned = manager.cleanup_expired()
    
    assert cleaned == 2
    assert len(manager.sessions) == 0


# Test get_execution_mode
def test_get_execution_mode_host_default():
    from loop import get_execution_mode, ExecutionMode
    import os
    
    old_val = os.environ.pop("LUMINAGUARD_MODE", None)
    
    try:
        mode = get_execution_mode()
        assert mode == ExecutionMode.HOST
    finally:
        if old_val:
            os.environ["LUMINAGUARD_MODE"] = old_val

def test_get_execution_mode_vm():
    from loop import get_execution_mode, ExecutionMode
    import os
    
    old_val = os.environ.get("LUMINAGUARD_MODE")
    os.environ["LUMINAGUARD_MODE"] = "vm"
    
    try:
        mode = get_execution_mode()
        assert mode == ExecutionMode.VM
    finally:
        if old_val is None:
            del os.environ["LUMINAGUARD_MODE"]
        else:
            os.environ["LUMINAGUARD_MODE"] = old_val

def test_get_execution_mode_invalid_fallback():
    from loop import get_execution_mode, ExecutionMode
    import os
    
    old_val = os.environ.get("LUMINAGUARD_MODE")
    os.environ["LUMINAGUARD_MODE"] = "invalid_mode"
    
    try:
        mode = get_execution_mode()
        assert mode == ExecutionMode.HOST
    finally:
        if old_val is None:
            del os.environ["LUMINAGUARD_MODE"]
        else:
            os.environ["LUMINAGUARD_MODE"] = old_val


# Test execute_tool_vm
def test_execute_tool_vm_success():
    from loop import execute_tool_vm, ToolCall, ActionKind
    
    mock_vsock = MagicMock()
    mock_vsock.execute_tool.return_value = {"result": "success"}
    
    call = ToolCall("read_file", {"path": "/tmp/test"}, ActionKind.GREEN)
    result = execute_tool_vm(call, mock_vsock)
    
    assert result["status"] == "ok"
    assert result["action_kind"] == "green"

def test_execute_tool_vm_error():
    from loop import execute_tool_vm, ToolCall, ActionKind
    
    mock_vsock = MagicMock()
    mock_vsock.execute_tool.side_effect = Exception("Connection failed")
    
    call = ToolCall("read_file", {"path": "/tmp/test"}, ActionKind.GREEN)
    result = execute_tool_vm(call, mock_vsock)
    
    assert result["status"] == "error"
    assert "Connection failed" in result["error"]


# Test present_diff_card fallback
def test_present_diff_card_red_approves_yes():
    from loop import present_diff_card, ToolCall, ActionKind
    
    with patch('builtins.input', return_value='y'):
        action = ToolCall("delete_file", {"path": "test.txt"}, ActionKind.RED)
        result = present_diff_card(action)
        assert result is True

def test_present_diff_card_red_rejects_no():
    from loop import present_diff_card, ToolCall, ActionKind
    
    with patch('builtins.input', return_value='n'):
        action = ToolCall("delete_file", {"path": "test.txt"}, ActionKind.RED)
        result = present_diff_card(action)
        assert result is False


# Test run_loop with execution mode
def test_run_loop_with_vm_mode():
    from loop import run_loop, AgentState, ExecutionMode
    
    mock_vsock = MagicMock()
    mock_vsock.execute_tool.return_value = {"result": "success"}
    
    state = run_loop(
        "test task", 
        ["read_file"], 
        vsock_client=mock_vsock,
        execution_mode=ExecutionMode.VM
    )
    
    assert isinstance(state, AgentState)
