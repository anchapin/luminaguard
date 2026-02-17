#!/usr/bin/env python3
"""
VM Communication Client - Vsock-based Host-Guest Communication

This module provides the client-side communication with the Rust orchestrator
running on the host. It's used when the agent runs inside a Firecracker VM.

The protocol uses JSON messages over Unix domain sockets with length-prefixed frames.
"""

import json
import os
import socket
import struct
import sys
from typing import Dict, Optional


def is_ci_environment() -> bool:
    """
    Check if we're running in a CI environment.
    
    Returns:
        True if running in CI, False otherwise.
    """
    # Check common CI environment variables
    ci_vars = [
        "CI",  # Generic CI flag
        "GITHUB_ACTIONS",  # GitHub Actions
        "GITLAB_CI",  # GitLab CI
        "JENKINS_URL",  # Jenkins
        "CIRCLECI",  # CircleCI
        "TRAVIS",  # Travis CI
        "TWISTED",  # Twisted (for testing)
        "CONTINUOUS_INTEGRATION",  # Generic
    ]
    return any(os.environ.get(var) for var in ci_vars)


def is_vsock_available() -> bool:
    """
    Check if VSOCK socket is available (socket file exists).
    
    This function checks if the default or configured VSOCK socket
    exists. If running in CI, this will return False since VSOCK
    devices are typically not available in CI environments.
    
    Returns:
        True if VSOCK is available, False otherwise.
    """
    # In CI environments, VSOCK is typically not available
    if is_ci_environment():
        return False
    
    # Check if socket file exists
    socket_path = os.environ.get(
        "LUMINAGUARD_VSOCK_PATH", 
        "/tmp/luminaguard/vsock/host.sock"
    )
    return os.path.exists(socket_path)


class VsockClient:
    """Client for communicating with host orchestrator via vsock/Unix socket"""

    def __init__(self, socket_path: Optional[str] = None):
        """
        Initialize the vsock client.
        
        Args:
            socket_path: Path to the vsock socket. If None, uses default path.
        """
        if socket_path is None:
            socket_path = os.environ.get(
                "LUMINAGUARD_VSOCK_PATH", 
                "/tmp/luminaguard/vsock/host.sock"
            )
        self.socket_path = socket_path
        self.socket: Optional[socket.socket] = None
        self.request_id = 0

    def connect(self) -> bool:
        """
        Connect to the host orchestrator.
        
        Returns:
            True if connected successfully, False otherwise.
        """
        try:
            self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.socket.connect(self.socket_path)
            return True
        except FileNotFoundError:
            print(f"ERROR: Socket not found: {self.socket_path}")
            return False
        except ConnectionRefusedError:
            print(f"ERROR: Connection refused: {self.socket_path}")
            return False
        except Exception as e:
            print(f"ERROR: Failed to connect: {e}")
            return False

    def disconnect(self) -> None:
        """Disconnect from the host orchestrator."""
        if self.socket:
            try:
                self.socket.close()
            except Exception:
                pass
            finally:
                self.socket = None

    def _send_message(self, msg_type: str, method: str = None, params: Dict = None, 
                      msg_id: str = None) -> bool:
        """
        Send a message to the host.
        
        Args:
            msg_type: Type of message (Request, Response, Notification)
            method: Method name (for Request/Notification)
            params: Method parameters (for Request/Notification)
            msg_id: Message ID (for Response)
            
        Returns:
            True if sent successfully, False otherwise.
        """
        if not self.socket:
            print("ERROR: Not connected")
            return False

        try:
            # Build message
            if msg_type == "Request":
                if msg_id is None:
                    msg_id = str(self.request_id)
                    self.request_id += 1
                msg = {
                    "Request": {
                        "id": msg_id,
                        "method": method,
                        "params": params or {}
                    }
                }
            elif msg_type == "Notification":
                msg = {
                    "Notification": {
                        "method": method,
                        "params": params or {}
                    }
                }
            elif msg_type == "Response":
                msg = {
                    "Response": msg_id
                }
            else:
                return False

            # Serialize and send with length prefix
            data = json.dumps(msg).encode('utf-8')
            length = struct.pack('>I', len(data))
            self.socket.sendall(length + data)
            return True

        except Exception as e:
            print(f"ERROR: Failed to send message: {e}")
            return False

    def _recv_message(self) -> Optional[Dict]:
        """
        Receive a message from the host.
        
        Returns:
            Parsed message dict, or None on error.
        """
        if not self.socket:
            return None

        try:
            # Read length prefix (4 bytes)
            length_bytes = self.socket.recv(4)
            if not length_bytes:
                return None
            
            length = struct.unpack('>I', length_bytes)[0]
            
            # Read message body
            data = b''
            while len(data) < length:
                chunk = self.socket.recv(length - len(data))
                if not chunk:
                    return None
                data += chunk
            
            return json.loads(data.decode('utf-8'))

        except Exception as e:
            print(f"ERROR: Failed to receive message: {e}")
            return None

    def send_request(self, method: str, params: Dict = None) -> Optional[Dict]:
        """
        Send a request and wait for response.
        
        Args:
            method: Method name to call
            params: Method parameters
            
        Returns:
            Response dict, or None on error.
        """
        if not self._send_message("Request", method, params):
            return None
        
        # Wait for response
        response = self._recv_message()
        if response is None:
            return None
            
        if "Response" in response:
            resp = response["Response"]
            if "error" in resp and resp["error"]:
                print(f"ERROR: {resp['error']}")
                return None
            return resp.get("result")
        
        return None

    def send_notification(self, method: str, params: Dict = None) -> bool:
        """
        Send a notification (no response expected).
        
        Args:
            method: Method name
            params: Method parameters
            
        Returns:
            True if sent successfully.
        """
        return self._send_message("Notification", method, params)

    # High-level API methods

    def get_available_tools(self) -> Optional[list]:
        """Get list of available tools from host."""
        return self.send_request("tools/list")

    def execute_tool(self, tool_name: str, arguments: Dict) -> Optional[Dict]:
        """
        Execute a tool via host.
        
        Args:
            tool_name: Name of the tool
            arguments: Tool arguments
            
        Returns:
            Tool execution result, or None on error.
        """
        return self.send_request("tools/execute", {
            "name": tool_name,
            "arguments": arguments
        })

    def get_vm_info(self) -> Optional[Dict]:
        """Get VM information from host."""
        return self.send_request("vm/info")

    def health_check(self) -> bool:
        """Check if host is reachable."""
        result = self.send_request("health")
        return result is not None


def create_vm_client() -> VsockClient:
    """
    Create a vsock client for VM communication.
    
    Returns:
        VsockClient instance (not connected)
    """
    return VsockClient()


if __name__ == "__main__":
    # Test connection
    client = create_vm_client()
    if client.connect():
        print("Connected to host")
        
        # Test health check
        if client.health_check():
            print("Host is healthy")
        
        # Test get VM info
        info = client.get_vm_info()
        if info:
            print(f"VM Info: {info}")
        
        client.disconnect()
    else:
        print("Failed to connect")
        sys.exit(1)
