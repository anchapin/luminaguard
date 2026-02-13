#!/usr/bin/env python3
"""
Tests for Style class in agent/loop.py
"""

import os
import sys
import subprocess
import pytest
from loop import Style


class TestStyle:
    """Tests for the Style class in loop.py"""

    def test_methods_use_constants(self):
        """Test that methods use the class constants"""
        # Patch the constants on the class to verify methods use them
        # We use a context manager to restore them afterwards (though patch automatically does)
        # Note: We need to patch the class attributes, not instances

        # Save original values
        orig_bold = Style.BOLD
        orig_cyan = Style.CYAN
        orig_reset = Style.RESET

        try:
            Style.BOLD = "<B>"
            Style.CYAN = "<C>"
            Style.RESET = "<R>"

            assert Style.bold("text") == "<B>text<R>"
            assert Style.cyan("text") == "<C>text<R>"
        finally:
            # Restore
            Style.BOLD = orig_bold
            Style.CYAN = orig_cyan
            Style.RESET = orig_reset

    def test_no_color_env_var(self):
        """Test that NO_COLOR=1 results in empty constants (subprocess)"""
        cmd = [
            sys.executable,
            "-c",
            "from loop import Style; print(f'BOLD={repr(Style.BOLD)}'); print(f'CYAN={repr(Style.CYAN)}');",
        ]
        env = os.environ.copy()
        env["NO_COLOR"] = "1"
        # Run in agent directory
        cwd = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

        result = subprocess.run(cmd, env=env, cwd=cwd, capture_output=True, text=True)
        assert result.returncode == 0
        assert "BOLD=''" in result.stdout
        assert "CYAN=''" in result.stdout

    def test_color_env_var(self):
        """Test that normal env results in color constants (subprocess)"""
        cmd = [
            sys.executable,
            "-c",
            "from loop import Style; print(f'BOLD={repr(Style.BOLD)}'); print(f'CYAN={repr(Style.CYAN)}');",
        ]
        env = os.environ.copy()
        if "NO_COLOR" in env:
            del env["NO_COLOR"]

        cwd = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

        result = subprocess.run(cmd, env=env, cwd=cwd, capture_output=True, text=True)
        assert result.returncode == 0
        assert "BOLD='\\x1b[1m'" in result.stdout or "BOLD='\\033[1m'" in result.stdout
        assert (
            "CYAN='\\x1b[36m'" in result.stdout or "CYAN='\\033[36m'" in result.stdout
        )
