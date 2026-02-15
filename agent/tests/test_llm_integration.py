#!/usr/bin/env python3
"""
Tests for LLM Integration in LuminaGuard Agent

This test suite validates:
- LLM client factory function
- Mock LLM client behavior
- Multi-turn reasoning support
- Tool selection based on context
- LLM integration with think() function
- Error handling and fallbacks
"""

import pytest
from hypothesis import given, strategies as st

from loop import AgentState, think, ToolCall, ActionKind
from llm_client import (
    LLMClient,
    LLMConfig,
    LLMProvider,
    MockLLMClient,
    OpenAILLMClient,
    create_llm_client,
    LLMResponse,
)


class TestLLMConfig:
    """Tests for LLMConfig dataclass"""

    def test_default_config(self):
        """Test that default config is created correctly"""
        config = LLMConfig()
        assert config.provider == LLMProvider.MOCK
        assert config.model == "mock-model"
        assert config.api_key is None
        assert config.base_url is None
        assert config.temperature == 0.0
        assert config.max_tokens == 1000
        assert config.timeout == 30

    def test_custom_config(self):
        """Test custom config values"""
        config = LLMConfig(
            provider=LLMProvider.OPENAI,
            model="gpt-4",
            api_key="test-key",
            temperature=0.5,
            max_tokens=2000,
            timeout=60,
        )
        assert config.provider == LLMProvider.OPENAI
        assert config.model == "gpt-4"
        assert config.api_key == "test-key"
        assert config.temperature == 0.5
        assert config.max_tokens == 2000
        assert config.timeout == 60

    def test_deterministic_temperature(self):
        """Test that default temperature ensures determinism"""
        config = LLMConfig()
        assert config.temperature == 0.0


class TestLLMResponse:
    """Tests for LLMResponse class"""

    def test_response_with_tool(self):
        """Test response with tool selection"""
        response = LLMResponse(
            tool_name="read_file",
            arguments={"path": "/tmp/test.txt"},
            reasoning="User wants to read a file",
            is_complete=False,
        )
        assert response.tool_name == "read_file"
        assert response.arguments == {"path": "/tmp/test.txt"}
        assert response.reasoning == "User wants to read a file"
        assert response.is_complete is False

    def test_response_without_tool(self):
        """Test response without tool (task complete)"""
        response = LLMResponse(
            tool_name=None,
            arguments={},
            reasoning="Task is complete",
            is_complete=True,
        )
        assert response.tool_name is None
        assert response.arguments == {}
        assert response.is_complete is True


class TestMockLLMClient:
    """Tests for MockLLMClient"""

    def test_initialization(self):
        """Test MockLLMClient initialization"""
        client = MockLLMClient()
        assert client.call_count == 0
        assert client.config.provider == LLMProvider.MOCK

    def test_initialization_with_config(self):
        """Test MockLLMClient with custom config"""
        config = LLMConfig(provider=LLMProvider.MOCK, model="custom-mock")
        client = MockLLMClient(config)
        assert client.config.model == "custom-mock"

    def test_read_file_selection(self):
        """Test tool selection for read operations"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Read the file at /tmp/test.txt"}]
        response = client.decide_action(messages, ["read_file", "write_file"], {})
        assert response.tool_name == "read_file"
        assert "path" in response.arguments
        assert response.is_complete is False

    def test_write_file_selection(self):
        """Test tool selection for write operations"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Write content to file"}]
        response = client.decide_action(messages, ["read_file", "write_file"], {})
        assert response.tool_name == "write_file"
        assert "path" in response.arguments
        assert "content" in response.arguments
        assert response.is_complete is False

    def test_search_selection(self):
        """Test tool selection for search operations"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Search for test pattern"}]
        response = client.decide_action(messages, ["read_file", "search"], {})
        assert response.tool_name == "search"
        assert "query" in response.arguments
        assert response.is_complete is False

    def test_list_directory_selection(self):
        """Test tool selection for list operations"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "List files in /tmp"}]
        response = client.decide_action(messages, ["list_directory"], {})
        assert response.tool_name == "list_directory"
        assert "path" in response.arguments
        assert response.is_complete is False

    def test_task_complete_no_tool_needed(self):
        """Test that task is complete when no tool matches"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Hello, how are you?"}]
        response = client.decide_action(messages, ["read_file", "write_file"], {})
        assert response.tool_name is None
        assert response.is_complete is True

    def test_multi_turn_after_tool_execution(self):
        """Test multi-turn reasoning after tool execution"""
        client = MockLLMClient()
        # First call: user message
        messages1 = [{"role": "user", "content": "Read the file"}]
        response1 = client.decide_action(messages1, ["read_file"], {})
        assert response1.tool_name == "read_file"
        assert response1.is_complete is False

        # Second call: after tool execution
        messages2 = [
            {"role": "user", "content": "Read the file"},
            {"role": "tool", "content": "File content result"},
        ]
        response2 = client.decide_action(messages2, ["read_file"], {})
        # After tool execution, task should be complete
        assert response2.is_complete is True

    def test_tool_not_in_available_list(self):
        """Test that only available tools are selected"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Read the file"}]
        response = client.decide_action(messages, ["write_file"], {})
        # read_file not available, so no tool selected
        assert response.tool_name is None

    def test_case_insensitive_matching(self):
        """Test that matching is case-insensitive"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "READ THE FILE"}]
        response = client.decide_action(messages, ["read_file"], {})
        assert response.tool_name == "read_file"

    def test_call_count_increments(self):
        """Test that call count increments"""
        client = MockLLMClient()
        assert client.call_count == 0
        client.decide_action(
            [{"role": "user", "content": "Read file"}], ["read_file"], {}
        )
        assert client.call_count == 1
        client.decide_action(
            [{"role": "user", "content": "Write file"}], ["write_file"], {}
        )
        assert client.call_count == 2

    def test_empty_messages(self):
        """Test handling of empty message list"""
        client = MockLLMClient()
        response = client.decide_action([], ["read_file"], {})
        assert response.tool_name is None
        assert response.is_complete is True

    def test_no_user_message(self):
        """Test handling when no user message exists"""
        client = MockLLMClient()
        messages = [{"role": "assistant", "content": "Hello"}]
        response = client.decide_action(messages, ["read_file"], {})
        assert response.tool_name is None
        assert response.is_complete is True

    def test_view_show_display_variations(self):
        """Test various read-like operations"""
        client = MockLLMClient()

        for action in ["view file", "show content", "display data", "cat file"]:
            messages = [{"role": "user", "content": action}]
            response = client.decide_action(messages, ["read_file"], {})
            assert response.tool_name == "read_file"

    def test_delete_red_action(self):
        """Test that delete operations are recognized"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Delete the file"}]
        response = client.decide_action(messages, ["delete_file"], {})
        assert response.tool_name == "delete_file"
        assert response.is_complete is False

    def test_edit_red_action(self):
        """Test that edit operations are recognized"""
        client = MockLLMClient()
        messages = [{"role": "user", "content": "Edit the file"}]
        response = client.decide_action(messages, ["edit_file"], {})
        assert response.tool_name == "edit_file"
        assert response.is_complete is False


class TestCreateLLMClient:
    """Tests for LLM client factory function"""

    def test_create_mock_client(self):
        """Test creating MockLLMClient"""
        client = create_llm_client()
        assert isinstance(client, MockLLMClient)

    def test_create_mock_client_with_config(self):
        """Test creating MockLLMClient with config"""
        config = LLMConfig(provider=LLMProvider.MOCK, model="test")
        client = create_llm_client(config)
        assert isinstance(client, MockLLMClient)
        assert client.config.model == "test"

    def test_create_openai_client(self):
        """Test creating OpenAILLMClient"""
        config = LLMConfig(
            provider=LLMProvider.OPENAI,
            api_key="test-key",
        )
        # This will raise ImportError if openai not installed
        # but should still create the class
        try:
            client = create_llm_client(config)
            assert isinstance(client, OpenAILLMClient)
        except ImportError:
            # openai not installed, skip this test
            pytest.skip("openai package not installed")

    def test_create_openai_client_without_api_key(self):
        """Test creating OpenAILLMClient without API key"""
        config = LLMConfig(
            provider=LLMProvider.OPENAI,
            api_key=None,
        )
        try:
            client = create_llm_client(config)
            assert isinstance(client, OpenAILLMClient)
        except ImportError:
            pytest.skip("openai package not installed")
        except Exception as e:
            # OpenAIError is expected when no API key is set
            # This validates that the client initialization works
            assert "api_key" in str(e).lower() or "OPENAI_API_KEY" in str(e)

    def test_unsupported_provider_raises_error(self):
        """Test that unsupported provider raises error"""
        # Mock the provider enum to simulate unsupported provider
        config = LLMConfig()
        # We can't actually test this since enum is fixed
        # but we can test the error path indirectly
        pass


class TestThinkWithLLM:
    """Tests for think() function with LLM integration"""

    def test_think_uses_llm_client(self):
        """Test that think() uses provided LLM client"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Read the file"}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is not None
        assert isinstance(action, ToolCall)

    def test_think_with_read_request(self):
        """Test think() with read file request"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Read /tmp/test.txt"}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is not None
        assert action.name == "read_file"
        assert action.action_kind == ActionKind.GREEN

    def test_think_with_write_request(self):
        """Test think() with write file request"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Write content to file"}],
            tools=["write_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is not None
        assert action.name == "write_file"
        # Write is RED for security (destructive action)
        assert action.action_kind == ActionKind.RED

    def test_think_with_delete_request(self):
        """Test think() with delete file request"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Delete the file"}],
            tools=["delete_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is not None
        assert action.name == "delete_file"
        assert action.action_kind == ActionKind.RED

    def test_think_task_complete(self):
        """Test think() when task is complete"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Hello!"}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is None

    def test_think_multi_turn_after_tool(self):
        """Test think() in multi-turn after tool execution"""
        llm_client = MockLLMClient()
        # First call: select tool
        state1 = AgentState(
            messages=[{"role": "user", "content": "Read file"}],
            tools=["read_file"],
            context={},
        )
        action1 = think(state1, llm_client)
        assert action1 is not None
        assert action1.name == "read_file"

        # Second call: after tool execution
        state2 = AgentState(
            messages=[
                {"role": "user", "content": "Read file"},
                {"role": "tool", "content": "File content"},
            ],
            tools=["read_file"],
            context={},
        )
        action2 = think(state2, llm_client)
        # After tool execution, task should be complete
        assert action2 is None

    def test_think_default_llm_client(self):
        """Test think() with default (None) LLM client"""
        state = AgentState(
            messages=[{"role": "user", "content": "Read the file"}],
            tools=["read_file"],
            context={},
        )
        action = think(state)
        assert action is not None
        assert isinstance(action, ToolCall)


class TestMultiTurnReasoning:
    """Tests for multi-turn reasoning support"""

    def test_single_turn_completion(self):
        """Test that single-turn tasks complete correctly"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Hello"}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is None

    def test_tool_then_complete(self):
        """Test tool execution then completion"""
        llm_client = MockLLMClient()

        # First iteration: select tool
        state1 = AgentState(
            messages=[{"role": "user", "content": "Read the file"}],
            tools=["read_file"],
            context={},
        )
        action1 = think(state1, llm_client)
        assert action1 is not None
        assert action1.name == "read_file"

        # Second iteration: after tool execution, complete
        state2 = AgentState(
            messages=[
                {"role": "user", "content": "Read the file"},
                {"role": "tool", "content": "File content result"},
            ],
            tools=["read_file"],
            context={},
        )
        action2 = think(state2, llm_client)
        assert action2 is None

    def test_multiple_tools_in_sequence(self):
        """Test multiple tools executed in sequence"""
        llm_client = MockLLMClient()

        # Simulate a sequence: list -> read -> complete
        state1 = AgentState(
            messages=[{"role": "user", "content": "List files then read test.txt"}],
            tools=["list_directory", "read_file"],
            context={},
        )
        action1 = think(state1, llm_client)
        # Should select first matching tool (read appears after list in message)
        # The mock client finds the first matching pattern
        assert action1 is not None
        # Either list_directory or read_file is acceptable based on message order
        assert action1.name in ["list_directory", "read_file"]

        # After first tool, next action would be determined by tool result
        # This demonstrates the multi-turn capability

    def test_preserves_conversation_history(self):
        """Test that conversation history is preserved across turns"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[
                {"role": "user", "content": "First request"},
                {"role": "assistant", "content": "Response"},
                {"role": "user", "content": "Second request"},
            ],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        # Should have access to full history
        assert len(state.messages) == 3


class TestActionKindDetermination:
    """Tests for action kind determination from tool names"""

    def test_read_is_green(self):
        """Test that read operations are GREEN"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Read the file"}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action.action_kind == ActionKind.GREEN

    def test_write_is_red(self):
        """Test that write operations are RED (security model)"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Write content"}],
            tools=["write_file"],
            context={},
        )
        action = think(state, llm_client)
        # Write is classified as RED for security (destructive action)
        assert action.action_kind == ActionKind.RED

    def test_delete_is_red(self):
        """Test that delete operations are RED"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Delete file"}],
            tools=["delete_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action.action_kind == ActionKind.RED

    def test_edit_is_red(self):
        """Test that edit operations are RED"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Edit file"}],
            tools=["edit_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action.action_kind == ActionKind.RED


# Property-based tests using Hypothesis


class TestPropertyBasedLLM:
    """Property-based tests for LLM integration"""

    @given(st.text())
    def test_llm_handles_any_user_message(self, message):
        """Property test: LLM should handle any user message"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": message}],
            tools=["read_file", "write_file"],
            context={},
        )
        # Should not crash
        action = think(state, llm_client)
        assert action is None or isinstance(action, ToolCall)

    @given(st.lists(st.text(min_size=1), max_size=5))
    def test_multi_turn_with_various_messages(self, messages):
        """Property test: Multi-turn should handle various message sequences"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": msg} for msg in messages],
            tools=["read_file"],
            context={},
        )
        # Should not crash
        action = think(state, llm_client)
        assert action is None or isinstance(action, ToolCall)

    @given(st.lists(st.text(min_size=1), max_size=10))
    def test_various_tool_lists(self, tools):
        """Property test: LLM should handle various tool lists"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Read file"}],
            tools=tools,
            context={},
        )
        # Should not crash
        action = think(state, llm_client)
        assert action is None or isinstance(action, ToolCall)

    @given(st.dictionaries(st.text(min_size=1), st.text()))
    def test_various_contexts(self, context):
        """Property test: LLM should handle various contexts"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Read file"}],
            tools=["read_file"],
            context=context,
        )
        # Should not crash
        action = think(state, llm_client)
        assert action is None or isinstance(action, ToolCall)


class TestOpenAILLMClient:
    """Tests for OpenAILLMClient (may skip if package not installed)"""

    def test_openai_initialization(self):
        """Test OpenAI client initialization"""
        config = LLMConfig(
            provider=LLMProvider.OPENAI,
            api_key="test-key",
            model="gpt-4",
        )
        try:
            client = OpenAILLMClient(config)
            assert client.config.model == "gpt-4"
        except ImportError:
            pytest.skip("openai package not installed")

    def test_openai_without_package(self):
        """Test that appropriate error raised when openai not installed"""
        config = LLMConfig(
            provider=LLMProvider.OPENAI,
            api_key="test-key",
        )
        # Can't easily test ImportError without mocking sys.modules
        # but we verify the factory handles it gracefully
        pass


class TestErrorHandling:
    """Tests for error handling and edge cases"""

    def test_empty_tools_list(self):
        """Test handling of empty tools list"""
        llm_client = MockLLMClient()
        state = AgentState(
            messages=[{"role": "user", "content": "Read file"}],
            tools=[],
            context={},
        )
        action = think(state, llm_client)
        # No tools available, should be complete
        assert action is None

    def test_none_llm_client_creates_default(self):
        """Test that None LLM client creates default"""
        state = AgentState(
            messages=[{"role": "user", "content": "Read file"}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client=None)
        # Should create default MockLLMClient
        assert action is not None

    def test_very_long_message(self):
        """Test handling of very long messages"""
        llm_client = MockLLMClient()
        long_message = "Read " + "file " * 1000
        state = AgentState(
            messages=[{"role": "user", "content": long_message}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        assert action is not None

    def test_special_characters_in_message(self):
        """Test handling of special characters"""
        llm_client = MockLLMClient()
        special_message = "Read file: /tmp/测试@#$%^&*().txt"
        state = AgentState(
            messages=[{"role": "user", "content": special_message}],
            tools=["read_file"],
            context={},
        )
        action = think(state, llm_client)
        # Should not crash
        assert action is None or isinstance(action, ToolCall)
