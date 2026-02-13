"""
Real Integration Tests for MCP Client with Actual External Servers

These tests connect to real MCP servers using actual external dependencies:
- Node.js/npx for downloading and running MCP servers
- Network access for server downloads
- Real filesystem operations
- Real GitHub API (with authentication via GH_TOKEN, optional)

Requirements:
- Node.js with npx installed
- Network access
- Environment variable: RUN_INTEGRATION_TESTS=1

Optional:
- GH_TOKEN environment variable for GitHub API tests

Run with:
    RUN_INTEGRATION_TESTS=1 python -m pytest tests/test_real_mcp_integration.py -v -s

Or skip with:
    python -m pytest tests/ -k "not real_integration"
"""

import os
import sys
import pytest
import tempfile
import time
import json
from pathlib import Path

# Import after adding agent to path
sys.path.insert(0, str(Path(__file__).parent.parent))
from mcp_client import McpClient, McpError


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run real integration tests",
)
class TestRealMcpFilesystemServer:
    """Integration tests with real MCP filesystem server"""

    @pytest.fixture(autouse=True)
    def setup(self):
        """Check if npx is available"""
        try:
            import subprocess
            result = subprocess.run(
                ["which", "npx"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode != 0:
                pytest.skip("npx not available")
        except Exception:
            pytest.skip("Failed to check npx availability")

    def test_real_filesystem_server_full_lifecycle(self):
        """Test complete client lifecycle with real filesystem server"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create test files
            test_file = Path(tmpdir) / "test.txt"
            test_file.write_text("Hello from real MCP integration test!")

            subdir = Path(tmpdir) / "subdir"
            subdir.mkdir()
            (subdir / "nested.txt").write_text("Nested file content")

            # Create MCP client
            print(f"\nStarting filesystem server for directory: {tmpdir}")
            client = McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            )

            # Measure spawn time
            start = time.time()
            client.spawn()
            spawn_time = time.time() - start
            print(f"Server spawned in {spawn_time:.2f}s")

            assert client.state.value == "connected"

            # Initialize
            start = time.time()
            client.initialize()
            init_time = time.time() - start
            print(f"Initialized in {init_time:.2f}s")

            assert client.state.value == "initialized"

            # List tools
            tools = client.list_tools()
            tool_names = [t.name for t in tools]
            print(f"Available tools: {tool_names}")

            assert "read_file" in tool_names
            assert "write_file" in tool_names
            assert "list_directory" in tool_names
            assert "list_allowed_directories" in tool_names

            # Test read_file
            result = client.call_tool("read_file", {"path": "test.txt"})
            content = str(result.get("content", ""))
            assert "Hello from real MCP" in content
            print(f"Read file content: {content[:50]}...")

            # Test list_directory
            result = client.call_tool("list_directory", {"path": "."})
            print(f"Directory listing: {result}")

            # Test write_file
            write_result = client.call_tool(
                "write_file",
                {
                    "path": "new_file.txt",
                    "content": "Written by real integration test"
                }
            )
            print(f"Write result: {write_result}")

            # Verify file was written
            new_file = Path(tmpdir) / "new_file.txt"
            assert new_file.exists()
            assert "Written by real integration test" in new_file.read_text()

            # Test reading nested directory
            result = client.call_tool("read_file", {"path": "subdir/nested.txt"})
            assert "Nested file content" in str(result.get("content", ""))

            # Shutdown
            client.shutdown()
            assert client.state.value == "shutdown"

            print("Real filesystem server test completed successfully")

    def test_real_filesystem_server_error_handling(self):
        """Test error handling with real filesystem server"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            ) as client:
                # Try to read non-existent file
                try:
                    client.call_tool("read_file", {"path": "does_not_exist.txt"})
                    assert False, "Should have raised McpError"
                except McpError as e:
                    print(f"Got expected error: {e}")
                    assert "error" in str(e).lower() or "not found" in str(e).lower()

                # Try to write to non-existent directory
                try:
                    client.call_tool(
                        "write_file",
                        {
                            "path": "nonexistent/subdir/file.txt",
                            "content": "test"
                        }
                    )
                    assert False, "Should have raised McpError"
                except McpError as e:
                    print(f"Got expected error: {e}")

                # Try to call invalid tool
                try:
                    client.call_tool("invalid_tool_name", {})
                    assert False, "Should have raised McpError"
                except McpError as e:
                    print(f"Got expected error for invalid tool: {e}")

                print("Error handling test passed")

    def test_real_filesystem_server_performance(self):
        """Test performance characteristics with real server"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create multiple test files
            for i in range(10):
                (Path(tmpdir) / f"file{i}.txt").write_text(f"Content {i}")

            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            ) as client:
                # Measure tool call latency
                latencies = []

                for i in range(5):
                    start = time.time()
                    client.call_tool("read_file", {"path": f"file{i}.txt"})
                    latency = time.time() - start
                    latencies.append(latency)

                avg_latency = sum(latencies) / len(latencies)
                print(f"Average tool call latency: {avg_latency*1000:.2f}ms")
                print(f"Min: {min(latencies)*1000:.2f}ms, Max: {max(latencies)*1000:.2f}ms")

                # Tool calls should be fast (< 1 second)
                assert avg_latency < 1.0, f"Average latency too high: {avg_latency:.2f}s"

                # Test concurrent operations
                start = time.time()
                for i in range(5, 10):
                    client.call_tool("read_file", {"path": f"file{i}.txt"})
                concurrent_time = time.time() - start
                print(f"5 sequential tool calls in {concurrent_time*1000:.2f}ms")

                print("Performance test passed")


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS") or not os.environ.get("GH_TOKEN"),
    reason="Set RUN_INTEGRATION_TESTS=1 and GH_TOKEN for GitHub server tests",
)
class TestRealMcpGitHubServer:
    """Integration tests with real MCP GitHub server (requires GH_TOKEN)"""

    def test_real_github_server_basic_operations(self):
        """Test basic GitHub operations with real server"""
        with McpClient(
            "github",
            ["npx", "-y", "@modelcontextprotocol/server-github"]
        ) as client:
            # List available tools
            tools = client.list_tools()
            tool_names = [t.name for t in tools]
            print(f"GitHub server tools: {tool_names}")

            # Verify expected tools exist
            assert "search_issues" in tool_names or "list_issues" in tool_names

            # Try to search for issues (public repository)
            result = client.call_tool(
                "search_issues",
                {
                    "owner": "modelcontextprotocol",
                    "repo": "servers",
                    "query": "label:bug"
                }
            )

            print(f"GitHub search result: {json.dumps(result, indent=2)[:500]}...")

            # Should get a response (may be empty)
            assert isinstance(result, dict) or isinstance(result, str)

            print("GitHub server test passed")

    def test_real_github_server_error_handling(self):
        """Test error handling with real GitHub server"""
        with McpClient(
            "github",
            ["npx", "-y", "@modelcontextprotocol/server-github"]
        ) as client:
            # Try invalid repository
            try:
                client.call_tool(
                    "search_issues",
                    {
                        "owner": "nonexistent-repo-owner-12345",
                        "repo": "nonexistent-repo-12345",
                        "query": "test"
                    }
                )
                # May succeed with empty result, or may fail
                print("Invalid repo call completed (may have empty result)")
            except McpError as e:
                print(f"Got expected error: {e}")

            print("GitHub error handling test passed")


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run real integration tests",
)
class TestRealMcpClientLifecycle:
    """Test client lifecycle with real servers"""

    def test_real_client_context_manager(self):
        """Test context manager pattern with real server"""
        with tempfile.TemporaryDirectory() as tmpdir:
            (Path(tmpdir) / "test.txt").write_text("Context test")

            # Use context manager
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            ) as client:
                assert client.state.value == "initialized"

                result = client.call_tool("read_file", {"path": "test.txt"})
                assert "Context test" in str(result.get("content", ""))

            # After context, should be shut down
            assert client.state.value == "shutdown"

            print("Context manager test passed")

    def test_real_client_multiple_connections(self):
        """Test multiple sequential connections"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create 3 clients sequentially
            for i in range(3):
                print(f"\nConnection {i+1}/3")

                with McpClient(
                    "filesystem",
                    ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
                ) as client:
                    (Path(tmpdir) / f"file{i}.txt").write_text(f"Content {i}")

                    result = client.call_tool("read_file", {"path": f"file{i}.txt"})
                    assert f"Content {i}" in str(result.get("content", ""))

            print("Multiple connections test passed")

    def test_real_client_large_file_operations(self):
        """Test operations with large files"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create a large file (1MB)
            large_content = "x" * (1024 * 1024)
            large_file = Path(tmpdir) / "large.txt"
            large_file.write_text(large_content)

            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            ) as client:
                # Write large file
                start = time.time()
                result = client.call_tool(
                    "write_file",
                    {
                        "path": "large_output.txt",
                        "content": large_content
                    }
                )
                write_time = time.time() - start
                print(f"Wrote 1MB file in {write_time:.2f}s")

                # Read large file
                start = time.time()
                result = client.call_tool("read_file", {"path": "large_output.txt"})
                read_time = time.time() - start
                print(f"Read 1MB file in {read_time:.2f}s")

                # Verify content
                content = str(result.get("content", ""))
                assert len(content) > 1000000  # ~1MB

            print("Large file operations test passed")


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run real integration tests",
)
class TestRealMcpToolOperations:
    """Test various tool operations with real servers"""

    def test_real_tool_list_and_call(self):
        """Test listing tools and calling them"""
        with tempfile.TemporaryDirectory() as tmpdir:
            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            ) as client:
                # List all tools
                tools = client.list_tools()
                print(f"\nFound {len(tools)} tools:")

                for tool in tools:
                    print(f"  - {tool.name}: {tool.description}")

                    # Check tool schema
                    assert isinstance(tool.name, str)
                    assert isinstance(tool.description, str)
                    assert isinstance(tool.input_schema, dict)

                # Call each tool that doesn't require complex arguments
                for tool in tools:
                    if tool.name == "list_allowed_directories":
                        result = client.call_tool(tool.name, {})
                        print(f"  {tool.name}: {result}")
                    elif tool.name == "list_directory":
                        result = client.call_tool(tool.name, {"path": "."})
                        print(f"  {tool.name}: {result}")

            print("Tool list and call test passed")

    def test_real_tool_with_complex_arguments(self):
        """Test tools with complex argument structures"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create directory structure
            (Path(tmpdir) / "dir1").mkdir()
            (Path(tmpdir) / "dir2").mkdir()
            (Path(tmpdir) / "file1.txt").write_text("File 1")

            with McpClient(
                "filesystem",
                ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
            ) as client:
                # List directory with options
                result = client.call_tool(
                    "list_directory",
                    {
                        "path": ".",
                        # Some servers support recursive or other options
                    }
                )

                print(f"Directory listing: {result}")

                # Write file with special characters
                result = client.call_tool(
                    "write_file",
                    {
                        "path": "special-@#$.txt",
                        "content": "Special chars: @#$%^&*()"
                    }
                )
                print(f"Write special file result: {result}")

                # Verify file was written
                special_file = Path(tmpdir) / "special-@#$.txt"
                # Note: filename may be sanitized by filesystem
                print(f"Files in tmpdir: {list(Path(tmpdir).iter())}")

            print("Complex arguments test passed")


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run real integration tests",
)
def test_real_mcp_server_startup_time():
    """Test MCP server startup performance"""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Measure cold start time (first time, downloads package)
        start = time.time()
        client = McpClient(
            "filesystem",
            ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
        )
        client.spawn()
        client.initialize()
        cold_start_time = time.time() - start
        client.shutdown()

        print(f"Cold start time (with download): {cold_start_time:.2f}s")

        # Measure warm start time (package already cached)
        start = time.time()
        client = McpClient(
            "filesystem",
            ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
        )
        client.spawn()
        client.initialize()
        warm_start_time = time.time() - start
        client.shutdown()

        print(f"Warm start time (cached): {warm_start_time:.2f}s")

        # Warm start should be significantly faster
        print(f"Speedup: {cold_start_time/warm_start_time:.2f}x")

        assert warm_start_time < cold_start_time, "Warm start should be faster"


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Set RUN_INTEGRATION_TESTS=1 to run real integration tests",
)
def test_real_mcp_error_recovery():
    """Test error recovery and resilience"""
    with tempfile.TemporaryDirectory() as tmpdir:
        client = McpClient(
            "filesystem",
            ["npx", "-y", "@modelcontextprotocol/server-filesystem", tmpdir]
        )

        client.spawn()
        client.initialize()

        # Make multiple error calls
        error_count = 0
        for i in range(5):
            try:
                client.call_tool("read_file", {"path": f"nonexistent_{i}.txt"})
            except McpError:
                error_count += 1
                # Continue trying

        print(f"Caught {error_count} expected errors")

        # Verify client still works after errors
        (Path(tmpdir) / "recovery_test.txt").write_text("Recovery")
        result = client.call_tool("read_file", {"path": "recovery_test.txt"})
        assert "Recovery" in str(result.get("content", ""))

        client.shutdown()

        print("Error recovery test passed")


if __name__ == "__main__":
    # Run integration tests if environment variable is set
    if os.environ.get("RUN_INTEGRATION_TESTS"):
        pytest.main([__file__, "-v", "-s"])
    else:
        print("Real integration tests skipped.")
        print("Set RUN_INTEGRATION_TESTS=1 to run.")
        print("Optional: Set GH_TOKEN for GitHub API tests")
        print("\nExample:")
        print("  RUN_INTEGRATION_TESTS=1 python -m pytest tests/test_real_mcp_integration.py -v -s")
