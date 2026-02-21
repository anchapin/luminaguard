//! Mesh Module Error Types
//!
//! This module defines all error types that can occur during mesh operations.

/// Error types for mesh operations
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    /// Discovery service error
    #[error("Discovery error: {0}")]
    Discovery(String),

    /// mDNS/Bonjour error
    #[error("mDNS error: {0}")]
    Mdns(String),

    /// Peer not found
    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    /// Invalid peer certificate
    #[error("Invalid certificate: {0}")]
    InvalidCertificate(String),

    /// Invalid message signature
    #[error("Invalid signature from peer: {0}")]
    InvalidSignature(String),

    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Decryption error
    #[error("Decryption error: {0}")]
    Decryption(String),

    /// Protocol version mismatch
    #[error("Protocol version mismatch: expected {0}, got {1}")]
    VersionMismatch(String, String),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Timeout error
    #[error("Operation timed out after {0}ms")]
    Timeout(u64),

    /// Message too large
    #[error("Message size {0} exceeds maximum {1} bytes")]
    MessageTooLarge(usize, usize),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Session expired
    #[error("Session expired")]
    SessionExpired,

    /// Peer offline
    #[error("Peer {0} is offline (last seen {1}ms ago)")]
    PeerOffline(String, u64),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Routing error
    #[error("Routing error: {0}")]
    Routing(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: too many requests in {0}ms")]
    RateLimitExceeded(u64),
}

impl From<std::io::Error> for MeshError {
    fn from(err: std::io::Error) -> Self {
        MeshError::Network(err.to_string())
    }
}
