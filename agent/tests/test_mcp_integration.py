"""
Integration tests for MCP client with real MCP servers

These tests require:
- Node.js with npx installed
- Network access (for downloading MCP servers)
- Environment variable: RUN_INTEGRATION_TESTS=1

Run with:
    RUN_INTEGRATION_TESTS=1 python -m pytest tests/test_mcp_integration.py -v

Or skip with:
    python -m pytest tests/ -k "not integration"
"""

import os
import sys
import pytest
import tempfile
from pathlib import Path

# Import after adding agent to path
sys.path.insert(0, str(Path(__file__).parent.parent))
from mcp_client import McpClient, McpError


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run integration tests",
)
class TestMcpFilesystemServer:
    """Integration tests with MCP filesystem server"""

    def test_full_lifecycle_with_filesystem_server(self):
        """Test complete client lifecycle with real filesystem server"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create test file
            test_file = Path(tmpdir) / "test.txt"
            test_file.write_text("Hello from LuminaGuard MCP integration test!")

            # Create MCP client for filesystem server
            client = McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            )

            # Test spawn
            client.spawn()
            assert client.state.value == "connected"

            # Test initialize
            client.initialize()
            assert client.state.value == "initialized"

            # Test list_tools
            tools = client.list_tools()
            assert len(tools) > 0
            tool_names = [t.name for t in tools]
            assert "read_file" in tool_names
            assert "write_file" in tool_names
            assert "list_allowed_directories" in tool_names

            # Test call_tool - read file
            result = client.call_tool("read_file", {"path": "test.txt"})
            assert "content" in result
            content = (
                result["content"][0]
                if isinstance(result["content"], list)
                else result["content"]
            )
            assert "Hello from LuminaGuard in str(content)

            # Test call_tool - write file
            write_result = client.call_tool(
                "write_file",
                {
                    "path": "new_file.txt",
                    "content": "New content from integration test",
                },
            )
            assert "content" in write_result or write_result is not None

            # Verify file was written
            new_file = Path(tmpdir) / "new_file.txt"
            assert new_file.exists()
            assert "New content from integration test" in new_file.read_text()

            # Test shutdown
            client.shutdown()
            assert client.state.value == "shutdown"

    def test_context_manager_with_filesystem_server(self):
        """Test context manager usage with real filesystem server"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create test file
            test_file = Path(tmpdir) / "context_test.txt"
            test_file.write_text("Context manager test")

            # Use context manager
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                assert client.state.value == "initialized"

                tools = client.list_tools()
                assert len(tools) > 0

                result = client.call_tool("read_file", {"path": "context_test.txt"})
                assert "Context manager test" in str(result.get("content", ""))

            # After context, should be shut down
            assert client.state.value == "shutdown"

    def test_error_handling_with_invalid_path(self):
        """Test error handling when file doesn't exist"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                # Try to read non-existent file
                with pytest.raises(McpError):
                    client.call_tool("read_file", {"path": "does_not_exist.txt"})


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run integration tests",
)
class TestMcpServerCapabilities:
    """Test MCP server capabilities and protocol compliance"""

    def test_initialize_response_structure(self):
        """Test that initialize returns proper protocol structure"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                # Check that client received server capabilities
                # (This would be stored if we expanded the client to save them)
                assert client.state.value == "initialized"

    def test_tools_have_required_fields(self):
        """Test that all tools have required fields"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                tools = client.list_tools()

                for tool in tools:
                    # All tools must have a name
                    assert tool.name, "Tool missing name"
                    assert isinstance(tool.name, str)

                    # All tools should have description (may be empty)
                    assert isinstance(tool.description, str)

                    # All tools should have input schema
                    assert isinstance(tool.input_schema, dict)

    def test_concurrent_tool_calls(self):
        """Test making multiple tool calls in sequence"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create multiple test files
            for i in range(5):
                (Path(tmpdir) / f"file{i}.txt").write_text(f"Content {i}")

            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                # Read all files
                results = []
                for i in range(5):
                    result = client.call_tool("read_file", {"path": f"file{i}.txt"})
                    results.append(result)

                # Verify all succeeded
                assert len(results) == 5
                for i, result in enumerate(results):
                    assert f"Content {i}" in str(result.get("content", ""))


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run integration tests",
)
class TestMcpErrorHandling:
    """Test error handling in real MCP server scenarios"""

    def test_invalid_tool_name(self):
        """Test calling a non-existent tool"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                with pytest.raises(McpError):
                    client.call_tool("invalid_tool_name", {})

    def test_missing_required_parameters(self):
        """Test calling tool without required parameters"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                # read_file requires "path" parameter
                with pytest.raises(McpError):
                    client.call_tool("read_file", {})

    def test_disallowed_directory_access(self):
        """Test that accessing files outside allowed directory is blocked"""
        # Filesystem server should only allow access to tmpdir
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
            ) as client:
                # Try to read file outside allowed directory
                with pytest.raises(McpError):
                    client.call_tool("read_file", {"path": "/etc/passwd"})


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run integration tests",
)
def test_mcp_client_performance():
    """Test performance characteristics with real server"""
    import time

    with tempfile.TemporaryDirectory() as tmpdir:
        # Measure spawn time
        start = time.time()
        client = McpClient(
            "filesystem",
            ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir],
        )
        client.spawn()
        client.initialize()
        spawn_time = time.time() - start

        # Spawn should be reasonably fast (< 10 seconds for npx download)
        assert spawn_time < 10.0, f"Spawn took too long: {spawn_time:.2f}s"

        # Measure tool call latency
        start = time.time()
        tools = client.list_tools()
        list_time = time.time() - start

        # List tools should be fast (< 1 second)
        assert list_time < 1.0, f"List tools took too long: {list_time:.2f}s"

        # Measure tool call time
        (Path(tmpdir) / "perf_test.txt").write_text("Performance test")
        start = time.time()
        result = client.call_tool("read_file", {"path": "perf_test.txt"})
        call_time = time.time() - start

        # Tool call should be fast (< 1 second)
        assert call_time < 1.0, f"Tool call took too long: {call_time:.2f}s"

        client.shutdown()


if __name__ == "__main__":
    # Run integration tests if environment variable is set
    if os.environ.get("RUN_INTEGRATION_TESTS"):
        pytest.main([__file__, "-v", "-s"])
    else:
        print("Integration tests skipped. Set RUN_INTEGRATION_TESTS=1 to run.")
        print(
            "Example: RUN_INTEGRATION_TESTS=1 python -m pytest tests/test_mcp_integration.py -v"
        )
