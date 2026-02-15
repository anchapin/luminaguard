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
    from agent.mcp_client import McpClient
except ImportError:
    # When run directly
    from mcp_client import McpClient


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


def think(state: AgentState, llm_client=None) -> Optional[ToolCall]:
    """
    Main reasoning loop - decides next action based on state.

    This is the core "brain" of LuminaGuard. It analyzes:
    1. Current task and context
    2. Available tools
    3. Message history
    4. Desired outcome

    Args:
        state: Current agent state with messages and context
        llm_client: Optional LLM client (defaults to MockLLMClient)

    Returns:
        ToolCall if action needed, None if task complete

    Invariant:
        Must remain deterministic and observable.
        All logging must be explicit.

    Multi-turn reasoning:
        The agent can think multiple times as it processes tool results
        and updates its understanding of the task.
    """
    # Import LLM client (lazy import to avoid circular dependency)
    try:
        from llm_client import MockLLMClient, LLMConfig

        # Create LLM client if not provided
        if llm_client is None:
            llm_client = MockLLMClient(LLMConfig())

        # Use LLM to decide next action
        response = llm_client.decide_action(
            messages=state.messages,
            available_tools=state.tools,
            context=state.context,
        )

        # Log reasoning (observable invariant)
        print(f"  Reasoning: {response.reasoning}")

        # If task is complete, return None
        if response.is_complete or response.tool_name is None:
            return None

        # Determine action kind based on tool name (security invariant)
        action_kind = determine_action_kind(response.tool_name)

        # Create tool call
        tool_call = ToolCall(
            name=response.tool_name,
            arguments=response.arguments,
            action_kind=action_kind,
        )

        return tool_call

    except ImportError:
        # Fallback to simple implementation if llm_client not available
        # This should not happen in normal operation
        print("  Warning: LLM client not available, using fallback")
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
    task: str,
    tools: List[str],
    mcp_client: Optional[McpClient] = None,
    llm_client=None,
) -> AgentState:
    """
    Run the agent reasoning loop for a given task.

    This is the main entry point for the agent.

    Args:
        task: User task description
        tools: List of available tools (currently informational only)
        mcp_client: Optional McpClient instance for tool execution
        llm_client: Optional LLM client for reasoning (defaults to MockLLMClient)

    Returns:
        Final agent state

    Loop:
        1. Think: Decide next action
        2. Execute: Run tool (if action chosen)
        3. Update: Add result to state
        4. Repeat: Until task complete

    Multi-turn reasoning:
        The agent can execute multiple tools in sequence as it works
        through complex tasks, using the LLM to decide each step.

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
        # Think about next action (using LLM for decision-making)
        print(f"\n🧠 Thinking... (Iteration {iteration + 1}/{max_iterations})")
        action = think(state, llm_client)

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
    if "delete" in action.name or "remove" in action.name:
        return "CRITICAL (Permanent deletion)"
    if "write" in action.name or "edit" in action.name:
        return "HIGH (Destructive)"
    return "MEDIUM (External action)"
