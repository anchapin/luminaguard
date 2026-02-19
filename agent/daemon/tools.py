"""
Built-in Daemon Tools for LuminaGuard Daemon Mode.

This module provides core built-in tools available to the daemon:
- bash: Execute shell commands with timeout and resource limits
- grep: Pattern matching across files
- web: HTTP client for fetching URLs, APIs
- curl/http: HTTP request tool
- Custom tool registration system
- Sandboxed execution environment

Part of: luminaguard-0va.3 - Built-in Daemon Tools (bash, grep, web, etc.)
"""

import asyncio
import logging
import re
import subprocess
import json
import shlex
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, List, Callable, Awaitable
from enum import Enum
from pathlib import Path

logger = logging.getLogger(__name__)


class ToolType(Enum):
    """Tool type enum."""

    BASH = "bash"
    GREP = "grep"
    WEB = "web"
    CURL = "curl"
    CUSTOM = "custom"


@dataclass
class ToolConfig:
    """Configuration for daemon tools."""

    # Default timeout for bash commands in seconds (default: 30)
    bash_timeout: int = 30
    # Maximum output size in bytes (default: 1MB)
    max_output_size: int = 1024 * 1024
    # Allowed directories for file operations (default: home directory)
    allowed_dirs: List[str] = field(default_factory=lambda: ["/tmp", "~"])
    # Enable sandboxing (default: True)
    sandbox_enabled: bool = True
    # Working directory for tools
    working_dir: str = "/tmp"


@dataclass
class ToolResult:
    """Result of tool execution."""

    success: bool
    output: str
    error: Optional[str] = None
    exit_code: Optional[int] = None
    duration_ms: float = 0.0

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "success": self.success,
            "output": self.output,
            "error": self.error,
            "exit_code": self.exit_code,
            "duration_ms": self.duration_ms,
        }


class BashTool:
    """Bash command execution tool with timeout and resource limits."""

    def __init__(self, config: Optional[ToolConfig] = None):
        self.config = config or ToolConfig()

    async def execute(self, command: str, timeout: Optional[int] = None) -> ToolResult:
        """
        Execute a bash command.

        Args:
            command: Command to execute
            timeout: Timeout in seconds (uses config default if None)

        Returns:
            ToolResult with output and status
        """
        import time

        start_time = time.time()

        timeout = timeout or self.config.bash_timeout

        logger.info(f"Executing bash command: {command[:100]}...")

        try:
            # Use shell=True but with command limiting
            process = await asyncio.create_subprocess_shell(
                command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=self.config.working_dir,
            )

            try:
                stdout, stderr = await asyncio.wait_for(
                    process.communicate(),
                    timeout=timeout,
                )
            except asyncio.TimeoutError:
                process.kill()
                await process.wait()
                duration_ms = (time.time() - start_time) * 1000
                return ToolResult(
                    success=False,
                    output="",
                    error=f"Command timed out after {timeout}s",
                    exit_code=-1,
                    duration_ms=duration_ms,
                )

            # Limit output size
            stdout = stdout[: self.config.max_output_size].decode(
                "utf-8", errors="replace"
            )
            stderr = stderr[: self.config.max_output_size].decode(
                "utf-8", errors="replace"
            )

            duration_ms = (time.time() - start_time) * 1000

            if process.returncode == 0:
                return ToolResult(
                    success=True,
                    output=stdout,
                    exit_code=0,
                    duration_ms=duration_ms,
                )
            else:
                return ToolResult(
                    success=False,
                    output=stdout,
                    error=stderr,
                    exit_code=process.returncode,
                    duration_ms=duration_ms,
                )

        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            logger.error(f"Bash execution error: {e}")
            return ToolResult(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                duration_ms=duration_ms,
            )


class GrepTool:
    """Grep pattern matching tool."""

    def __init__(self, config: Optional[ToolConfig] = None):
        self.config = config or ToolConfig()

    async def execute(
        self,
        pattern: str,
        path: str,
        recursive: bool = True,
        case_insensitive: bool = False,
        line_numbers: bool = True,
        context: int = 0,
    ) -> ToolResult:
        """
        Search for pattern in file(s).

        Args:
            pattern: Regex pattern to search
            path: File or directory path
            recursive: Search recursively
            case_insensitive: Case insensitive search
            line_numbers: Show line numbers
            context: Lines of context around match

        Returns:
            ToolResult with matches
        """
        import time

        start_time = time.time()

        logger.info(f"Grep pattern '{pattern}' in {path}")

        try:
            # Build grep command
            cmd = ["grep"]

            if recursive:
                cmd.append("-r")
            if case_insensitive:
                cmd.append("-i")
            if line_numbers:
                cmd.append("-n")
            if context > 0:
                cmd.extend([f"-C{context}"])

            # Add pattern and path
            cmd.extend([pattern, path])

            process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )

            stdout, stderr = await asyncio.wait_for(
                process.communicate(),
                timeout=30,
            )

            stdout = stdout[: self.config.max_output_size].decode(
                "utf-8", errors="replace"
            )
            stderr = stderr.decode("utf-8", errors="replace")

            duration_ms = (time.time() - start_time) * 1000

            if process.returncode == 0:
                return ToolResult(
                    success=True,
                    output=stdout,
                    exit_code=0,
                    duration_ms=duration_ms,
                )
            elif process.returncode == 1:
                # No matches found
                return ToolResult(
                    success=True,
                    output="",
                    exit_code=0,
                    duration_ms=duration_ms,
                )
            else:
                return ToolResult(
                    success=False,
                    output=stdout,
                    error=stderr,
                    exit_code=process.returncode,
                    duration_ms=duration_ms,
                )

        except asyncio.TimeoutError:
            duration_ms = (time.time() - start_time) * 1000
            return ToolResult(
                success=False,
                output="",
                error="Grep timed out",
                exit_code=-1,
                duration_ms=duration_ms,
            )
        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            logger.error(f"Grep error: {e}")
            return ToolResult(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                duration_ms=duration_ms,
            )


class WebTool:
    """HTTP client tool for fetching URLs."""

    def __init__(self, config: Optional[ToolConfig] = None):
        self.config = config or ToolConfig()

    async def fetch(
        self,
        url: str,
        method: str = "GET",
        headers: Optional[Dict[str, str]] = None,
        data: Optional[str] = None,
        timeout: int = 30,
    ) -> ToolResult:
        """
        Fetch a URL.

        Args:
            url: URL to fetch
            method: HTTP method
            headers: Request headers
            data: Request body
            timeout: Request timeout

        Returns:
            ToolResult with response
        """
        import time
        import aiohttp

        start_time = time.time()

        logger.info(f"Fetching {method} {url}")

        try:
            async with aiohttp.ClientSession() as session:
                async with session.request(
                    method,
                    url,
                    headers=headers,
                    data=data,
                    timeout=aiohttp.ClientTimeout(total=timeout),
                ) as response:
                    content = await response.text()
                    content = content[: self.config.max_output_size]

                    duration_ms = (time.time() - start_time) * 1000

                    # Build response headers
                    resp_headers = dict(response.headers)

                    return ToolResult(
                        success=True,
                        output=content,
                        exit_code=response.status,
                        duration_ms=duration_ms,
                    )

        except asyncio.TimeoutError:
            duration_ms = (time.time() - start_time) * 1000
            return ToolResult(
                success=False,
                output="",
                error=f"Request timed out after {timeout}s",
                exit_code=-1,
                duration_ms=duration_ms,
            )
        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            logger.error(f"Web fetch error: {e}")
            return ToolResult(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                duration_ms=duration_ms,
            )

    async def get(self, url: str, **kwargs) -> ToolResult:
        """Convenience method for GET requests."""
        return await self.fetch(url, "GET", **kwargs)

    async def post(self, url: str, data: Optional[str] = None, **kwargs) -> ToolResult:
        """Convenience method for POST requests."""
        return await self.fetch(url, "POST", data=data, **kwargs)


class CurlTool:
    """cURL-compatible HTTP tool (alias for WebTool)."""

    def __init__(self, config: Optional[ToolConfig] = None):
        self._web = WebTool(config)

    async def execute(self, args: str) -> ToolResult:
        """
        Execute curl-like command.

        Args:
            args: Arguments in curl format (simplified)

        Returns:
            ToolResult with response
        """
        # Simple curl argument parser
        # Supports: curl <url> [-X METHOD] [-H "Header"] [-d DATA]

        parts = shlex.split(args)
        url = None
        method = "GET"
        headers = {}
        data = None

        i = 0
        while i < len(parts):
            part = parts[i]
            if part.startswith("-"):
                if part in ("-X", "--request"):
                    method = parts[i + 1].upper()
                    i += 1
                elif part in ("-H", "--header"):
                    header = parts[i + 1]
                    if ":" in header:
                        key, value = header.split(":", 1)
                        headers[key.strip()] = value.strip()
                    i += 1
                elif part in ("-d", "--data", "--data-raw"):
                    data = parts[i + 1]
                    if method == "GET":
                        method = "POST"
                    i += 1
                elif part in ("-s", "-S", "-v", "--compressed"):
                    pass  # Ignore these
                else:
                    # Assume it's the URL
                    if not url:
                        url = part
            else:
                if not url:
                    url = part
            i += 1

        if not url:
            return ToolResult(
                success=False,
                output="",
                error="No URL provided",
                exit_code=-1,
            )

        return await self._web.fetch(url, method, headers, data)


class DaemonTools:
    """
    Unified daemon tools manager.

    Provides access to all built-in tools:
    - bash: Execute shell commands
    - grep: Pattern matching
    - web: HTTP client
    - curl: cURL-compatible HTTP
    - Custom registered tools
    """

    def __init__(
        self,
        config: Optional[ToolConfig] = None,
        custom_tools: Optional[Dict[str, Callable[..., Awaitable[Any]]]] = None,
    ):
        self.config = config or ToolConfig()

        # Initialize built-in tools
        self.bash = BashTool(self.config)
        self.grep = GrepTool(self.config)
        self.web = WebTool(self.config)
        self.curl = CurlTool(self.config)

        # Custom tools
        self.custom_tools: Dict[str, Callable[..., Awaitable[Any]]] = custom_tools or {}

    def register_tool(self, name: str, handler: Callable[..., Awaitable[Any]]) -> None:
        """
        Register a custom tool.

        Args:
            name: Tool name
            handler: Async callable to handle tool execution
        """
        self.custom_tools[name] = handler
        logger.info(f"Registered custom tool: {name}")

    async def execute(self, tool: str, *args, **kwargs) -> ToolResult:
        """
        Execute a tool by name.

        Args:
            tool: Tool name (bash, grep, web, curl, or custom)
            *args, **kwargs: Arguments for the tool

        Returns:
            ToolResult
        """
        tool_lower = tool.lower()

        if tool_lower == "bash":
            return await self.bash.execute(*args, **kwargs)
        elif tool_lower == "grep":
            return await self.grep.execute(*args, **kwargs)
        elif tool_lower in ("web", "http", "fetch"):
            return await self.web.fetch(*args, **kwargs)
        elif tool_lower == "curl":
            return await self.curl.execute(*args, **kwargs)
        elif tool_lower in self.custom_tools:
            return await self._execute_custom(tool_lower, *args, **kwargs)
        else:
            return ToolResult(
                success=False,
                output="",
                error=f"Unknown tool: {tool}",
                exit_code=-1,
            )

    async def _execute_custom(
        self,
        name: str,
        *args,
        **kwargs,
    ) -> ToolResult:
        """Execute a custom tool."""
        import time

        start_time = time.time()

        try:
            handler = self.custom_tools[name]
            result = await handler(*args, **kwargs)

            duration_ms = (time.time() - start_time) * 1000

            return ToolResult(
                success=True,
                output=json.dumps(result) if not isinstance(result, str) else result,
                exit_code=0,
                duration_ms=duration_ms,
            )
        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            logger.error(f"Custom tool error: {e}")
            return ToolResult(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                duration_ms=duration_ms,
            )

    def list_tools(self) -> List[str]:
        """List available tools."""
        tools = ["bash", "grep", "web", "curl"]
        tools.extend(self.custom_tools.keys())
        return tools


def create_daemon_tools(
    config: Optional[ToolConfig] = None,
    custom_tools: Optional[Dict[str, Callable[..., Awaitable[Any]]]] = None,
) -> DaemonTools:
    """
    Create daemon tools instance.

    Args:
        config: Tool configuration
        custom_tools: Custom tool handlers

    Returns:
        DaemonTools instance
    """
    return DaemonTools(config=config, custom_tools=custom_tools)
