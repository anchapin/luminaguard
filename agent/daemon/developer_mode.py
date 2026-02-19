"""Hackable installation mode for developer-friendly setup."""

import asyncio
import json
import logging
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Dict, Any, List

logger = logging.getLogger(__name__)


class InstallationType:
    """Installation type constants."""

    SOURCE = "source"
    BINARY = "binary"
    DOCKER = "docker"


@dataclass
class DeveloperConfig:
    """Configuration for developer mode."""

    enable_hot_reload: bool = True
    enable_debug_mode: bool = True
    enable_profiling: bool = False
    watch_paths: List[Path] = None
    rebuild_on_change: bool = True
    auto_restart: bool = True
    log_level: str = "DEBUG"

    def __post_init__(self):
        """Initialize defaults for mutable fields."""
        if self.watch_paths is None:
            self.watch_paths = [Path("agent"), Path("orchestrator")]


class SourceInstaller:
    """Handles source code installation from git."""

    def __init__(self, repo_path: Path):
        """Initialize source installer.

        Args:
            repo_path: Path to repository
        """
        self.repo_path = Path(repo_path)

    async def clone_repository(
        self, git_url: str, target_path: Optional[Path] = None
    ) -> bool:
        """Clone git repository.

        Args:
            git_url: Git repository URL
            target_path: Target directory for clone

        Returns:
            True if successful
        """
        target = target_path or self.repo_path
        try:
            logger.info(f"Cloning repository: {git_url}")
            result = subprocess.run(
                ["git", "clone", git_url, str(target)],
                capture_output=True,
                text=True,
                timeout=300,
            )
            if result.returncode == 0:
                logger.info(f"Repository cloned to {target}")
                return True
            else:
                logger.error(f"Clone failed: {result.stderr}")
                return False
        except Exception as e:
            logger.error(f"Repository clone failed: {e}")
            return False

    async def install_dependencies(self) -> bool:
        """Install project dependencies.

        Returns:
            True if successful
        """
        try:
            logger.info("Installing dependencies")

            # Check for requirements.txt
            if (self.repo_path / "agent" / "pyproject.toml").exists():
                logger.info("Installing Python dependencies via poetry")
                result = subprocess.run(
                    ["poetry", "install"],
                    cwd=str(self.repo_path / "agent"),
                    capture_output=True,
                    text=True,
                    timeout=600,
                )
                if result.returncode != 0:
                    logger.error(f"Poetry install failed: {result.stderr}")
                    return False

            # Check for package.json
            if (self.repo_path / "package.json").exists():
                logger.info("Installing Node dependencies")
                result = subprocess.run(
                    ["npm", "install"],
                    cwd=str(self.repo_path),
                    capture_output=True,
                    text=True,
                    timeout=600,
                )
                if result.returncode != 0:
                    logger.error(f"npm install failed: {result.stderr}")
                    return False

            logger.info("Dependencies installed successfully")
            return True
        except Exception as e:
            logger.error(f"Dependency installation failed: {e}")
            return False

    async def build_project(self) -> bool:
        """Build project from source.

        Returns:
            True if successful
        """
        try:
            logger.info("Building project")

            # Build orchestrator if Makefile exists
            if (self.repo_path / "Makefile").exists():
                result = subprocess.run(
                    ["make", "build"],
                    cwd=str(self.repo_path),
                    capture_output=True,
                    text=True,
                    timeout=600,
                )
                if result.returncode != 0:
                    logger.warning(f"Make build had issues: {result.stderr}")

            logger.info("Project built successfully")
            return True
        except Exception as e:
            logger.error(f"Build failed: {e}")
            return False


class HotReloader:
    """Implements hot-reload functionality for development."""

    def __init__(self, watch_paths: List[Path], callback=None):
        """Initialize hot reloader.

        Args:
            watch_paths: Paths to watch for changes
            callback: Async callback to invoke on changes
        """
        self.watch_paths = watch_paths
        self.callback = callback
        self.last_modified: Dict[Path, float] = {}
        self._running = False

    async def start(self) -> None:
        """Start watching for file changes."""
        self._running = True
        logger.info(f"Hot reload started, watching: {self.watch_paths}")

        while self._running:
            try:
                changed = await self._check_changes()
                if changed and self.callback:
                    await self.callback(changed)
                await asyncio.sleep(1)  # Poll every second
            except Exception as e:
                logger.error(f"Hot reload error: {e}")
                await asyncio.sleep(5)

    async def stop(self) -> None:
        """Stop watching for file changes."""
        self._running = False
        logger.info("Hot reload stopped")

    async def _check_changes(self) -> List[Path]:
        """Check for modified files.

        Returns:
            List of changed file paths
        """
        changed = []
        for watch_path in self.watch_paths:
            if not watch_path.exists():
                continue

            for file_path in watch_path.rglob("*.py"):
                if file_path.stat().st_mtime > self.last_modified.get(file_path, 0):
                    changed.append(file_path)
                    self.last_modified[file_path] = file_path.stat().st_mtime
                    logger.debug(f"File changed: {file_path}")

        return changed


class DebugTools:
    """Provides debugging tools for developers."""

    @staticmethod
    async def enable_profiling(output_file: Path) -> bool:
        """Enable performance profiling.

        Args:
            output_file: File to write profiling data

        Returns:
            True if successful
        """
        try:
            logger.info(f"Enabling profiling to {output_file}")
            output_file.parent.mkdir(parents=True, exist_ok=True)
            # Profiling setup would be done in actual daemon
            return True
        except Exception as e:
            logger.error(f"Profiling setup failed: {e}")
            return False

    @staticmethod
    async def dump_debug_info(output_file: Path) -> bool:
        """Dump debug information.

        Args:
            output_file: File to write debug info

        Returns:
            True if successful
        """
        try:
            logger.info(f"Dumping debug info to {output_file}")
            output_file.parent.mkdir(parents=True, exist_ok=True)

            debug_info = {
                "python_version": __import__("sys").version,
                "platform": __import__("platform").platform(),
            }

            with open(output_file, "w") as f:
                json.dump(debug_info, f, indent=2)

            return True
        except Exception as e:
            logger.error(f"Debug dump failed: {e}")
            return False

    @staticmethod
    async def run_tests(test_dir: Path, pattern: str = "test_*.py") -> bool:
        """Run test suite.

        Args:
            test_dir: Directory containing tests
            pattern: Test file pattern

        Returns:
            True if all tests pass
        """
        try:
            logger.info(f"Running tests in {test_dir}")
            result = subprocess.run(
                ["pytest", str(test_dir), "-v", "-k", pattern],
                capture_output=True,
                text=True,
                timeout=300,
            )
            logger.info(result.stdout)
            if result.returncode != 0:
                logger.error(f"Tests failed: {result.stderr}")
                return False
            return True
        except Exception as e:
            logger.error(f"Test run failed: {e}")
            return False


class DeveloperMode:
    """Main developer mode orchestrator."""

    def __init__(self, repo_path: Path, config: Optional[DeveloperConfig] = None):
        """Initialize developer mode.

        Args:
            repo_path: Path to repository root
            config: DeveloperConfig instance
        """
        self.repo_path = Path(repo_path)
        self.config = config or DeveloperConfig()
        self.installer = SourceInstaller(self.repo_path)
        self.hot_reloader: Optional[HotReloader] = None
        self.debug_tools = DebugTools()

    async def setup_development_environment(
        self, git_url: Optional[str] = None
    ) -> bool:
        """Set up complete development environment.

        Args:
            git_url: Optional git URL to clone

        Returns:
            True if successful
        """
        try:
            # Clone if URL provided
            if git_url:
                if not await self.installer.clone_repository(git_url):
                    return False

            # Install dependencies
            if not await self.installer.install_dependencies():
                logger.warning("Dependency installation had issues, continuing...")

            # Build project
            if not await self.installer.build_project():
                logger.warning("Build had issues, continuing...")

            # Set up hot reload
            if self.config.enable_hot_reload:
                self.hot_reloader = HotReloader(self.config.watch_paths)

            logger.info("Development environment setup complete")
            return True
        except Exception as e:
            logger.error(f"Development environment setup failed: {e}")
            return False

    async def start_dev_server(self) -> bool:
        """Start development server with hot reload.

        Returns:
            True if server started
        """
        try:
            if self.hot_reloader:
                # Start hot reloader in background
                asyncio.create_task(self.hot_reloader.start())
                logger.info("Hot reload enabled for development")

            logger.info("Development server ready")
            return True
        except Exception as e:
            logger.error(f"Failed to start dev server: {e}")
            return False

    async def stop_dev_server(self) -> None:
        """Stop development server."""
        if self.hot_reloader:
            await self.hot_reloader.stop()
        logger.info("Development server stopped")

    async def run_developer_workflow(self) -> bool:
        """Run complete developer workflow.

        Returns:
            True if successful
        """
        try:
            logger.info("Starting developer workflow")

            # Setup environment
            if not await self.setup_development_environment():
                return False

            # Run tests
            if not await self.debug_tools.run_tests(self.repo_path / "agent" / "tests"):
                logger.warning("Some tests failed")

            # Start dev server
            if not await self.start_dev_server():
                return False

            logger.info("Developer workflow ready")
            return True
        except Exception as e:
            logger.error(f"Developer workflow failed: {e}")
            return False
