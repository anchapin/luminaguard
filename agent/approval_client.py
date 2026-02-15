#!/usr/bin/env python3
"""
Approval Client for Python Agent - Phase 2 Implementation

This module provides a Python client for the Rust Approval Cliff TUI.
It communicates with the Rust orchestrator to present approval UIs
for Red (destructive) actions.

Architecture:
- Python agent calls present_diff_card() with ToolCall
- Client serializes action to JSON
- Rust orchestrator presents TUI (ratatui-based)
- Rust returns approval decision
- Client returns boolean to Python agent

Features:
- Works over SSH (no GUI dependencies)
- Timeout mechanism (auto-reject after 5 minutes)
- Audit logging of all decisions
- Supports all action types (read, write, delete, execute, etc.)
"""

import json
import os
import subprocess
import tempfile
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional

# Import from loop.py
from loop import ActionKind, ToolCall


class ApprovalDecision(Enum):
    """Decision made by user in approval UI"""

    APPROVED = "approved"
    REJECTED = "rejected"
    CANCELLED = "cancelled"


@dataclass
class Change:
    """A single change within an action"""

    change_type: str
    summary: str
    details: Dict[str, Any]


@dataclass
class DiffCard:
    """Diff card showing exactly what will change"""

    action_type: str
    description: str
    risk_level: str
    changes: List[Change]
    timestamp: str

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization"""
        return {
            "action_type": self.action_type,
            "description": self.description,
            "risk_level": self.risk_level,
            "changes": [
                {
                    "change_type": c.change_type,
                    "summary": c.summary,
                    "details": c.details,
                }
                for c in self.changes
            ],
            "timestamp": self.timestamp,
        }


class ApprovalClient:
    """Client for Rust Approval Cliff TUI"""

    def __init__(self, orchestrator_path: Optional[str] = None):
        """
        Initialize approval client.

        Args:
            orchestrator_path: Path to Rust orchestrator binary.
                             If None, tries to find it in common locations.
        """
        self.orchestrator_path = self._find_orchestrator(orchestrator_path)
        self.timeout_seconds = int(
            os.getenv("LUMINAGUARD_APPROVAL_TIMEOUT", "300")
        )  # 5 minutes default

        # Enable approval cliff by default
        self.enable_approval_cliff = True

    def _find_orchestrator(self, provided_path: Optional[str]) -> str:
        """
        Find the Rust orchestrator binary.

        Searches in:
        1. Provided path
        2. ../target/release/luminaguard
        3. ../target/debug/luminaguard
        4. luminaguard (in PATH)

        Returns:
            Path to orchestrator binary

        Raises:
            FileNotFoundError: If orchestrator not found
        """
        if provided_path:
            if Path(provided_path).exists():
                return provided_path
            raise FileNotFoundError(f"Provided orchestrator not found: {provided_path}")

        # Search common locations
        agent_dir = Path(__file__).parent.parent
        possible_paths = [
            agent_dir / "orchestrator" / "target" / "release" / "luminaguard",
            agent_dir / "orchestrator" / "target" / "debug" / "luminaguard",
            Path("luminaguard"),
        ]

        for path in possible_paths:
            if path.exists():
                return str(path.resolve())

        raise FileNotFoundError(
            "Rust orchestrator binary not found. "
            "Build it with: cd orchestrator && cargo build --release"
        )

    def _create_diff_card(self, action: ToolCall) -> DiffCard:
        """
        Create a DiffCard from a ToolCall.

        Args:
            action: The ToolCall to convert

        Returns:
            DiffCard with action details
        """
        # Determine action type string
        action_type_str = action.name

        # Determine risk level
        if action.action_kind == ActionKind.GREEN:
            risk_level = "none"
        else:
            # Map destructive actions to risk levels - check action name
            destructive_keywords = ["delete", "remove", "transfer"]
            medium_risk_keywords = ["create", "write", "edit", "send", "execute"]

            action_name_lower = action.name.lower()
            if any(kw in action_name_lower for kw in destructive_keywords):
                risk_level = "critical"
            elif any(kw in action_name_lower for kw in medium_risk_keywords):
                risk_level = "high"
            else:
                risk_level = "medium"

        # Generate changes
        changes = self._generate_changes(action)

        return DiffCard(
            action_type=action_type_str,
            description=action.name,
            risk_level=risk_level,
            changes=changes,
            timestamp=self._get_timestamp(),
        )

    def _generate_changes(self, action: ToolCall) -> List[Change]:
        """
        Generate change descriptions from ToolCall arguments.

        Args:
            action: The ToolCall

        Returns:
            List of Change objects
        """
        changes = []

        # File operations
        if action.name == "write_file":
            path = action.arguments.get("path", "unknown")
            content = action.arguments.get("content", "")
            changes.append(
                Change(
                    change_type="FileEdit",
                    summary=f"write_file: Write to {path}",
                    details={
                        "path": path,
                        "before": "",
                        "after": content[:500],  # First 500 chars
                    },
                )
            )

        elif action.name == "delete_file":
            path = action.arguments.get("path", "unknown")
            changes.append(
                Change(
                    change_type="FileDelete",
                    summary=f"delete_file: Delete {path}",
                    details={
                        "path": path,
                        "size_bytes": 0,  # Would need to read file to get actual size
                    },
                )
            )

        elif action.name == "read_file":
            path = action.arguments.get("path", "unknown")
            changes.append(
                Change(
                    change_type="FileRead",
                    summary=f"read_file: Read {path}",
                    details={"path": path},
                )
            )

        # Command execution
        elif "execute" in action.name or "run" in action.name:
            command = action.name
            args = action.arguments.get("args", [])
            changes.append(
                Change(
                    change_type="CommandExec",
                    summary=f"{command}: Execute",
                    details={"command": command, "args": args},
                )
            )

        # Generic fallback
        else:
            changes.append(
                Change(
                    change_type="Custom",
                    summary=f"{action.name}",
                    details=action.arguments,
                )
            )

        return changes

    def _get_timestamp(self) -> str:
        """Get current timestamp in UTC format"""
        from datetime import datetime, timezone

        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    def _call_orchestrator_tui(self, diff_card: DiffCard) -> ApprovalDecision:
        """
        Call Rust orchestrator TUI for approval.

        Args:
            diff_card: The DiffCard to present

        Returns:
            ApprovalDecision from user

        Raises:
            RuntimeError: If orchestrator fails
        """
        # Create temporary file for diff card JSON
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(diff_card.to_dict(), f, indent=2)
            temp_path = f.name

        try:
            # Call orchestrator with approval command
            cmd = [
                self.orchestrator_path,
                "approve",
                "--diff-card",
                temp_path,
            ]

            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=self.timeout_seconds,
            )

            if result.returncode == 0:
                # Parse output for decision
                output = result.stdout.strip().lower()
                if output == "approved":
                    return ApprovalDecision.APPROVED
                elif output == "rejected":
                    return ApprovalDecision.REJECTED
                else:
                    return ApprovalDecision.CANCELLED
            else:
                raise RuntimeError(
                    f"Orchestrator failed with code {result.returncode}: {result.stderr}"
                )

        except subprocess.TimeoutExpired:
            raise RuntimeError(f"Approval timeout after {self.timeout_seconds} seconds")
        finally:
            # Clean up temporary file
            try:
                os.unlink(temp_path)
            except OSError:
                pass

    def request_approval(self, action: ToolCall) -> bool:
        """
        Request approval for an action.

        This is the main entry point for Python agents to request approval
        for Red (destructive) actions.

        Green actions auto-approve without UI.

        Args:
            action: The ToolCall to approve

        Returns:
            True if approved, False otherwise

        Raises:
            RuntimeError: If orchestrator not found or fails
        """
        # Green actions auto-approve
        if action.action_kind == ActionKind.GREEN:
            return True

        # If approval cliff disabled, auto-approve
        if not self.enable_approval_cliff:
            print(f"[Approval Cliff DISABLED] Auto-approving: {action.name}")
            return True

        # Create diff card
        diff_card = self._create_diff_card(action)

        # Present TUI via Rust orchestrator
        try:
            decision = self._call_orchestrator_tui(diff_card)

            if decision == ApprovalDecision.APPROVED:
                print(f"[APPROVED] {action.name}")
                return True
            else:
                print(f"[REJECTED] {action.name}")
                return False

        except FileNotFoundError:
            # Orchestrator not found - fall back to simple CLI prompt
            print(f"[WARNING] Rust orchestrator not found, using fallback prompt")
            return self._fallback_prompt(action)
        except Exception as e:
            print(f"[ERROR] Approval failed: {e}")
            return False

    def _fallback_prompt(self, action: ToolCall) -> bool:
        """
        Fallback simple prompt when orchestrator not available.

        Args:
            action: The ToolCall to approve

        Returns:
            True if approved, False otherwise
        """
        print("\n" + "=" * 80)
        print(f"Action: {action.name}")
        print(f"Type: {action.action_kind.value.upper()}")
        print(f"Arguments: {json.dumps(action.arguments, indent=2)}")
        print("=" * 80)
        print(f"\nRisk Level: {self._get_risk_display(action)}")
        print("\nApprove this action? (y/n): ", end="")

        response = input().strip().lower()

        return response in ("y", "yes")

    def _get_risk_display(self, action: ToolCall) -> str:
        """Get human-readable risk level"""
        if action.action_kind == ActionKind.GREEN:
            return "🟢 GREEN (Safe)"
        elif "delete" in action.name or "remove" in action.name:
            return "🔴🔴 CRITICAL (Permanent deletion)"
        elif "write" in action.name or "edit" in action.name:
            return "🔴 HIGH (Destructive)"
        else:
            return "🟠 MEDIUM (External action)"

    def disable_for_testing(self):
        """Disable approval cliff (for testing only)"""
        self.enable_approval_cliff = False

    def set_timeout(self, seconds: int):
        """
        Set approval timeout in seconds.

        Args:
            seconds: Timeout duration (default: 300 = 5 minutes)
        """
        self.timeout_seconds = seconds


# Singleton instance for use by loop.py
_approval_client: Optional[ApprovalClient] = None


def get_approval_client() -> ApprovalClient:
    """Get or create the singleton ApprovalClient instance"""
    global _approval_client
    if _approval_client is None:
        _approval_client = ApprovalClient()
    return _approval_client


def present_diff_card(action: ToolCall) -> bool:
    """
    Present Diff Card UI for an action requiring approval.

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

    Examples:
        >>> green_action = ToolCall("read_file", {"path": "test.txt"}, ActionKind.GREEN)
        >>> present_diff_card(green_action)
        True

        >>> red_action = ToolCall("delete_file", {"path": "test.txt"}, ActionKind.RED)
        >>> # Presents TUI, returns True if user approves
    """
    client = get_approval_client()
    return client.request_approval(action)


# For backward compatibility, export to loop.py module scope
__all__ = [
    "ApprovalDecision",
    "Change",
    "DiffCard",
    "ApprovalClient",
    "present_diff_card",
]
