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

Execution Modes:
- host: Agent runs on host, tools execute via MCP to host servers
- vm: Agent runs inside Firecracker VM, communicates with host via vsock
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
    from agent.vsock_client import VsockClient
except ImportError:
    # When run directly
    from mcp_client import McpClient, McpError
    from vsock_client import VsockClient


class ExecutionMode(Enum):
    """Execution mode for the agent"""
    HOST = "host"  # Agent runs on host (default)
    VM = "vm"      # Agent runs inside VM, communicates via vsock


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
    """
    message_lower = message.lower()

    for keyword in RED_KEYWORDS:
        if keyword in message_lower:
            return ActionKind.RED

    for keyword in GREEN_KEYWORDS:
        if keyword in message_lower:
            return ActionKind.GREEN

    return ActionKind.RED


def present_diff_card(action: ToolCall) -> bool:
    """Present the Diff Card UI for an action requiring approval."""
    try:
        from approval_client import present_diff_card as present_diff_card_tui
        return present_diff_card_tui(action)
    except ImportError:
        if action.action_kind == ActionKind.GREEN:
            return True
        else:
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
class Session:
    """Agent session - maintains state across multiple task executions"""
    
    session_id: str
    created_at: float
    last_activity: float
    state: AgentState
    metadata: Dict[str, Any]
    
    def is_expired(self, ttl_seconds: int = 3600) -> bool:
        """Check if session has expired based on TTL"""
        import time
        return (time.time() - self.last_activity) > ttl_seconds
    
    def update_activity(self) -> None:
        """Update last activity timestamp"""
        import time
        self.last_activity = time.time()


class SessionManager:
    """Manages agent sessions across multiple executions"""
    
    def __init__(self, ttl_seconds: int = 3600):
        self.sessions: Dict[str, Session] = {}
        self.ttl_seconds = ttl_seconds
    
    def create_session(self, session_id: str, tools: List[str]) -> Session:
        """Create a new session"""
        import time
        state = AgentState(
            messages=[],
            tools=tools,
            context={}
        )
        session = Session(
            session_id=session_id,
            created_at=time.time(),
            last_activity=time.time(),
            state=state,
            metadata={}
        )
        self.sessions[session_id] = session
        return session
    
    def get_session(self, session_id: str) -> Optional[Session]:
        """Get existing session or None"""
        session = self.sessions.get(session_id)
        if session and session.is_expired(self.ttl_seconds):
            del self.sessions[session_id]
            return None
        return session
    
    def remove_session(self, session_id: str) -> None:
        """Remove a session"""
        self.sessions.pop(session_id, None)
    
    def cleanup_expired(self) -> int:
        """Remove expired sessions, return count removed"""
        import time
        current_time = time.time()
        expired = [
            sid for sid, sess in self.sessions.items()
            if (current_time - sess.last_activity) > self.ttl_seconds
        ]
        for sid in expired:
            del self.sessions[sid]
        return len(expired)


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
    """Main reasoning loop - decides next action based on state."""
    try:
        from llm_client import MockLLMClient
        if llm_client is None:
            llm_client = MockLLMClient()

        response = llm_client.decide_action(
            messages=state.messages,
            available_tools=state.tools,
            context=state.context,
        )

        if response.is_complete or response.tool_name is None:
            return None

        action_kind = determine_action_kind(response.tool_name)
        return ToolCall(
            name=response.tool_name,
            arguments=response.arguments,
            action_kind=action_kind,
        )
    except ImportError:
        tool_responses = [m for m in state.messages if m["role"] == "tool"]
        if len(tool_responses) > 0:
            return None

        user_msgs = [m for m in state.messages if m["role"] == "user"]
        if not user_msgs:
            return None

        content = user_msgs[-1]["content"].lower()

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

        return None


def execute_tool(call: ToolCall, mcp_client) -> Dict[str, Any]:
    """Execute a tool via MCP connection."""
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


def execute_tool_vm(call: ToolCall, vsock_client: VsockClient) -> Dict[str, Any]:
    """Execute a tool via vsock connection (when running inside VM)."""
    try:
        result = vsock_client.execute_tool(call.name, call.arguments)
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


def get_execution_mode() -> ExecutionMode:
    """Determine execution mode from environment."""
    mode = os.environ.get("LUMINAGUARD_MODE", "host").lower()
    try:
        return ExecutionMode(mode)
    except ValueError:
        return ExecutionMode.HOST


def run_loop(
    task: str,
    tools: List[str],
    mcp_client: Optional[McpClient] = None,
    vsock_client: Optional[VsockClient] = None,
    execution_mode: Optional[ExecutionMode] = None,
) -> AgentState:
    """Run the agent reasoning loop for a given task."""
    if execution_mode is None:
        execution_mode = get_execution_mode()

    print(f"\n🚀 Starting task: {Style.bold(task)}")
    print(f"📍 Execution mode: {execution_mode.value}")

    state = AgentState(
        messages=[{"role": "user", "content": task}],
        tools=tools,
        context={"mode": execution_mode.value},
    )

    max_iterations = 100
    iteration = 0

    while iteration < max_iterations:
        print(f"\n🧠 Thinking... (Iteration {iteration + 1}/{max_iterations})")
        action = think(state)

        if action is None:
            print("\n✅ Task complete!")
            break

        print(f"🛠️  Executing tool: {Style.cyan(action.name)}")

        approved = present_diff_card(action)

        if not approved:
            print(f"\n⚠️  Action rejected by user. Skipping: {action.name}")
            state.add_message("tool", f"REJECTED: {action.name} - user denied approval")
            iteration += 1
            continue

        print(f"✅ Action approved, executing: {action.name}")

        # Execute tool based on mode
        if execution_mode == ExecutionMode.VM and vsock_client:
            result = execute_tool_vm(action, vsock_client)
        elif mcp_client:
            result = execute_tool(action, mcp_client)
        else:
            result = {
                "status": "mock",
                "result": f"Mock execution of {action.name}",
                "action_kind": action.action_kind.value,
            }

        state.add_message("tool", str(result))
        iteration += 1

    return state


def run_loop_vm(task: str, tools: List[str]) -> AgentState:
    """Run the agent inside a VM with vsock communication."""
    vsock_client = VsockClient()
    if not vsock_client.connect():
        print("ERROR: Failed to connect to host via vsock")
        sys.exit(1)

    print("Connected to host via vsock")

    try:
        return run_loop(task, tools, vsock_client=vsock_client, execution_mode=ExecutionMode.VM)
    finally:
        vsock_client.disconnect()


if __name__ == "__main__":
    if len(sys.argv) > 1:
        task = sys.argv[1]
    else:
        task = "Hello, LuminaGuard!"

    mode = get_execution_mode()
    if mode == ExecutionMode.VM:
        state = run_loop_vm(task, ["read_file", "write_file", "search"])
    else:
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
