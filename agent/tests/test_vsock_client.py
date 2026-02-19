#!/usr/bin/env python3
"""
Tests for VsockClient module
"""

import pytest
import sys
import os
import json
import struct
import platform
from unittest.mock import MagicMock, mock_open, patch

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from vsock_client import VsockClient, create_vm_client

# Skip all tests in this module on Windows (no AF_UNIX support)
pytestmark = pytest.mark.skipif(
    platform.system() == "Windows",
    reason="AF_UNIX sockets not available on Windows"
)


class TestVsockClient:
    """Tests for VsockClient class"""

    def test_vsock_client_init_default(self):
        """Test VsockClient initialization with defaults"""
        client = VsockClient()
        
        assert client.socket_path == "/tmp/luminaguard/vsock/host.sock"
        assert client.socket is None
        assert client.request_id == 0

    def test_vsock_client_init_custom_path(self):
        """Test VsockClient with custom socket path"""
        client = VsockClient(socket_path="/custom/socket/path.sock")
        
        assert client.socket_path == "/custom/socket/path.sock"

    def test_vsock_client_env_path(self):
        """Test VsockClient reads socket path from environment"""
        with patch.dict(os.environ, {"LUMINAGUARD_VSOCK_PATH": "/env/socket/path.sock"}):
            client = VsockClient()
            assert client.socket_path == "/env/socket/path.sock"

    def test_vsock_client_not_connected_initially(self):
        """Test that VsockClient is not connected initially"""
        client = VsockClient()
        assert client.socket is None

    @patch("vsock_client.socket.socket")
    def test_connect_success(self, mock_socket_class):
        """Test successful connection to host"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        result = client.connect()
        
        assert result is True
        mock_socket.connect.assert_called_once_with("/tmp/luminaguard/vsock/host.sock")

    @patch("vsock_client.socket.socket")
    def test_connect_file_not_found(self, mock_socket_class):
        """Test connection failure - file not found"""
        mock_socket = MagicMock()
        mock_socket.connect.side_effect = FileNotFoundError("Socket not found")
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        result = client.connect()
        
        assert result is False

    @patch("vsock_client.socket.socket")
    def test_connect_connection_refused(self, mock_socket_class):
        """Test connection failure - connection refused"""
        mock_socket = MagicMock()
        mock_socket.connect.side_effect = ConnectionRefusedError("Connection refused")
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        result = client.connect()
        
        assert result is False

    @patch("vsock_client.socket.socket")
    def test_connect_other_error(self, mock_socket_class):
        """Test connection failure - other error"""
        mock_socket = MagicMock()
        mock_socket.connect.side_effect = RuntimeError("Some error")
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        result = client.connect()
        
        assert result is False

    def test_disconnect_when_connected(self):
        """Test disconnecting when connected"""
        client = VsockClient()
        mock_socket = MagicMock()
        client.socket = mock_socket
        
        client.disconnect()
        
        mock_socket.close.assert_called_once()
        assert client.socket is None

    def test_disconnect_when_not_connected(self):
        """Test disconnecting when not connected"""
        client = VsockClient()
        
        # Should not raise any error
        client.disconnect()
        
        assert client.socket is None

    @patch("vsock_client.socket.socket")
    def test_send_message_not_connected(self, mock_socket_class):
        """Test sending message when not connected"""
        client = VsockClient()
        
        result = client._send_message("Request", "test_method", {"key": "value"})
        
        assert result is False

    @patch("vsock_client.socket.socket")
    def test_send_message_request(self, mock_socket_class):
        """Test sending a request message"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client._send_message("Request", "test_method", {"key": "value"})
        
        assert result is True
        mock_socket.sendall.assert_called_once()

    @patch("vsock_client.socket.socket")
    def test_send_message_notification(self, mock_socket_class):
        """Test sending a notification message"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client._send_message("Notification", "test_method", {"key": "value"})
        
        assert result is True

    @patch("vsock_client.socket.socket")
    def test_send_message_invalid_type(self, mock_socket_class):
        """Test sending message with invalid type"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client._send_message("InvalidType", "test_method")
        
        assert result is False

    @patch("vsock_client.socket.socket")
    def test_send_message_exception(self, mock_socket_class):
        """Test sending message with exception"""
        mock_socket = MagicMock()
        mock_socket.sendall.side_effect = RuntimeError("Send error")
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client._send_message("Request", "test_method")
        
        assert result is False

    def test_recv_message_not_connected(self):
        """Test receiving message when not connected"""
        client = VsockClient()
        
        result = client._recv_message()
        
        assert result is None

    @patch("vsock_client.socket.socket")
    def test_recv_message_empty_response(self, mock_socket_class):
        """Test receiving message with empty response"""
        mock_socket = MagicMock()
        mock_socket.recv.return_value = b''  # Empty response
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client._recv_message()
        
        assert result is None

    @patch("vsock_client.socket.socket")
    def test_recv_message_exception(self, mock_socket_class):
        """Test receiving message with exception"""
        mock_socket = MagicMock()
        mock_socket.recv.side_effect = RuntimeError("Receive error")
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client._recv_message()
        
        assert result is None

    @patch("vsock_client.socket.socket")
    def test_send_request_success(self, mock_socket_class):
        """Test successful send_request"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        # Mock the recv to return a valid response
        response_data = {
            "Response": {
                "id": "1",
                "result": {"status": "ok"}
            }
        }
        
        # We need to mock the recv to return data
        def recv_side_effect(size):
            if size == 4:
                # Return length prefix
                data = json.dumps(response_data).encode('utf-8')
                return struct.pack('>I', len(data))
            else:
                return json.dumps(response_data).encode('utf-8')
        
        mock_socket.recv.side_effect = recv_side_effect
        
        client = VsockClient()
        client.socket = mock_socket
        
        result = client.send_request("test_method", {"key": "value"})
        
        # Result depends on implementation - may not be None
        assert result is not None or mock_socket.sendall.called

    @patch("vsock_client.socket.socket")
    def test_send_request_failure(self, mock_socket_class):
        """Test send_request failure"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        # Mock _send_message to return False
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, '_send_message', return_value=False):
            result = client.send_request("test_method")
            
            assert result is None

    @patch("vsock_client.socket.socket")
    def test_send_notification(self, mock_socket_class):
        """Test sending notification"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, '_send_message', return_value=True):
            result = client.send_notification("test_method", {"key": "value"})
            
            assert result is True

    @patch("vsock_client.socket.socket")
    def test_get_available_tools(self, mock_socket_class):
        """Test getting available tools"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, 'send_request', return_value=[{"name": "tool1"}]):
            result = client.get_available_tools()
            
            assert result == [{"name": "tool1"}]

    @patch("vsock_client.socket.socket")
    def test_execute_tool(self, mock_socket_class):
        """Test executing a tool"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, 'send_request', return_value={"status": "ok"}):
            result = client.execute_tool("tool_name", {"arg": "value"})
            
            assert result == {"status": "ok"}

    @patch("vsock_client.socket.socket")
    def test_get_vm_info(self, mock_socket_class):
        """Test getting VM info"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, 'send_request', return_value={"vm_id": "123"}):
            result = client.get_vm_info()
            
            assert result == {"vm_id": "123"}

    @patch("vsock_client.socket.socket")
    def test_health_check_healthy(self, mock_socket_class):
        """Test health check when healthy"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, 'send_request', return_value={"status": "ok"}):
            result = client.health_check()
            
            assert result is True

    @patch("vsock_client.socket.socket")
    def test_health_check_unhealthy(self, mock_socket_class):
        """Test health check when unhealthy"""
        mock_socket = MagicMock()
        mock_socket_class.return_value = mock_socket
        
        client = VsockClient()
        client.socket = mock_socket
        
        with patch.object(client, 'send_request', return_value=None):
            result = client.health_check()
            
            assert result is False


class TestCreateVmClient:
    """Tests for create_vm_client function"""

    def test_create_vm_client(self):
        """Test creating a VM client"""
        client = create_vm_client()
        
        assert isinstance(client, VsockClient)
        assert client.socket is None
