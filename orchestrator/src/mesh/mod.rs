//! Private Mesh Protocol for Encrypted P2P Messaging
//!
//! This module provides secure peer-to-peer communication between LuminaGuard agents
//! using end-to-end encryption, message signing, and authenticated routing.
//!
//! # Architecture
//!
//! The mesh protocol uses:
//! - **Noise Protocol (XX pattern)**: For authenticated key exchange and encryption
//! - **Ed25519**: For message signing and authentication
//! - **ChaCha20-Poly1305**: For symmetric encryption (via Noise protocol)
//! - **MessagePack**: For efficient message serialization
//!
//! # Message Types
//!
//! - **Direct**: Point-to-point encrypted messages between agents
//! - **Broadcast**: Flood-style broadcast to all mesh peers
//! - **KeyRotation**: Secure key rotation messages
//!
//! # Security Properties
//!
//! - **Confidentiality**: All messages encrypted in transit
//! - **Integrity**: Messages signed to prevent tampering
//! - **Authentication**: Ed25519 signatures verify sender identity
//! - **Forward Secrecy**: Ephemeral keys provide forward secrecy
//! - **Replay Protection**: Nonces prevent message replay attacks
//!
//! # Example
//!
//! ```no_run
//! use luminaguard_orchestrator::mesh::{Messaging, MeshConfig};
//! use tokio::runtime::Runtime;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let rt = Runtime::new()?;
//! rt.block_on(async {
//!     let config = MeshConfig::new("researcher".to_string());
//!     let mut messaging = Messaging::new(config).await?;
//!
//!     // Send direct message
//!     messaging.send_direct(
//!         "peer-id",
//!         b"Hello from researcher!"
//!     ).await?;
//!
//!     // Broadcast message
//!     messaging.broadcast(b"Broadcast message").await?;
//!
//!     messaging.shutdown().await?;
//!     Ok(())
//! })
//! # }
//! ```

// pub mod messaging; // Temporarily disabled due to API compatibility issues
// pub use messaging::{
//     MeshConfig, MeshMessage, MeshMessageType, Messaging, MessagingError,
//     MessageId, PeerId, MeshStats,
// };
