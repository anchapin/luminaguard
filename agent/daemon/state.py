"""
Persistent State Management Module.

This module provides state persistence for daemon mode:
- Save/restore bot state across restarts
- Conversation history persistence
- Task queue for scheduled jobs
- State snapshots for recovery
- State encryption for sensitive data

Part of: luminaguard-0va - Daemon Mode: 24/7 Bot Service Architecture
Issue: #447 - Persistent State Management
"""

from __future__ import annotations

import os
import json
import time
import logging
import threading
import queue
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional, Dict, Any, List, Callable
from enum import Enum
from datetime import datetime
from collections import deque
import copy
import hashlib
import base64

try:
    from cryptography.fernet import Fernet
    CRYPTO_AVAILABLE = True
except ImportError:
    CRYPTO_AVAILABLE = False

logger = logging.getLogger(__name__)


class StateType(Enum):
    """Types of state that can be stored"""
    CONVERSATION = "conversation"
    TASK = "task"
    CONFIG = "config"
    SNAPSHOT = "snapshot"
    CUSTOM = "custom"


@dataclass
class ConversationMessage:
    """A single conversation message"""
    role: str
    content: str
    timestamp: float = field(default_factory=time.time)
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass
class ConversationHistory:
    """Conversation history for a session"""
    session_id: str
    messages: List[ConversationMessage] = field(default_factory=list)
    created_at: float = field(default_factory=time.time)
    last_updated: float = field(default_factory=time.time)
    
    def add_message(self, role: str, content: str, metadata: Optional[Dict] = None) -> None:
        """Add a message to the conversation"""
        msg = ConversationMessage(
            role=role,
            content=content,
            metadata=metadata or {}
        )
        self.messages.append(msg)
        self.last_updated = time.time()
    
    def get_messages(self, limit: Optional[int] = None) -> List[ConversationMessage]:
        """Get messages, optionally limited to recent ones"""
        if limit:
            return self.messages[-limit:]
        return self.messages
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return {
            "session_id": self.session_id,
            "messages": [
                {
                    "role": m.role,
                    "content": m.content,
                    "timestamp": m.timestamp,
                    "metadata": m.metadata,
                }
                for m in self.messages
            ],
            "created_at": self.created_at,
            "last_updated": self.last_updated,
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> ConversationHistory:
        """Create from dictionary"""
        messages = [
            ConversationMessage(
                role=m["role"],
                content=m["content"],
                timestamp=m.get("timestamp", time.time()),
                metadata=m.get("metadata", {}),
            )
            for m in data.get("messages", [])
        ]
        return cls(
            session_id=data["session_id"],
            messages=messages,
            created_at=data.get("created_at", time.time()),
            last_updated=data.get("last_updated", time.time()),
        )


@dataclass
class TaskState:
    """State of a scheduled task"""
    task_id: str
    name: str
    status: str  # pending, running, completed, failed
    created_at: float = field(default_factory=time.time)
    started_at: Optional[float] = None
    completed_at: Optional[float] = None
    result: Optional[str] = None
    error: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> TaskState:
        """Create from dictionary"""
        return cls(**data)


@dataclass
class StateSnapshot:
    """A snapshot of the entire daemon state"""
    snapshot_id: str
    timestamp: float = field(default_factory=time.time)
    version: str = "1.0"
    conversations: Dict[str, Dict[str, Any]] = field(default_factory=dict)
    tasks: Dict[str, Dict[str, Any]] = field(default_factory=dict)
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> StateSnapshot:
        """Create from dictionary"""
        return cls(**data)


class StateEncryption:
    """Handles state encryption/decryption"""
    
    def __init__(self, key: Optional[str] = None):
        self._key = None
        self._fernet = None
        
        if CRYPTO_AVAILABLE:
            if key:
                # Use provided key (must be 32 bytes base64 encoded)
                try:
                    self._key = key.encode() if isinstance(key, str) else key
                    self._fernet = Fernet(self._key)
                except Exception as e:
                    logger.warning(f"Invalid encryption key: {e}")
            else:
                # Generate new key
                self._key = Fernet.generate_key()
                self._fernet = Fernet(self._key)
                logger.info("Generated new encryption key")
    
    @property
    def key(self) -> Optional[str]:
        """Get the encryption key (base64 encoded)"""
        if self._key:
            return base64.b64encode(self._key).decode()
        return None
    
    @property
    def is_enabled(self) -> bool:
        """Check if encryption is enabled"""
        return self._fernet is not None
    
    def encrypt(self, data: str) -> str:
        """Encrypt string data"""
        if not self._fernet:
            return data
        return self._fernet.encrypt(data.encode()).decode()
    
    def decrypt(self, data: str) -> str:
        """Decrypt string data"""
        if not self._fernet:
            return data
        return self._fernet.decrypt(data.encode()).decode()
    
    def encrypt_dict(self, data: Dict[str, Any]) -> Dict[str, Any]:
        """Encrypt dictionary values"""
        if not self._fernet:
            return data
        
        encrypted = {}
        for key, value in data.items():
            if isinstance(value, str):
                encrypted[key] = self.encrypt(value)
            elif isinstance(value, dict):
                encrypted[key] = self.encrypt_dict(value)
            else:
                encrypted[key] = value
        return encrypted
    
    def decrypt_dict(self, data: Dict[str, Any]) -> Dict[str, Any]:
        """Decrypt dictionary values"""
        if not self._fernet:
            return data
        
        decrypted = {}
        for key, value in data.items():
            if isinstance(value, str) and len(value) > 0:
                try:
                    decrypted[key] = self.decrypt(value)
                except Exception:
                    decrypted[key] = value
            elif isinstance(value, dict):
                decrypted[key] = self.decrypt_dict(value)
            else:
                decrypted[key] = value
        return decrypted


class StateStorage:
    """Handles reading and writing state to disk"""
    
    def __init__(self, state_dir: str, encryption: Optional[StateEncryption] = None):
        self.state_dir = Path(state_dir)
        self.state_dir.mkdir(parents=True, exist_ok=True)
        self.encryption = encryption or StateEncryption()
        
        # Create subdirectories
        self.conversations_dir = self.state_dir / "conversations"
        self.tasks_dir = self.state_dir / "tasks"
        self.snapshots_dir = self.state_dir / "snapshots"
        
        for d in [self.conversations_dir, self.tasks_dir, self.snapshots_dir]:
            d.mkdir(parents=True, exist_ok=True)
    
    def _get_conversation_path(self, session_id: str) -> Path:
        """Get path for conversation file"""
        return self.conversations_dir / f"{session_id}.json"
    
    def _get_task_path(self, task_id: str) -> Path:
        """Get path for task file"""
        return self.tasks_dir / f"{task_id}.json"
    
    def _get_snapshot_path(self, snapshot_id: str) -> Path:
        """Get path for snapshot file"""
        return self.snapshots_dir / f"{snapshot_id}.json"
    
    def save_conversation(self, conversation: ConversationHistory) -> None:
        """Save conversation history"""
        path = self._get_conversation_path(conversation.session_id)
        data = conversation.to_dict()
        
        if self.encryption.is_enabled:
            # Only encrypt content, keep structure
            for msg in data.get("messages", []):
                if "content" in msg:
                    msg["content"] = self.encryption.encrypt(msg["content"])
        
        path.write_text(json.dumps(data, indent=2))
        logger.debug(f"Saved conversation {conversation.session_id}")
    
    def load_conversation(self, session_id: str) -> Optional[ConversationHistory]:
        """Load conversation history"""
        path = self._get_conversation_path(session_id)
        
        if not path.exists():
            return None
        
        try:
            data = json.loads(path.read_text())
            
            if self.encryption.is_enabled:
                data = self.encryption.decrypt_dict(data)
            
            return ConversationHistory.from_dict(data)
        except Exception as e:
            logger.error(f"Failed to load conversation {session_id}: {e}")
            return None
    
    def delete_conversation(self, session_id: str) -> None:
        """Delete conversation history"""
        path = self._get_conversation_path(session_id)
        if path.exists():
            path.unlink()
    
    def list_conversations(self) -> List[str]:
        """List all conversation IDs"""
        return [p.stem for p in self.conversations_dir.glob("*.json")]
    
    def save_task(self, task: TaskState) -> None:
        """Save task state"""
        path = self._get_task_path(task.task_id)
        data = task.to_dict()
        
        if self.encryption.is_enabled and task.result:
            data["result"] = self.encryption.encrypt(task.result)
        
        path.write_text(json.dumps(data, indent=2))
        logger.debug(f"Saved task {task.task_id}")
    
    def load_task(self, task_id: str) -> Optional[TaskState]:
        """Load task state"""
        path = self._get_task_path(task_id)
        
        if not path.exists():
            return None
        
        try:
            data = json.loads(path.read_text())
            
            if self.encryption.is_enabled:
                data = self.encryption.decrypt_dict(data)
            
            return TaskState.from_dict(data)
        except Exception as e:
            logger.error(f"Failed to load task {task_id}: {e}")
            return None
    
    def delete_task(self, task_id: str) -> None:
        """Delete task state"""
        path = self._get_task_path(task_id)
        if path.exists():
            path.unlink()
    
    def list_tasks(self) -> List[str]:
        """List all task IDs"""
        return [p.stem for p in self.tasks_dir.glob("*.json")]
    
    def save_snapshot(self, snapshot: StateSnapshot) -> None:
        """Save state snapshot"""
        path = self._get_snapshot_path(snapshot.snapshot_id)
        data = snapshot.to_dict()
        path.write_text(json.dumps(data, indent=2))
        logger.info(f"Saved snapshot {snapshot.snapshot_id}")
    
    def load_snapshot(self, snapshot_id: str) -> Optional[StateSnapshot]:
        """Load state snapshot"""
        path = self._get_snapshot_path(snapshot_id)
        
        if not path.exists():
            return None
        
        try:
            data = json.loads(path.read_text())
            return StateSnapshot.from_dict(data)
        except Exception as e:
            logger.error(f"Failed to load snapshot {snapshot_id}: {e}")
            return None
    
    def delete_snapshot(self, snapshot_id: str) -> None:
        """Delete snapshot"""
        path = self._get_snapshot_path(snapshot_id)
        if path.exists():
            path.unlink()
    
    def list_snapshots(self) -> List[str]:
        """List all snapshot IDs"""
        return [p.stem for p in self.snapshots_dir.glob("*.json")]
    
    def cleanup_old_snapshots(self, keep_count: int = 10) -> int:
        """Clean up old snapshots, keeping only the most recent"""
        snapshots = sorted(
            self.snapshots_dir.glob("*.json"),
            key=lambda p: p.stat().st_mtime,
            reverse=True
        )
        
        deleted = 0
        for snapshot in snapshots[keep_count:]:
            snapshot.unlink()
            deleted += 1
        
        return deleted


class TaskQueue:
    """Thread-safe task queue for scheduled jobs"""
    
    def __init__(self, maxsize: int = 1000):
        self._queue: queue.Queue = queue.Queue(maxsize=maxsize)
        self._lock = threading.Lock()
        self._tasks: Dict[str, TaskState] = {}
    
    def put(self, task: TaskState) -> bool:
        """Add a task to the queue"""
        try:
            self._queue.put_nowait(task)
            with self._lock:
                self._tasks[task.task_id] = task
            return True
        except queue.Full:
            logger.warning("Task queue is full")
            return False
    
    def get(self, timeout: float = 1.0) -> Optional[TaskState]:
        """Get a task from the queue"""
        try:
            task = self._queue.get(timeout=timeout)
            return task
        except queue.Empty:
            return None
    
    def get_task(self, task_id: str) -> Optional[TaskState]:
        """Get task by ID"""
        with self._lock:
            return self._tasks.get(task_id)
    
    def update_task(self, task: TaskState) -> None:
        """Update task state"""
        with self._lock:
            self._tasks[task.task_id] = task
    
    def remove_task(self, task_id: str) -> None:
        """Remove task from tracking"""
        with self._lock:
            self._tasks.pop(task_id, None)
    
    def list_tasks(self, status: Optional[str] = None) -> List[TaskState]:
        """List all tasks, optionally filtered by status"""
        with self._lock:
            tasks = list(self._tasks.values())
        
        if status:
            tasks = [t for t in tasks if t.status == status]
        
        return sorted(tasks, key=lambda t: t.created_at)
    
    def clear(self) -> None:
        """Clear all tasks"""
        with self._lock:
            self._tasks.clear()
        # Drain the queue
        while not self._queue.empty():
            try:
                self._queue.get_nowait()
            except queue.Empty:
                break


class PersistentStateManager:
    """
    Main class for managing persistent daemon state.
    
    Features:
    - Conversation history persistence
    - Task queue management
    - State snapshots
    - Encryption support
    - Automatic backup
    """
    
    def __init__(
        self,
        state_dir: str = "/var/lib/luminaguard/state",
        encryption_key: Optional[str] = None,
        max_history_messages: int = 1000,
        snapshot_interval_seconds: int = 300,
    ):
        self.state_dir = state_dir
        self.max_history_messages = max_history_messages
        self.snapshot_interval = snapshot_interval_seconds
        
        # Initialize components
        self.encryption = StateEncryption(encryption_key)
        self.storage = StateStorage(state_dir, self.encryption)
        self.task_queue = TaskQueue()
        
        # In-memory state
        self._conversations: Dict[str, ConversationHistory] = {}
        self._lock = threading.RLock()
        
        # Snapshot thread
        self._snapshot_thread: Optional[threading.Thread] = None
        self._stop_snapshot = threading.Event()
        
        # Callbacks
        self._on_state_change: List[Callable] = []
    
    def start(self) -> None:
        """Start the state manager"""
        # Load existing conversations
        for session_id in self.storage.list_conversations():
            conv = self.storage.load_conversation(session_id)
            if conv:
                self._conversations[session_id] = conv
        
        # Load existing tasks
        for task_id in self.storage.list_tasks():
            task = self.storage.load_task(task_id)
            if task:
                self.task_queue.update_task(task)
        
        # Start snapshot thread
        self._snapshot_thread = threading.Thread(
            target=self._snapshot_loop,
            daemon=True,
        )
        self._snapshot_thread.start()
        
        logger.info("State manager started")
    
    def stop(self) -> None:
        """Stop the state manager"""
        self._stop_snapshot.set()
        if self._snapshot_thread:
            self._snapshot_thread.join(timeout=5)
        
        # Save all state
        self.save_all()
        
        logger.info("State manager stopped")
    
    def _snapshot_loop(self) -> None:
        """Background loop for periodic snapshots"""
        while not self._stop_snapshot.wait(self.snapshot_interval):
            try:
                self.create_snapshot()
            except Exception as e:
                logger.error(f"Failed to create snapshot: {e}")
    
    def _prune_conversations(self) -> None:
        """Prune old messages from conversations"""
        for conv in self._conversations.values():
            if len(conv.messages) > self.max_history_messages:
                conv.messages = conv.messages[-self.max_history_messages:]
    
    # Conversation Management
    
    def get_or_create_conversation(self, session_id: str) -> ConversationHistory:
        """Get existing conversation or create new one"""
        with self._lock:
            if session_id not in self._conversations:
                self._conversations[session_id] = ConversationHistory(session_id=session_id)
            return self._conversations[session_id]
    
    def add_message(
        self,
        session_id: str,
        role: str,
        content: str,
        metadata: Optional[Dict] = None,
    ) -> None:
        """Add a message to conversation"""
        conv = self.get_or_create_conversation(session_id)
        conv.add_message(role, content, metadata)
        
        # Save to disk
        self.storage.save_conversation(conv)
        
        # Prune if needed
        self._prune_conversations()
        
        # Notify callbacks
        self._notify_change()
    
    def get_conversation(self, session_id: str) -> Optional[ConversationHistory]:
        """Get conversation by ID"""
        with self._lock:
            return self._conversations.get(session_id)
    
    def list_conversations(self) -> List[str]:
        """List all conversation IDs"""
        with self._lock:
            return list(self._conversations.keys())
    
    def delete_conversation(self, session_id: str) -> None:
        """Delete conversation"""
        with self._lock:
            self._conversations.pop(session_id, None)
        self.storage.delete_conversation(session_id)
        self._notify_change()
    
    # Task Management
    
    def create_task(
        self,
        task_id: str,
        name: str,
        metadata: Optional[Dict] = None,
    ) -> TaskState:
        """Create a new task"""
        task = TaskState(
            task_id=task_id,
            name=name,
            status="pending",
            metadata=metadata or {}
        )
        self.task_queue.put(task)
        self.storage.save_task(task)
        return task
    
    def start_task(self, task_id: str) -> Optional[TaskState]:
        """Mark task as started"""
        task = self.task_queue.get_task(task_id)
        if task:
            task.status = "running"
            task.started_at = time.time()
            self.task_queue.update_task(task)
            self.storage.save_task(task)
            self._notify_change()
            return task
        return None
    
    def complete_task(self, task_id: str, result: str) -> Optional[TaskState]:
        """Mark task as completed"""
        task = self.task_queue.get_task(task_id)
        if task:
            task.status = "completed"
            task.completed_at = time.time()
            task.result = result
            self.task_queue.update_task(task)
            self.storage.save_task(task)
            self._notify_change()
            return task
        return None
    
    def fail_task(self, task_id: str, error: str) -> Optional[TaskState]:
        """Mark task as failed"""
        task = self.task_queue.get_task(task_id)
        if task:
            task.status = "failed"
            task.completed_at = time.time()
            task.error = error
            self.task_queue.update_task(task)
            self.storage.save_task(task)
            self._notify_change()
            return task
        return None
    
    def get_task(self, task_id: str) -> Optional[TaskState]:
        """Get task by ID"""
        return self.task_queue.get_task(task_id)
    
    def list_tasks(self, status: Optional[str] = None) -> List[TaskState]:
        """List tasks, optionally filtered by status"""
        return self.task_queue.list_tasks(status)
    
    # Snapshot Management
    
    def create_snapshot(self, metadata: Optional[Dict] = None) -> StateSnapshot:
        """Create a state snapshot"""
        snapshot_id = hashlib.sha256(
            f"{time.time()}{os.urandom(16)}".encode()
        ).hexdigest()[:16]
        
        snapshot = StateSnapshot(
            snapshot_id=snapshot_id,
            metadata=metadata or {},
        )
        
        # Capture conversations
        with self._lock:
            for session_id, conv in self._conversations.items():
                snapshot.conversations[session_id] = conv.to_dict()
        
        # Capture tasks
        for task in self.task_queue.list_tasks():
            snapshot.tasks[task.task_id] = task.to_dict()
        
        # Save snapshot
        self.storage.save_snapshot(snapshot)
        
        # Cleanup old snapshots
        self.storage.cleanup_old_snapshots(10)
        
        logger.info(f"Created snapshot {snapshot_id}")
        return snapshot
    
    def restore_snapshot(self, snapshot_id: str) -> bool:
        """Restore from a snapshot"""
        snapshot = self.storage.load_snapshot(snapshot_id)
        if not snapshot:
            return False
        
        # Restore conversations
        with self._lock:
            self._conversations = {}
            for session_id, data in snapshot.conversations.items():
                self._conversations[session_id] = ConversationHistory.from_dict(data)
        
        # Restore tasks
        for task_id, data in snapshot.tasks.items():
            task = TaskState.from_dict(data)
            self.task_queue.update_task(task)
        
        # Save restored state
        self.save_all()
        
        logger.info(f"Restored from snapshot {snapshot_id}")
        return True
    
    def list_snapshots(self) -> List[str]:
        """List all snapshots"""
        return self.storage.list_snapshots()
    
    # Utility Methods
    
    def save_all(self) -> None:
        """Save all in-memory state to disk"""
        with self._lock:
            for conv in self._conversations.values():
                self.storage.save_conversation(conv)
        
        for task in self.task_queue.list_tasks():
            self.storage.save_task(task)
        
        logger.debug("Saved all state")
    
    def register_change_callback(self, callback: Callable) -> None:
        """Register a callback for state changes"""
        self._on_state_change.append(callback)
    
    def _notify_change(self) -> None:
        """Notify all registered callbacks of state change"""
        for callback in self._on_state_change:
            try:
                callback()
            except Exception as e:
                logger.error(f"Error in state change callback: {e}")
    
    @property
    def encryption_key(self) -> Optional[str]:
        """Get encryption key (for saving)"""
        return self.encryption.key


def create_state_manager(
    state_dir: str = "/var/lib/luminaguard/state",
    encryption_key: Optional[str] = None,
    max_history_messages: int = 1000,
    snapshot_interval_seconds: int = 300,
) -> PersistentStateManager:
    """Factory function to create a PersistentStateManager"""
    return PersistentStateManager(
        state_dir=state_dir,
        encryption_key=encryption_key,
        max_history_messages=max_history_messages,
        snapshot_interval_seconds=snapshot_interval_seconds,
    )


# CLI support
def main():
    """CLI for state management"""
    import argparse
    
    parser = argparse.ArgumentParser(description="LuminaGuard State Management")
    subparsers = parser.add_subparsers(dest="command", help="Commands")
    
    # List command
    list_parser = subparsers.add_parser("list", help="List state")
    list_parser.add_argument("type", choices=["conversations", "tasks", "snapshots"])
    
    # Show command
    show_parser = subparsers.add_parser("show", help="Show state")
    show_parser.add_argument("type", choices=["conversation", "task", "snapshot"])
    show_parser.add_argument("id", help="ID of the item to show")
    
    # Delete command
    delete_parser = subparsers.add_parser("delete", help="Delete state")
    delete_parser.add_argument("type", choices=["conversation", "task", "snapshot"])
    delete_parser.add_argument("id", help="ID of the item to delete")
    
    # Snapshot command
    snapshot_parser = subparsers.add_parser("snapshot", help="Manage snapshots")
    snapshot_parser.add_argument("action", choices=["create", "restore", "list"])
    snapshot_parser.add_argument("--id", help="Snapshot ID (for restore)")
    
    args = parser.parse_args()
    
    # Default state directory
    state_dir = os.environ.get("LUMINAGUARD_STATE_DIR", "/var/lib/luminaguard/state")
    manager = create_state_manager(state_dir=state_dir)
    
    if args.command == "list":
        if args.type == "conversations":
            conversations = manager.list_conversations()
            print(f"Conversations ({len(conversations)}):")
            for c in conversations:
                print(f"  - {c}")
        elif args.type == "tasks":
            tasks = manager.list_tasks()
            print(f"Tasks ({len(tasks)}):")
            for t in tasks:
                print(f"  - {t.task_id}: {t.name} ({t.status})")
        elif args.type == "snapshots":
            snapshots = manager.list_snapshots()
            print(f"Snapshots ({len(snapshots)}):")
            for s in snapshots:
                print(f"  - {s}")
    
    elif args.command == "show":
        if args.type == "conversation":
            conv = manager.get_conversation(args.id)
            if conv:
                print(json.dumps(conv.to_dict(), indent=2))
            else:
                print(f"Conversation {args.id} not found")
        elif args.type == "task":
            task = manager.get_task(args.id)
            if task:
                print(json.dumps(task.to_dict(), indent=2))
            else:
                print(f"Task {args.id} not found")
        elif args.type == "snapshot":
            snapshot = manager.storage.load_snapshot(args.id)
            if snapshot:
                print(json.dumps(snapshot.to_dict(), indent=2))
            else:
                print(f"Snapshot {args.id} not found")
    
    elif args.command == "delete":
        if args.type == "conversation":
            manager.delete_conversation(args.id)
            print(f"Deleted conversation {args.id}")
        elif args.type == "task":
            manager.task_queue.remove_task(args.id)
            manager.storage.delete_task(args.id)
            print(f"Deleted task {args.id}")
        elif args.type == "snapshot":
            manager.storage.delete_snapshot(args.id)
            print(f"Deleted snapshot {args.id}")
    
    elif args.command == "snapshot":
        if args.action == "create":
            snapshot = manager.create_snapshot()
            print(f"Created snapshot {snapshot.snapshot_id}")
        elif args.action == "restore":
            if not args.id:
                print("Error: --id required for restore")
                return 1
            if manager.restore_snapshot(args.id):
                print(f"Restored from snapshot {args.id}")
            else:
                print(f"Failed to restore snapshot {args.id}")
                return 1
        elif args.action == "list":
            snapshots = manager.list_snapshots()
            print(f"Snapshots ({len(snapshots)}):")
            for s in snapshots:
                print(f"  - {s}")


if __name__ == "__main__":
    main()
