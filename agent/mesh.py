#!/usr/bin/env python3
"""
Private Mesh Protocol for Multi-Agent Collaboration

This module enables multiple LuminaGuard instances to securely communicate
over an encrypted local mesh network without data touching the public internet.

Features:
- Encrypted peer-to-peer communication between agents
- Automatic peer discovery on local network
- Secure data transfer between Researcher and Coder agents
- Support for multi-device setups (Mac Mini to MacBook Pro)

Architecture:
- Uses UDP broadcast for peer discovery
- Uses TCP for reliable encrypted data transfer
- Implements WireGuard-like key exchange for encryption
- Each agent has a unique mesh ID and encryption keys
"""

import asyncio
import json
import logging
import os
import socket
import struct
import threading
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from typing import Any, Callable, Optional
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import x25519
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.backends import default_backend

logger = logging.getLogger(__name__)

# Constants
MESH_DISCOVERY_PORT = 45678
MESH_DATA_PORT = 45679
MESH_BROADCAST_INTERVAL = 5  # seconds
MESH_PEER_TIMEOUT = 30  # seconds
MESH_MAGIC = b"LUMINAGUARD_MESH_V1"
MAX_MESSAGE_SIZE = 16 * 1024 * 1024  # 16MB


@dataclass
class MeshPeer:
    """Represents a peer in the mesh network."""
    mesh_id: str
    hostname: str
    ip_address: str
    port: int
    public_key: bytes
    last_seen: datetime = field(default_factory=datetime.now)
    agent_role: str = "unknown"  # researcher, coder, etc.
    device_name: str = ""


@dataclass 
class MeshMessage:
    """Message format for mesh communication."""
    source_mesh_id: str
    source_role: str
    target_mesh_id: Optional[str]  # None for broadcast
    message_type: str  # discovery, data, ack, etc.
    payload: bytes
    timestamp: datetime = field(default_factory=datetime.now)
    nonce: bytes = field(default_factory=lambda: os.urandom(12))


class MeshKeyManager:
    """Manages encryption keys for mesh communication."""
    
    def __init__(self):
        # Generate our own keypair
        self.private_key = x25519.X25519PrivateKey.generate()
        self.public_key = self.private_key.public_key()
        
        # Derive shared secret from our keypair
        self._shared_secrets: dict[bytes, bytes] = {}
        
    def get_public_key_bytes(self) -> bytes:
        """Get our public key as bytes."""
        return self.public_key.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw
        )
    
    def derive_shared_secret(self, peer_public_key: bytes) -> bytes:
        """Derive shared secret with a peer's public key."""
        if peer_public_key in self._shared_secrets:
            return self._shared_secrets[peer_public_key]
        
        # Parse peer's public key
        peer_key = x25519.X25519PublicKey.from_public_bytes(peer_public_key)
        
        # Perform key exchange
        shared = self.private_key.exchange(peer_key)
        
        # Derive symmetric key using HKDF
        hkdf = HKDF(
            algorithm=hashes.SHA256(),
            length=32,
            salt=b"luminaguard-mesh-v1",
            info=b"mesh-channel-key",
            backend=default_backend()
        )
        key = hkdf.derive(shared)
        
        self._shared_secrets[peer_public_key] = key
        return key
    
    def encrypt_message(self, peer_public_key: bytes, plaintext: bytes) -> bytes:
        """Encrypt a message for a specific peer."""
        key = self.derive_shared_secret(peer_public_key)
        nonce = os.urandom(12)
        cipher = ChaCha20Poly1305(key)
        ciphertext = cipher.encrypt(nonce, plaintext, None)
        
        # Prepend nonce to ciphertext
        return nonce + ciphertext
    
    def decrypt_message(self, peer_public_key: bytes, ciphertext: bytes) -> bytes:
        """Decrypt a message from a specific peer."""
        key = self.derive_shared_secret(peer_public_key)
        nonce = ciphertext[:12]
        actual_ciphertext = ciphertext[12:]
        
        cipher = ChaCha20Poly1305(key)
        return cipher.decrypt(nonce, actual_ciphertext, None)


class MeshProtocol:
    """
    Private Mesh Protocol implementation for secure multi-agent collaboration.
    
    This protocol enables:
    - Automatic peer discovery via UDP broadcast
    - Encrypted TCP communication between agents
    - Support for Researcher and Coder agent collaboration
    - Multi-device setups (e.g., Mac Mini to MacBook Pro)
    """
    
    def __init__(
        self,
        mesh_id: Optional[str] = None,
        agent_role: str = "agent",
        device_name: str = "",
    ):
        self.mesh_id = mesh_id or str(uuid.uuid4())[:8]
        self.agent_role = agent_role
        self.device_name = device_name or socket.gethostname()
        
        # Key management
        self.key_manager = MeshKeyManager()
        
        # Peer management
        self.peers: dict[str, MeshPeer] = {}
        self._peer_lock = asyncio.Lock()
        
        # Communication
        self._running = False
        self._discovery_socket: Optional[socket.socket] = None
        self._data_socket: Optional[socket.socket] = None
        self._broadcast_thread: Optional[threading.Thread] = None
        
        # Message handlers
        self._handlers: dict[str, Callable] = {}
        
        # Statistics
        self.stats = {
            "messages_sent": 0,
            "messages_received": 0,
            "peers_discovered": 0,
        }
        
    def on_message(self, message_type: str) -> Callable:
        """Decorator to register message handlers."""
        def decorator(func: Callable) -> Callable:
            self._handlers[message_type] = func
            return func
        return decorator
    
    async def start(self):
        """Start the mesh protocol."""
        logger.info(f"Starting mesh protocol with ID: {self.mesh_id}")
        self._running = True
        
        # Start discovery listener
        asyncio.create_task(self._run_discovery_listener())
        
        # Start data listener
        asyncio.create_task(self._run_data_listener())
        
        # Start broadcast thread
        self._broadcast_thread = threading.Thread(target=self._broadcast_loop, daemon=True)
        self._broadcast_thread.start()
        
        logger.info(f"Mesh protocol started: {self.mesh_id}")
    
    async def stop(self):
        """Stop the mesh protocol."""
        logger.info(f"Stopping mesh protocol: {self.mesh_id}")
        self._running = False
        
        # Close sockets
        if self._discovery_socket:
            self._discovery_socket.close()
        if self._data_socket:
            self._data_socket.close()
        
        # Wait for broadcast thread
        if self._broadcast_thread:
            self._broadcast_thread.join(timeout=2)
        
        logger.info("Mesh protocol stopped")
    
    def _broadcast_loop(self):
        """Broadcast our presence periodically."""
        while self._running:
            try:
                self._send_discovery_broadcast()
            except Exception as e:
                logger.debug(f"Broadcast error: {e}")
            
            threading.Event().wait(MESH_BROADCAST_INTERVAL)
    
    def _send_discovery_broadcast(self):
        """Send a discovery broadcast to find peers."""
        # Create discovery message
        discovery_data = {
            "magic": MESH_MAGIC.decode(),
            "mesh_id": self.mesh_id,
            "hostname": self.device_name,
            "role": self.agent_role,
            "public_key": self.key_manager.get_public_key_bytes().hex(),
            "port": MESH_DATA_PORT,
        }
        
        message = json.dumps(discovery_data).encode()
        
        # Create UDP socket for broadcast
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        
        try:
            sock.sendto(message, ("<broadcast>", MESH_DISCOVERY_PORT))
        finally:
            sock.close()
    
    async def _run_discovery_listener(self):
        """Listen for discovery broadcasts from other peers."""
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.settimeout(1.0)
        
        try:
            sock.bind(("", MESH_DISCOVERY_PORT))
        except OSError as e:
            logger.warning(f"Discovery port bind failed: {e}")
            return
        
        self._discovery_socket = sock
        
        while self._running:
            try:
                data, addr = sock.recvfrom(4096)
                await self._handle_discovery_message(data, addr[0])
            except socket.timeout:
                continue
            except Exception as e:
                logger.debug(f"Discovery listener error: {e}")
    
    async def _handle_discovery_message(self, data: bytes, source_ip: str):
        """Handle incoming discovery message."""
        try:
            msg = json.loads(data.decode())
            
            # Verify magic
            if msg.get("magic") != MESH_MAGIC.decode():
                return
            
            # Skip our own messages
            if msg["mesh_id"] == self.mesh_id:
                return
            
            # Parse peer info
            peer = MeshPeer(
                mesh_id=msg["mesh_id"],
                hostname=msg.get("hostname", ""),
                ip_address=source_ip,
                port=msg.get("port", MESH_DATA_PORT),
                public_key=bytes.fromhex(msg["public_key"]),
                agent_role=msg.get("role", "unknown"),
            )
            
            # Update or add peer
            async with self._peer_lock:
                is_new = peer.mesh_id not in self.peers
                self.peers[peer.mesh_id] = peer
                
                if is_new:
                    self.stats["peers_discovered"] += 1
                    logger.info(f"Discovered peer: {peer.mesh_id} ({peer.hostname}) - {peer.agent_role}")
                    
                    # Notify handlers
                    if "peer_discovered" in self._handlers:
                        await self._handlers["peer_discovered"](peer)
                        
        except Exception as e:
            logger.debug(f"Discovery parse error: {e}")
    
    async def _run_data_listener(self):
        """Listen for incoming encrypted messages."""
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.settimeout(1.0)
        
        try:
            sock.bind(("", MESH_DATA_PORT))
            sock.listen(5)
        except OSError as e:
            logger.warning(f"Data port bind failed: {e}")
            return
        
        self._data_socket = sock
        
        while self._running:
            try:
                client_sock, addr = sock.accept()
                asyncio.create_task(self._handle_data_connection(client_sock, addr))
            except socket.timeout:
                continue
            except Exception as e:
                logger.debug(f"Data listener error: {e}")
    
    async def _handle_data_connection(self, client_sock: socket.socket, addr):
        """Handle incoming data connection."""
        try:
            client_sock.settimeout(10.0)
            
            # Receive message length
            length_data = client_sock.recv(4)
            if len(length_data) < 4:
                return
            
            length = struct.unpack("!I", length_data)[0]
            
            if length > MAX_MESSAGE_SIZE:
                logger.warning(f"Message too large: {length}")
                return
            
            # Receive message
            data = b""
            while len(data) < length:
                chunk = client_sock.recv(length - len(data))
                if not chunk:
                    break
                data += chunk
            
            # Parse and decrypt message
            await self._handle_incoming_message(data, addr[0])
            
        except Exception as e:
            logger.debug(f"Data connection error: {e}")
        finally:
            client_sock.close()
    
    async def _handle_incoming_message(self, data: bytes, source_ip: str):
        """Handle decrypted incoming message."""
        try:
            # Parse message envelope
            source_mesh_id = data[:8].decode()
            
            # Find sender in peers
            async with self._peer_lock:
                peer = self.peers.get(source_mesh_id)
            
            if not peer:
                logger.debug(f"Message from unknown peer: {source_mesh_id}")
                return
            
            # Decrypt message
            try:
                plaintext = self.key_manager.decrypt_message(peer.public_key, data[8:])
            except Exception as e:
                logger.warning(f"Decryption failed: {e}")
                return
            
            # Parse message
            msg = MeshMessage(
                source_mesh_id=source_mesh_id,
                source_role=peer.agent_role,
                target_mesh_id=None,
                message_type="data",
                payload=plaintext,
            )
            
            self.stats["messages_received"] += 1
            
            # Call handlers
            if msg.message_type in self._handlers:
                await self._handlers[msg.message_type](msg, peer)
                
        except Exception as e:
            logger.debug(f"Message handling error: {e}")
    
    async def send_to_peer(
        self,
        peer_id: str,
        message_type: str,
        payload: bytes,
    ) -> bool:
        """Send an encrypted message to a specific peer."""
        async with self._peer_lock:
            peer = self.peers.get(peer_id)
        
        if not peer:
            logger.warning(f"Peer not found: {peer_id}")
            return False
        
        try:
            # Encrypt payload for peer
            encrypted = self.key_manager.encrypt_message(peer.public_key, payload)
            
            # Create message envelope
            envelope = peer.mesh_id.encode() + encrypted
            
            # Send via TCP
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(5.0)
            
            try:
                sock.connect((peer.ip_address, peer.port))
                
                # Send length prefix
                length = len(envelope)
                sock.sendall(struct.pack("!I", length))
                
                # Send message
                sock.sendall(envelope)
                
                self.stats["messages_sent"] += 1
                logger.debug(f"Sent message to {peer_id}: {len(payload)} bytes")
                return True
                
            finally:
                sock.close()
                
        except Exception as e:
            logger.warning(f"Failed to send to {peer_id}: {e}")
            return False
    
    async def broadcast(
        self,
        message_type: str,
        payload: bytes,
    ) -> int:
        """Broadcast a message to all known peers."""
        count = 0
        peer_ids = list(self.peers.keys())
        
        for peer_id in peer_ids:
            if await self.send_to_peer(peer_id, message_type, payload):
                count += 1
        
        return count
    
    async def get_peers(self, role: Optional[str] = None) -> list[MeshPeer]:
        """Get list of peers, optionally filtered by role."""
        async with self._peer_lock:
            peers = list(self.peers.values())
        
        # Filter by role if specified
        if role:
            peers = [p for p in peers if p.agent_role == role]
        
        # Remove stale peers
        now = datetime.now()
        peers = [p for p in peers if now - p.last_seen < timedelta(seconds=MESH_PEER_TIMEOUT)]
        
        return peers
    
    def get_stats(self) -> dict:
        """Get mesh statistics."""
        return {
            **self.stats,
            "peer_count": len(self.peers),
            "mesh_id": self.mesh_id,
        }


# Example usage
async def main():
    """Example: Running mesh protocol."""
    # Create mesh node
    mesh = MeshProtocol(
        agent_role="researcher",
        device_name="mac-mini"
    )
    
    # Register message handler
    @mesh.on_message("data")
    async def handle_data(msg: MeshMessage, peer: MeshPeer):
        print(f"Received from {peer.mesh_id} ({peer.agent_role}): {msg.payload.decode()}")
    
    @mesh.on_message("peer_discovered")
    async def handle_discovery(peer: MeshPeer):
        print(f"New peer: {peer.mesh_id} - {peer.agent_role}")
    
    # Start
    await mesh.start()
    
    print(f"Mesh started with ID: {mesh.mesh_id}")
    print("Waiting for peers...")
    
    # Wait for peers
    await asyncio.sleep(10)
    
    # Get discovered peers
    peers = await mesh.get_peers()
    print(f"Found {len(peers)} peers:")
    for peer in peers:
        print(f"  - {peer.mesh_id}: {peer.agent_role}@{peer.hostname}")
    
    # Send message to all coders
    coder_peers = await mesh.get_peers(role="coder")
    if coder_peers:
        for peer in coder_peers:
            await mesh.send_to_peer(
                peer.mesh_id,
                "data",
                b"Hello from researcher!"
            )
    
    # Keep running
    try:
        while True:
            await asyncio.sleep(30)
            print(f"Stats: {mesh.get_stats()}")
    except KeyboardInterrupt:
        pass
    finally:
        await mesh.stop()


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    asyncio.run(main())
