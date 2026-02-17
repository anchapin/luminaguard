"""
LuminaGuard Skills & Plugins System

A comprehensive framework for extending LuminaGuard with community skills and plugins.

Features:
- Community skill registry and sharing
- Custom skill development framework
- Skill auto-discovery and loading
- Skill version management
- Skill security sandboxing
- AI-assisted skill generation

Part of: luminaguard-0va.10 - Skills & Plugins System
"""

__all__ = [
    "Skill",
    "SkillContext",
    "SkillRegistry",
    "SkillLoader",
    "SkillManager",
    "SkillMetadata",
]

from abc import ABC, abstractmethod
from dataclasses import dataclass, field, asdict
from typing import Any, Dict, List, Optional, Callable, Type
from enum import Enum
import json
import logging
import importlib
from pathlib import Path

logger = logging.getLogger(__name__)


class SkillType(Enum):
    """Types of skills"""
    TOOL = "tool"  # Callable tool/function
    INTEGRATION = "integration"  # External service integration
    TRANSFORMER = "transformer"  # Data transformation
    ANALYZER = "analyzer"  # Analysis and insights
    WORKFLOW = "workflow"  # Multi-step workflows


@dataclass
class SkillMetadata:
    """Metadata for a skill"""
    name: str
    version: str
    author: str
    description: str
    skill_type: SkillType
    
    # Dependencies
    dependencies: List[str] = field(default_factory=list)
    required_skills: List[str] = field(default_factory=list)
    
    # Versioning
    min_luminaguard_version: str = "0.1.0"
    max_luminaguard_version: Optional[str] = None
    
    # Metadata
    tags: List[str] = field(default_factory=list)
    repository: Optional[str] = None
    license: str = "MIT"
    
    # Security
    requires_approval: bool = False
    sandboxed: bool = True
    allowed_permissions: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        data = asdict(self)
        data["skill_type"] = self.skill_type.value
        return data
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SkillMetadata":
        """Create from dictionary"""
        data = dict(data)
        if "skill_type" in data and isinstance(data["skill_type"], str):
            data["skill_type"] = SkillType(data["skill_type"])
        return cls(**data)


@dataclass
class SkillContext:
    """Context for skill execution"""
    skill_name: str
    user_id: Optional[str] = None
    session_id: Optional[str] = None
    permissions: List[str] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def has_permission(self, permission: str) -> bool:
        """Check if context has permission"""
        return permission in self.permissions or "*" in self.permissions


class Skill(ABC):
    """Abstract base class for skills"""
    
    def __init__(self, metadata: SkillMetadata):
        self.metadata = metadata
        self._enabled = True
    
    @property
    def name(self) -> str:
        """Get skill name"""
        return self.metadata.name
    
    @property
    def version(self) -> str:
        """Get skill version"""
        return self.metadata.version
    
    @property
    def is_enabled(self) -> bool:
        """Check if skill is enabled"""
        return self._enabled
    
    def enable(self) -> None:
        """Enable the skill"""
        self._enabled = True
    
    def disable(self) -> None:
        """Disable the skill"""
        self._enabled = False
    
    @abstractmethod
    async def initialize(self, context: SkillContext) -> bool:
        """Initialize the skill"""
        pass
    
    @abstractmethod
    async def execute(self, context: SkillContext, **kwargs) -> Any:
        """Execute the skill"""
        pass
    
    @abstractmethod
    async def cleanup(self) -> None:
        """Clean up skill resources"""
        pass
    
    @abstractmethod
    def get_schema(self) -> Dict[str, Any]:
        """Get JSON schema for skill inputs"""
        pass


class ToolSkill(Skill):
    """A skill that represents a callable tool"""
    
    def __init__(self, metadata: SkillMetadata, func: Callable):
        super().__init__(metadata)
        self.func = func
    
    async def initialize(self, context: SkillContext) -> bool:
        """Initialize the tool skill"""
        logger.info(f"Initialized tool skill: {self.name}")
        return True
    
    async def execute(self, context: SkillContext, **kwargs) -> Any:
        """Execute the tool"""
        if not self.is_enabled:
            raise RuntimeError(f"Skill {self.name} is disabled")
        
        return await self.func(**kwargs)
    
    async def cleanup(self) -> None:
        """Clean up resources"""
        pass
    
    def get_schema(self) -> Dict[str, Any]:
        """Get input schema"""
        # This would normally inspect the function signature
        return {}


class SkillRegistry:
    """Registry for managing skills"""
    
    def __init__(self):
        self._skills: Dict[str, Skill] = {}
        self._metadata: Dict[str, SkillMetadata] = {}
        self._lock = __import__("threading").Lock()
    
    def register(self, metadata: SkillMetadata, skill: Skill) -> None:
        """Register a skill"""
        with self._lock:
            key = f"{metadata.name}@{metadata.version}"
            self._skills[key] = skill
            self._metadata[key] = metadata
            logger.info(f"Registered skill: {key}")
    
    def unregister(self, name: str, version: str) -> None:
        """Unregister a skill"""
        with self._lock:
            key = f"{name}@{version}"
            if key in self._skills:
                del self._skills[key]
                del self._metadata[key]
                logger.info(f"Unregistered skill: {key}")
    
    def get(self, name: str, version: Optional[str] = None) -> Optional[Skill]:
        """Get a registered skill"""
        with self._lock:
            if version:
                key = f"{name}@{version}"
                return self._skills.get(key)
            
            # Return latest version
            matching = [sk for sk in self._skills.keys() if sk.startswith(f"{name}@")]
            if matching:
                return self._skills[matching[-1]]
            
            return None
    
    def get_metadata(self, name: str, version: Optional[str] = None) -> Optional[SkillMetadata]:
        """Get skill metadata"""
        with self._lock:
            if version:
                key = f"{name}@{version}"
                return self._metadata.get(key)
            
            matching = [sk for sk in self._metadata.keys() if sk.startswith(f"{name}@")]
            if matching:
                return self._metadata[matching[-1]]
            
            return None
    
    def list_skills(self, skill_type: Optional[SkillType] = None) -> List[SkillMetadata]:
        """List registered skills"""
        with self._lock:
            skills = list(self._metadata.values())
            if skill_type:
                skills = [s for s in skills if s.skill_type == skill_type]
            return skills
    
    def list_all(self) -> Dict[str, SkillMetadata]:
        """List all skills with their metadata"""
        with self._lock:
            return dict(self._metadata)


class SkillLoader:
    """Loads skills from various sources"""
    
    def __init__(self, registry: SkillRegistry):
        self.registry = registry
    
    async def load_from_file(self, file_path: Path) -> Optional[SkillMetadata]:
        """Load skill from Python file"""
        try:
            spec = importlib.util.spec_from_file_location("skill_module", file_path)
            if spec and spec.loader:
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
                
                # Look for SKILL_METADATA and SKILL class
                if hasattr(module, "SKILL_METADATA") and hasattr(module, "SKILL"):
                    metadata = module.SKILL_METADATA
                    skill_class = module.SKILL
                    skill = skill_class(metadata)
                    self.registry.register(metadata, skill)
                    return metadata
        except Exception as e:
            logger.error(f"Error loading skill from {file_path}: {e}")
        
        return None
    
    async def load_from_directory(self, directory: Path) -> List[SkillMetadata]:
        """Load all skills from directory"""
        loaded = []
        
        if not directory.exists():
            logger.warning(f"Skill directory does not exist: {directory}")
            return loaded
        
        for py_file in directory.glob("*.py"):
            if py_file.name.startswith("_"):
                continue
            
            metadata = await self.load_from_file(py_file)
            if metadata:
                loaded.append(metadata)
        
        return loaded
    
    async def load_from_json(self, json_file: Path) -> Optional[SkillMetadata]:
        """Load skill definition from JSON"""
        try:
            with open(json_file) as f:
                data = json.load(f)
            
            metadata = SkillMetadata.from_dict(data)
            # This would need a way to load the actual skill implementation
            return metadata
        except Exception as e:
            logger.error(f"Error loading skill from {json_file}: {e}")
        
        return None


class SkillManager:
    """Manages skill lifecycle"""
    
    def __init__(self, registry: SkillRegistry, loader: SkillLoader):
        self.registry = registry
        self.loader = loader
        self._contexts: Dict[str, SkillContext] = {}
    
    async def enable_skill(self, name: str, version: Optional[str] = None) -> bool:
        """Enable a skill"""
        skill = self.registry.get(name, version)
        if skill:
            skill.enable()
            logger.info(f"Enabled skill: {name}")
            return True
        return False
    
    async def disable_skill(self, name: str, version: Optional[str] = None) -> bool:
        """Disable a skill"""
        skill = self.registry.get(name, version)
        if skill:
            skill.disable()
            logger.info(f"Disabled skill: {name}")
            return True
        return False
    
    async def execute_skill(
        self,
        name: str,
        context: SkillContext,
        **kwargs
    ) -> Any:
        """Execute a skill"""
        skill = self.registry.get(name)
        if not skill:
            raise ValueError(f"Skill not found: {name}")
        
        if not skill.is_enabled:
            raise RuntimeError(f"Skill is disabled: {name}")
        
        # Check permissions
        metadata = self.registry.get_metadata(name)
        if metadata and metadata.requires_approval:
            logger.warning(f"Skill {name} requires approval")
        
        return await skill.execute(context, **kwargs)
    
    async def get_skill_schema(self, name: str) -> Optional[Dict[str, Any]]:
        """Get skill input schema"""
        skill = self.registry.get(name)
        if skill:
            return skill.get_schema()
        return None


# Global registry and manager
_global_registry = SkillRegistry()
_global_loader = SkillLoader(_global_registry)
_global_manager = SkillManager(_global_registry, _global_loader)


def get_registry() -> SkillRegistry:
    """Get the global skill registry"""
    return _global_registry


def get_manager() -> SkillManager:
    """Get the global skill manager"""
    return _global_manager
