#!/usr/bin/env python3
"""
IronClaw Agent - Reasoning Loop
================================

This module implements the agent decision-making logic.
Forked from Nanobot core philosophy.

Invariant: Must remain under 4,000 lines of code (enforced by CI/CD).

Architecture Principles:
- Minimal: Under 4,000 LOC for auditability
- Deterministic: No randomness in core logic
- Secure: All tool use goes through MCP client
- Observable: All decisions logged
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional
from dataclasses import dataclass
from enum import Enum
import os
import sys

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

try:
    # When imported as module
    from agent.mcp_client import McpClient, McpError
except ImportError:
    # When run directly
    from mcp_client import McpClient, McpError


class ActionKind(Enum):
    """Type of action (for Approval Cliff)"""

    GREEN = "green"  # Autonomous: read-only, safe
    RED = "red"  # Requires approval: destructive, external


@dataclass
class ToolCall:
    """A tool call request"""

    name: str
    arguments: Dict[str, Any]
    action_kind: ActionKind


@dataclass
class AgentState:
    """Current state of the agent"""

    messages: List[Dict[str, Any]]
    tools: List[str]
    context: Dict[str, Any]

    def add_message(self, role: str, content: str) -> None:
        """Add a message to the history"""
        self.messages.append({"role": role, "content": content})


def think(state: AgentState) -> Optional[ToolCall]:
    """
    Main reasoning loop - decides next action based on state.

    This is the core "brain" of IronClaw. It analyzes:
    1. Current task and context
    2. Available tools
    3. Message history
    4. Desired outcome

    Returns:
        ToolCall if action needed, None if task complete

    Invariant:
        Must remain deterministic and observable.
        All logging must be explicit.
    """
    # TODO: Implement Nanobot-style reasoning loop
    # For now, this is a placeholder that returns None
    return None


def execute_tool(call: ToolCall, mcp_client) -> Dict[str, Any]:
    """
    Execute a tool via MCP connection.

    Args:
        call: ToolCall with name and arguments
        mcp_client: McpClient instance (from mcp_client.py)

    Returns:
        Tool execution result

    Note:
        This communicates with the Rust Orchestrator's MCP client.
        All tool execution happens inside JIT Micro-VMs (future).

    Example:
        >>> client = McpClient("filesystem", ["npx", "-y", "@server"])
        >>> client.initialize()
        >>> execute_tool(tool_call, client)
    """
    try:
        result = mcp_client.call_tool(call.name, call.arguments)
        return {
            "status": "ok",
            "result": result,
            "action_kind": call.action_kind.value,
        }
    except Exception as e:
        return {
            "status": "error",
            "error": str(e),
            "action_kind": call.action_kind.value,
        }


def run_loop(
    task: str, tools: List[str], mcp_client: Optional[McpClient] = None
) -> AgentState:
    """
    Run the agent reasoning loop for a given task.

    This is the main entry point for the agent.

    Args:
        task: User task description
        tools: List of available tools (currently informational only)
        mcp_client: Optional McpClient instance for tool execution

    Returns:
        Final agent state

    Loop:
        1. Think: Decide next action
        2. Execute: Run tool (if action chosen)
        3. Update: Add result to state
        4. Repeat: Until task complete

    Example:
        >>> client = McpClient("filesystem", ["npx", "-y", "@server"])
        >>> client.spawn()
        >>> client.initialize()
        >>> state = run_loop("Read /tmp/test.txt", ["read_file"], client)
        >>> client.shutdown()
    """
    state = AgentState(
        messages=[{"role": "user", "content": task}], tools=tools, context={}
    )

    max_iterations = 100
    iteration = 0

    while iteration < max_iterations:
        # Think about next action
        action = think(state)

        if action is None:
            # Task complete
            break

        # Execute tool (if MCP client provided)
        if mcp_client is not None:
            result = execute_tool(action, mcp_client)
        else:
            # Fallback: Mock execution
            result = {
                "status": "mock",
                "result": f"Mock execution of {action.name}",
                "action_kind": action.action_kind.value,
            }

        # Update state with result
        state.add_message("tool", str(result))

        iteration += 1

    return state


if __name__ == "__main__":
    # CLI entry point for testing
    import sys

    if len(sys.argv) > 1:
        task = sys.argv[1]
    else:
        task = "Hello, IronClaw!"

    state = run_loop(task, ["read_file", "write_file", "search"])
    print(f"Final state: {len(state.messages)} messages")
