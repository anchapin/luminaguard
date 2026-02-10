#!/usr/bin/env python3
"""
MCP Client - Python Interface to Rust Orchestrator
================================================

This module provides a Python client for communicating with the IronClaw
Rust Orchestrator's MCP (Model Context Protocol) client.

It handles:
- Spawning the orchestrator MCP client process
- Sending JSON-RPC 2.0 requests
- Receiving and parsing responses
- Tool listing and invocation

Architecture:
    Python Agent Loop (this module)
         ↓ (JSON over stdin/stdout)
    Rust Orchestrator (MCP client)
         ↓ (stdio/HTTP)
    MCP Server (filesystem, github, slack, etc.)

Example:
    >>> client = McpClient("filesystem", ["npx", "-y", "@modelcontextprotocol/server-filesystem"])
    >>> client.initialize()
    >>> tools = client.list_tools()
    >>> result = client.call_tool("read_file", {"path": "/tmp/test.txt"})
    >>> client.shutdown()
"""

from __future__ import annotations

import json
import subprocess
import sys
from typing import Any, Dict, List, Optional
from dataclasses import dataclass
from enum import Enum


class McpError(Exception):
    """MCP protocol or connection error"""

    pass


class McpState(Enum):
    """MCP client state machine"""

    DISCONNECTED = "disconnected"
    CONNECTED = "connected"
    INITIALIZED = "initialized"
    SHUTDOWN = "shutdown"


@dataclass
class Tool:
    """MCP tool description"""

    name: str
    description: str
    input_schema: Dict[str, Any]


class McpClient:
    """
    Python client for IronClaw MCP Orchestrator

    Manages communication with the Rust orchestrator's MCP client
    via JSON-RPC 2.0 over stdin/stdout.

    Lifecycle:
        1. Create (spawn orchestrator process)
        2. Initialize (MCP handshake)
        3. Use (list tools, call tools)
        4. Shutdown (terminate process)

    Example:
        >>> client = McpClient(
        ...     "filesystem",
        ...     ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
        ...     root_dir="/tmp"
        ... )
        >>> client.initialize()
        >>> tools = client.list_tools()
        >>> result = client.call_tool("read_file", {"path": "test.txt"})
        >>> client.shutdown()
    """

    def __init__(
        self,
        server_name: str,
        command: List[str],
        root_dir: Optional[str] = None,
        args: Optional[List[str]] = None,
    ):
        """
        Create MCP client and spawn orchestrator process.

        Args:
            server_name: Name of MCP server (for logging)
            command: Command to spawn MCP server (e.g., ["npx", "-y", "@modelcontextprotocol/server-filesystem"])
            root_dir: Root directory for filesystem operations (optional)
            args: Additional arguments for orchestrator (optional)

        Raises:
            McpError: If command validation fails
        """
        self.server_name = server_name
        self.command = self._validate_command(command)
        self.root_dir = root_dir
        self.args = args or []

        self._process: Optional[subprocess.Popen] = None
        self._state = McpState.DISCONNECTED

        # Request/Response tracking
        self._request_id = 0

    @property
    def state(self) -> McpState:
        """Get current client state"""
        return self._state

    def _validate_command(self, command: List[str]) -> List[str]:
        """
        Validate and sanitize user-provided command.

        This implements defense-in-depth for command injection prevention.
        While subprocess.Popen with list args prevents shell injection,
        we still validate to catch obvious mistakes and malicious inputs.

        Args:
            command: Command list to validate

        Returns:
            Validated command list

        Raises:
            McpError: If command fails validation
        """
        if not command or not isinstance(command, list):
            raise McpError("Command must be a non-empty list")

        if not all(isinstance(arg, str) for arg in command):
            raise McpError("All command arguments must be strings")

        # Check for shell metacharacters that could enable injection
        shell_metachars = [';', '&', '|', '$', '`', '(', ')', '<', '>', '\n', '\r']
        for arg in command:
            if any(char in arg for char in shell_metachars):
                raise McpError(
                    f"Command argument contains shell metacharacter: {arg!r}. "
                    "This may indicate an attempted command injection."
                )

        # Allowlist of known-safe base commands
        # This is not a security boundary (the subprocess runs locally as the user),
        # but prevents accidental mistakes and documents expected commands.
        safe_commands = {
            'npx',           # Node.js package runner
            'python', 'python3',  # Python interpreters
            'node',          # Node.js runtime
            'cargo',         # Rust toolchain (for testing)
            'echo',          # Testing (benign)
            'true',          # Testing (benign)
            'cat',           # File operations (for trusted input)
        }

        base_cmd = command[0]
        # Allow paths (e.g., ./node_modules/.bin/npx) by checking base name
        base_name = base_cmd.split('/')[-1].split('\\')[-1]

        if base_name not in safe_commands:
            # Log warning but don't fail - user may have custom setup
            import sys
            print(
                f"Warning: Command '{base_name}' not in known-safe list. "
                f"Ensure this command is trusted and does not accept untrusted input.",
                file=sys.stderr
            )

        return command

    def _send_request(
        self, method: str, params: Optional[Dict[str, Any]] = None
    ) -> Dict[str, Any]:
        """
        Send JSON-RPC 2.0 request to orchestrator.

        Args:
            method: MCP method name (e.g., "tools/list", "tools/call")
            params: Method parameters

        Returns:
            Parsed JSON-RPC response

        Raises:
            McpError: On communication or protocol error
        """
        if self._state == McpState.SHUTDOWN:
            raise McpError("Cannot send request: client is shut down")

        # Increment request ID
        self._request_id += 1

        # Build JSON-RPC 2.0 request
        request = {
            "jsonrpc": "2.0",
            "id": self._request_id,
            "method": method,
            "params": params or {},
        }

        # Send request via stdin
        request_json = json.dumps(request) + "\n"
        try:
            self._process.stdin.write(request_json.encode())
            self._process.stdin.flush()
        except (BrokenPipeError, OSError) as e:
            raise McpError(f"Failed to send request: {e}") from e

        # Read response from stdout
        try:
            response_line = self._process.stdout.readline()
            if not response_line:
                raise McpError("No response from orchestrator (process died?)")
        except OSError as e:
            raise McpError(f"Failed to read response: {e}") from e

        # Parse JSON-RPC 2.0 response
        try:
            response = json.loads(response_line)
        except json.JSONDecodeError as e:
            raise McpError(f"Invalid JSON response: {e}") from e

        # Check for JSON-RPC error
        if "error" in response:
            error = response["error"]
            raise McpError(f"MCP error {error.get('code')}: {error.get('message')}")

        return response

    def spawn(self) -> None:
        """
        Spawn the orchestrator MCP client process.

        This starts the ironclaw CLI with the `mcp stdio` subcommand,
        which will spawn the MCP server and communicate with it.

        Security Note:
            This method spawns a subprocess using user-provided command.
            While we validate commands for obvious injection patterns,
            the subprocess runs with the same permissions as the user.
            This is safe for local-first use where the user controls the
            MCP server configuration. For untrusted input, additional
            sandboxing (e.g., JIT Micro-VMs) should be used.

        Raises:
            McpError: If process fails to start
        """
        if self._state != McpState.DISCONNECTED:
            raise McpError(f"Cannot spawn: client is {self._state.value}")

        # Build orchestrator command
        orch_cmd = ["cargo", "run", "--", "mcp", "stdio"]

        # Add server command
        orch_cmd.extend(self.command)

        # Spawn process
        try:
            self._process = subprocess.Popen(
                orch_cmd,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,  # Line buffered
            )
        except (FileNotFoundError, OSError) as e:
            raise McpError(f"Failed to spawn orchestrator: {e}") from e

        # Wait briefly for process to start
        # (In production, would check if process is ready)
        import time

        time.sleep(0.1)

        self._state = McpState.CONNECTED

    def initialize(self) -> None:
        """
        Initialize MCP connection with server.

        Sends MCP "initialize" handshake to establish session.

        Raises:
            McpError: If initialization fails
        """
        if self._state != McpState.CONNECTED:
            raise McpError(f"Cannot initialize: client is {self._state.value}")

        # Build initialize params
        params = {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "ironclaw-py",
                "version": "0.1.0",
            },
        }

        # Send initialize request
        response = self._send_request("initialize", params)

        # Check result
        if "result" not in response:
            raise McpError(f"Initialize failed: {response}")

        self._state = McpState.INITIALIZED

    def list_tools(self) -> List[Tool]:
        """
        List available tools from MCP server.

        Returns:
            List of Tool objects

        Raises:
            McpError: If request fails
        """
        if self._state != McpState.INITIALIZED:
            raise McpError(f"Cannot list tools: client is {self._state.value}")

        response = self._send_request("tools/list")

        if "result" not in response:
            raise McpError(f"tools/list failed: {response}")

        result = response["result"]
        tools = result.get("tools", [])

        # Parse tool descriptions
        return [
            Tool(
                name=tool["name"],
                description=tool.get("description", ""),
                input_schema=tool.get("inputSchema", {}),
            )
            for tool in tools
        ]

    def call_tool(self, name: str, arguments: Dict[str, Any]) -> Dict[str, Any]:
        """
        Call a tool on the MCP server.

        Args:
            name: Tool name (e.g., "read_file", "write_file")
            arguments: Tool arguments (schema depends on tool)

        Returns:
            Tool execution result

        Raises:
            McpError: If call fails

        Example:
            >>> result = client.call_tool("read_file", {"path": "/tmp/test.txt"})
            >>> print(result["content"])
        """
        if self._state != McpState.INITIALIZED:
            raise McpError(f"Cannot call tool: client is {self._state.value}")

        response = self._send_request(
            "tools/call",
            {
                "name": name,
                "arguments": arguments,
            },
        )

        if "result" not in response:
            raise McpError(f"tools/call failed: {response}")

        return response["result"]

    def shutdown(self) -> None:
        """
        Shutdown MCP client and terminate orchestrator process.

        Gracefully terminates the orchestrator process and the MCP server.
        After this call, the client cannot be used again.
        """
        if self._state == McpState.SHUTDOWN:
            return  # Already shut down

        if self._process:
            # Try graceful shutdown first
            try:
                self._process.stdin.close()
            except:
                pass

            # Terminate process
            try:
                self._process.terminate()
                self._process.wait(timeout=5)
            except:
                self._process.kill()

            self._process = None

        self._state = McpState.SHUTDOWN

    def __enter__(self):
        """Context manager entry"""
        self.spawn()
        self.initialize()
        return self

    def __exit__(self, _exc_type, _exc_val, _exc_tb):
        """Context manager exit"""
        self.shutdown()
        return False


def main():
    """
    Test MCP client with filesystem server.

    Usage:
        python -m agent.mcp_client
    """
    import sys
    import tempfile

    print("IronClaw MCP Client - Test")
    print("=" * 40)

    # Create temporary directory for testing
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create test file
        test_file = f"{tmpdir}/test.txt"
        with open(test_file, "w") as f:
            f.write("Hello from IronClaw MCP!")

        # Create MCP client
        print(f"\n1. Spawning MCP client for filesystem server...")
        client = McpClient(
            "filesystem",
            ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
            root_dir=tmpdir,
        )

        try:
            print("2. Initializing...")
            client.spawn()
            client.initialize()

            print("3. Listing tools...")
            tools = client.list_tools()
            print(f"   Available tools: {', '.join(t.name for t in tools)}")

            print("4. Calling tool: read_file")
            result = client.call_tool("read_file", {"path": "test.txt"})
            content = result.get("content", [])
            print(f"   File content: {content[0][:50]}...")

        except McpError as e:
            print(f"   Error: {e}")
            return 1

        finally:
            print("5. Shutting down...")
            client.shutdown()

    print("\n✓ Test complete")
    return 0


if __name__ == "__main__":
    sys.exit(main())
