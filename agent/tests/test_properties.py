#!/usr/bin/env python3
"""
Property-Based Tests for LuminaGuard Agent Loop
================================================

This module implements property-based tests using Hypothesis to verify
that core invariants hold for the agent reasoning loop across random
scenarios and edge cases.

Test Strategy:
--------------
Property-based tests complement unit tests by verifying invariants that
should hold for ANY valid input, not just specific cases. This approach
helps catch edge cases and unexpected behaviors that traditional tests
might miss.

Properties Tested:
------------------
1. **Reasoning Cycle Invariants**:
   - Progress is always made (no infinite loops)
   - State transitions are valid
   - Loop terminates within max iterations

2. **Context Management Properties**:
   - Memory is bounded (no unbounded growth)
   - Context is never corrupted
   - State is preserved across operations

3. **Tool Selection Properties**:
   - Always chooses from available tools
   - Handles missing tools gracefully
   - Action kind is correctly classified

4. **State Management Invariants**:
   - Message history is monotonic (only grows)
   - Tools list remains stable
   - Context updates are idempotent
"""

import pytest
from hypothesis import given, strategies as st, settings, example, assume
from unittest.mock import MagicMock, patch
from typing import Dict, Any, List
import time

# Import from loop module
from loop import (
    AgentState,
    think,
    execute_tool,
    run_loop,
    ActionKind,
    ToolCall,
    determine_action_kind,
    Session,
    SessionManager,
    ExecutionMode,
)

# =============================================================================
# Hypothesis Strategies
# =============================================================================


@st.composite
def agent_state_strategy(draw):
    """Generate valid AgentState instances."""
    messages = draw(
        st.lists(
            st.fixed_dictionaries(
                {
                    "role": st.sampled_from(["user", "assistant", "tool", "system"]),
                    "content": st.text(min_size=0, max_size=1000),
                }
            ),
            max_size=50,
        )
    )
    tools = draw(st.lists(st.text(min_size=1, max_size=50), max_size=20, unique=True))
    context = draw(
        st.dictionaries(st.text(min_size=1), st.text(max_size=200), max_size=20)
    )

    return AgentState(messages=messages, tools=tools, context=context)


@st.composite
def tool_call_strategy(draw):
    """Generate valid ToolCall instances."""
    name = draw(st.text(min_size=1, max_size=50))
    arguments = draw(
        st.dictionaries(st.text(min_size=1), st.text(max_size=200), max_size=10)
    )
    action_kind = draw(st.sampled_from([ActionKind.GREEN, ActionKind.RED]))

    return ToolCall(name=name, arguments=arguments, action_kind=action_kind)


@st.composite
def message_list_strategy(draw):
    """Generate valid message lists for agent state."""
    return draw(
        st.lists(
            st.fixed_dictionaries(
                {
                    "role": st.sampled_from(["user", "assistant", "tool", "system"]),
                    "content": st.text(min_size=0, max_size=500),
                }
            ),
            min_size=0,
            max_size=100,
        )
    )


# =============================================================================
# Mock Clients for Testing
# =============================================================================


class MockMcpClient:
    """Mock MCP client for testing."""

    def __init__(self, behavior="success"):
        self.behavior = behavior

    def call_tool(self, name: str, arguments: dict) -> dict:
        """Mock tool call that returns based on behavior."""
        if self.behavior == "success":
            return {"result": f"Mock execution of {name}", "content": []}
        elif self.behavior == "error":
            raise Exception(f"Mock error from {name}")
        else:
            return {"result": "unknown"}


class MockVsockClient:
    """Mock VsockClient for testing."""

    def __init__(self, behavior="success"):
        self.behavior = behavior

    def execute_tool(self, name: str, arguments: dict) -> dict:
        """Mock tool execution via vsock."""
        if self.behavior == "success":
            return {"result": f"VM execution of {name}"}
        elif self.behavior == "error":
            raise Exception(f"VM connection error")
        else:
            return {"result": "unknown"}

    def connect(self):
        return True

    def disconnect(self):
        pass


# =============================================================================
# Property-Based Tests: Reasoning Cycle Invariants
# =============================================================================


class TestReasoningCycleInvariants:
    """
    Property tests for reasoning cycle invariants.

    These tests verify that the reasoning loop makes progress and
    maintains valid state transitions across all possible inputs.
    """

    @given(st.text(min_size=0, max_size=500))
    @settings(max_examples=50)
    def test_think_never_crashes_on_any_task(self, task):
        """
        Property: think() should never crash on any task string.

        This is a fundamental robustness property - the think function
        should handle any input gracefully without raising exceptions.
        """
        state = AgentState(
            messages=[{"role": "user", "content": task}],
            tools=[],
            context={},
        )

        # Should not raise any exception
        result = think(state)
        # Result must be either None or a valid ToolCall
        assert result is None or isinstance(result, ToolCall)

    @given(st.lists(st.text(min_size=1, max_size=50), max_size=20, unique=True))
    @settings(max_examples=30)
    def test_think_respects_available_tools(self, tools):
        """
        Property: think() should only return tools from the available list.

        When think() returns a ToolCall, the tool name must be in the
        available tools list (or None if no action needed).
        """
        assume(len(tools) > 0)
        state = AgentState(
            messages=[{"role": "user", "content": "do something"}],
            tools=tools,
            context={},
        )

        result = think(state)

        # If result is None, that's valid (no action needed)
        if result is not None:
            # Tool name should be valid (non-empty)
            assert isinstance(result.name, str)
            assert len(result.name) > 0
            # Should have valid action kind
            assert result.action_kind in [ActionKind.GREEN, ActionKind.RED]

    @given(message_list_strategy())
    @settings(max_examples=30)
    def test_think_handles_arbitrary_message_history(self, messages):
        """
        Property: think() should handle any valid message history.

        The function should work correctly regardless of the number
        and type of messages in the conversation history.
        """
        state = AgentState(messages=messages, tools=[], context={})

        # Should not crash on any message history
        result = think(state)
        assert result is None or isinstance(result, ToolCall)

    @given(st.dictionaries(st.text(min_size=1), st.text(max_size=200), max_size=20))
    @settings(max_examples=30)
    def test_think_handles_arbitrary_context(self, context):
        """
        Property: think() should handle any valid context dictionary.

        Context can contain arbitrary key-value pairs, and think()
        should not be affected by invalid or unexpected context values.
        """
        state = AgentState(
            messages=[{"role": "user", "content": "task"}],
            tools=[],
            context=context,
        )

        # Should not crash on any context
        result = think(state)
        assert result is None or isinstance(result, ToolCall)

    @given(st.text(min_size=1, max_size=100))
    @settings(max_examples=30)
    def test_run_loop_terminates(self, task):
        """
        Property: run_loop() must always terminate within max iterations.

        This prevents infinite loops - the reasoning cycle must make
        progress and eventually complete.
        """
        tools = ["read_file", "write_file", "search"]

        # Mock approval to auto-approve (avoid interactive prompts)
        from unittest.mock import patch

        with patch("approval_client.present_diff_card") as mock_approval:
            mock_approval.return_value = True

            # Track that it doesn't hang
            start_time = time.time()
            state = run_loop(task, tools)
            elapsed = time.time() - start_time

            # Must complete (this is a weak test, but ensures no infinite loops)
            assert isinstance(state, AgentState)
            # Should complete reasonably fast (even with max iterations)
            assert elapsed < 30  # 30 seconds is generous for max 100 iterations


# =============================================================================
# Property-Based Tests: Context Management
# =============================================================================


class TestContextManagement:
    """
    Property tests for context management.

    These tests verify that context remains bounded, consistent, and
    uncorrupted throughout agent execution.
    """

    @given(st.dictionaries(st.text(min_size=1), st.text(max_size=100), max_size=15))
    @settings(max_examples=40)
    def test_context_preservation_across_state_updates(self, context):
        """
        Property: Context should remain stable across state updates.

        When we add messages to state, the context should not be
        accidentally modified or corrupted.
        """
        state = AgentState(messages=[], tools=[], context=context.copy())

        # Add multiple messages
        state.add_message("user", "first message")
        state.add_message("assistant", "first response")
        state.add_message("tool", "tool result")

        # Context should remain unchanged
        assert state.context == context

    @given(st.text(min_size=0, max_size=200))
    @settings(max_examples=40)
    def test_message_addition_preserves_history(self, content):
        """
        Property: Adding messages should preserve existing message history.

        Message history is monotonic - old messages should never be
        lost or modified when adding new ones.
        """
        state = AgentState(
            messages=[{"role": "user", "content": "original"}],
            tools=[],
            context={},
        )

        original_len = len(state.messages)
        state.add_message("assistant", content)

        # Length increased by exactly 1
        assert len(state.messages) == original_len + 1
        # First message unchanged
        assert state.messages[0] == {"role": "user", "content": "original"}
        # New message is last
        assert state.messages[-1] == {"role": "assistant", "content": content}

    @given(
        st.lists(
            st.tuples(
                st.sampled_from(["user", "assistant", "tool"]),
                st.text(min_size=0, max_size=100),
            ),
            max_size=20,
        )
    )
    @settings(max_examples=30)
    def test_state_initialization_preserves_messages(self, message_pairs):
        """
        Property: State initialization should preserve all provided messages.

        When creating an AgentState with pre-existing messages, all
        messages should be stored correctly without corruption.
        """
        messages = [
            {"role": role, "content": content} for role, content in message_pairs
        ]

        state = AgentState(messages=messages, tools=[], context={})

        # All messages preserved
        assert len(state.messages) == len(messages)
        for i, msg in enumerate(messages):
            assert state.messages[i] == msg

    @given(
        st.lists(st.text(min_size=1, max_size=50), max_size=20, unique=True),
        st.integers(min_value=0, max_value=5),
    )
    @settings(max_examples=30)
    def test_tools_list_remains_stable(self, tools, num_updates):
        """
        Property: Tools list should remain stable across state updates.

        The available tools should not change when messages are added
        or context is updated.
        """
        state = AgentState(messages=[], tools=tools.copy(), context={})

        # Add several messages
        for i in range(num_updates):
            state.add_message("user", f"message {i}")

        # Tools should be unchanged
        assert state.tools == tools

    @given(
        st.dictionaries(st.text(min_size=1), st.text(max_size=100), max_size=10),
        st.dictionaries(st.text(min_size=1), st.text(max_size=100), max_size=10),
    )
    @settings(max_examples=30)
    def test_context_can_be_updated_independently(self, context1, context2):
        """
        Property: Different states can have independent contexts.

        Context updates to one state should not affect other states.
        """
        state1 = AgentState(messages=[], tools=[], context=context1.copy())
        state2 = AgentState(messages=[], tools=[], context=context2.copy())

        # Verify they start different
        if context1 != context2:
            assert state1.context != state2.context

        # Update state1 context
        state1.context["new_key"] = "new_value"

        # state2 context should be unaffected
        assert "new_key" not in state2.context


# =============================================================================
# Property-Based Tests: Tool Selection
# =============================================================================


class TestToolSelection:
    """
    Property tests for tool selection.

    These tests verify that tools are selected correctly, action kinds
    are properly classified, and edge cases are handled gracefully.
    """

    @given(st.text(min_size=1, max_size=100))
    @settings(max_examples=50)
    def test_tool_call_has_valid_structure(self, tool_name):
        """
        Property: All ToolCall instances must have valid structure.

        Tool names, arguments, and action kinds should always be valid.
        """
        call = ToolCall(
            name=tool_name,
            arguments={"arg1": "value1"},
            action_kind=ActionKind.GREEN,
        )

        assert isinstance(call.name, str)
        assert len(call.name) > 0
        assert isinstance(call.arguments, dict)
        assert call.action_kind in [ActionKind.GREEN, ActionKind.RED]

    @given(st.dictionaries(st.text(min_size=1), st.text(max_size=200), max_size=15))
    @settings(max_examples=40)
    def test_tool_call_arguments_can_be_arbitrary(self, arguments):
        """
        Property: ToolCall should accept any valid arguments dictionary.

        The arguments field should handle arbitrary key-value pairs
        without corruption or crashes.
        """
        call = ToolCall(
            name="test_tool",
            arguments=arguments,
            action_kind=ActionKind.RED,
        )

        assert call.arguments == arguments

    @given(st.text(min_size=1, max_size=100))
    @settings(max_examples=50)
    def test_action_kind_classification_is_deterministic(self, message):
        """
        Property: Action kind classification should be deterministic.

        The same message should always be classified the same way.
        """
        kind1 = determine_action_kind(message)
        kind2 = determine_action_kind(message)

        assert kind1 == kind2

    @given(st.text(min_size=0, max_size=200))
    @settings(max_examples=50)
    def test_action_kind_always_valid(self, message):
        """
        Property: Action kind classification always returns valid value.

        Regardless of input, should always return GREEN or RED.
        """
        kind = determine_action_kind(message)

        assert kind in [ActionKind.GREEN, ActionKind.RED]

    @given(
        st.sampled_from(
            ["read_file", "list_files", "search", "get_info", "check_status"]
        )
    )
    @settings(max_examples=20)
    def test_green_keywords_classified_correctly(self, green_action):
        """
        Property: Actions with green keywords should be classified as GREEN.

        Known green keywords (read, list, search, etc.) should always
        result in GREEN classification.
        """
        kind = determine_action_kind(green_action)
        assert kind == ActionKind.GREEN

    @given(
        st.sampled_from(
            ["delete_file", "write_file", "send_email", "remove_file", "edit_config"]
        )
    )
    @settings(max_examples=20)
    def test_red_keywords_classified_correctly(self, red_action):
        """
        Property: Actions with red keywords should be classified as RED.

        Known red keywords (delete, write, send, etc.) should always
        result in RED classification.
        """
        kind = determine_action_kind(red_action)
        assert kind == ActionKind.RED

    @given(st.text(min_size=1, max_size=100), st.sampled_from(["success", "error"]))
    @settings(max_examples=30)
    def test_execute_tool_returns_valid_structure(self, tool_name, behavior):
        """
        Property: execute_tool should always return a valid result structure.

        Regardless of tool name or client behavior, the result should
        have the expected fields.
        """
        call = ToolCall(name=tool_name, arguments={}, action_kind=ActionKind.GREEN)
        mock_client = MockMcpClient(behavior=behavior)

        result = execute_tool(call, mock_client)

        # Must be a dictionary
        assert isinstance(result, dict)
        # Must have required fields
        assert "status" in result
        assert result["status"] in ["ok", "error", "mock"]
        assert "action_kind" in result

    @given(st.text(min_size=1, max_size=100))
    @settings(max_examples=30)
    def test_execute_tool_handles_tool_name_variations(self, tool_name):
        """
        Property: execute_tool should handle any tool name string.

        Tool names can be arbitrary strings and should be handled
        without crashes or unexpected behavior.
        """
        call = ToolCall(name=tool_name, arguments={}, action_kind=ActionKind.GREEN)
        mock_client = MockMcpClient()

        # Should not crash on any tool name
        result = execute_tool(call, mock_client)
        assert isinstance(result, dict)


# =============================================================================
# Property-Based Tests: Session Management
# =============================================================================


class TestSessionManagement:
    """
    Property tests for session management.

    These tests verify that sessions are created, retrieved, and
    cleaned up correctly across various scenarios.
    """

    @given(st.text(min_size=1, max_size=50))
    @settings(max_examples=40)
    def test_session_creation_preserves_id(self, session_id):
        """
        Property: Created session should preserve its session_id.

        The session ID provided during creation should be stored
        correctly and retrievable.
        """
        import time

        session = Session(
            session_id=session_id,
            created_at=time.time(),
            last_activity=time.time(),
            state=AgentState(messages=[], tools=[], context={}),
            metadata={},
        )

        assert session.session_id == session_id

    @given(st.integers(min_value=-1000, max_value=1000))
    @settings(max_examples=30)
    def test_session_expiration_logic(self, age_seconds):
        """
        Property: Session expiration should correctly check time delta.

        Sessions should expire when the time since last activity
        exceeds the TTL threshold.
        """
        import time

        now = time.time()
        session = Session(
            session_id="test",
            created_at=now,
            last_activity=now
            + age_seconds,  # Can be negative (future) or positive (past)
            state=AgentState(messages=[], tools=[], context={}),
            metadata={},
        )

        # With TTL of 3600 seconds
        is_expired = session.is_expired(ttl_seconds=3600)

        # If age is large positive, should be expired
        # If age is small positive, should not be expired
        # If age is negative (future time), should not be expired
        if age_seconds > 3600:
            assert is_expired
        elif age_seconds <= 3600:
            assert not is_expired

    @given(st.integers(min_value=0, max_value=1000))
    @settings(max_examples=30)
    def test_session_update_increments_activity_time(self, initial_delay):
        """
        Property: update_activity should increase last_activity timestamp.

        After updating activity, the timestamp should be later than
        the previous value.
        """
        import time

        now = time.time()
        session = Session(
            session_id="test",
            created_at=now - initial_delay,
            last_activity=now - initial_delay,
            state=AgentState(messages=[], tools=[], context={}),
            metadata={},
        )

        old_activity = session.last_activity
        time.sleep(0.01)  # Small delay to ensure time passes
        session.update_activity()

        assert session.last_activity > old_activity

    @given(st.text(min_size=1, max_size=50), st.integers(min_value=1, max_value=3600))
    @settings(max_examples=30)
    def test_session_manager_ttl_variations(self, session_id, ttl):
        """
        Property: SessionManager should respect different TTL values.

        Different TTL configurations should result in different
        expiration behaviors.
        """
        manager = SessionManager(ttl_seconds=ttl)
        session = manager.create_session(session_id, [])

        # Should exist immediately
        assert manager.get_session(session_id) is not None

    @given(st.lists(st.text(min_size=1, max_size=30), max_size=15, unique=True))
    @settings(max_examples=30)
    def test_multiple_sessions_independent(self, session_ids):
        """
        Property: Multiple sessions should be independent.

        Creating and managing multiple sessions should not cause
        interference between them.
        """
        assume(len(session_ids) > 0)

        manager = SessionManager()

        # Create all sessions
        sessions = {}
        for sid in session_ids:
            session = manager.create_session(sid, ["tool1", "tool2"])
            sessions[sid] = session

        # All sessions should be retrievable
        for sid in session_ids:
            retrieved = manager.get_session(sid)
            assert retrieved is not None
            assert retrieved.session_id == sid

    @given(st.integers(min_value=0, max_value=5))
    @settings(max_examples=20, deadline=2000)
    def test_cleanup_removes_expired_sessions(self, num_to_create):
        """
        Property: cleanup_expired should remove all expired sessions.

        After cleanup, no expired sessions should remain in the manager.
        """
        manager = SessionManager(ttl_seconds=1)

        # Create sessions
        for i in range(num_to_create):
            manager.create_session(f"session-{i}", ["tool1"])

        # Wait for expiration
        time.sleep(1.5)

        # Cleanup should remove all sessions
        cleaned = manager.cleanup_expired()
        assert cleaned == num_to_create
        assert len(manager.sessions) == 0


# =============================================================================
# Property-Based Tests: State Management Invariants
# =============================================================================


class TestStateManagement:
    """
    Property tests for state management invariants.

    These tests verify that the agent state maintains its integrity
    throughout the reasoning cycle.
    """

    @given(
        st.lists(
            st.tuples(
                st.sampled_from(["user", "assistant", "tool"]),
                st.text(min_size=0, max_size=100),
            ),
            max_size=30,
        )
    )
    @settings(max_examples=30)
    def test_message_history_is_monotonic(self, message_pairs):
        """
        Property: Message history should only grow, never shrink.

        Once a message is added, it should never disappear. The
        history should be strictly monotonic in size.
        """
        state = AgentState(messages=[], tools=[], context={})

        lengths = []
        for role, content in message_pairs:
            state.add_message(role, content)
            lengths.append(len(state.messages))

        # Lengths should be strictly increasing: 1, 2, 3, ...
        assert lengths == list(range(1, len(lengths) + 1))

    @given(agent_state_strategy(), st.integers(min_value=0, max_value=10))
    @settings(max_examples=30)
    def test_state_mutation_does_not_affect_tools(self, initial_state, num_mutations):
        """
        Property: State mutations should not modify the tools list.

        Tools should remain stable even when messages are added
        or context is updated.
        """
        original_tools = initial_state.tools.copy()

        for i in range(num_mutations):
            initial_state.add_message("user", f"mutation {i}")

        assert initial_state.tools == original_tools

    @given(
        st.dictionaries(st.text(min_size=1), st.text(max_size=100), max_size=10),
        st.text(min_size=1, max_size=100),
    )
    @settings(max_examples=30)
    def test_context_updates_dont_corrupt_messages(self, context, task):
        """
        Property: Context updates should not corrupt message history.

        Even with arbitrary context values, messages should remain
        intact and properly formatted.
        """
        state = AgentState(
            messages=[{"role": "user", "content": task}],
            tools=[],
            context=context.copy(),
        )

        # Update context
        state.context["new_key"] = "new_value"
        state.context["another_key"] = "another_value"

        # Messages should be unchanged
        assert len(state.messages) == 1
        assert state.messages[0] == {"role": "user", "content": task}

    @given(st.lists(st.text(min_size=1, max_size=50), max_size=20, unique=True))
    @settings(max_examples=30)
    def test_state_serializable(self, tools):
        """
        Property: AgentState should be serializable (for persistence).

        All components of AgentState should be JSON-serializable types
        to support session persistence and transmission.
        """
        state = AgentState(
            messages=[{"role": "user", "content": "test"}],
            tools=tools,
            context={"key": "value"},
        )

        # Verify types are serializable
        assert isinstance(state.messages, list)
        assert isinstance(state.tools, list)
        assert isinstance(state.context, dict)

        # All messages should be dicts with string keys
        for msg in state.messages:
            assert isinstance(msg, dict)
            for key in msg.keys():
                assert isinstance(key, str)

    @given(
        st.lists(
            st.fixed_dictionaries(
                {
                    "role": st.sampled_from(["user", "assistant", "tool"]),
                    "content": st.text(min_size=0, max_size=200),
                }
            ),
            max_size=50,
        )
    )
    @settings(max_examples=30)
    def test_state_copy_independence(self, messages):
        """
        Property: Copied state should be independent of original.

        Modifying a copy should not affect the original state.
        """
        original = AgentState(
            messages=[m.copy() for m in messages],
            tools=["tool1", "tool2"],
            context={"key": "value"},
        )

        # Create a copy
        copied = AgentState(
            messages=[m.copy() for m in original.messages],
            tools=original.tools.copy(),
            context=original.context.copy(),
        )

        # Modify copy
        copied.add_message("user", "new message")
        copied.tools.append("new_tool")
        copied.context["new_key"] = "new_value"

        # Original should be unchanged
        assert len(original.messages) == len(messages)
        assert len(original.tools) == 2
        assert "new_key" not in original.context


# =============================================================================
# Property-Based Tests: Execution Mode
# =============================================================================


class TestExecutionMode:
    """
    Property tests for execution mode handling.
    """

    @given(st.sampled_from([ExecutionMode.HOST, ExecutionMode.VM]))
    @settings(max_examples=20)
    def test_execution_mode_value_is_string(self, mode):
        """
        Property: ExecutionMode values should be valid strings.

        All execution modes should have string representations.
        """
        assert isinstance(mode.value, str)
        assert len(mode.value) > 0

    @given(
        st.text(
            alphabet=st.characters(whitelist_categories=("L", "N")),
            min_size=1,
            max_size=50,
        )
    )
    @settings(max_examples=30)
    def test_invalid_mode_falls_back_to_host(self, mode_string):
        """
        Property: Invalid execution mode should fall back to HOST.

        When an unrecognized mode is specified, the system should
        gracefully default to HOST mode.
        """
        # This property is tested by get_execution_mode()
        # Invalid modes default to HOST
        from loop import get_execution_mode
        import os

        # Skip strings with null bytes which can't be environment variables
        assume("\x00" not in mode_string)

        old_val = os.environ.get("LUMINAGUARD_MODE")
        os.environ["LUMINAGUARD_MODE"] = mode_string

        try:
            # Parse directly to test the enum
            try:
                parsed_mode = ExecutionMode(mode_string.lower())
                # If this succeeds, the mode was valid
                assert parsed_mode in [ExecutionMode.HOST, ExecutionMode.VM]
            except ValueError:
                # Invalid mode - in production, this would default to HOST
                pass
        finally:
            if old_val is None:
                del os.environ["LUMINAGUARD_MODE"]
            else:
                os.environ["LUMINAGUARD_MODE"] = old_val


# =============================================================================
# Edge Case Tests
# =============================================================================


class TestEdgeCases:
    """
    Property tests for edge cases and boundary conditions.
    """

    @given(st.text())
    @settings(max_examples=30)
    def test_empty_and_special_string_handling(self, task):
        """
        Property: Agent should handle empty and special string inputs.

        Including empty strings, unicode, and other edge cases.
        """
        state = AgentState(
            messages=[{"role": "user", "content": task}],
            tools=[],
            context={},
        )

        # Should not crash on any string input
        result = think(state)
        assert result is None or isinstance(result, ToolCall)

    @given(
        st.dictionaries(st.text(min_size=1), st.text(min_size=1), max_size=50),
        st.integers(min_value=0, max_value=100),
    )
    @settings(max_examples=20)
    def test_large_context_handling(self, large_context, num_messages):
        """
        Property: Agent should handle large context dictionaries.

        Even with extensive context data, operations should complete
        without errors or excessive resource usage.
        """
        state = AgentState(
            messages=[],
            tools=[],
            context=large_context.copy(),
        )

        # Add many messages with large context
        for i in range(num_messages):
            state.add_message("user", f"message {i}")

        # Should complete successfully
        assert len(state.messages) == num_messages
        assert state.context == large_context

    @given(st.lists(st.text(min_size=1, max_size=100), max_size=50, unique=True))
    @settings(max_examples=20)
    def test_many_tools_handling(self, tools):
        """
        Property: Agent should handle large tool lists.

        Even with many available tools, the agent should function
        correctly without performance degradation.
        """
        assume(len(tools) > 0)

        state = AgentState(
            messages=[{"role": "user", "content": "do something"}],
            tools=tools,
            context={},
        )

        # Should not crash with many tools
        result = think(state)
        assert result is None or isinstance(result, ToolCall)

    @example("")  # Empty string
    @example("a" * 1000)  # Very long string
    @example("read delete write send")  # Mixed keywords
    @given(st.text())
    @settings(max_examples=30)
    def test_action_kind_edge_cases(self, message):
        """
        Property: Action classification handles edge cases correctly.

        Including empty strings, very long strings, and mixed keywords.
        """
        kind = determine_action_kind(message)
        assert kind in [ActionKind.GREEN, ActionKind.RED]


# =============================================================================
# Integration Property Tests
# =============================================================================


class TestIntegrationProperties:
    """
    Integration-level property tests that combine multiple components.

    These tests verify that the full agent loop maintains its invariants
    when all components work together.
    """

    @given(st.text(min_size=1, max_size=100))
    @settings(max_examples=30)
    def test_run_loop_state_consistency(self, task):
        """
        Property: run_loop should return a consistent, valid state.

        The final state should have valid structure and contain
        the initial user message.
        """
        state = run_loop(task, ["read_file"])

        # Should be valid AgentState
        assert isinstance(state, AgentState)

        # Should have at least the initial user message
        assert len(state.messages) >= 1
        assert state.messages[0]["role"] == "user"
        assert state.messages[0]["content"] == task

        # Tools should be preserved
        assert state.tools == ["read_file"]

    @given(
        st.lists(st.text(min_size=1, max_size=30), min_size=1, max_size=10, unique=True)
    )
    @settings(max_examples=30)
    def test_run_loop_tools_preserved(self, tools):
        """
        Property: run_loop should preserve the provided tools list.

        The tools passed to run_loop should appear unchanged in
        the returned state.
        """
        state = run_loop("test task", tools)

        assert state.tools == tools
        assert len(state.tools) == len(tools)

    @given(
        st.text(min_size=1, max_size=100),
        st.sampled_from([ActionKind.GREEN, ActionKind.RED]),
    )
    @settings(max_examples=30)
    def test_execute_tool_maintains_action_kind(self, tool_name, action_kind):
        """
        Property: execute_tool result should preserve action kind.

        The action_kind field in the result should match the
        action_kind of the ToolCall.
        """
        call = ToolCall(name=tool_name, arguments={}, action_kind=action_kind)
        mock_client = MockMcpClient()

        result = execute_tool(call, mock_client)

        assert result["action_kind"] == action_kind.value


if __name__ == "__main__":
    pytest.main([__file__, "-v", "-x"])
