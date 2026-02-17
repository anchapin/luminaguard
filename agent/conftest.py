#!/usr/bin/env python3
"""
Pytest configuration for agent tests.

This module provides fixtures and configuration for running tests,
including skipping tests that require VSOCK when not available.
"""

import pytest


def pytest_configure(config):
    """Register custom markers."""
    config.addinivalue_line(
        "markers", "vsock: tests that require VSOCK to be available"
    )


def pytest_collection_modifyitems(config, items):
    """
    Automatically skip tests that require VSOCK when running in CI
    or when VSOCK is not available.
    """
    # Import here to avoid import errors if vsock_client is not available
    try:
        from vsock_client import is_ci_environment, is_vsock_available
    except ImportError:
        # If we can't import, assume we're not in CI and VSOCK is available
        return
    
    skip_vsock = pytest.mark.skip(
        reason="VSOCK not available in CI environment"
    )
    
    for item in items:
        # Check if the test requires VSOCK
        # This could be via marker or test name
        if "vsock" in item.name.lower() or "vm_mode" in item.name.lower():
            # Check if we're in CI or VSOCK is not available
            if is_ci_environment() or not is_vsock_available():
                item.add_marker(skip_vsock)
