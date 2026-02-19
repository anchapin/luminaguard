"""Bot persona and onboarding system for daemon mode."""

import json
from dataclasses import dataclass, asdict, field
from pathlib import Path
from typing import Optional, Dict, Any
import logging

logger = logging.getLogger(__name__)


@dataclass
class PersonaConfig:
    """Configuration for bot persona."""

    name: str
    description: str
    behavior_traits: Dict[str, Any] = field(default_factory=dict)
    custom_instructions: str = ""
    system_prompt: str = ""
    max_context_window: int = 8192
    temperature: float = 0.7
    model_preferences: Dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return asdict(self)

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "PersonaConfig":
        """Create from dictionary."""
        return cls(**data)


@dataclass
class OnboardingProfile:
    """User profile collected during onboarding."""

    username: str
    email: Optional[str] = None
    use_case: str = ""
    expertise_level: str = "intermediate"  # novice, intermediate, expert
    preferred_languages: list = field(default_factory=lambda: ["en"])
    timezone: str = "UTC"
    notification_preferences: Dict[str, bool] = field(
        default_factory=lambda: {"errors": True, "info": False, "debug": False}
    )
    api_keys: Dict[str, str] = field(default_factory=dict)
    custom_context: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return asdict(self)

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "OnboardingProfile":
        """Create from dictionary."""
        return cls(**data)


class PersonaManager:
    """Manages bot personas and their persistence."""

    def __init__(self, config_dir: Path):
        """Initialize persona manager.

        Args:
            config_dir: Directory to store persona configurations
        """
        self.config_dir = Path(config_dir)
        self.config_dir.mkdir(parents=True, exist_ok=True)
        self.persona_file = self.config_dir / "persona.json"
        self.onboarding_file = self.config_dir / "onboarding.json"

    def load_persona(self) -> Optional[PersonaConfig]:
        """Load saved persona configuration."""
        if not self.persona_file.exists():
            return None

        try:
            with open(self.persona_file) as f:
                data = json.load(f)
            persona = PersonaConfig.from_dict(data)
            logger.info(f"Loaded persona: {persona.name}")
            return persona
        except Exception as e:
            logger.error(f"Failed to load persona: {e}")
            return None

    def save_persona(self, persona: PersonaConfig) -> bool:
        """Save persona configuration."""
        try:
            with open(self.persona_file, "w") as f:
                json.dump(persona.to_dict(), f, indent=2)
            logger.info(f"Saved persona: {persona.name}")
            return True
        except Exception as e:
            logger.error(f"Failed to save persona: {e}")
            return False

    def load_onboarding_profile(self) -> Optional[OnboardingProfile]:
        """Load saved onboarding profile."""
        if not self.onboarding_file.exists():
            return None

        try:
            with open(self.onboarding_file) as f:
                data = json.load(f)
            profile = OnboardingProfile.from_dict(data)
            logger.info(f"Loaded onboarding profile for: {profile.username}")
            return profile
        except Exception as e:
            logger.error(f"Failed to load onboarding profile: {e}")
            return None

    def save_onboarding_profile(self, profile: OnboardingProfile) -> bool:
        """Save onboarding profile."""
        try:
            with open(self.onboarding_file, "w") as f:
                json.dump(profile.to_dict(), f, indent=2)
            logger.info(f"Saved onboarding profile for: {profile.username}")
            return True
        except Exception as e:
            logger.error(f"Failed to save onboarding profile: {e}")
            return False

    def get_system_prompt(self, persona: PersonaConfig) -> str:
        """Generate system prompt from persona."""
        if persona.system_prompt:
            return persona.system_prompt

        prompt = f"""You are {persona.name}. {persona.description}

Behavior Traits:
"""
        for trait, value in persona.behavior_traits.items():
            prompt += f"- {trait}: {value}\n"

        if persona.custom_instructions:
            prompt += f"\nCustom Instructions:\n{persona.custom_instructions}\n"

        return prompt


class OnboardingFlow:
    """Guided onboarding flow for initial setup."""

    def __init__(self, persona_manager: PersonaManager):
        """Initialize onboarding flow.

        Args:
            persona_manager: PersonaManager instance
        """
        self.persona_manager = persona_manager

    def run_interactive_onboarding(self) -> tuple[OnboardingProfile, PersonaConfig]:
        """Run interactive onboarding flow.

        Returns:
            Tuple of (onboarding_profile, persona_config)
        """
        # Check for existing profile
        existing_profile = self.persona_manager.load_onboarding_profile()
        existing_persona = self.persona_manager.load_persona()

        if existing_profile and existing_persona:
            logger.info("Using existing onboarding profile and persona")
            return existing_profile, existing_persona

        # Collect user information
        username = input("Enter your username: ").strip()
        email = input("Enter your email (optional): ").strip() or None
        use_case = input("What is your primary use case? ").strip()
        expertise = (
            input(
                "What is your expertise level? (novice/intermediate/expert) [intermediate]: "
            ).strip()
            or "intermediate"
        )

        # Create onboarding profile
        profile = OnboardingProfile(
            username=username,
            email=email,
            use_case=use_case,
            expertise_level=expertise,
        )

        # Create default persona
        persona = PersonaConfig(
            name=f"{username}'s Bot",
            description=f"Bot configured for {use_case}",
            behavior_traits={
                "responsiveness": "immediate",
                "verbosity": "concise" if expertise == "expert" else "detailed",
                "error_handling": "graceful",
            },
        )

        # Save both
        self.persona_manager.save_onboarding_profile(profile)
        self.persona_manager.save_persona(persona)

        logger.info(f"Onboarding completed for {username}")
        return profile, persona

    def prime_context(
        self, profile: OnboardingProfile, persona: PersonaConfig
    ) -> Dict[str, Any]:
        """Prime initial context from profile and persona.

        Args:
            profile: Onboarding profile
            persona: Persona config

        Returns:
            Context dictionary for LLM initialization
        """
        context = {
            "user": {
                "username": profile.username,
                "email": profile.email,
                "timezone": profile.timezone,
                "expertise_level": profile.expertise_level,
            },
            "bot": {
                "name": persona.name,
                "description": persona.description,
                "traits": persona.behavior_traits,
            },
            "preferences": {
                "languages": profile.preferred_languages,
                "notifications": profile.notification_preferences,
            },
            "system_prompt": self.persona_manager.get_system_prompt(persona),
        }

        # Add custom context if provided
        if profile.custom_context:
            context["custom"] = profile.custom_context

        return context
