#!/usr/bin/env python3
"""
Tests for Approval Cliff TUI integration
"""

import pytest
import sys
import os
import tempfile
import json
from unittest.mock import Mock, patch, MagicMock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


class TestApprovalClient:
    """Tests for the ApprovalClient"""

    def test_client_initialization(self):
        """Test that ApprovalClient initializes correctly"""
        from approval_client import ApprovalClient

        # Default initialization
        client = ApprovalClient()
        assert client.timeout_seconds == 300  # 5 minutes default
        assert client.enable_approval_cliff is True

    def test_client_custom_timeout(self):
        """Test setting custom timeout"""
        from approval_client import ApprovalClient

        client = ApprovalClient()
        client.set_timeout(60)
        assert client.timeout_seconds == 60

    def test_client_disable_for_testing(self):
        """Test disabling approval cliff for testing"""
        from approval_client import ApprovalClient

        client = ApprovalClient()
        client.disable_for_testing()
        assert client.enable_approval_cliff is False

    def test_create_diff_card_green_action(self):
        """Test DiffCard creation for Green action"""
        from approval_client import ApprovalClient, DiffCard, Change
        from loop import ToolCall, ActionKind

        client = ApprovalClient()
        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN,
        )

        diff_card = client._create_diff_card(action)

        assert isinstance(diff_card, DiffCard)
        assert diff_card.action_type == "read_file"
        assert diff_card.risk_level == "none"
        assert len(diff_card.changes) > 0

    def test_create_diff_card_red_action(self):
        """Test DiffCard creation for Red action"""
        from approval_client import ApprovalClient, DiffCard
        from loop import ToolCall, ActionKind

        client = ApprovalClient()
        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        diff_card = client._create_diff_card(action)

        assert isinstance(diff_card, DiffCard)
        assert diff_card.action_type == "delete_file"
        assert diff_card.risk_level == "critical"
        assert len(diff_card.changes) > 0

    def test_generate_changes_write_file(self):
        """Test change generation for write_file"""
        from approval_client import ApprovalClient, Change
        from loop import ToolCall, ActionKind

        client = ApprovalClient()
        action = ToolCall(
            name="write_file",
            arguments={
                "path": "/tmp/test.txt",
                "content": "Hello, World!",
            },
            action_kind=ActionKind.RED,
        )

        changes = client._generate_changes(action)

        assert len(changes) == 1
        assert changes[0].change_type == "FileEdit"
        assert "write_file" in changes[0].summary

    def test_generate_changes_delete_file(self):
        """Test change generation for delete_file"""
        from approval_client import ApprovalClient, Change
        from loop import ToolCall, ActionKind

        client = ApprovalClient()
        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        changes = client._generate_changes(action)

        assert len(changes) == 1
        assert changes[0].change_type == "FileDelete"
        assert "delete" in changes[0].summary

    def test_diff_card_to_dict(self):
        """Test DiffCard serialization to dictionary"""
        from approval_client import DiffCard, Change

        diff_card = DiffCard(
            action_type="write_file",
            description="Write to test file",
            risk_level="high",
            changes=[
                Change(
                    change_type="FileEdit",
                    summary="Write to: /tmp/test.txt",
                    details={
                        "path": "/tmp/test.txt",
                        "before": "",
                        "after": "Hello",
                    },
                )
            ],
            timestamp="2024-01-01T00:00:00Z",
        )

        data = diff_card.to_dict()

        assert data["action_type"] == "write_file"
        assert data["description"] == "Write to test file"
        assert data["risk_level"] == "high"
        assert len(data["changes"]) == 1

    def test_green_action_auto_approves(self):
        """Test that Green actions auto-approve without prompting"""
        from approval_client import ApprovalClient
        from loop import ToolCall, ActionKind

        client = ApprovalClient()
        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN,
        )

        # Green actions should auto-approve
        result = client.request_approval(action)
        assert result is True


class TestApprovalFallback:
    """Tests for fallback approval prompt"""

    def test_fallback_prompt_green_action(self):
        """Test fallback prompt with Green action"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN,
        )

        # Green actions auto-approve
        with patch("builtins.input", return_value="n"):
            # Should still approve even with "n" input for Green actions
            result = present_diff_card(action)
            assert result is True

    @patch("builtins.input", return_value="y")
    def test_fallback_prompt_red_action_approved(self, mock_input):
        """Test fallback prompt with Red action (approved)"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        # Mock approval_client import to force fallback
        with patch.dict("sys.modules", {"approval_client": None}):
            with patch("builtins.print") as mock_print:
                result = present_diff_card(action)

                assert result is True
                mock_print.assert_called()

    @patch("builtins.input", return_value="n")
    def test_fallback_prompt_red_action_rejected(self, mock_input):
        """Test fallback prompt with Red action (rejected)"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        # Mock approval_client import to force fallback
        with patch.dict("sys.modules", {"approval_client": None}):
            with patch("builtins.print") as mock_print:
                result = present_diff_card(action)

                assert result is False
                mock_print.assert_called()

    @patch("builtins.input", return_value="yes")
    def test_fallback_prompt_yes_input(self, mock_input):
        """Test that 'yes' input is accepted"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        with patch.dict("sys.modules", {"approval_client": None}):
            result = present_diff_card(action)
            assert result is True

    @patch("builtins.input", return_value="Y")
    def test_fallback_prompt_uppercase_y(self, mock_input):
        """Test that uppercase Y is accepted"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        with patch.dict("sys.modules", {"approval_client": None}):
            result = present_diff_card(action)
            assert result is True


class TestRiskDisplay:
    """Tests for risk level display"""

    def test_green_action_risk_display(self):
        """Test risk display for Green action"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN,
        )

        risk = _get_risk_display(action)
        assert "GREEN" in risk
        assert "Safe" in risk

    def test_delete_action_risk_display(self):
        """Test risk display for delete action"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        risk = _get_risk_display(action)
        assert "CRITICAL" in risk
        assert "deletion" in risk

    def test_write_action_risk_display(self):
        """Test risk display for write action"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="write_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED,
        )

        risk = _get_risk_display(action)
        assert "HIGH" in risk
        assert "Destructive" in risk

    def test_other_red_action_risk_display(self):
        """Test risk display for other Red actions"""
        from loop import ToolCall, ActionKind

        action = ToolCall(
            name="send_email",
            arguments={"to": "test@example.com"},
            action_kind=ActionKind.RED,
        )

        risk = _get_risk_display(action)
        assert "MEDIUM" in risk


class TestApprovalDecision:
    """Tests for ApprovalDecision enum"""

    def test_approval_decision_values(self):
        """Test ApprovalDecision enum values"""
        from approval_client import ApprovalDecision

        assert ApprovalDecision.APPROVED.value == "approved"
        assert ApprovalDecision.REJECTED.value == "rejected"
        assert ApprovalDecision.CANCELLED.value == "cancelled"

    def test_approval_decision_comparison(self):
        """Test ApprovalDecision comparison"""
        from approval_client import ApprovalDecision

        assert ApprovalDecision.APPROVED == ApprovalDecision.APPROVED
        assert ApprovalDecision.APPROVED != ApprovalDecision.REJECTED
        assert ApprovalDecision.APPROVED != ApprovalDecision.CANCELLED


# Import present_diff_card and _get_risk_display for testing
try:
    from loop import present_diff_card, _get_risk_display
except ImportError:
    # If not yet imported, we'll skip those tests
    pass
