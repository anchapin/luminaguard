// vsock-based Host-Guest Communication
//
// This module implements secure communication between host and guest VMs
// using vsock (Virtual Socket) protocol. vsock provides a lightweight,
// low-latency communication channel that doesn't require traditional networking.
//
// Key invariants:
// - No external network access required
// - Low latency communication
// - Secure by design (isolated communication channel)

#![cfg(unix)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// vsock communication protocol version
pub const VSOCK_PROTOCOL_VERSION: u32 = 1;

/// Maximum message size (1MB)
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Message types for vsock protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VsockMessage {
    /// Request from client to server
    Request {
        id: String,
        method: String,
        params: serde_json::Value,
    },
    /// Response from server to client
    Response {
        id: String,
        result: Option<serde_json::Value>,
        error: Option<String>,
    },
    /// Notification (one-way message)
    Notification {
        method: String,
        params: serde_json::Value,
    },
}

impl VsockMessage {
    /// Create a new request message
    pub fn request(id: String, method: String, params: serde_json::Value) -> Self {
        Self::Request { id, method, params }
    }

    /// Create a new response message
    pub fn response(id: String, result: Option<serde_json::Value>, error: Option<String>) -> Self {
        Self::Response { id, result, error }
    }

    /// Create a new notification message
    pub fn notification(method: String, params: serde_json::Value) -> Self {
        Self::Notification { method, params }
    }

    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).context("Failed to serialize vsock message")
    }

    /// Deserialize message from JSON
    pub fn from_json(data: &[u8]) -> Result<Self> {
        if data.len() > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message size exceeds limit");
        }
        serde_json::from_slice(data).context("Failed to deserialize vsock message")
    }
}

/// Host-side vsock listener
pub struct VsockHostListener {
    socket_path: PathBuf,
    listener: UnixListener,
}

impl VsockHostListener {
    /// Create a new host listener bound to a Unix socket
    pub async fn bind(socket_path: &str) -> Result<Self> {
        let path = PathBuf::from(socket_path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create socket directory")?;
        }

        // Remove existing socket file if it exists
        if path.exists() {
            fs::remove_file(&path)
                .await
                .context("Failed to remove existing socket file")?;
        }

        let listener = UnixListener::bind(&path).context("Failed to bind Unix socket")?;

        Ok(Self {
            socket_path: path,
            listener,
        })
    }

    /// Accept a new connection
    pub async fn accept(&self) -> Result<VsockConnection> {
        let (socket, _addr) = self
            .listener
            .accept()
            .await
            .context("Failed to accept connection")?;
        Ok(VsockConnection::new(socket))
    }

    /// Get socket path
    pub fn path(&self) -> &PathBuf {
        &self.socket_path
    }
}

/// Client-side vsock connector
pub struct VsockClient {
    socket_path: String,
}

impl VsockClient {
    /// Create a new vsock client
    pub fn new(socket_path: String) -> Self {
        Self { socket_path }
    }

    /// Connect to the host socket
    pub async fn connect(&self) -> Result<VsockConnection> {
        let socket = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to Unix socket")?;
        Ok(VsockConnection::new(socket))
    }
}

/// Active vsock connection
pub struct VsockConnection {
    stream: UnixStream,
}

impl VsockConnection {
    /// Create a new connection wrapper
    pub fn new(stream: UnixStream) -> Self {
        Self { stream }
    }

    /// Send a message
    pub async fn send(&mut self, message: &VsockMessage) -> Result<()> {
        let data = message.to_json()?;
        // Write length prefix (u32, big endian)
        let len = data.len() as u32;
        self.stream
            .write_u32(len)
            .await
            .context("Failed to write message length")?;
        // Write data
        self.stream
            .write_all(&data)
            .await
            .context("Failed to write message data")?;
        self.stream
            .flush()
            .await
            .context("Failed to flush stream")?;
        Ok(())
    }

    /// Receive a message
    pub async fn receive(&mut self) -> Result<VsockMessage> {
        // Read length prefix
        let len = self
            .stream
            .read_u32()
            .await
            .context("Failed to read message length")?;

        if len as usize > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message size {} exceeds limit", len);
        }

        // Read data
        let mut buffer = vec![0u8; len as usize];
        self.stream
            .read_exact(&mut buffer)
            .await
            .context("Failed to read message data")?;

        VsockMessage::from_json(&buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_vsock_message_size_limit() {
        use serde_json::json;

        // Test 1: Normal-sized message works
        let msg = VsockMessage::request(
            "test-id".to_string(),
            "test_method".to_string(),
            json!({"data": "test"}),
        );
        assert!(msg.to_json().is_ok());

        // Test 2: Oversized message fails deserialization
        let huge_data = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let json_bytes = serde_json::to_vec(&huge_data).unwrap();
        // Here we're testing from_json which checks the size
        assert!(VsockMessage::from_json(&json_bytes).is_err());
    }

    #[test]
    fn test_vsock_message_serialization() {
        let msg = VsockMessage::request("1".to_string(), "test".to_string(), serde_json::json!({}));
        let json = msg.to_json().unwrap();
        let decoded = VsockMessage::from_json(&json).unwrap();
        match decoded {
            VsockMessage::Request { id, method, .. } => {
                assert_eq!(id, "1");
                assert_eq!(method, "test");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_vsock_message_response_creation() {
        let msg = VsockMessage::response(
            "1".to_string(),
            Some(serde_json::json!({"status": "ok"})),
            None,
        );
        if let VsockMessage::Response { id, result, error } = msg {
            assert_eq!(id, "1");
            assert!(result.is_some());
            assert!(error.is_none());
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_vsock_message_notification_creation() {
        let msg = VsockMessage::notification("update".to_string(), serde_json::json!({}));
        if let VsockMessage::Notification { method, .. } = msg {
            assert_eq!(method, "update");
        } else {
            panic!("Wrong message type");
        }
    }

    #[tokio::test]
    async fn test_vsock_host_listener_creation() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        let path_str = socket_path.to_str().unwrap();

        let listener = VsockHostListener::bind(path_str).await.unwrap();
        assert!(socket_path.exists());
        assert_eq!(listener.path(), &socket_path);
    }

    #[tokio::test]
    async fn test_vsock_message_round_trip() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("roundtrip.sock");
        let path_str = socket_path.to_str().unwrap().to_string();

        // Server task
        let path_clone = path_str.clone();
        let server_handle = tokio::spawn(async move {
            let listener = VsockHostListener::bind(&path_clone).await.unwrap();
            let mut conn = listener.accept().await.unwrap();
            let msg = conn.receive().await.unwrap();
            if let VsockMessage::Request { id, .. } = msg {
                let response = VsockMessage::response(id, Some(serde_json::json!("pong")), None);
                conn.send(&response).await.unwrap();
            }
        });

        // Give server time to bind
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Client task
        let client = VsockClient::new(path_str);
        let mut conn = client.connect().await.unwrap();
        let req = VsockMessage::request("1".to_string(), "ping".to_string(), serde_json::json!({}));
        conn.send(&req).await.unwrap();
        let resp = conn.receive().await.unwrap();

        match resp {
            VsockMessage::Response { result, .. } => {
                assert_eq!(result.unwrap(), serde_json::json!("pong"));
            }
            _ => panic!("Expected response"),
        }

        server_handle.await.unwrap();
    }
}
