#!/usr/bin/env python3
"""
LuminaGuard Agent - Reasoning Loop
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


class Style:
    """Terminal styling helper that respects NO_COLOR"""

    _no_color = bool(os.environ.get("NO_COLOR"))

    BOLD = "" if _no_color else "\033[1m"
    CYAN = "" if _no_color else "\033[36m"
    RESET = "" if _no_color else "\033[0m"

    @classmethod
    def bold(cls, text: str) -> str:
        return f"{cls.BOLD}{text}{cls.RESET}"

    @classmethod
    def cyan(cls, text: str) -> str:
        return f"{cls.CYAN}{text}{cls.RESET}"


class ActionKind(Enum):
    """Type of action (for Approval Cliff)"""

    GREEN = "green"  # Autonomous: read-only, safe
    RED = "red"  # Requires approval: destructive, external


# Keywords for automatic action classification
GREEN_KEYWORDS = [
    "read",
    "list",
    "search",
    "check",
    "get",
    "show",
    "view",
    "display",
    "find",
    "locate",
    "query",
    "fetch",
    "inspect",
    "examine",
    "monitor",
    "status",
    "info",
    "help",
]

RED_KEYWORDS = [
    "delete",
    "remove",
    "write",
    "edit",
    "modify",
    "create",
    "update",
    "change",
    "send",
    "post",
    "transfer",
    "execute",
    "run",
    "deploy",
    "install",
    "uninstall",
    "commit",
    "push",
    "publish",
]


@dataclass
class ToolCall:
    """A tool call request"""

    name: str
    arguments: Dict[str, Any]
    action_kind: ActionKind


def determine_action_kind(message: str) -> ActionKind:
    """
    Determine if an action is GREEN (autonomous) or RED (requires approval).

    This function uses keyword matching to classify actions based on safety.
    Unknown actions default to RED for safety (fail-secure).

    Args:
        message: The action description or tool name

    Returns:
        ActionKind.GREEN if the action is safe, ActionKind.RED otherwise

    Examples:
        >>> determine_action_kind("read_file")
        <ActionKind.GREEN: 'green'>
        >>> determine_action_kind("delete_file")
        <ActionKind.RED: 'red'>
    """
    message_lower = message.lower()

    # Check for red keywords first (more restrictive)
    for keyword in RED_KEYWORDS:
        if keyword in message_lower:
            return ActionKind.RED

    # Check for green keywords
    for keyword in GREEN_KEYWORDS:
        if keyword in message_lower:
            return ActionKind.GREEN

    # Default: RED (safe by default)
    return ActionKind.RED


def present_diff_card(action: ToolCall) -> bool:
    """
    Present the Diff Card UI for an action requiring approval.

    This function integrates with the Rust orchestrator's TUI for Red actions.
    Green actions auto-approve without UI.

    Args:
        action: The ToolCall to present

    Returns:
        True if approved, False otherwise

    Note:
        - Green actions: Auto-approve
        - Red actions: Present TUI via Rust orchestrator
        - Timeout: Auto-reject after 5 minutes (configurable via env var)
        - Works over SSH (no GUI requirement)
        - Full audit logging of all decisions

    Examples:
        >>> green_action = ToolCall("read_file", {"path": "test.txt"}, ActionKind.GREEN)
        >>> present_diff_card(green_action)
        True

        >>> red_action = ToolCall("delete_file", {"path": "test.txt"}, ActionKind.RED)
        >>> # Presents TUI, returns True if user approves

    Examples:
        >>> green_action = ToolCall("read_file", {"path": "test.txt"}, ActionKind.GREEN)
        >>> present_diff_card(green_action)
        True

        >>> red_action = ToolCall("delete_file", {"path": "test.txt"}, ActionKind.RED)
        >>> # Would prompt user in real implementation
    """
    # Try to import approval_client for Phase 2 TUI integration
    try:
        from approval_client import present_diff_card as present_diff_card_tui

        return present_diff_card_tui(action)
    except ImportError:
        # Fallback to simple implementation if approval_client not available
        if action.action_kind == ActionKind.GREEN:
            # Green actions auto-approve
            return True
        else:
            # Red actions require approval - simple prompt as fallback
            print("\n" + "=" * 80)
            print(f"Action: {action.name}")
            print(f"Type: {action.action_kind.value.upper()}")
            print(f"Arguments: {action.arguments}")
            print("=" * 80)
            print(f"\nRisk Level: {_get_risk_display(action)}")
            print("\nApprove this action? (y/n): ", end="")

            response = input().strip().lower()

            return response in ("y", "yes")


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

    This is the core "brain" of LuminaGuard. It analyzes:
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
    # Check if we have already executed a tool (simple one-shot agent for now)
    tool_responses = [m for m in state.messages if m["role"] == "tool"]
    if len(tool_responses) > 0:
        return None

    # Get the last user message
    user_msgs = [m for m in state.messages if m["role"] == "user"]
    if not user_msgs:
        return None

    content = user_msgs[-1]["content"].lower()

    # Simple keyword-based reasoning for testing
    # TODO: Replace with real LLM reasoning logic in Phase 2
    if "read" in content:
        return ToolCall(
            name="read_file",
            arguments={"path": "test.txt"},
            action_kind=ActionKind.GREEN,
        )
    elif "write" in content:
        return ToolCall(
            name="write_file",
            arguments={"path": "test.txt", "content": "Hello"},
            action_kind=ActionKind.GREEN,
        )

    # Default: Task complete
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
    print(f"\n🚀 Starting task: {Style.bold(task)}")
    state = AgentState(
        messages=[{"role": "user", "content": task}], tools=tools, context={}
    )

    max_iterations = 100
    iteration = 0

    while iteration < max_iterations:
        # Think about next action
        print(f"\n🧠 Thinking... (Iteration {iteration + 1}/{max_iterations})")
        action = think(state)

        if action is None:
            # Task complete
            print("\n✅ Task complete!")
            break

        print(f"🛠️  Executing tool: {Style.cyan(action.name)}")
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
        task = "Hello, LuminaGuard!"

    state = run_loop(task, ["read_file", "write_file", "search"])
    print(f"Final state: {len(state.messages)} messages")


def _get_risk_display(action: ToolCall) -> str:
    """Get human-readable risk level for an action"""
    if action.action_kind == ActionKind.GREEN:
        return "GREEN (Safe)"
    elif "delete" in action.name or "remove" in action.name:
        return "CRITICAL (Permanent deletion)"
    elif "write" in action.name or "edit" in action.name:
        return "HIGH (Destructive)"
    else:
        return "MEDIUM (External action)"
