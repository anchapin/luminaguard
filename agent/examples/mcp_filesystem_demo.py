#!/usr/bin/env python3
"""
LuminaGuard MCP Client - Filesystem Server Demo
=============================================

Demonstrates LuminaGuard's MCP client integration with the
@modelcontextprotocol/server-filesystem server.

This example shows:
1. Spawning an MCP server process
2. Listing available tools
3. Reading files from the filesystem
4. Writing files to the filesystem
5. Cleanup and shutdown

Usage:
    python agent/examples/mcp_filesystem_demo.py
"""

import sys
import os
import tempfile
from pathlib import Path

# Add repo root to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from agent.mcp_client import McpClient, McpError


def print_section(title: str):
    """Print a section header"""
    print(f"\n{'=' * 60}")
    print(f" {title}")
    print("=" * 60)


def demo_filesystem_operations():
    """
    Demonstrate MCP filesystem operations.

    Operations:
    1. Write file
    2. Read file
    3. List directory
    4. Get file info
    """
    print_section("LuminaGuard MCP Client - Filesystem Demo")

    # Create temporary directory for testing
    with tempfile.TemporaryDirectory() as tmpdir:
        print(f"\n📁 Working directory: {tmpdir}")

        # Step 1: Create MCP client
        print("\n📡 Step 1: Creating MCP client...")
        client = McpClient(
            server_name="filesystem",
            command=["npx", "-y", "@modelcontextprotocol/server-filesystem"],
            root_dir=tmpdir,
        )

        try:
            # Step 2: Spawn and initialize
            print("📡 Step 2: Spawning and initializing MCP client...")
            client.spawn()
            client.initialize()
            print("   ✓ Connected")

            # Step 3: List available tools
            print("\n🔧 Step 3: Listing available tools...")
            tools = client.list_tools()
            for tool in tools:
                print(f"   - {tool.name}: {tool.description}")
            print(f"   ✓ Found {len(tools)} tools")

            # Step 4: Write a file
            print("\n✍️  Step 4: Writing file...")
            test_file = Path(tmpdir) / "luminaguard_demo.txt"
            write_result = client.call_tool(
                "write_file",
                {
                    "path": str(test_file),
                    "content": "Hello from LuminaGuard MCP!\nThis is a test file created via MCP.",
                },
            )
            print(f"   ✓ Written: {test_file.name}")
            print(f"   Result: {write_result}")

            # Step 5: Read the file
            print(f"\n📖 Step 5: Reading file: {test_file.name}...")
            read_result = client.call_tool("read_file", {"path": str(test_file)})
            content = read_result.get("content", [])
            for line in content:
                print(f"   {line}")
            print("   ✓ Read complete")

            # Step 6: List directory
            print(f"\n📂 Step 6: Listing directory: {tmpdir}...")
            list_result = client.call_tool("list_directory", {"path": str(tmpdir)})
            files = list_result.get("files", [])
            for file_info in files[:5]:  # Show first 5 files
                name = file_info.get("name", "?")
                size = file_info.get("size", 0)
                print(f"   - {name}: {size} bytes")
            print(f"   ✓ Total: {len(files)} files")

            # Step 7: Get file info
            print(f"\nℹ️  Step 7: Getting file info: {test_file.name}...")
            info_result = client.call_tool("get_file_info", {"path": str(test_file)})
            info = info_result.get("info", {})
            print(f"   Name: {info.get('name')}")
            print(f"   Size: {info.get('size')} bytes")
            print(f"   Type: {info.get('type')}")
            print("   ✓ Info retrieved")

        except McpError as e:
            print(f"\n❌ Error: {e}")
            return 1

        finally:
            # Step 8: Shutdown
            print("\n🛑 Step 8: Shutting down...")
            client.shutdown()
            print("   ✓ Shutdown complete")

    print("\n✅ Demo complete!")
    return 0


def demo_with_context_manager():
    """
    Demonstrate MCP client usage with context manager.

    This is the recommended way to use the MCP client, as it
    automatically handles spawn, initialize, and shutdown.
    """
    print_section("Context Manager Demo")

    with tempfile.TemporaryDirectory() as tmpdir:
        print(f"\n📁 Working directory: {tmpdir}")

        # Using context manager (recommended)
        with McpClient(
            "filesystem",
            ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
            root_dir=tmpdir,
        ) as client:
            print("✓ Connected (via context manager)")

            # List tools
            tools = client.list_tools()
            print(f"✓ Found {len(tools)} tools")

            # Quick file operation
            test_file = Path(tmpdir) / "demo.txt"
            client.call_tool(
                "write_file", {"path": str(test_file), "content": "Quick demo!"}
            )
            print(f"✓ Created {test_file.name}")

        print("✓ Disconnected (via context manager)")

    print("\n✅ Context manager demo complete!")
    return 0


def main():
    """Run demonstrations"""
    import argparse

    parser = argparse.ArgumentParser(
        description="LuminaGuard MCP Client - Filesystem Demo"
    )
    parser.add_argument(
        "--mode",
        choices=["full", "context-manager"],
        default="full",
        help="Demo mode: full (default) or context-manager",
    )

    args = parser.parse_args()

    if args.mode == "context-manager":
        return demo_with_context_manager()
    else:
        return demo_filesystem_operations()


if __name__ == "__main__":
    sys.exit(main())
