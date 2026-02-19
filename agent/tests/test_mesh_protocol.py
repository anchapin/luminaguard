#!/usr/bin/env python3
"""
Unit tests for the Private Mesh Protocol.

These tests cover:
- MeshKeyManager: key generation, encryption/decryption
- MeshPeer and MeshMessage dataclasses
- MeshProtocol: lifecycle, peer management, statistics
"""

import asyncio
import pytest
from datetime import datetime, timedelta
import os
import sys

# Add agent directory to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from mesh import (
    MeshKeyManager,
    MeshPeer,
    MeshMessage,
    MeshProtocol,
    MESH_DISCOVERY_PORT,
    MESH_DATA_PORT,
    MESH_MAGIC,
    MESH_BROADCAST_INTERVAL,
    MESH_PEER_TIMEOUT,
)


class TestMeshKeyManager:
    """Tests for the MeshKeyManager class."""

    def test_key_generation(self):
        """Test that key manager generates valid keys."""
        km = MeshKeyManager()

        # Should have private and public key
        assert km.private_key is not None
        assert km.public_key is not None

        # Public key should be 32 bytes (X25519)
        pub_bytes = km.get_public_key_bytes()
        assert len(pub_bytes) == 32

    def test_key_derivation(self):
        """Test shared secret derivation between two key managers."""
        km1 = MeshKeyManager()
        km2 = MeshKeyManager()

        # Derive shared secrets
        secret1 = km1.derive_shared_secret(km2.get_public_key_bytes())
        secret2 = km2.derive_shared_secret(km1.get_public_key_bytes())

        # Both should derive the same key
        assert secret1 == secret2
        assert len(secret1) == 32  # SHA256 output

    def test_encryption_decryption(self):
        """Test message encryption and decryption."""
        km1 = MeshKeyManager()
        km2 = MeshKeyManager()

        plaintext = b"Hello, secure world!"

        # Encrypt with km1 for km2
        ciphertext = km1.encrypt_message(km2.get_public_key_bytes(), plaintext)

        # Decrypt with km2
        decrypted = km2.decrypt_message(km1.get_public_key_bytes(), ciphertext)

        assert decrypted == plaintext

    def test_encryption_produces_different_nonces(self):
        """Test that encryption uses random nonces (different ciphertext each time)."""
        km1 = MeshKeyManager()
        km2 = MeshKeyManager()

        plaintext = b"Same message"

        # Encrypt same message twice
        ciphertext1 = km1.encrypt_message(km2.get_public_key_bytes(), plaintext)
        ciphertext2 = km1.encrypt_message(km2.get_public_key_bytes(), plaintext)

        # Should have different ciphertexts due to random nonce
        assert ciphertext1 != ciphertext2

        # But both should decrypt to same plaintext
        assert km2.decrypt_message(km1.get_public_key_bytes(), ciphertext1) == plaintext
        assert km2.decrypt_message(km1.get_public_key_bytes(), ciphertext2) == plaintext

    def test_deterministic_shared_secret(self):
        """Test that derived shared secret is cached and consistent."""
        km1 = MeshKeyManager()
        km2 = MeshKeyManager()

        pub2 = km2.get_public_key_bytes()

        # Derive twice
        secret1 = km1.derive_shared_secret(pub2)
        secret2 = km1.derive_shared_secret(pub2)

        # Should be the same (cached)
        assert secret1 == secret2


class TestMeshPeer:
    """Tests for the MeshPeer dataclass."""

    def test_mesh_peer_creation(self):
        """Test creating a MeshPeer."""
        peer = MeshPeer(
            mesh_id="test123",
            hostname="test-host",
            ip_address="192.168.1.100",
            port=45679,
            public_key=b"a" * 32,
            agent_role="researcher",
            device_name="test-device",
        )

        assert peer.mesh_id == "test123"
        assert peer.hostname == "test-host"
        assert peer.ip_address == "192.168.1.100"
        assert peer.port == 45679
        assert peer.public_key == b"a" * 32
        assert peer.agent_role == "researcher"
        assert peer.device_name == "test-device"
        assert peer.last_seen is not None

    def test_mesh_peer_default_values(self):
        """Test MeshPeer default values."""
        peer = MeshPeer(
            mesh_id="test",
            hostname="host",
            ip_address="127.0.0.1",
            port=45679,
            public_key=b"b" * 32,
        )

        assert peer.agent_role == "unknown"
        assert peer.device_name == ""
        assert isinstance(peer.last_seen, datetime)


class TestMeshMessage:
    """Tests for the MeshMessage dataclass."""

    def test_mesh_message_creation(self):
        """Test creating a MeshMessage."""
        msg = MeshMessage(
            source_mesh_id="source123",
            source_role="coder",
            target_mesh_id="target456",
            message_type="data",
            payload=b"test payload",
        )

        assert msg.source_mesh_id == "source123"
        assert msg.source_role == "coder"
        assert msg.target_mesh_id == "target456"
        assert msg.message_type == "data"
        assert msg.payload == b"test payload"
        assert isinstance(msg.timestamp, datetime)
        assert len(msg.nonce) == 12

    def test_mesh_message_broadcast_target(self):
        """Test broadcast message (None target)."""
        msg = MeshMessage(
            source_mesh_id="source",
            source_role="researcher",
            target_mesh_id=None,  # Broadcast
            message_type="discovery",
            payload=b"discovery data",
        )

        assert msg.target_mesh_id is None


class TestMeshProtocol:
    """Tests for the MeshProtocol class."""

    @pytest.mark.asyncio
    async def test_protocol_initialization(self):
        """Test protocol initializes with correct defaults."""
        protocol = MeshProtocol(agent_role="researcher", device_name="test-device")

        assert protocol.mesh_id is not None
        assert len(protocol.mesh_id) <= 8
        assert protocol.agent_role == "researcher"
        assert protocol.device_name == "test-device"
        assert protocol.key_manager is not None
        assert protocol.peers == {}
        assert not protocol._running

    @pytest.mark.asyncio
    async def test_protocol_start_stop(self):
        """Test starting and stopping the protocol."""
        protocol = MeshProtocol(agent_role="tester")

        # Start
        await protocol.start()
        assert protocol._running is True

        # Stop
        await protocol.stop()
        assert protocol._running is False

    @pytest.mark.asyncio
    async def test_protocol_statistics(self):
        """Test protocol statistics."""
        protocol = MeshProtocol(agent_role="tester")

        stats = protocol.get_stats()

        assert "messages_sent" in stats
        assert "messages_received" in stats
        assert "peers_discovered" in stats
        assert "peer_count" in stats
        assert "mesh_id" in stats
        assert stats["messages_sent"] == 0
        assert stats["messages_received"] == 0
        assert stats["peers_discovered"] == 0
        assert stats["peer_count"] == 0

    @pytest.mark.asyncio
    async def test_get_peers_empty(self):
        """Test getting peers when none exist."""
        protocol = MeshProtocol(agent_role="tester")

        peers = await protocol.get_peers()
        assert peers == []

    @pytest.mark.asyncio
    async def test_get_peers_by_role(self):
        """Test filtering peers by role."""
        protocol = MeshProtocol(agent_role="tester")

        # Manually add test peers
        peer1 = MeshPeer(
            mesh_id="p1",
            hostname="host1",
            ip_address="10.0.0.1",
            port=45679,
            public_key=b"a" * 32,
            agent_role="researcher",
        )
        peer2 = MeshPeer(
            mesh_id="p2",
            hostname="host2",
            ip_address="10.0.0.2",
            port=45679,
            public_key=b"b" * 32,
            agent_role="coder",
        )

        protocol.peers["p1"] = peer1
        protocol.peers["p2"] = peer2

        # Get all
        all_peers = await protocol.get_peers()
        assert len(all_peers) == 2

        # Filter by role
        researchers = await protocol.get_peers(role="researcher")
        assert len(researchers) == 1
        assert researchers[0].mesh_id == "p1"

        coders = await protocol.get_peers(role="coder")
        assert len(coders) == 1
        assert coders[0].mesh_id == "p2"

    @pytest.mark.asyncio
    async def test_send_to_nonexistent_peer(self):
        """Test sending to a peer that doesn't exist."""
        protocol = MeshProtocol(agent_role="tester")
        await protocol.start()

        try:
            result = await protocol.send_to_peer("nonexistent", "data", b"test")
            assert result is False
        finally:
            await protocol.stop()

    @pytest.mark.asyncio
    async def test_broadcast_to_no_peers(self):
        """Test broadcasting when no peers exist."""
        protocol = MeshProtocol(agent_role="tester")

        count = await protocol.broadcast("data", b"test broadcast")
        assert count == 0

    @pytest.mark.asyncio
    async def test_message_handler_decorator(self):
        """Test registering message handlers."""
        protocol = MeshProtocol(agent_role="tester")

        @protocol.on_message("test_type")
        async def handler(msg, peer):
            return "handled"

        assert "test_type" in protocol._handlers
        assert protocol._handlers["test_type"] is handler


class TestMeshConstants:
    """Tests for mesh protocol constants."""

    def test_discovery_port(self):
        """Test discovery port is correct."""
        assert MESH_DISCOVERY_PORT == 45678

    def test_data_port(self):
        """Test data port is correct."""
        assert MESH_DATA_PORT == 45679

    def test_magic_bytes(self):
        """Test magic bytes are correct."""
        assert MESH_MAGIC == b"LUMINAGUARD_MESH_V1"

    def test_broadcast_interval(self):
        """Test broadcast interval."""
        assert MESH_BROADCAST_INTERVAL == 5

    def test_peer_timeout(self):
        """Test peer timeout."""
        assert MESH_PEER_TIMEOUT == 30


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
