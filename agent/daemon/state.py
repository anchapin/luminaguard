"""
Persistent State Management for LuminaGuard.

This module provides state persistence for daemon mode:
- Save/restore bot state across restarts
- Conversation history persistence
- Task queue for scheduled jobs
- State snapshots for recovery
- State encryption for sensitive data

Part of: luminaguard-0va.7 - Persistent State Management
"""

import asyncio
import json
import logging
import os
import time
import hashlib
import base64
import copy
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, List, Callable, Awaitable
from pathlib import Path
from datetime import datetime, timezone
from enum import Enum
from collections import deque
import threading

try:
    from cryptography.fernet import Fernet
    CRYPTO_AVAILABLE = True
except ImportError:
    CRYPTO_AVAILABLE = False

logger = logging.getLogger(__name__)


class StateVersion(Enum):
    """State version enum."""
    V1 = "v1"


@dataclass
class ConversationMessage:
    """A conversation message."""
    role: str
    content: str
    timestamp: float = field(default_factory=time.time)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "role": self.role,
            "content": self.content,
            "timestamp": self.timestamp,
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ConversationMessage":
        """Create from dictionary."""
        return cls(
            role=data["role"],
            content=data["content"],
            timestamp=data.get("timestamp", time.time()),
            metadata=data.get("metadata", {}),
        )


@dataclass
class ScheduledTask:
    """A scheduled task in the queue."""
    id: str
    name: str
    schedule: str  # Cron expression
    handler: str
    args: List[Any] = field(default_factory=list)
    kwargs: Dict[str, Any] = field(default_factory=dict)
    enabled: bool = True
    next_run: Optional[float] = None
    last_run: Optional[float] = None
    run_count: int = 0

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": self.id,
            "name": self.name,
            "schedule": self.schedule,
            "handler": self.handler,
            "args": self.args,
            "kwargs": self.kwargs,
            "enabled": self.enabled,
            "next_run": self.next_run,
            "last_run": self.last_run,
            "run_count": self.run_count,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ScheduledTask":
        """Create from dictionary."""
        return cls(
            id=data["id"],
            name=data["name"],
            schedule=data["schedule"],
            handler=data["handler"],
            args=data.get("args", []),
            kwargs=data.get("kwargs", {}),
            enabled=data.get("enabled", True),
            next_run=data.get("next_run"),
            last_run=data.get("last_run"),
            run_count=data.get("run_count", 0),
        )


@dataclass
class StateSnapshot:
    """A state snapshot for recovery."""
    id: str
    timestamp: float
    version: str
    data: Dict[str, Any]
    checksum: str

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": self.id,
            "timestamp": self.timestamp,
            "version": self.version,
            "data": self.data,
            "checksum": self.checksum,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "StateSnapshot":
        """Create from dictionary."""
        return cls(
            id=data["id"],
            timestamp=data["timestamp"],
            version=data["version"],
            data=data["data"],
            checksum=data["checksum"],
        )


@dataclass
class BotState:
    """
    Bot state that gets persisted.
    
    This is the main state object that gets saved and restored
    across daemon restarts.
    """
    # State version
    version: str = StateVersion.V1.value
    
    # Bot metadata
    created_at: float = field(default_factory=time.time)
    updated_at: float = field(default_factory=time.time)
    restart_count: int = 0
    
    # Session state
    session_id: Optional[str] = None
    session_data: Dict[str, Any] = field(default_factory=dict)
    
    # Conversation history
    conversations: Dict[str, List[ConversationMessage]] = field(default_factory=dict)
    
    # Active conversations (by channel)
    active_conversations: Dict[str, str] = field(default_factory=dict)
    
    # Scheduled tasks
    scheduled_tasks: List[ScheduledTask] = field(default_factory=list)
    
    # Custom state data
    custom_data: Dict[str, Any] = field(default_factory=dict)
    
    # Statistics
    stats: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "version": self.version,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "restart_count": self.restart_count,
            "session_id": self.session_id,
            "session_data": self.session_data,
            "conversations": {
                k: [msg.to_dict() for msg in v]
                for k, v in self.conversations.items()
            },
            "active_conversations": self.active_conversations,
            "scheduled_tasks": [task.to_dict() for task in self.scheduled_tasks],
            "custom_data": self.custom_data,
            "stats": self.stats,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "BotState":
        """Create from dictionary."""
        state = cls(
            version=data.get("version", StateVersion.V1.value),
            created_at=data.get("created_at", time.time()),
            updated_at=data.get("updated_at", time.time()),
            restart_count=data.get("restart_count", 0),
            session_id=data.get("session_id"),
            session_data=data.get("session_data", {}),
            active_conversations=data.get("active_conversations", {}),
            custom_data=data.get("custom_data", {}),
            stats=data.get("stats", {}),
        )
        
        # Load conversations
        conv_data = data.get("conversations", {})
        for channel_id, messages in conv_data.items():
            state.conversations[channel_id] = [
                ConversationMessage.from_dict(msg) for msg in messages
            ]
        
        # Load scheduled tasks
        task_data = data.get("scheduled_tasks", [])
        state.scheduled_tasks = [
            ScheduledTask.from_dict(task) for task in task_data
        ]
        
        return state


class StateEncryption:
    """State encryption using Fernet (symmetric encryption)."""
    
    def __init__(self, key: Optional[bytes] = None):
        """
        Initialize encryption.
        
        Args:
            key: Encryption key (generated if not provided)
        """
        if not CRYPTO_AVAILABLE:
            raise RuntimeError(
                "cryptography package not available. "
                "Install with: pip install cryptography"
            )
        
        if key is None:
            key = Fernet.generate_key()
        
        self._fernet = Fernet(key)
        self._key = key
    
    @property
    def key(self) -> bytes:
        """Get encryption key."""
        return self._key
    
    @classmethod
    def from_password(cls, password: str, salt: Optional[bytes] = None) -> "StateEncryption":
        """
        Create encryption from password.
        
        Args:
            password: Password to derive key from
            salt: Salt (generated if not provided)
            
        Returns:
            StateEncryption instance
        """
        if not CRYPTO_AVAILABLE:
            raise RuntimeError("cryptography package not available")
        
        if salt is None:
            import secrets
            salt = secrets.token_bytes(16)
        
        # Derive key using PBKDF2
        from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
        from cryptography.hazmat.backends import default_backend
        
        kdf = PBKDF2HMAC(
            algorithm=hashes.SHA256(),
            length=32,
            salt=salt,
            iterations=100000,
            backend=default_backend(),
        )
        key = base64.urlsafe_b64encode(kdf.derive(password.encode()))
        
        instance = cls(key)
        instance._salt = salt
        return instance
    
    def encrypt(self, data: str) -> str:
        """Encrypt data."""
        return self._fernet.encrypt(data.encode()).decode()
    
    def decrypt(self, data: str) -> str:
        """Decrypt data."""
        return self._fernet.decrypt(data.encode()).decode()


class StateManager:
    """
    Persistent state manager for daemon mode.
    
    Provides:
    - Save/restore bot state across restarts
    - Conversation history persistence
    - Task queue for scheduled jobs
    - State snapshots for recovery
    - State encryption for sensitive data
    """

    def __init__(
        self,
        state_file: str = "~/.luminaguard/state.json",
        encryption_key: Optional[bytes] = None,
        max_conversations: int = 100,
        max_messages_per_conversation: int = 1000,
        max_snapshots: int = 10,
    ):
        """
        Initialize state manager.
        
        Args:
            state_file: Path to state file
            encryption_key: Optional encryption key
            max_conversations: Maximum conversations to keep
            max_messages_per_conversation: Max messages per conversation
            max_snapshots: Maximum snapshots to keep
        """
        self._state_file = os.path.expanduser(state_file)
        self._encryption: Optional[StateEncryption] = None
        if encryption_key:
            self._encryption = StateEncryption(encryption_key)
        
        self._max_conversations = max_conversations
        self._max_messages_per_conversation = max_messages_per_conversation
        self._max_snapshots = max_snapshots
        
        self._state: Optional[BotState] = None
        self._snapshots: List[StateSnapshot] = []
        self._lock = threading.RLock()
        self._dirty = False
        self._save_interval = 60  # Save every 60 seconds
        self._last_save = time.time()
        self._save_task: Optional[asyncio.Task] = None
        
        # Callbacks
        self._on_load: Optional[Callable[[BotState], None]] = None
        self._on_save: Optional[Callable[[BotState], None]] = None
        self._on_state_change: Optional[Callable[[str, Any], None]] = None
    
    @property
    def state(self) -> BotState:
        """Get current state."""
        with self._lock:
            if self._state is None:
                self._state = self._load_state()
            return self._state
    
    def set_callbacks(
        self,
        on_load: Optional[Callable[[BotState], None]] = None,
        on_save: Optional[Callable[[BotState], None]] = None,
        on_state_change: Optional[Callable[[str, Any], None]] = None,
    ) -> None:
        """Set callbacks for state events."""
        self._on_load = on_load
        self._on_save = on_save
        self._on_state_change = on_state_change
    
    def _load_state(self) -> BotState:
        """Load state from disk."""
        try:
            path = Path(self._state_file)
            if not path.exists():
                logger.debug("No state file found, creating new state")
                return BotState()
            
            with open(path, "r") as f:
                data = json.load(f)
            
            # Decrypt if needed
            if isinstance(data, str) and self._encryption:
                data = json.loads(self._encryption.decrypt(data))
            
            state = BotState.from_dict(data)
            
            # Increment restart count
            state.restart_count += 1
            
            logger.info(f"Loaded state (restart #{state.restart_count})")
            
            if self._on_load:
                self._on_load(state)
            
            return state
            
        except Exception as e:
            logger.error(f"Failed to load state: {e}")
            return BotState()
    
    def _save_state(self) -> None:
        """Save state to disk."""
        try:
            path = Path(self._state_file)
            path.parent.mkdir(parents=True, exist_ok=True)
            
            # Update timestamp
            self._state.updated_at = time.time()
            
            # Convert to dict
            data = self._state.to_dict()
            
            # Encrypt if needed
            if self._encryption:
                data = self._encryption.encrypt(json.dumps(data))
            
            with open(path, "w") as f:
                json.dump(data, f, indent=2)
            
            self._dirty = False
            self._last_save = time.time()
            
            logger.debug(f"Saved state to {self._state_file}")
            
            if self._on_save:
                self._on_save(self._state)
                
        except Exception as e:
            logger.error(f"Failed to save state: {e}")
    
    def _create_snapshot(self) -> StateSnapshot:
        """Create a state snapshot."""
        state_data = copy.deepcopy(self._state.to_dict())
        
        # Calculate checksum
        data_str = json.dumps(state_data, sort_keys=True)
        checksum = hashlib.sha256(data_str.encode()).hexdigest()
        
        snapshot = StateSnapshot(
            id=f"snap_{int(time.time())}",
            timestamp=time.time(),
            version=StateVersion.V1.value,
            data=state_data,
            checksum=checksum,
        )
        
        # Add to snapshots list
        self._snapshots.append(snapshot)
        
        # Trim old snapshots
        if len(self._snapshots) > self._max_snapshots:
            self._snapshots = self._snapshots[-self._max_snapshots:]
        
        return snapshot
    
    def _verify_snapshot(self, snapshot: StateSnapshot) -> bool:
        """Verify snapshot integrity."""
        data_str = json.dumps(snapshot.data, sort_keys=True)
        checksum = hashlib.sha256(data_str.encode()).hexdigest()
        return checksum == snapshot.checksum
    
    async def start(self) -> None:
        """Start state manager."""
        # Load state
        with self._lock:
            self._state = self._load_state()
        
        # Start auto-save task
        self._save_task = asyncio.create_task(self._auto_save_loop())
        
        logger.info("State manager started")
    
    async def stop(self) -> None:
        """Stop state manager."""
        # Cancel auto-save task
        if self._save_task:
            self._save_task.cancel()
            try:
                await self._save_task
            except asyncio.CancelledError:
                pass
        
        # Save state one last time
        with self._lock:
            if self._dirty:
                self._save_state()
        
        logger.info("State manager stopped")
    
    async def _auto_save_loop(self) -> None:
        """Auto-save loop."""
        while True:
            try:
                await asyncio.sleep(self._save_interval)
                
                with self._lock:
                    if self._dirty:
                        self._save_state()
                        
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in auto-save loop: {e}")
    
    def save(self) -> None:
        """Save state immediately."""
        with self._lock:
            self._dirty = True
            self._save_state()
    
    def create_snapshot(self) -> StateSnapshot:
        """Create a snapshot for recovery."""
        with self._lock:
            return self._create_snapshot()
    
    def restore_snapshot(self, snapshot_id: str) -> bool:
        """
        Restore state from snapshot.
        
        Args:
            snapshot_id: ID of snapshot to restore
            
        Returns:
            True if restored successfully
        """
        with self._lock:
            # Find snapshot
            snapshot = None
            for s in self._snapshots:
                if s.id == snapshot_id:
                    snapshot = s
                    break
            
            if not snapshot:
                logger.error(f"Snapshot not found: {snapshot_id}")
                return False
            
            # Verify integrity
            if not self._verify_snapshot(snapshot):
                logger.error(f"Snapshot verification failed: {snapshot_id}")
                return False
            
            # Restore
            self._state = BotState.from_dict(snapshot.data)
            self._dirty = True
            self._save_state()
            
            logger.info(f"Restored state from snapshot: {snapshot_id}")
            return True
    
    def get_snapshots(self) -> List[Dict[str, Any]]:
        """Get list of snapshots."""
        return [s.to_dict() for s in self._snapshots]
    
    # State manipulation methods
    
    def update_session(self, session_id: str, data: Dict[str, Any]) -> None:
        """Update session data."""
        with self._lock:
            self.state.session_id = session_id
            self.state.session_data.update(data)
            self._dirty = True
    
    def add_message(
        self,
        channel_id: str,
        role: str,
        content: str,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Add a message to conversation history."""
        with self._lock:
            if channel_id not in self.state.conversations:
                self.state.conversations[channel_id] = deque(
                    maxlen=self._max_messages_per_conversation
                )
            
            message = ConversationMessage(
                role=role,
                content=content,
                metadata=metadata or {}
            )
            self.state.conversations[channel_id].append(message)
            self._dirty = True
    
    def get_conversation(
        self,
        channel_id: str,
        limit: Optional[int] = None,
    ) -> List[ConversationMessage]:
        """Get conversation history."""
        with self._lock:
            conv = self.state.conversations.get(channel_id, [])
            if limit:
                return list(conv)[-limit:]
            return list(conv)
    
    def clear_conversation(self, channel_id: str) -> None:
        """Clear conversation history."""
        with self._lock:
            if channel_id in self.state.conversations:
                self.state.conversations[channel_id].clear()
                self._dirty = True
    
    def set_active_conversation(self, channel_id: str, conversation_id: str) -> None:
        """Set active conversation for a channel."""
        with self._lock:
            self.state.active_conversations[channel_id] = conversation_id
            self._dirty = True
    
    def get_active_conversation(self, channel_id: str) -> Optional[str]:
        """Get active conversation for a channel."""
        return self.state.active_conversations.get(channel_id)
    
    def add_scheduled_task(self, task: ScheduledTask) -> None:
        """Add a scheduled task."""
        with self._lock:
            self.state.scheduled_tasks.append(task)
            self._dirty = True
    
    def remove_scheduled_task(self, task_id: str) -> bool:
        """Remove a scheduled task."""
        with self._lock:
            for i, task in enumerate(self.state.scheduled_tasks):
                if task.id == task_id:
                    self.state.scheduled_tasks.pop(i)
                    self._dirty = True
                    return True
            return False
    
    def get_scheduled_tasks(self) -> List[ScheduledTask]:
        """Get all scheduled tasks."""
        return list(self.state.scheduled_tasks)
    
    def set_custom_data(self, key: str, value: Any) -> None:
        """Set custom state data."""
        with self._lock:
            self.state.custom_data[key] = value
            self._dirty = True
    
    def get_custom_data(self, key: str, default: Any = None) -> Any:
        """Get custom state data."""
        return self.state.custom_data.get(key, default)
    
    def increment_stat(self, key: str, value: int = 1) -> None:
        """Increment a statistic."""
        with self._lock:
            current = self.state.stats.get(key, 0)
            self.state.stats[key] = current + value
            self._dirty = True
    
    def get_stats(self) -> Dict[str, Any]:
        """Get all statistics."""
        return dict(self.state.stats)
    
    def get_state_summary(self) -> Dict[str, Any]:
        """Get a summary of the current state."""
        with self._lock:
            return {
                "version": self.state.version,
                "created_at": self.state.created_at,
                "updated_at": self.state.updated_at,
                "restart_count": self.state.restart_count,
                "session_id": self.state.session_id,
                "conversation_count": len(self.state.conversations),
                "task_count": len(self.state.scheduled_tasks),
                "stats": self.state.stats,
            }


async def create_state_manager(
    state_file: str = "~/.luminaguard/state.json",
    encryption_key: Optional[bytes] = None,
    max_conversations: int = 100,
    max_messages_per_conversation: int = 1000,
) -> StateManager:
    """
    Create and start a state manager.
    
    Args:
        state_file: Path to state file
        encryption_key: Optional encryption key
        max_conversations: Maximum conversations to keep
        max_messages_per_conversation: Max messages per conversation
        
    Returns:
        Started StateManager instance
    """
    manager = StateManager(
        state_file=state_file,
        encryption_key=encryption_key,
        max_conversations=max_conversations,
        max_messages_per_conversation=max_messages_per_conversation,
    )
    await manager.start()
    return manager
