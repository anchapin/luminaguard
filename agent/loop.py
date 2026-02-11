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

import json
import os
import re
import sys
from dataclasses import dataclass
from enum import Enum
from typing import Any, Dict, List, Optional, Tuple

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

try:
    # When imported as module
    from agent.mcp_client import McpClient, McpError, Tool
    from agent.llm_client import LlmClient, LlmError
except ImportError:
    # When run directly or in tests without package structure
    try:
        from mcp_client import McpClient, McpError, Tool
        from llm_client import LlmClient, LlmError
    except ImportError:
        # Fallback for circular imports or when running from root
        from agent.mcp_client import McpClient, McpError, Tool
        from agent.llm_client import LlmClient, LlmError


class ActionKind(Enum):
    """Type of action (for Approval Cliff)"""

    GREEN = "green"  # Autonomous: read-only, safe
    RED = "red"  # Requires approval: destructive, external


# Keywords for action classification
GREEN_KEYWORDS = ("read_", "list_", "get_", "search_", "find_")
RED_KEYWORDS = ("write_", "delete_", "create_", "update_", "send_", "execute_")


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
    tools: List[Tool]
    context: Dict[str, Any]

    def add_message(self, role: str, content: str) -> None:
        """Add a message to the history"""
        self.messages.append({"role": role, "content": content})


def determine_action_kind(tool_name: str) -> ActionKind:
    """
    Determine if an action is GREEN (safe) or RED (requires approval).

    Heuristic based on tool naming patterns:
    - Green: read_*, list_*, get_*, search_*, find_*
    - Red: write_*, delete_*, create_*, update_*, send_*, execute_*
    - Default: Red (safe by default)
    """
    if tool_name.startswith(GREEN_KEYWORDS):
        return ActionKind.GREEN

    # Default to RED for safety
    return ActionKind.RED


def present_diff_card(action: ToolCall) -> str:
    """
    Present a Diff Card for user approval.
    """
    return f"I am about to {action.name}. Approve?"


def construct_system_prompt(tools: List[Tool]) -> str:
    """
    Construct the ReAct system prompt with tool definitions.
    """
    tool_descriptions = []
    for tool in tools:
        # Schema might be None or empty
        schema = json.dumps(tool.input_schema, indent=2) if tool.input_schema else "{}"
        tool_descriptions.append(
            f"<tool_definition>\n"
            f"<name>{tool.name}</name>\n"
            f"<description>{tool.description}</description>\n"
            f"<parameters>\n{schema}\n</parameters>\n"
            f"</tool_definition>"
        )

    tools_xml = "\n".join(tool_descriptions)

    return f"""You are IronClaw, a secure autonomous agent.

You have access to the following tools:

<tools>
{tools_xml}
</tools>

Instructions:
1. Analyze the user's request and the current state.
2. Decide on the next step.
3. Use a tool if necessary to gather information or perform an action.
4. If the task is complete, return a final answer.

Format your response as follows:
<thought>
Explain your reasoning here.
</thought>
<function_calls>
<function_call name="tool_name">
<arg name="arg_name">value</arg>
...
</function_call>
</function_calls>

Start with a <thought> block.
"""


def parse_response(response: str) -> Tuple[str, Optional[ToolCall]]:
    """
    Parse LLM response for thought and tool call.

    Returns:
        Tuple of (thought_content, ToolCall or None)
    """
    # Extract thought
    thought_match = re.search(r"<thought>(.*?)</thought>", response, re.DOTALL)
    thought = thought_match.group(1).strip() if thought_match else ""

    # Extract function calls
    # We look for <function_calls>...</function_calls>
    # Inside, we look for <function_call name="...">...</function_call>

    calls_match = re.search(
        r"<function_calls>(.*?)</function_calls>", response, re.DOTALL
    )
    if not calls_match:
        return thought, None

    calls_content = calls_match.group(1)

    # Naive XML parsing for function_call
    # Using regex to find the first function call (IronClaw currently supports one call per turn)
    # TODO: Support multiple calls if needed

    call_match = re.search(
        r'<function_call name="(.*?)">(.*?)</function_call>', calls_content, re.DOTALL
    )
    if not call_match:
        return thought, None

    tool_name = call_match.group(1)
    args_content = call_match.group(2)

    # Parse arguments
    # <arg name="key">value</arg>
    arguments = {}
    arg_matches = re.finditer(r'<arg name="(.*?)">(.*?)</arg>', args_content, re.DOTALL)
    for match in arg_matches:
        key = match.group(1)
        value = match.group(2).strip()
        arguments[key] = value

    action_kind = determine_action_kind(tool_name)

    return thought, ToolCall(
        name=tool_name, arguments=arguments, action_kind=action_kind
    )


def think(
    state: AgentState, llm_client: Optional[LlmClient] = None
) -> Optional[ToolCall]:
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
    if not llm_client:
        # Cannot think without a brain
        print("Warning: No LLM client provided to think()")
        return None

    # 1. Construct system prompt with tools
    system_prompt = construct_system_prompt(state.tools)

    # 2. Prepare messages for LLM
    # Ensure all content is string
    llm_messages = []
    for msg in state.messages:
        llm_messages.append({"role": msg["role"], "content": str(msg["content"])})

    # 3. Call LLM
    try:
        response = llm_client.complete(
            messages=llm_messages,
            system=system_prompt,
            temperature=0.0,  # Deterministic
            max_tokens=4096,
        )
    except Exception as e:
        print(f"Error during LLM completion: {e}")
        return None

    # 4. Update state with assistant's response (maintains history)
    state.add_message("assistant", response)

    # 5. Parse response
    thought, tool_call = parse_response(response)

    if thought:
        # Logging/Observation
        # In a real system, we might stream this.
        # Here we just rely on it being in history.
        pass

    return tool_call


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
    mcp_client: Optional[McpClient] = None,
    tools: Optional[List[Tool]] = None,
    llm_client: Optional[LlmClient] = None,
) -> AgentState:
    """
    Run the agent reasoning loop for a given task.

    Args:
        task: User task description
        mcp_client: Optional McpClient instance for tool execution
        tools: Optional list of tools (if mcp_client not provided or for testing)
        llm_client: Optional LlmClient instance (if None, attempts to create one)

    Returns:
        Final agent state
    """
    # Initialize LLM client if not provided
    if not llm_client:
        try:
            llm_client = LlmClient()
        except Exception as e:
            print(f"Warning: Failed to initialize LLM client: {e}")
            llm_client = None

    # Fetch tools from MCP client if available
    available_tools = []
    if mcp_client:
        try:
            available_tools = mcp_client.list_tools()
        except Exception as e:
            print(f"Warning: Failed to list tools: {e}")
            available_tools = []
    elif tools:
        available_tools = tools

    state = AgentState(
        messages=[{"role": "user", "content": task}], tools=available_tools, context={}
    )

    max_iterations = 100
    iteration = 0

    while iteration < max_iterations:
        # Think about next action
        action = think(state, llm_client)

        if action is None:
            # Task complete or no LLM
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
        # The result message should be from 'user' or a specific 'tool' role if the LLM supports it.
        # Claude typically uses 'user' for tool results in the prompt if not using native tool calling.
        # But here we are using ReAct prompting, so 'user' role is appropriate for observations.

        observation_content = f"<observation>\n{result}\n</observation>"
        state.add_message("user", observation_content)

        iteration += 1

    return state


if __name__ == "__main__":
    # CLI entry point for testing
    import sys

    if len(sys.argv) > 1:
        task = sys.argv[1]
    else:
        task = "Hello, IronClaw!"

    # Dummy tools for testing
    dummy_tools = [
        Tool(
            name="read_file", description="Read file", input_schema={"path": "string"}
        ),
        Tool(
            name="write_file",
            description="Write file",
            input_schema={"path": "string", "content": "string"},
        ),
    ]

    print(f"Running agent with task: {task}")
    state = run_loop(task, tools=dummy_tools)
    print(f"Final state: {len(state.messages)} messages")
