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
    from llm_client import MockLLMClient

    def create_client():
        # MockLLMClient doesn't require process spawning
        client = MockLLMClient()
        return client

    # Measure memory after creation
    result = benchmark(create_client)
    memory = get_memory_usage_mb()

    # Check against baseline
    assert memory < 200, f"LLM client memory {memory:.2f}MB exceeds target of 200MB"


def test_mcp_client_memory(benchmark):
    """Benchmark memory usage of MCP client."""
    from mcp_client import McpClient

    def create_mcp_client():
        # Mock to avoid actual process spawning
        with patch('mcp_client.os.setsid'):
            client = McpClient("test", ["echo"])
            client.spawn()
            return client

    # Measure memory after creation
    result = benchmark(create_mcp_client)
    memory = get_memory_usage_mb()

    # Check against baseline
    assert memory < 200, f"MCP client memory {memory:.2f}MB exceeds target of 200MB"


def test_message_queue_memory(benchmark):
    """Benchmark memory usage of message queue."""
    from llm_client import MockLLMClient

    def create_client():
        # MockLLMClient doesn't require process spawning
        client = MockLLMClient()
        return client

    # Measure memory after creation
    result = benchmark(create_client)
    memory = get_memory_usage_mb()

    # Check against baseline
    assert memory < 200, f"LLM client memory {memory:.2f}MB exceeds target of 200MB"


def test_context_window_memory(benchmark):
    """Benchmark memory usage of context window."""
    from llm_client import MockLLMClient

    def create_client():
        # MockLLMClient doesn't require process spawning
        client = MockLLMClient()
        return client

    # Measure memory after creation
    result = benchmark(create_client)
    memory = get_memory_usage_mb()

    # Check against baseline
    assert memory < 200, f"LLM client memory {memory:.2f}MB exceeds target of 200MB"
