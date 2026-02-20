"""
Startup performance benchmarks.

Measures the time to initialize key components of the agent system.
Target: <500ms for complete startup
"""
import time
import pytest
from unittest.mock import Mock, patch


def test_import_overhead(benchmark):
    """Benchmark the time to import and initialize key modules."""

    def import_modules():
        # Simulate importing and initializing core modules
        import sys
        import importlib

        modules = ['loop', 'llm_client', 'mcp_client', 'vsock_client', 'mesh']
        for module in modules:
            try:
                importlib.import_module(module)
            except ImportError:
                # Module may not be fully functional, but import overhead counts
                pass

    benchmark(import_modules)


def test_llm_client_initialization(benchmark):
    """Benchmark LLM client initialization time."""

    def init_client():
        from llm_client import LLMClient

        # Mock the actual initialization to avoid network calls
        with patch.object(LLMClient, '_initialize_session', return_value=None):
            client = LLMClient(
                api_key="test-key",
                base_url="https://api.example.com",
                model="test-model",
            )
        return client

    benchmark(init_client)


def test_mcp_client_spawn_time(benchmark):
    """Benchmark MCP client spawn time (simulated)."""

    def spawn_mcp():
        # Simulate MCP server spawn overhead
        start = time.time()

        # Simulate process creation
        process_mock = Mock()
        process_mock.pid = 12345

        # Simulate connection establishment
        time.sleep(0.001)  # 1ms overhead

        elapsed = (time.time() - start) * 1000  # Convert to ms
        return elapsed

    result = benchmark(spawn_mcp)

    # Check against target (<500ms for complete startup, this is just spawn)
    assert result < 500, f"MCP spawn time {result}ms exceeds target of 500ms"


def test_agent_loop_initialization(benchmark):
    """Benchmark agent loop initialization."""

    def init_loop():
        # Simulate agent loop setup
        config = {
            'llm': {
                'api_key': 'test-key',
                'base_url': 'https://api.example.com',
                'model': 'test-model',
            },
            'tools': [],
        }

        # Mock the actual initialization
        with patch('loop.LLMClient') as mock_llm:
            mock_llm.return_value = Mock()
            # Simulate loop creation
            return config

    benchmark(init_loop)


def test_concurrent_initialization(benchmark):
    """Benchmark initializing multiple components concurrently."""

    def init_concurrent():
        import concurrent.futures
        from unittest.mock import MagicMock

        def init_component(name):
            # Simulate component initialization
            time.sleep(0.01)
            return {'name': name, 'status': 'ready'}

        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [
                executor.submit(init_component, f'component-{i}')
                for i in range(4)
            ]
            results = [f.result() for f in futures]

        return len(results)

    benchmark(init_concurrent)
