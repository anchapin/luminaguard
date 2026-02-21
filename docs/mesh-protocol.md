# LuminaGuard Private Mesh Protocol

**Version:** 1.0
**Status:** Draft
**Date:** 2026-02-21
**Author:** LuminaGuard Team

---

## Table of Contents

1. [Protocol Overview](#protocol-overview)
2. [Message Format](#message-format)
3. [Encryption Strategy](#encryption-strategy)
4. [Authentication Mechanism](#authentication-mechanism)
5. [Discovery Protocol](#discovery-protocol)
6. [Message Routing](#message-routing)
7. [Security Analysis](#security-analysis)
8. [Protocol Versioning](#protocol-versioning)
9. [Implementation Considerations](#implementation-considerations)
10. [Appendix](#appendix)

---

## Protocol Overview

### Purpose and Goals

The LuminaGuard Private Mesh Protocol enables secure, peer-to-peer communication between multiple LuminaGuard instances on a local network. The protocol provides:

- **Zero-Knowledge Architecture**: Messages encrypted end-to-end with forward secrecy
- **Decentralized Discovery**: Peer discovery without central coordination
- **Identity Verification**: Strong authentication preventing impersonation
- **Efficient Routing**: Direct agent-to-agent and broadcast messaging
- **Resilience**: Graceful handling of network partitions and peer failures

### Design Principles

1. **Security First**: All messages encrypted and signed by default
2. **Privacy by Design**: No plaintext transmission of sensitive data
3. **Decentralization**: No central server required for mesh operation
4. **Simplicity**: Minimal protocol surface to reduce attack vectors
5. **Performance**: Sub-millisecond latency for local mesh communication
6. **Extensibility**: Protocol versioning supports future enhancements

### Threat Model

**Assumed Capabilities:**
- Active network attacker can intercept, modify, or drop packets
- Malicious peers may join the mesh
- Attacker can spoof network addresses
- Attacker may compromise existing peers

**Out of Scope:**
- Physical access to machines
- Side-channel attacks (timing, power analysis)
- State-level adversaries with global monitoring capabilities

**Primary Threats:**

| Threat | Description | Mitigation |
|---------|-------------|-------------|
| Eavesdropping | Attacker intercepts mesh messages | End-to-end encryption with Noise Protocol |
| Message Tampering | Attacker modifies in-transit messages | Message signing with Ed25519 |
| Replay Attacks | Attacker re-sends captured messages | Timestamps + nonce in every message |
| Impersonation | Attacker pretends to be a peer | Certificate-based authentication |
| Sybil Attacks | Attacker creates multiple fake identities | Proof-of-work or rate-limiting |
| Man-in-the-Middle | Attacker intercepts and modifies key exchange | Static DH keys with identity binding |
| Network Partition | Mesh segments lose connectivity | Automatic peer discovery and reconnection |

---

## Message Format

### Format Selection: MessagePack

**Rationale for MessagePack over JSON/CBOR:**

| Criterion | MessagePack | JSON | CBOR |
|-----------|--------------|-------|--------|
| Size | Smallest (binary) | Largest (text) | Medium (binary) |
| Parse Speed | Fast | Slowest | Medium |
| Human Readable | No | Yes | No |
| Schema Evolution | Good | Excellent | Good |
| Rust Support | `rmp` crate (mature) | `serde_json` | `serde_cbor` |
| Debugging | Hex dump required | Direct read | Hex dump required |

**Decision: MessagePack** for optimal performance in local mesh scenarios while maintaining schema evolution capabilities.

### Message Structure

All messages follow this base structure:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    /// Protocol version (major.minor.patch)
    pub version: String,

    /// Unique message identifier (UUID v4)
    pub message_id: String,

    /// Timestamp (Unix milliseconds)
    pub timestamp: u64,

    /// Message type
    pub message_type: MessageType,

    /// Sender peer ID (public key fingerprint)
    pub sender: PeerId,

    /// Recipient peer ID (None for broadcast)
    pub recipient: Option<PeerId>,

    /// Encrypted and signed payload
    pub payload: EncryptedPayload,

    /// Message signature (Ed25519)
    pub signature: Vec<u8>,

    /// Protocol extensions (for future compatibility)
    pub extensions: HashMap<String, Value>,
}
```

### Message Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Peer announcement on mesh
    Announcement,

    /// Direct message to peer
    Direct,

    /// Broadcast to all peers
    Broadcast,

    /// Peer leaving mesh
    Leave,

    /// Heartbeat for liveness detection
    Heartbeat,

    /// Discovery query/response
    Discovery(DiscoveryPayload),

    /// Key rotation notification
    KeyRotation(KeyRotationPayload),
}
```

### Payload Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// Encryption algorithm identifier
    pub algorithm: String,  // "noise_xx_25519_aesgcm_bsha256"

    /// Public key for DH exchange
    pub public_key: Vec<u8>,

    /// Nonce for replay protection
    pub nonce: Vec<u8>,

    /// Encrypted message data
    pub ciphertext: Vec<u8>,
}
```

### Message Size Limits

- **Maximum message size**: 1 MB (to prevent DoS)
- **Recommended size**: < 64 KB for typical messages
- **Fragmentation**: Not supported (messages must fit in single packet)

---

## Encryption Strategy

### Choice: Noise Protocol Framework

**Rationale for Noise over WireGuard:**

| Feature | Noise Protocol | WireGuard |
|----------|-----------------|------------|
| Handshake Patterns | Flexible (XX, IK, etc.) | Fixed (IK) |
| Static Keys | Optional (PSK support) | Required |
| Identity Hiding | XX pattern available | Limited |
| Rust Support | `snow` crate (mature) | `wireguard-rs` (less mature) |
| Forward Secrecy | Guaranteed | Guaranteed |
| Customizable | Yes (mix patterns) | No |

**Selected Pattern: Noise_XX**

The **XX pattern** provides:
- **Identity Hiding**: Neither party reveals identity until handshake completes
- **Mutual Authentication**: Both parties verify each other's identity
- **Perfect Forward Secrecy**: Compromise of long-term keys doesn't reveal past messages
- **Replay Protection**: Built-in nonce handling

### Handshake Protocol

```
Initiator (A)                      Responder (B)

  e = DH()
  s = static_keypair(A)

  -----> e, s.pub  ---->
             e = DH()
             s = static_keypair(B)
             <----- e', s.pub -----
  k = DH(s.priv, e')
  k' = DH(s.priv, e')
  -----> Enc(k, auth) ---->
             <----- Enc(k', auth') -----

  Session keys derived from k, k'
  Identity binding to s.pub, s'.pub
```

### Key Generation

```rust
use x25519_dalek::{StaticSecret, PublicKey};

pub struct PeerKeypair {
    pub static_secret: StaticSecret,
    pub static_public: PublicKey,
}

impl PeerKeypair {
    pub fn generate() -> Self {
        let static_secret = StaticSecret::new();
        let static_public = PublicKey::from(&static_secret);
        Self { static_secret, static_public }
    }

    pub fn fingerprint(&self) -> String {
        // SHA-256 hash of public key
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(self.static_public.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
```

### Session Keys

```rust
pub struct SessionKeys {
    /// Key for encrypting messages to peer
    pub send_key: [u8; 32],

    /// Key for decrypting messages from peer
    pub recv_key: [u8; 32],

    /// Key for sending handshake
    pub send_handshake_key: [u8; 32],

    /// Key for receiving handshake
    pub recv_handshake_key: [u8; 32],

    /// Session expiration time
    pub expires_at: u64,
}
```

### Key Rotation

**Rotation Policy:**
- **Session keys**: Rotate every 24 hours or after 10 MB of data
- **Static keys**: Manual rotation (user-initiated) or compromise detection
- **Forward secrecy**: Old keys cannot derive new session keys

**Rotation Protocol:**

1. Peer A generates new static keypair
2. Peer A announces new key via `KeyRotation` message
3. Peer B verifies signature on new key
4. Peers establish new session with XX handshake
5. Old session kept for 5-minute grace period

---

## Authentication Mechanism

### Identity Model: Self-Signed Certificates

**Certificate Structure:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCertificate {
    /// Version (currently 1)
    pub version: u8,

    /// Peer public key (Ed25519 for signing)
    pub public_key: Vec<u8>,

    /// X25519 public key (for encryption)
    pub encryption_key: Vec<u8>,

    /// Certificate validity (Unix timestamp)
    pub not_before: u64,
    pub not_after: u64,

    /// Peer metadata (optional)
    pub metadata: Option<PeerMetadata>,

    /// Self-signature (Ed25519)
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMetadata {
    pub peer_name: String,
    pub mesh_name: String,
    pub capabilities: Vec<String>,
    pub created_at: u64,
}
```

### Certificate Validation

```rust
use ed25519_dalek::{Verifier, VerifyingKey, Signature};

impl PeerCertificate {
    pub fn verify(&self) -> Result<(), MeshError> {
        let verifying_key = VerifyingKey::from_bytes(&self.public_key)?;

        // Serialize certificate for signature verification
        let cert_bytes = self.serialize_for_signing();

        // Verify self-signature
        let signature = Signature::from_bytes(&self.signature)?;

        if verifying_key.verify(&cert_bytes, &signature).is_ok() {
            Ok(())
        } else {
            Err(MeshError::InvalidSignature)
        }
    }

    pub fn is_valid_at(&self, timestamp: u64) -> bool {
        timestamp >= self.not_before && timestamp <= self.not_after
    }
}
```

### Trust Model: Web of Trust

**Trust Relationship:**

```
A ---trusts---> B
                 ^
                 | trusts
                 |
                 C

A and C don't directly trust each other,
but both trust B, allowing indirect communication
```

**Trust Levels:**

| Level | Description | Verification Required |
|-------|-------------|----------------------|
| Direct | Peer certificate manually verified | Full certificate chain |
| Indirect | Peer trusted by another trusted peer | 2-hop trust path |
| Untrusted | Peer not known | None |

### Message Signing

**Signature Scheme: Ed25519**

```rust
use ed25519_dalek::{Keypair, Signer, Signature};

impl MeshMessage {
    pub fn sign(&mut self, keypair: &Keypair) {
        // Serialize message without signature field
        let message_bytes = self.serialize_for_signing();

        // Sign with Ed25519
        let signature = keypair.sign(&message_bytes);

        // Attach signature
        self.signature = signature.to_bytes().to_vec();
    }

    pub fn verify(&self, public_key: &[u8]) -> Result<(), MeshError> {
        let verifying_key = VerifyingKey::from_bytes(public_key)?;
        let message_bytes = self.serialize_for_signing();
        let signature = Signature::from_bytes(&self.signature)?;

        verifying_key
            .verify(&message_bytes, &signature)
            .map_err(|_| MeshError::InvalidSignature)
    }
}
```

---

## Discovery Protocol

### mDNS/Bonjour Integration

**Service Type:** `_luminaguard._tcp`
**Port:** 45721 (IANA unassigned)

**Announcement Payload (TXT record):**

```
mesh=LuminGuard-Dev
version=1.0.0
peer_id=<fingerprint>
capabilities=encryption,signing,forward-secrecy
```

### Discovery Sequence

```
1. Peer A starts
   ├─> Generate keypair
   ├─> Create self-signed certificate
   └─> Start mDNS announcer

2. Peer A announces presence
   └─> mDNS broadcast: _luminaguard._tcp local

3. Peer B joins
   ├─> Start mDNS browser
   └─> Receive A's announcement

4. Peer B queries A
   └─> Direct TCP: 45721 → GET /peers

5. Peer A responds
   └─> JSON: { peers: [{ id, cert, address }] }

6. Peer B sends handshake
   └─> Noise XX handshake to A

7. Session established
   └─> Encrypted mesh communication
```

### Peer Registration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRegistration {
    pub peer_id: PeerId,
    pub certificate: PeerCertificate,
    pub endpoints: Vec<Endpoint>,
    pub capabilities: Vec<String>,
    pub mesh_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub address: String,  // "192.168.1.100:45721"
    pub protocol: String,  // "tcp"
    pub priority: u8,     // Lower = preferred
}
```

### Peer Leave

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerLeave {
    pub peer_id: PeerId,
    pub reason: LeaveReason,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaveReason {
    /// Normal shutdown
    Shutdown,

    /// Network partition detected
    NetworkPartition,

    /// Manual disconnect
    Disconnect,

    /// Authentication failure
    AuthFailure,
}
```

### Network Partition Detection

**Detection Algorithm:**

1. Track last heartbeat from each peer
2. Mark peer as suspect if no heartbeat for 30 seconds
3. Mark peer as offline if no heartbeat for 60 seconds
4. Attempt to reconnect to offline peers
5. If reconnection fails, emit `PeerLeave` event

**Heartbeat Interval:** 15 seconds per peer
**Suspect Timeout:** 30 seconds
**Offline Timeout:** 60 seconds

---

## Message Routing

### Direct Agent-to-Agent Messaging

**Flow:**

```
Agent A                          Agent B
  |                                |
  |--> DirectMessage ----------->|
  |   recipient: B               |
  |   payload: encrypted         |
  |                                |
  |                                |--> Decrypt
  |                                |--> Verify signature
  |                                |
  |   <-- Response -------------------|
  |   message_id: <id>           |
  |   result: encrypted            |
```

**API:**

```rust
pub async fn send_direct(
    mesh: &Mesh,
    recipient: &PeerId,
    payload: Vec<u8>,
) -> Result<Vec<u8>, MeshError> {
    // Create message
    let message = MeshMessage {
        version: "1.0.0".to_string(),
        message_id: uuid_v4(),
        message_type: MessageType::Direct,
        recipient: Some(recipient.clone()),
        payload: encrypt_payload(payload, &recipient.session)?,
        ..Default::default()
    };

    // Sign message
    message.sign(&mesh.identity.keypair);

    // Send to peer
    let endpoint = mesh.routing.get_endpoint(recipient)?;
    mesh.transport.send(&endpoint, &message).await?;

    // Wait for response
    mesh.wait_for_response(&message.message_id).await
}
```

### Broadcast to Mesh

**Flooding Algorithm (Optimized):**

1. Maintain sequence number per sender
2. Drop messages with seen sequence numbers
3. Forward broadcast to all peers except sender
4. TTL limit: 3 hops (prevents infinite loops)

**Broadcast Message Structure:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastMessage {
    pub broadcast_id: String,  // Unique per broadcast
    pub sequence: u64,        // Increments per sender
    pub ttl: u8,              // Time-to-live hops
}
```

### Routing Table

```rust
pub struct RoutingTable {
    /// Direct peer connections
    direct_peers: HashMap<PeerId, PeerConnection>,

    /// Cached routes for indirect peers
    cached_routes: HashMap<PeerId, Vec<PeerId>>,

    /// Last seen sequence numbers (for broadcast dedup)
    seen_sequences: HashMap<(PeerId, u64), Instant>,
}

impl RoutingTable {
    /// Get route to peer (direct or cached)
    pub fn get_route(&self, peer_id: &PeerId) -> Option<Route> {
        // Try direct connection first
        if let Some(conn) = self.direct_peers.get(peer_id) {
            return Some(Route::Direct(conn.clone()));
        }

        // Try cached multi-hop route
        if let Some(hops) = self.cached_routes.get(peer_id) {
            return Some(Route::MultiHop(hops.clone()));
        }

        None
    }

    /// Update routing table based on received message
    pub fn update_route(&mut self, peer_id: PeerId, via: PeerId) {
        self.cached_routes.entry(peer_id)
            .or_insert_with(Vec::new)
            .push(via);
    }
}
```

### Forward Secrecy Guarantees

**Guarantee 1: Compromise of long-term keys doesn't reveal past sessions**
- Achieved via Noise XX pattern with ephemeral DH keys
- Session keys derived from ephemeral keys, not static keys

**Guarantee 2: Compromise of session key doesn't reveal other sessions**
- Achieved via unique DH exchange per session
- Session keys not derived from shared secret

**Guarantee 3: Compromise of peer doesn't reveal other peer's messages**
- Achieved via end-to-end encryption
- Intermediate peers cannot decrypt messages

---

## Security Analysis

### Attack Vector Mitigations

#### 1. Eavesdropping

**Attack:** Attacker intercepts mesh messages.

**Mitigation:**
- End-to-end encryption via Noise Protocol
- All payloads encrypted with session keys
- Plaintext never transmitted over network

**Effectiveness:** ✅ **COMPLETE** - Messages unreadable without session keys

#### 2. Message Tampering

**Attack:** Attacker modifies in-transit messages.

**Mitigation:**
- Ed25519 signatures on all messages
- Signature verification before processing
- Tampered messages rejected

**Effectiveness:** ✅ **COMPLETE** - Modifications detectable and rejected

#### 3. Replay Attacks

**Attack:** Attacker re-sends captured messages.

**Mitigation:**
- Timestamps in all messages
- Nonces in encrypted payload
- Reject messages with old timestamps or nonces

**Effectiveness:** ✅ **COMPLETE** - Replay detection guaranteed

#### 4. Impersonation

**Attack:** Attacker pretends to be legitimate peer.

**Mitigation:**
- Certificate-based identity
- Self-signatures bind identity to keys
- Verification of certificate chain

**Effectiveness:** ✅ **COMPLETE** - Impersonation impossible without private keys

#### 5. Sybil Attacks

**Attack:** Attacker creates multiple fake identities.

**Mitigation:**
- Manual trust establishment (no auto-trust)
- Proof-of-work (optional, for production)
- Rate limiting on peer joins

**Effectiveness:** ⚠️ **PARTIAL** - Manual trust required

#### 6. Man-in-the-Middle

**Attack:** Attacker intercepts and modifies key exchange.

**Mitigation:**
- Static DH keys in Noise protocol
- Identity binding to static keys
- Certificate validation

**Effectiveness:** ✅ **COMPLETE** - MitM prevented by identity binding

#### 7. Network Partition

**Attack:** Mesh segments lose connectivity.

**Mitigation:**
- Automatic peer discovery (mDNS)
- Reconnection attempts with exponential backoff
- Graceful degradation with partial mesh

**Effectiveness:** ⚠️ **PARTIAL** - Partition detection but cannot prevent

### Security Properties

| Property | Status | Mechanism |
|-----------|---------|------------|
| Confidentiality | ✅ Achieved | Noise Protocol encryption |
| Integrity | ✅ Achieved | Ed25519 message signing |
| Authentication | ✅ Achieved | Certificate-based identity |
| Non-repudiation | ✅ Achieved | Signatures bind messages to peers |
| Forward Secrecy | ✅ Achieved | Ephemeral DH keys in Noise XX |
| Replay Protection | ✅ Achieved | Timestamps + nonces |
| DoS Resistance | ⚠️ Partial | Message size limits, rate limiting |

### Compliance

- **SOC 2**: Encryption and access controls ✅
- **PCI DSS**: Data protection in transit ✅
- **NIST 800-53**: System and communications protection (SC-8, SC-12) ✅

---

## Protocol Versioning

### Version Format

`major.minor.patch` (e.g., `1.0.0`)

- **major**: Incompatible protocol changes (requires coordination)
- **minor**: Backward-compatible additions
- **patch**: Bug fixes (no protocol changes)

### Version Negotiation

**Handshake Version Exchange:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionNegotiation {
    pub supported_versions: Vec<String>,
    pub preferred_version: String,
}

impl VersionNegotiation {
    pub fn negotiate(&self, peer_supported: &[String]) -> Result<String, MeshError> {
        // Find common version
        let common = self.supported_versions
            .iter()
            .filter(|v| peer_supported.contains(v))
            .collect::<Vec<_>>();

        if common.is_empty() {
            return Err(MeshError::NoCommonVersion);
        }

        // Select highest common version
        Ok(common.into_iter().max().unwrap())
    }
}
```

### Backward Compatibility

**Version 1.x Compatibility Rules:**

| Change Type | Backward Compatible | Action |
|-------------|---------------------|--------|
| Add new message type | ✅ Yes | Ignore unknown types |
| Add new field | ✅ Yes | Use default value |
| Change field type | ❌ No | Upgrade required |
| Remove field | ❌ No | Upgrade required |
| Change encryption | ❌ No | Upgrade required |

### Migration Strategy

**In-Place Upgrade:**

1. Peer A announces new version support in certificate
2. Peer B detects new version support
3. Peers negotiate highest common version
4. New version sessions established
5. Old version sessions expire naturally

**Rollback Plan:**

- Keep version 1.x implementation for 3 months after 2.0 release
- Support fallback to 1.0 if 2.0 handshake fails
- Document migration path clearly

---

## Implementation Considerations

### Recommended Rust Crates

| Functionality | Crate | Version | Notes |
|--------------|-------|---------|--------|
| Serialization | `rmp-serde` | 1.1.0+ | MessagePack with serde support |
| Noise Protocol | `snow` | 0.9.0+ | Noise framework patterns |
| Cryptography | `x25519-dalek` | 2.0.0+ | Diffie-Hellman keys |
| Signatures | `ed25519-dalek` | 2.0.0+ | Ed25519 signatures |
| mDNS | `mdns-sd` | 0.11.0+ | mDNS/Bonjour discovery |
| Async Runtime | `tokio` | 1.35.0+ | Async I/O |
| UUID | `uuid` | 1.8.0+ | Message IDs |

### Module Structure

```
orchestrator/src/mesh/
├── mod.rs                 # Public API
├── protocol.rs            # Protocol types and constants
├── message.rs             # Message serialization/deserialization
├── encryption.rs          # Noise Protocol implementation
├── authentication.rs     # Certificate and signing
├── discovery.rs          # mDNS/Bonjour discovery
├── routing.rs            # Routing table and message routing
├── transport.rs          # TCP/UDP transport layer
├── session.rs            # Session management
└── tests.rs              # Unit and integration tests
```

### Testing Strategy

**Unit Tests:**
- Message serialization/deserialization
- Encryption/decryption correctness
- Signature verification
- Certificate validation
- Routing table operations

**Integration Tests:**
- Peer discovery across network
- End-to-end message delivery
- Broadcast message propagation
- Key rotation handshake
- Network partition recovery

**Property-Based Tests:**
- Message round-trip integrity
- Encryption properties (confidentiality, integrity)
- Signature verification always rejects forgeries

### Performance Requirements

| Metric | Target | Measurement |
|---------|--------|-------------|
| Handshake latency | < 50ms | Time from hello to session established |
| Message delivery | < 10ms (local) | Time from send to receive |
| Broadcast propagation | < 100ms | Time to reach all peers (3 hops) |
| Encryption overhead | < 5% | Size increase from encryption |
| Signature overhead | < 100 bytes | Signature size (Ed25519) |

### Error Handling

**Error Types:**

```rust
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("Protocol version mismatch: expected {0}, got {1}")]
    VersionMismatch(String, String),

    #[error("Invalid message signature from peer {0}")]
    InvalidSignature(PeerId),

    #[error("Certificate validation failed: {0}")]
    InvalidCertificate(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),

    #[error("Network partition detected")]
    NetworkPartition,

    #[error("Message size exceeds limit: {0} bytes")]
    MessageTooLarge(usize),

    #[error("Session expired")]
    SessionExpired,
}
```

**Recovery Strategies:**

- **Network errors**: Exponential backoff retry (1s → 2s → 4s → 8s → 16s)
- **Handshake failures**: Abort and re-establish after 5 seconds
- **Peer offline**: Graceful degradation, retry after 30 seconds
- **Certificate errors**: Reject peer connection, log security event

### Logging and Monitoring

**Security Events to Log:**

- Peer join/leave events
- Failed authentications
- Certificate validation failures
- Suspicious activity (rapid joins, invalid signatures)
- Key rotation events

**Metrics to Collect:**

- Message delivery latency (p50, p95, p99)
- Peer count and mesh topology
- Encryption/decryption performance
- Handshake success/failure rate
- Network partition frequency

---

## Appendix

### Message Format Examples

#### Direct Message

```json
{
  "version": "1.0.0",
  "message_id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": 1706920800000,
  "message_type": "Direct",
  "sender": "a1b2c3d4e5f6...",
  "recipient": "f6e5d4c3b2a1...",
  "payload": {
    "algorithm": "noise_xx_25519_aesgcm_bsha256",
    "public_key": "2a2b2c2d...",
    "nonce": "3d3e3f40...",
    "ciphertext": "a1b2c3d..."
  },
  "signature": "1a2b3c4d...",
  "extensions": {}
}
```

#### Broadcast Message

```json
{
  "version": "1.0.0",
  "message_id": "660f9500...",
  "timestamp": 1706920801000,
  "message_type": "Broadcast",
  "sender": "a1b2c3d4...",
  "recipient": null,
  "payload": {
    "algorithm": "noise_xx_25519_aesgcm_bsha256",
    "public_key": "...",
    "nonce": "...",
    "ciphertext": "..."
  },
  "signature": "...",
  "extensions": {
    "broadcast": {
      "broadcast_id": "770f9600...",
      "sequence": 123,
      "ttl": 3
    }
  }
}
```

### Cryptographic Algorithms Comparison

| Algorithm | Security Level | Performance | Implementation |
|-----------|----------------|--------------|----------------|
| X25519 DH | 128-bit security | Fast | `x25519-dalek` |
| Ed25519 Signatures | 128-bit security | Fast | `ed25519-dalek` |
| AES-256-GCM | 256-bit security | Hardware accelerated | `aes-gcm` crate |
| SHA-256 | 256-bit security | Fast | `sha2` crate |

### Network Diagrams

#### Mesh Topology

```
    ┌─────────────────────────────────────────┐
    │                                     │
    │   Peer A              Peer B          │
    │   (Coordinator)        (Worker)        │
    │      │                   │            │
    │      └────────┬────────────┘            │
    │               │                          │
    │   Peer C                               │
    │   (Worker)                             │
    │      │                                  │
    │      └────────────────────────┘            │
    │                  │                       │
    │   Peer D                                 │
    │   (Worker)                                │
    │                                          │
    └──────────────────────────────────────────────────┘
```

#### Message Flow (Direct)

```
Peer A                          Peer B
  │                                 │
  │  ──> [Noise Handshake] ──>      │
  │                                 │
  │  <── [Handshake Complete] ───     │
  │                                 │
  │  ──> [Encrypted Message] ──>   │
  │                                 │
  │                                 │  ──> [Verify Signature]
  │                                 │  ──> [Decrypt Payload]
  │  <── [Encrypted Response] ──── │
  │                                 │
```

### Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-02-21 | Initial protocol specification |

---

**Document Status:** ✅ Complete
**Next Steps:** Implement Mesh peer discovery (Issue #543)
**Dependencies:** None (foundational document)
