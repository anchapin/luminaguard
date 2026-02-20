"""
Memory footprint benchmarks.

Measures memory usage of various operations.
Target: <200MB baseline memory footprint
"""
import pytest
import psutil
import gc
from unittest.mock import Mock, patch


def get_memory_usage_mb():
    """Get current process memory usage in MB."""
    process = psutil.Process()
    return process.memory_info().rss / 1024 / 1024


@pytest.fixture(autouse=True)
def setup_benchmark():
    """Setup and cleanup for memory benchmarks."""
    gc.collect()
    initial_memory = get_memory_usage_mb()
    yield
    gc.collect()
    # Verify memory was cleaned up
    final_memory = get_memory_usage_mb()
    # Allow 10MB overhead for testing framework
    assert final_memory - initial_memory < 10, f"Memory leak detected: {final_memory - initial_memory:.2f}MB"


def test_llm_client_memory(benchmark):
    """Benchmark memory usage of LLM client."""

    def create_client():
        from llm_client import LLMClient

        # Mock to avoid actual API calls
        with patch.object(LLMClient, '_initialize_session', return_value=None):
            client = LLMClient(
                api_key="test-key" * 100,  # Simulate larger key
                base_url="https://api.example.com",
                model="test-model",
            )
        return client

    # Measure memory after creation
    result = benchmark(create_client)
    memory = get_memory_usage_mb()

    # Check against baseline
    assert memory < 200, f"LLM client memory {memory:.2f}MB exceeds target of 200MB"


def test_mcp_client_memory(benchmark):
    """Benchmark memory usage of MCP client."""

    def create_mcp_client():
        from mcp_client import McpClient

        # Mock to avoid actual process spawning
        with patch('mcp_client.subprocess.Popen') as mock_popen:
            mock_process = Mock()
            mock_process.poll.return_value = None
            mock_process.stdout = Mock()
            mock_process.stdin = Mock()
            mock_process.stderr = Mock()
            mock_popen.return_value = mock_process

            with patch('mcp_client.os.setsid'):
                client = McpClient("test", ["echo"])
                client.spawn()

        return client

    result = benchmark(create_mcp_client)
    memory = get_memory_usage_mb()

    assert memory < 200, f"MCP client memory {memory:.2f}MB exceeds target of 200MB"


def test_message_queue_memory(benchmark):
    """Benchmark memory usage of message queue."""

    def create_queue():
        # Simulate message queue with various messages
        messages = [
            {
                "role": "user",
                "content": "Test message " * 10,
            }
            for _ in range(100)
        ]
        return messages

    result = benchmark(create_queue)
    memory = get_memory_usage_mb()

    # 100 messages should not exceed 20MB
    assert memory < 20, f"Message queue memory {memory:.2f}MB exceeds 20MB target"


def test_context_window_memory(benchmark):
    """Benchmark memory usage of context window."""

    def create_context():
        # Simulate a context window with tokens
        # Assume ~4 bytes per token average
        num_tokens = 8000  # Typical context size
        tokens = [f"token{i}" for i in range(num_tokens)]
        return tokens

    result = benchmark(create_context)
    memory = get_memory_usage_mb()

    # 8000 tokens should be <10MB
    assert memory < 10, f"Context window memory {memory:.2f}MB exceeds 10MB target"


def test_tool_cache_memory(benchmark):
    """Benchmark memory usage of tool cache."""

    def create_tool_cache():
        # Simulate tool definitions cache
        tools = [
            {
                "name": f"tool_{i}",
                "description": f"Tool description {i}" * 10,
                "parameters": {
                    "type": "object",
                    "properties": {
                        f"param_{j}": {"type": "string"}
                        for j in range(5)
                    },
                },
            }
            for i in range(50)
        ]
        return tools

    result = benchmark(create_tool_cache)
    memory = get_memory_usage_mb()

    # 50 tools should be <5MB
    assert memory < 5, f"Tool cache memory {memory:.2f}MB exceeds 5MB target"


def test_file_buffer_memory(benchmark):
    """Benchmark memory usage of file operations buffer."""

    def create_buffer():
        # Simulate reading a large file into memory
        # 1MB buffer
        buffer = bytearray(1024 * 1024)
        return buffer

    result = benchmark(create_buffer)
    memory = get_memory_usage_mb()

    # 1MB buffer should be <2MB total overhead
    assert memory < 2, f"File buffer memory {memory:.2f}MB exceeds 2MB target"


def test_memory_cleanup(benchmark):
    """Verify memory is properly cleaned up after operations."""

    def create_and_cleanup():
        # Create large objects
        large_objects = [bytearray(1024 * 100) for _ in range(10)]

        # Cleanup
        del large_objects
        gc.collect()

        # Return memory usage after cleanup
        return get_memory_usage_mb()

    final_memory = benchmark(create_and_cleanup)

    # Memory should be reasonable after cleanup
    assert final_memory < 200, f"Memory after cleanup {final_memory:.2f}MB exceeds 200MB"
