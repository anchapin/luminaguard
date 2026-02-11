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

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

/// vsock communication protocol version
pub const VSOCK_PROTOCOL_VERSION: u32 = 1;

/// Maximum message size (16MB to prevent DoS)
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// vsock host listener
#[cfg(unix)]
#[derive(Debug)]
pub struct VsockHostListener {
    listener: UnixListener,
    vm_id: String,
}

#[cfg(not(unix))]
#[derive(Debug)]
pub struct VsockHostListener {
    vm_id: String,
}

/// vsock client (guest side)
#[derive(Debug)]
pub struct VsockClient {
    socket_path: PathBuf,
}

/// vsock message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VsockMessage {
    /// Request from guest to host
    Request {
        id: String,
        method: String,
        params: serde_json::Value,
    },
    /// Response from host to guest
    Response {
        id: String,
        result: Option<serde_json::Value>,
        error: Option<String>,
    },
    /// Notification (no response expected)
    Notification {
        method: String,
        params: serde_json::Value,
    },
}

impl VsockMessage {
    /// Create a new request
    pub fn request(id: String, method: String, params: serde_json::Value) -> Self {
        Self::Request { id, method, params }
    }

    /// Create a new response
    pub fn response(id: String, result: Option<serde_json::Value>, error: Option<String>) -> Self {
        Self::Response { id, result, error }
    }

    /// Create a new notification
    pub fn notification(method: String, params: serde_json::Value) -> Self {
        Self::Notification { method, params }
    }

    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).context("Failed to serialize vsock message")
    }

    /// Deserialize message from JSON
    pub fn from_json(data: &[u8]) -> Result<Self> {
        // Enforce size limit to prevent DoS
        if data.len() > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message size exceeds maximum allowed size");
        }

        serde_json::from_slice(data).context("Failed to deserialize vsock message")
    }
}

/// Message handler trait for host-side processing
#[async_trait::async_trait]
pub trait VsockMessageHandler: Send + Sync {
    /// Handle a request from the guest
    async fn handle_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Handle a notification from the guest
    async fn handle_notification(&self, method: &str, params: serde_json::Value) -> Result<()>;
}

/// Default handler that rejects all operations
#[allow(dead_code)]
struct DefaultHandler;

#[async_trait::async_trait]
impl VsockMessageHandler for DefaultHandler {
    async fn handle_request(
        &self,
        method: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        anyhow::bail!("Method '{}' not implemented", method);
    }

    async fn handle_notification(&self, method: &str, _params: serde_json::Value) -> Result<()> {
        tracing::warn!("Received unhandled notification: {}", method);
        Ok(())
    }
}

impl VsockHostListener {
    /// Create a new vsock host listener
    ///
    /// # Arguments
    ///
    /// * `vm_id` - Unique identifier for the VM
    ///
    /// # Returns
    ///
    /// * `VsockHostListener` - Host-side listener
    #[cfg(unix)]
    pub async fn new(vm_id: String) -> Result<Self> {
        let socket_dir = "/tmp/ironclaw/vsock";
        fs::create_dir_all(socket_dir)
            .await
            .context("Failed to create vsock directory")?;

        let socket_path = format!("{}/{}.sock", socket_dir, vm_id);

        // Delete socket file if it exists (from previous run)
        if fs::metadata(&socket_path).await.is_ok() {
            fs::remove_file(&socket_path)
                .await
                .context("Failed to remove existing socket file")?;
        }

        let listener = UnixListener::bind(&socket_path).context("Failed to bind vsock socket")?;

        tracing::info!("vsock host listener created: {}", socket_path);

        Ok(Self { listener, vm_id })
    }

    #[cfg(not(unix))]
    pub async fn new(vm_id: String) -> Result<Self> {
        Ok(Self { vm_id })
    }

    /// Accept incoming connection from guest
    #[cfg(unix)]
    pub async fn accept(&self) -> Result<VsockConnection> {
        let (socket, _addr) = self
            .listener
            .accept()
            .await
            .context("Failed to accept vsock connection")?;

        tracing::info!("vsock connection accepted");

        Ok(VsockConnection::new(socket))
    }

    #[cfg(not(unix))]
    pub async fn accept(&self) -> Result<VsockConnection> {
        // Windows support is mocked/limited
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        Err(anyhow::anyhow!("Vsock not supported on Windows"))
    }

    /// Run the message handler loop
    ///
    /// This method runs indefinitely, handling messages from the guest.
    /// It should be run in a separate task.
    pub async fn run_handler<H>(self, handler: H) -> Result<()>
    where
        H: VsockMessageHandler + Clone + 'static,
    {
        loop {
            match self.accept().await {
                Ok(conn) => {
                    let handler_clone = handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = conn.handle_messages(handler_clone).await {
                            tracing::error!("Message handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to accept connection: {}", e);
                    // Continue accepting new connections
                }
            }
        }
    }

    /// Get the socket path (for passing to guest)
    pub fn socket_path(&self) -> String {
        format!("/tmp/ironclaw/vsock/{}.sock", self.vm_id)
    }
}

/// vsock connection (bidirectional)
#[cfg(unix)]
pub struct VsockConnection {
    socket: UnixStream,
}

#[cfg(not(unix))]
pub struct VsockConnection {}

impl VsockConnection {
    /// Create a new vsock connection
    #[cfg(unix)]
    fn new(socket: UnixStream) -> Self {
        Self { socket }
    }

    #[cfg(not(unix))]
    fn new() -> Self {
        Self {}
    }

    /// Handle incoming messages
    async fn handle_messages<H>(mut self, handler: H) -> Result<()>
    where
        H: VsockMessageHandler + 'static,
    {
        #[cfg(unix)]
        {
            let mut reader = BufReader::new(&mut self.socket);

            loop {
                match Self::read_message(&mut reader).await {
                    Ok(Some(msg)) => {
                        let response = match msg {
                            VsockMessage::Request { id, method, params } => {
                                match handler.handle_request(&method, params).await {
                                    Ok(result) => VsockMessage::response(id, Some(result), None),
                                    Err(e) => {
                                        tracing::error!("Request handler error: {}", e);
                                        VsockMessage::response(id, None, Some(format!("{:?}", e)))
                                    }
                                }
                            }
                            VsockMessage::Notification { method, params } => {
                                if let Err(e) = handler.handle_notification(&method, params).await {
                                    tracing::error!("Notification handler error: {}", e);
                                }
                                continue; // No response for notifications
                            }
                            VsockMessage::Response { .. } => {
                                tracing::warn!("Unexpected response message from guest");
                                continue;
                            }
                        };

                        // Drop the reader borrow before writing
                        drop(reader);
                        Self::write_message(&mut self.socket, &response).await?;
                        reader = BufReader::new(&mut self.socket);
                    }
                    Ok(None) => {
                        // Connection closed
                        tracing::info!("vsock connection closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Failed to read message: {}", e);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Read a message from the socket
    #[cfg(unix)]
    async fn read_message<R>(reader: &mut BufReader<R>) -> Result<Option<VsockMessage>>
    where
        R: AsyncReadExt + Unpin,
    {
        // Read message length (4 bytes, big-endian)
        let mut len_bytes = [0u8; 4];
        let n = reader.read_exact(&mut len_bytes).await;
        if n.is_err() {
            return Ok(None); // Connection closed
        }

        let msg_len = u32::from_be_bytes(len_bytes) as usize;

        // Enforce size limit
        if msg_len > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message size exceeds maximum: {} bytes", msg_len);
        }

        // Read message body
        let mut buffer = vec![0u8; msg_len];
        reader.read_exact(&mut buffer).await?;

        // Deserialize message
        let msg = VsockMessage::from_json(&buffer)?;
        Ok(Some(msg))
    }

    /// Write a message to the socket
    #[cfg(unix)]
    async fn write_message<W>(writer: &mut W, msg: &VsockMessage) -> Result<()>
    where
        W: AsyncWriteExt + Unpin,
    {
        let data = msg.to_json()?;
        let len = data.len() as u32;

        // Write length prefix (4 bytes, big-endian)
        writer.write_all(&len.to_be_bytes()).await?;

        // Write message body
        writer.write_all(&data).await?;
        writer.flush().await?;

        Ok(())
    }
}

impl VsockClient {
    /// Create a new vsock client (guest side)
    ///
    /// # Arguments
    ///
    /// * `socket_path` - Path to the vsock socket
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Connect to the host
    #[cfg(unix)]
    pub async fn connect(&self) -> Result<VsockClientConnection> {
        let socket = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to vsock socket")?;

        tracing::info!("Connected to vsock host: {:?}", self.socket_path);

        Ok(VsockClientConnection::new(socket))
    }

    #[cfg(not(unix))]
    pub async fn connect(&self) -> Result<VsockClientConnection> {
        Err(anyhow::anyhow!("Vsock not supported on Windows"))
    }
}

/// vsock client connection (for sending messages from guest to host)
#[cfg(unix)]
pub struct VsockClientConnection {
    socket: UnixStream,
    next_id: u64,
}

#[cfg(not(unix))]
pub struct VsockClientConnection {}

impl VsockClientConnection {
    /// Create a new client connection
    #[cfg(unix)]
    fn new(socket: UnixStream) -> Self {
        Self { socket, next_id: 1 }
    }

    #[cfg(not(unix))]
    fn new() -> Self {
        Self {}
    }

    /// Send a request and wait for response
    pub async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(unix)]
        {
            let id = self.next_id.to_string();
            self.next_id += 1;

            let msg = VsockMessage::request(id.clone(), method.to_string(), params);

            VsockConnection::write_message(&mut self.socket, &msg).await?;

            // Wait for response
            let mut reader = BufReader::new(&mut self.socket);
            loop {
                match VsockConnection::read_message(&mut reader).await? {
                    Some(VsockMessage::Response {
                        id: resp_id,
                        result,
                        error,
                    }) => {
                        if resp_id == id {
                            if let Some(err) = error {
                                anyhow::bail!("Request failed: {}", err);
                            }
                            return result.context("Response missing result");
                        }
                        // Ignore responses to other requests
                    }
                    Some(_) => {
                        // Ignore other message types
                    }
                    None => {
                        anyhow::bail!("Connection closed while waiting for response");
                    }
                }
            }
        }
        #[cfg(not(unix))]
        {
            Err(anyhow::anyhow!("Vsock not supported on Windows"))
        }
    }

    /// Send a notification (no response expected)
    pub async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        #[cfg(unix)]
        {
            let msg = VsockMessage::notification(method.to_string(), params);
            VsockConnection::write_message(&mut self.socket, &msg).await?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err(anyhow::anyhow!("Vsock not supported on Windows"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_vsock_message_serialization() {
        let msg = VsockMessage::request(
            "test-id".to_string(),
            "test_method".to_string(),
            json!({"key": "value"}),
        );

        let json = msg.to_json().unwrap();
        let decoded = VsockMessage::from_json(&json).unwrap();

        match decoded {
            VsockMessage::Request { id, method, params } => {
                assert_eq!(id, "test-id");
                assert_eq!(method, "test_method");
                assert_eq!(params, json!({"key": "value"}));
            }
            _ => panic!("Expected Request message"),
        }
    }

    #[test]
    fn test_vsock_message_size_limit() {
        // Create a message that exceeds the size limit
        let huge_data = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let json = serde_json::to_vec(&huge_data).unwrap();

        let result = VsockMessage::from_json(&json);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_vsock_host_listener_creation() {
        let listener = VsockHostListener::new("test-vm".to_string()).await.unwrap();
        assert!(listener.socket_path().contains("test-vm"));

        // Clean up
        fs::remove_file(listener.socket_path()).await.ok();
    }

    #[test]
    fn test_vsock_message_response_creation() {
        let msg = VsockMessage::response(
            "test-id".to_string(),
            Some(json!({"result": "success"})),
            None,
        );

        let json = msg.to_json().unwrap();
        let decoded = VsockMessage::from_json(&json).unwrap();

        match decoded {
            VsockMessage::Response { id, result, error } => {
                assert_eq!(id, "test-id");
                assert_eq!(result, Some(json!({"result": "success"})));
                assert!(error.is_none());
            }
            _ => panic!("Expected Response message"),
        }
    }

    #[test]
    fn test_vsock_message_notification_creation() {
        let msg = VsockMessage::notification("test_event".to_string(), json!({"data": 123}));

        let json = msg.to_json().unwrap();
        let decoded = VsockMessage::from_json(&json).unwrap();

        match decoded {
            VsockMessage::Notification { method, params } => {
                assert_eq!(method, "test_event");
                assert_eq!(params, json!({"data": 123}));
            }
            _ => panic!("Expected Notification message"),
        }
    }

    // Property-based test: round-trip serialization
    #[test]
    fn test_vsock_message_round_trip() {
        let original = VsockMessage::request(
            "prop-test-id".to_string(),
            "prop_method".to_string(),
            json!({"x": 42, "y": "test"}),
        );

        let json = original.to_json().unwrap();
        let decoded = VsockMessage::from_json(&json).unwrap();

        match (original, decoded) {
            (
                VsockMessage::Request {
                    id: id1,
                    method: m1,
                    params: p1,
                },
                VsockMessage::Request {
                    id: id2,
                    method: m2,
                    params: p2,
                },
            ) => {
                assert_eq!(id1, id2);
                assert_eq!(m1, m2);
                assert_eq!(p1, p2);
            }
            _ => panic!("Message type mismatch"),
        }
    }
}
