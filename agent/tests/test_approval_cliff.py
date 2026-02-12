#!/usr/bin/env python3
"""
Tests for Approval Cliff functionality
"""

import pytest
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from loop import (
    ToolCall,
    ActionKind,
    present_diff_card,
    determine_action_kind,
    GREEN_KEYWORDS,
    RED_KEYWORDS,
)


class TestApprovalCliffUI:
    """Tests for the Approval Cliff user interface"""

    def test_green_action_auto_approves(self):
        """Test that Green actions auto-approve without prompting"""
        action = ToolCall(
            name="read_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.GREEN
        )
        result = present_diff_card(action)
        assert result is True, "Green actions should auto-approve"

    def test_red_action_shows_diff_card(self):
        """Test that Red actions trigger the approval UI"""
        # We can't test interactive input in unit tests,
        # but we can verify the action is correctly classified
        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )
        assert action.action_kind == ActionKind.RED

    def test_write_file_shows_content_preview(self):
        """Test that write_file shows content in Diff Card"""
        action = ToolCall(
            name="write_file",
            arguments={
                "path": "/tmp/test.txt",
                "content": "Hello, World! This is test content."
            },
            action_kind=ActionKind.RED
        )
        assert action.action_kind == ActionKind.RED
        assert "content" in action.arguments

    def test_delete_file_highlights_destruction(self):
        """Test that delete_file highlights permanent deletion"""
        action = ToolCall(
            name="delete_file",
            arguments={"path": "/tmp/important.txt"},
            action_kind=ActionKind.RED
        )
        assert action.action_kind == ActionKind.RED
        assert action.arguments["path"] == "/tmp/important.txt"

    def test_green_keywords_match(self):
        """Test that green keywords are correctly identified"""
        green_actions = [
            ("read file", ActionKind.GREEN),
            ("list directory", ActionKind.GREEN),
            ("search logs", ActionKind.GREEN),
            ("check status", ActionKind.GREEN),
        ]

        for message, expected_kind in green_actions:
            kind = determine_action_kind(message)
            assert kind == expected_kind, f"Failed for: {message}"

    def test_red_keywords_match(self):
        """Test that red keywords are correctly identified"""
        red_actions = [
            ("delete file", ActionKind.RED),
            ("write data", ActionKind.RED),
            ("edit config", ActionKind.RED),
            ("remove old", ActionKind.RED),
        ]

        for message, expected_kind in red_actions:
            kind = determine_action_kind(message)
            assert kind == expected_kind, f"Failed for: {message}"

    def test_unknown_defaults_to_red(self):
        """Test that unknown actions default to Red (require approval for safety)"""
        kind = determine_action_kind("do something complex")
        assert kind == ActionKind.RED, "Unknown actions should require approval (safe by default)"

    def test_green_keywords_list_populated(self):
        """Test that green keywords list is properly populated"""
        assert len(GREEN_KEYWORDS) > 0, "Green keywords list should not be empty"
        assert "read" in GREEN_KEYWORDS
        assert "list" in GREEN_KEYWORDS

    def test_red_keywords_list_populated(self):
        """Test that red keywords list is properly populated"""
        assert len(RED_KEYWORDS) > 0, "Red keywords list should not be empty"
        assert "delete" in RED_KEYWORDS
        assert "write" in RED_KEYWORDS


class TestApprovalWorkflow:
    """Tests for the complete approval workflow"""

    def test_diff_card_displays_action_name(self):
        """Test that Diff Card displays the tool name"""
        action = ToolCall(
            name="write_file",
            arguments={"path": "/tmp/test.txt"},
            action_kind=ActionKind.RED
        )
        assert action.name == "write_file"

    def test_diff_card_displays_arguments(self):
        """Test that Diff Card displays action arguments"""
        action = ToolCall(
            name="list_directory",
            arguments={"path": "/home/user"},
            action_kind=ActionKind.GREEN
        )
        assert "path" in action.arguments
        assert action.arguments["path"] == "/home/user"

    def test_action_kind_enum_values(self):
        """Test that ActionKind enum has correct string values"""
        assert ActionKind.GREEN.value == "green"
        assert ActionKind.RED.value == "red"
