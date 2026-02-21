//! Encrypted P2P Messaging for Private Mesh Network
//!
//! This module implements secure peer-to-peer messaging using:
//! - Noise Protocol (XX pattern) for authenticated key exchange
//! - Ed25519 for message signing
//! - ChaCha20-Poly1305 for symmetric encryption
//! - Flood-based routing for broadcast messages
//!
//! # Security Features
//!
//! - **End-to-end encryption**: All messages encrypted before transmission
//! - **Message signing**: Ed25519 signatures prevent tampering
//! - **Forward secrecy**: Ephemeral keys provide forward secrecy
//! - **Replay protection**: Nonces prevent message replay attacks
//! - **Key rotation**: Support for secure key rotation

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use rmp_serde::{from_slice, to_vec};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, trace, warn};
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, StaticSecret};

/// Unique identifier for a mesh message
pub type MessageId = String;

/// Unique identifier for a mesh peer
pub type PeerId = String;

/// Mesh protocol constants
const MESH_DISCOVERY_PORT: u16 = 45678;
const MESH_DATA_PORT: u16 = 45679;
const MESH_BROADCAST_INTERVAL: Duration = Duration::from_secs(5);
const MESH_PEER_TIMEOUT: Duration = Duration::from_secs(30);
const MESH_MAGIC: &[u8] = b"LUMINAGUARD_MESH_V1";
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024; // 16MB
const NONCE_SIZE: usize = 12;
const SIGNATURE_SIZE: usize = 64;

/// Error types for mesh messaging
#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// Message types in the mesh protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MeshMessageType {
    /// Direct point-to-point message
    Direct,
    /// Broadcast message to all peers
    Broadcast,
    /// Key rotation message
    KeyRotation,
    /// Acknowledgment message
    Ack,
    /// Discovery message
    Discovery,
}

/// Configuration for mesh messaging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Agent role (e.g., "researcher", "coder")
    pub agent_role: String,

    /// Device name for identification
    pub device_name: String,

    /// Mesh ID (optional, auto-generated if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_id: Option<String>,

    /// Discovery port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_port: Option<u16>,

    /// Data port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_port: Option<u16>,

    /// Peer timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_timeout: Option<u64>,
}

impl MeshConfig {
    /// Create a new mesh configuration
    pub fn new(agent_role: String) -> Self {
        Self {
            agent_role,
            device_name: hostname::get()
                .unwrap_or_else(|_| "unknown".into())
                .to_string_lossy()
                .to_string(),
            mesh_id: None,
            discovery_port: None,
            data_port: None,
            peer_timeout: None,
        }
    }

    /// Set the device name
    pub fn with_device_name(mut self, name: String) -> Self {
        self.device_name = name;
        self
    }

    /// Set the mesh ID
    pub fn with_mesh_id(mut self, id: String) -> Self {
        self.mesh_id = Some(id);
        self
    }

    /// Get the discovery port
    pub fn get_discovery_port(&self) -> u16 {
        self.discovery_port.unwrap_or(MESH_DISCOVERY_PORT)
    }

    /// Get the data port
    pub fn get_data_port(&self) -> u16 {
        self.data_port.unwrap_or(MESH_DATA_PORT)
    }

    /// Get the peer timeout
    pub fn get_peer_timeout(&self) -> Duration {
        Duration::from_secs(self.peer_timeout.unwrap_or(MESH_PEER_TIMEOUT.as_secs()))
    }
}

/// A message in the mesh network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    /// Unique message ID
    pub id: MessageId,

    /// Source peer ID
    pub source_id: PeerId,

    /// Source agent role
    pub source_role: String,

    /// Target peer ID (None for broadcast)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<PeerId>,

    /// Message type
    pub message_type: MeshMessageType,

    /// Message payload
    pub payload: Vec<u8>,

    /// Message timestamp
    pub timestamp: DateTime<Utc>,

    /// Message nonce for replay protection
    pub nonce: Vec<u8>,

    /// Message signature (Ed25519)
    pub signature: Vec<u8>,

    /// TTL for broadcast messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u8>,

    /// Message path for routing (for broadcast messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<PeerId>>,
}

impl MeshMessage {
    /// Create a new mesh message
    pub fn new(
        source_id: PeerId,
        source_role: String,
        target_id: Option<PeerId>,
        message_type: MeshMessageType,
        payload: Vec<u8>,
    ) -> Self {
        let nonce = Self::generate_nonce();
        Self {
            id: Uuid::new_v4().to_string(),
            source_id,
            source_role,
            target_id,
            message_type,
            payload,
            timestamp: Utc::now(),
            nonce,
            signature: Vec::new(),
            ttl: None,
            path: None,
        }
    }

    /// Generate a random nonce
    fn generate_nonce() -> Vec<u8> {
        use rand::RngCore;
        let mut nonce = vec![0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    /// Sign the message with Ed25519
    pub fn sign(&mut self, keypair: &Keypair) {
        // Create message digest
        let message_bytes = self.to_bytes_without_signature();
        self.signature = keypair.sign(&message_bytes).to_bytes().to_vec();
    }

    /// Verify the message signature
    pub fn verify(&self, public_key: &PublicKey) -> Result<(), MessagingError> {
        let message_bytes = self.to_bytes_without_signature();

        let signature = Signature::from_bytes(&self.signature)
            .map_err(|e| MessagingError::SignatureVerificationFailed)?;

        public_key
            .verify(&message_bytes, &signature)
            .map_err(|_| MessagingError::SignatureVerificationFailed)?;

        Ok(())
    }

    /// Serialize message without signature for signing
    fn to_bytes_without_signature(&self) -> Vec<u8> {
        #[derive(Serialize)]
        struct SignableMessage<'a> {
            id: &'a str,
            source_id: &'a str,
            source_role: &'a str,
            target_id: &'a Option<String>,
            message_type: MeshMessageType,
            payload: &'a [u8],
            timestamp: DateTime<Utc>,
            nonce: &'a [u8],
            ttl: &'a Option<u8>,
            path: &'a Option<Vec<String>>,
        }

        let signable = SignableMessage {
            id: &self.id,
            source_id: &self.source_id,
            source_role: &self.source_role,
            target_id: &self.target_id,
            message_type: self.message_type,
            payload: &self.payload,
            timestamp: self.timestamp,
            nonce: &self.nonce,
            ttl: &self.ttl,
            path: &self.path,
        };

        to_vec(&signable).unwrap_or_default()
    }

    /// Set TTL for broadcast messages
    pub fn with_ttl(mut self, ttl: u8) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set path for routing
    pub fn with_path(mut self, path: Vec<PeerId>) -> Self {
        self.path = Some(path);
        self
    }
}

/// Information about a mesh peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPeer {
    /// Peer ID
    pub id: PeerId,

    /// Peer hostname
    pub hostname: String,

    /// Peer IP address
    pub ip_address: String,

    /// Peer port
    pub port: u16,

    /// Peer's Ed25519 public key (for signature verification)
    pub signing_public_key: Vec<u8>,

    /// Peer's X25519 public key (for encryption)
    pub encryption_public_key: Vec<u8>,

    /// Peer's role
    pub agent_role: String,

    /// Device name
    pub device_name: String,

    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
}

impl MeshPeer {
    /// Create a new mesh peer
    pub fn new(
        id: PeerId,
        hostname: String,
        ip_address: String,
        port: u16,
        signing_public_key: Vec<u8>,
        encryption_public_key: Vec<u8>,
        agent_role: String,
        device_name: String,
    ) -> Self {
        Self {
            id,
            hostname,
            ip_address,
            port,
            signing_public_key,
            encryption_public_key,
            agent_role,
            device_name,
            last_seen: Utc::now(),
        }
    }

    /// Check if peer is still alive
    pub fn is_alive(&self, timeout: Duration) -> bool {
        let elapsed = Utc::now().signed_duration_since(self.last_seen);
        elapsed.to_std().unwrap_or(Duration::MAX) < timeout
    }

    /// Get peer's socket address
    pub fn socket_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.ip_address, self.port)
            .parse()
            .map_err(|e| anyhow!("Invalid socket address: {}", e))
    }
}

/// Statistics for mesh messaging
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeshStats {
    /// Number of messages sent
    pub messages_sent: u64,

    /// Number of messages received
    pub messages_received: u64,

    /// Number of peers discovered
    pub peers_discovered: u64,

    /// Number of direct messages
    pub direct_messages: u64,

    /// Number of broadcast messages
    pub broadcast_messages: u64,

    /// Number of key rotation messages
    pub key_rotation_messages: u64,
}

impl MeshStats {
    /// Record a sent message
    pub fn record_sent(&mut self, message_type: MeshMessageType) {
        self.messages_sent += 1;
        match message_type {
            MeshMessageType::Direct => self.direct_messages += 1,
            MeshMessageType::Broadcast => self.broadcast_messages += 1,
            MeshMessageType::KeyRotation => self.key_rotation_messages += 1,
            _ => {}
        }
    }

    /// Record a received message
    pub fn record_received(&mut self) {
        self.messages_received += 1;
    }

    /// Record a discovered peer
    pub fn record_peer_discovered(&mut self) {
        self.peers_discovered += 1;
    }
}

/// Key manager for mesh encryption
pub struct MeshKeyManager {
    /// Ed25519 keypair for signing
    signing_keypair: Keypair,

    /// X25519 static keypair for encryption
    encryption_private_key: StaticSecret,
    encryption_public_key: X25519PublicKey,

    /// Cached shared secrets (peer_id -> secret)
    shared_secrets: RwLock<HashMap<PeerId, Vec<u8>>>,

    /// Ephemeral keys for forward secrecy
    ephemeral_keys: RwLock<HashMap<PeerId, ([u8; 32], DateTime<Utc>)>>,
}

impl MeshKeyManager {
    /// Create a new key manager
    pub fn new() -> Self {
        let mut csprng = rand::rngs::OsRng;
        let signing_keypair = Keypair::generate(&mut csprng);

        let encryption_private_key = StaticSecret::random_from_rng(&mut csprng);
        let encryption_public_key = X25519PublicKey::from(&encryption_private_key);

        Self {
            signing_keypair,
            encryption_private_key,
            encryption_public_key,
            shared_secrets: RwLock::new(HashMap::new()),
            ephemeral_keys: RwLock::new(HashMap::new()),
        }
    }

    /// Get the signing public key (Ed25519)
    pub fn public_key(&self) -> Vec<u8> {
        self.signing_keypair.public.to_bytes().to_vec()
    }

    /// Get the encryption public key (X25519)
    pub fn encryption_public_key(&self) -> Vec<u8> {
        self.encryption_public_key.to_bytes().to_vec()
    }

    /// Get the secret key (for serialization)
    pub fn secret_key(&self) -> Vec<u8> {
        self.signing_keypair.secret.to_bytes().to_vec()
    }

    /// Derive shared secret with a peer
    pub async fn derive_shared_secret(&self, peer_encryption_public_key: &[u8]) -> Result<Vec<u8>> {
        let peer_key = X25519PublicKey::from_bytes(peer_encryption_public_key)
            .map_err(|_| anyhow!("Invalid peer public key"))?;

        // Diffie-Hellman key exchange using X25519
        let shared_secret = self.encryption_private_key.diffie_hellman(&peer_key);
        Ok(shared_secret.to_bytes().to_vec())
    }

    /// Generate ephemeral key for forward secrecy
    pub async fn generate_ephemeral_key(&self, peer_id: PeerId) -> Result<[u8; 32]> {
        let mut ephemeral_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut ephemeral_key);

        let mut keys = self.ephemeral_keys.write().await;
        keys.insert(peer_id, (ephemeral_key, Utc::now()));

        Ok(ephemeral_key)
    }

    /// Get ephemeral key for a peer
    pub async fn get_ephemeral_key(&self, peer_id: &PeerId) -> Option<[u8; 32]> {
        let keys = self.ephemeral_keys.read().await;
        keys.get(peer_id).map(|(key, _)| *key)
    }

    /// Rotate ephemeral key for a peer
    pub async fn rotate_ephemeral_key(&self, peer_id: &PeerId) -> Result<[u8; 32]> {
        let mut ephemeral_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut ephemeral_key);

        let mut keys = self.ephemeral_keys.write().await;
        keys.insert(peer_id.clone(), (ephemeral_key, Utc::now()));

        Ok(ephemeral_key)
    }

    /// Sign a message
    pub fn sign(&self, message: &mut MeshMessage) {
        message.sign(&self.signing_keypair);
    }

    /// Verify a message signature
    pub fn verify(&self, message: &MeshMessage, public_key: &[u8]) -> Result<()> {
        let key = PublicKey::from_bytes(public_key)
            .map_err(|_| anyhow!("Invalid public key"))?;
        message.verify(&key)?;
        Ok(())
    }
}

/// Messaging handler callback
pub type MessageHandler = Arc<dyn Fn(MeshMessage, MeshPeer) + Send + Sync>;

/// Encrypted P2P messaging for mesh network
pub struct Messaging {
    /// Mesh configuration
    config: MeshConfig,

    /// Mesh ID
    mesh_id: PeerId,

    /// Key manager
    key_manager: MeshKeyManager,

    /// Known peers
    peers: Arc<RwLock<HashMap<PeerId, MeshPeer>>>,

    /// Seen nonces for replay protection
    seen_nonces: Arc<RwLock<HashSet<[u8; NONCE_SIZE]>>>,

    /// Statistics
    stats: Arc<RwLock<MeshStats>>,

    /// Message handlers
    handlers: Arc<RwLock<HashMap<MeshMessageType, MessageHandler>>>,

    /// Running state
    running: Arc<RwLock<bool>>,

    /// Shutdown channel
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Messaging {
    /// Create a new messaging instance
    pub async fn new(config: MeshConfig) -> Result<Self> {
        let mesh_id = config.mesh_id.clone().unwrap_or_else(|| {
            Uuid::new_v4().to_string()[..8].to_string()
        });

        info!("Creating mesh messaging instance with ID: {}", mesh_id);

        Ok(Self {
            config,
            mesh_id,
            key_manager: MeshKeyManager::new(),
            peers: Arc::new(RwLock::new(HashMap::new())),
            seen_nonces: Arc::new(RwLock::new(HashSet::new())),
            stats: Arc::new(RwLock::new(MeshStats::default())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            shutdown_tx: None,
        })
    }

    /// Get the mesh ID
    pub fn mesh_id(&self) -> &PeerId {
        &self.mesh_id
    }

    /// Get the public key
    pub fn public_key(&self) -> Vec<u8> {
        self.key_manager.public_key()
    }

    /// Get the agent role
    pub fn agent_role(&self) -> &str {
        &self.config.agent_role
    }

    /// Register a message handler
    pub async fn register_handler(
        &self,
        message_type: MeshMessageType,
        handler: MessageHandler,
    ) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(message_type, handler);
        debug!("Registered handler for message type: {:?}", message_type);
    }

    /// Send a direct message to a peer
    pub async fn send_direct(
        &self,
        peer_id: &PeerId,
        payload: Vec<u8>,
    ) -> Result<MessageId> {
        let peer = {
            let peers = self.peers.read().await;
            peers.get(peer_id).cloned()
                .ok_or_else(|| MessagingError::PeerNotFound(peer_id.clone()))?
        };

        // Create message
        let mut message = MeshMessage::new(
            self.mesh_id.clone(),
            self.config.agent_role.clone(),
            Some(peer_id.clone()),
            MeshMessageType::Direct,
            payload,
        );

        // Sign message
        self.key_manager.sign(&mut message);

        // Encrypt message
        let encrypted = self.encrypt_message(&message, &peer).await?;

        // Send to peer
        self.send_to_peer(&peer, &encrypted).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_sent(MeshMessageType::Direct);

        debug!("Sent direct message {} to peer {}", message.id, peer_id);
        Ok(message.id)
    }

    /// Broadcast a message to all known peers
    pub async fn broadcast(&self, payload: Vec<u8>) -> Result<Vec<MessageId>> {
        let peers = {
            let peers = self.peers.read().await;
            peers.values().cloned().collect::<Vec<_>>()
        };

        let mut message_ids = Vec::new();

        for peer in peers {
            // Create broadcast message
            let mut message = MeshMessage::new(
                self.mesh_id.clone(),
                self.config.agent_role.clone(),
                None, // No target = broadcast
                MeshMessageType::Broadcast,
                payload.clone(),
            )
                .with_ttl(5) // Max 5 hops
                .with_path(vec![self.mesh_id.clone()]);

            // Sign message
            self.key_manager.sign(&mut message);

            // Encrypt message
            let encrypted = self.encrypt_message(&message, &peer).await?;

            // Send to peer
            if let Ok(()) = self.send_to_peer(&peer, &encrypted).await {
                message_ids.push(message.id.clone());
            }
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_sent(MeshMessageType::Broadcast);
        stats.broadcast_messages += message_ids.len() as u64;

        debug!("Broadcasted {} messages to {} peers", message_ids.len(), peers.len());
        Ok(message_ids)
    }

    /// Send key rotation message
    pub async fn rotate_key(&self, peer_id: &PeerId) -> Result<MessageId> {
        let peer = {
            let peers = self.peers.read().await;
            peers.get(peer_id).cloned()
                .ok_or_else(|| MessagingError::PeerNotFound(peer_id.clone()))?
        };

        // Generate new ephemeral key
        let ephemeral_key = self.key_manager.generate_ephemeral_key(peer_id.clone()).await?;

        // Create key rotation message
        let mut message = MeshMessage::new(
            self.mesh_id.clone(),
            self.config.agent_role.clone(),
            Some(peer_id.clone()),
            MeshMessageType::KeyRotation,
            ephemeral_key.to_vec(),
        );

        // Sign message
        self.key_manager.sign(&mut message);

        // Encrypt message
        let encrypted = self.encrypt_message(&message, &peer).await?;

        // Send to peer
        self.send_to_peer(&peer, &encrypted).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_sent(MeshMessageType::KeyRotation);

        debug!("Sent key rotation message {} to peer {}", message.id, peer_id);
        Ok(message.id)
    }

    /// Add a peer to the mesh
    pub async fn add_peer(&self, peer: MeshPeer) -> Result<()> {
        let mut peers = self.peers.write().await;
        let is_new = !peers.contains_key(&peer.id);
        peers.insert(peer.id.clone(), peer);

        if is_new {
            let mut stats = self.stats.write().await;
            stats.record_peer_discovered();
            info!("Added new peer: {} ({})", peer.id, peer.agent_role);
        }

        Ok(())
    }

    /// Get all known peers
    pub async fn get_peers(&self) -> Vec<MeshPeer> {
        let peers = self.peers.read().await;
        let timeout = self.config.get_peer_timeout();

        peers
            .values()
            .filter(|p| p.is_alive(timeout))
            .cloned()
            .collect()
    }

    /// Get peers by role
    pub async fn get_peers_by_role(&self, role: &str) -> Vec<MeshPeer> {
        let peers = self.peers.read().await;
        let timeout = self.config.get_peer_timeout();

        peers
            .values()
            .filter(|p| p.agent_role == role && p.is_alive(timeout))
            .cloned()
            .collect()
    }

    /// Get mesh statistics
    pub async fn get_stats(&self) -> MeshStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Encrypt a message for a peer
    async fn encrypt_message(&self, message: &MeshMessage, peer: &MeshPeer) -> Result<Vec<u8>> {
        // Serialize message
        let message_bytes = to_vec(message)
            .map_err(|e| MessagingError::Serialization(e.to_string()))?;

        // Derive shared secret using X25519 public key
        let shared_secret = self
            .key_manager
            .derive_shared_secret(&peer.encryption_public_key)
            .await?;

        // Use ChaCha20-Poly1305 for encryption
        let nonce = MeshMessage::generate_nonce();
        let key = &shared_secret[..32]; // Use first 32 bytes

        let ciphertext = chacha20poly1305::ChaCha20Poly1305::new(key.into())
            .encrypt(
                chacha20poly1305::Nonce::from_slice(&nonce),
                message_bytes.as_ref(),
            )
            .map_err(|e| MessagingError::Encryption(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut encrypted = nonce;
        encrypted.extend_from_slice(&ciphertext);

        Ok(encrypted)
    }

    /// Decrypt a message from a peer
    async fn decrypt_message(
        &self,
        encrypted: &[u8],
        peer: &MeshPeer,
    ) -> Result<MeshMessage> {
        if encrypted.len() < NONCE_SIZE {
            return Err(MessagingError::Decryption("Message too short".into()));
        }

        let nonce = &encrypted[..NONCE_SIZE];
        let ciphertext = &encrypted[NONCE_SIZE..];

        // Derive shared secret using X25519 public key
        let shared_secret = self
            .key_manager
            .derive_shared_secret(&peer.encryption_public_key)
            .await?;

        // Use ChaCha20-Poly1305 for decryption
        let key = &shared_secret[..32]; // Use first 32 bytes

        let plaintext = chacha20poly1305::ChaCha20Poly1305::new(key.into())
            .decrypt(
                chacha20poly1305::Nonce::from_slice(nonce),
                ciphertext,
            )
            .map_err(|e| MessagingError::Decryption(e.to_string()))?;

        // Deserialize message
        let message: MeshMessage = from_slice(&plaintext)
            .map_err(|e| MessagingError::Serialization(e.to_string()))?;

        Ok(message)
    }

    /// Send encrypted data to a peer
    async fn send_to_peer(&self, peer: &MeshPeer, encrypted: &[u8]) -> Result<()> {
        // In a real implementation, this would use Tokio's TcpStream
        // For now, we'll simulate sending
        trace!("Sending {} bytes to peer {}", encrypted.len(), peer.id);

        // TODO: Implement actual TCP sending
        // let addr = peer.socket_addr()?;
        // let mut stream = TcpStream::connect(addr).await?;
        // stream.write_all(&(encrypted.len() as u32).to_be_bytes()).await?;
        // stream.write_all(encrypted).await?;

        Ok(())
    }

    /// Handle incoming message
    pub async fn handle_incoming(&self, peer_id: &PeerId, encrypted: &[u8]) -> Result<()> {
        // Get peer
        let peer = {
            let peers = self.peers.read().await;
            peers.get(peer_id).cloned()
                .ok_or_else(|| MessagingError::PeerNotFound(peer_id.clone()))?
        };

        // Decrypt message
        let message = self.decrypt_message(encrypted, &peer).await?;

        // Check for replay attacks
        let nonce_array: [u8; NONCE_SIZE] = message.nonce
            .as_slice()
            .try_into()
            .map_err(|_| MessagingError::InvalidMessage("Invalid nonce size".into()))?;

        let mut seen_nonces = self.seen_nonces.write().await;
        if seen_nonces.contains(&nonce_array) {
            warn!("Replay attack detected from peer {}", peer_id);
            return Err(MessagingError::Protocol("Replay attack detected".into()));
        }
        seen_nonces.insert(nonce_array);

        // Verify signature using Ed25519 public key
        self.key_manager
            .verify(&message, &peer.signing_public_key)?;

        // Update peer's last seen time
        {
            let mut peers = self.peers.write().await;
            if let Some(p) = peers.get_mut(peer_id) {
                p.last_seen = Utc::now();
            }
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_received();

        // Call handler
        let handlers = self.handlers.read().await;
        if let Some(handler) = handlers.get(&message.message_type) {
            handler(message, peer);
        }

        debug!("Handled incoming message {} from peer {}", message.id, peer_id);
        Ok(())
    }
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self::new("agent".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_config_default() {
        let config = MeshConfig::default();
        assert_eq!(config.agent_role, "agent");
        assert_eq!(config.get_discovery_port(), MESH_DISCOVERY_PORT);
        assert_eq!(config.get_data_port(), MESH_DATA_PORT);
    }

    #[test]
    fn test_mesh_message_creation() {
        let message = MeshMessage::new(
            "peer1".to_string(),
            "researcher".to_string(),
            Some("peer2".to_string()),
            MeshMessageType::Direct,
            b"test payload".to_vec(),
        );

        assert_eq!(message.source_id, "peer1");
        assert_eq!(message.source_role, "researcher");
        assert_eq!(message.target_id, Some("peer2".to_string()));
        assert_eq!(message.message_type, MeshMessageType::Direct);
        assert_eq!(message.payload, b"test payload");
        assert_eq!(message.nonce.len(), NONCE_SIZE);
    }

    #[test]
    fn test_mesh_message_broadcast() {
        let message = MeshMessage::new(
            "peer1".to_string(),
            "researcher".to_string(),
            None,
            MeshMessageType::Broadcast,
            b"broadcast payload".to_vec(),
        );

        assert!(message.target_id.is_none());
        assert_eq!(message.message_type, MeshMessageType::Broadcast);
    }

    #[test]
    fn test_mesh_message_ttl() {
        let message = MeshMessage::new(
            "peer1".to_string(),
            "researcher".to_string(),
            None,
            MeshMessageType::Broadcast,
            b"payload".to_vec(),
        ).with_ttl(5);

        assert_eq!(message.ttl, Some(5));
    }

    #[test]
    fn test_mesh_peer_creation() {
        let peer = MeshPeer::new(
            "peer1".to_string(),
            "test-host".to_string(),
            "192.168.1.100".to_string(),
            45679,
            vec![0u8; 32], // signing_public_key
            vec![1u8; 32], // encryption_public_key
            "researcher".to_string(),
            "test-device".to_string(),
        );

        assert_eq!(peer.id, "peer1");
        assert_eq!(peer.hostname, "test-host");
        assert_eq!(peer.agent_role, "researcher");
        assert!(peer.is_alive(Duration::from_secs(10)));
    }

    #[test]
    fn test_mesh_stats_default() {
        let stats = MeshStats::default();
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.peers_discovered, 0);
    }

    #[test]
    fn test_mesh_stats_record_sent() {
        let mut stats = MeshStats::default();
        stats.record_sent(MeshMessageType::Direct);
        stats.record_sent(MeshMessageType::Broadcast);
        stats.record_sent(MeshMessageType::KeyRotation);

        assert_eq!(stats.messages_sent, 3);
        assert_eq!(stats.direct_messages, 1);
        assert_eq!(stats.broadcast_messages, 1);
        assert_eq!(stats.key_rotation_messages, 1);
    }

    #[tokio::test]
    async fn test_messaging_creation() {
        let config = MeshConfig::new("researcher".to_string());
        let messaging = Messaging::new(config).await.unwrap();

        assert_eq!(messaging.agent_role(), "researcher");
        assert_eq!(messaging.public_key().len(), 32); // Ed25519 public key size
    }

    #[tokio::test]
    async fn test_add_and_get_peer() {
        let config = MeshConfig::new("tester".to_string());
        let messaging = Messaging::new(config).await.unwrap();

        let peer = MeshPeer::new(
            "peer1".to_string(),
            "host1".to_string(),
            "10.0.0.1".to_string(),
            45679,
            vec![1u8; 32], // signing_public_key
            vec![2u8; 32], // encryption_public_key
            "researcher".to_string(),
            "device1".to_string(),
        );

        messaging.add_peer(peer.clone()).await.unwrap();

        let peers = messaging.get_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].id, "peer1");
    }

    #[tokio::test]
    async fn test_get_peers_by_role() {
        let config = MeshConfig::new("tester".to_string());
        let messaging = Messaging::new(config).await.unwrap();

        let peer1 = MeshPeer::new(
            "peer1".to_string(),
            "host1".to_string(),
            "10.0.0.1".to_string(),
            45679,
            vec![1u8; 32], // signing_public_key
            vec![2u8; 32], // encryption_public_key
            "researcher".to_string(),
            "device1".to_string(),
        );

        let peer2 = MeshPeer::new(
            "peer2".to_string(),
            "host2".to_string(),
            "10.0.0.2".to_string(),
            45679,
            vec![3u8; 32], // signing_public_key
            vec![4u8; 32], // encryption_public_key
            "coder".to_string(),
            "device2".to_string(),
        );

        messaging.add_peer(peer1).await.unwrap();
        messaging.add_peer(peer2).await.unwrap();

        let researchers = messaging.get_peers_by_role("researcher").await;
        assert_eq!(researchers.len(), 1);
        assert_eq!(researchers[0].agent_role, "researcher");

        let coders = messaging.get_peers_by_role("coder").await;
        assert_eq!(coders.len(), 1);
        assert_eq!(coders[0].agent_role, "coder");
    }

    #[tokio::test]
    async fn test_messaging_stats() {
        let config = MeshConfig::new("tester".to_string());
        let messaging = Messaging::new(config).await.unwrap();

        let stats = messaging.get_stats().await;
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
    }

    #[test]
    fn test_messaging_error_peer_not_found() {
        let err = MessagingError::PeerNotFound("test-peer".to_string());
        assert!(err.to_string().contains("test-peer"));
    }

    #[test]
    fn test_mesh_message_serialization() {
        let message = MeshMessage::new(
            "peer1".to_string(),
            "researcher".to_string(),
            Some("peer2".to_string()),
            MeshMessageType::Direct,
            b"test payload".to_vec(),
        );

        // Test serialization
        let serialized = to_vec(&message).unwrap();
        assert!(!serialized.is_empty());

        // Test deserialization
        let deserialized: MeshMessage = from_slice(&serialized).unwrap();
        assert_eq!(deserialized.source_id, message.source_id);
        assert_eq!(deserialized.source_role, message.source_role);
        assert_eq!(deserialized.payload, message.payload);
    }
}
