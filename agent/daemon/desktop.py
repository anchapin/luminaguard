"""Cross-platform desktop environment integration."""

import asyncio
import logging
import platform
import subprocess
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Optional, Dict, Any, List

logger = logging.getLogger(__name__)


class Platform(Enum):
    """Supported operating systems."""

    MACOS = "darwin"
    WINDOWS = "win32"
    LINUX = "linux"


class NotificationType(Enum):
    """Types of system notifications."""

    INFO = "info"
    SUCCESS = "success"
    WARNING = "warning"
    ERROR = "error"


@dataclass
class DesktopConfig:
    """Configuration for desktop integration."""

    auto_detect: bool = True
    notification_enabled: bool = True
    file_watcher_enabled: bool = True
    app_launch_enabled: bool = True


class PlatformDetector:
    """Detects and provides platform-specific information."""

    @staticmethod
    def get_platform() -> Platform:
        """Get current platform.

        Returns:
            Platform enum value
        """
        system = platform.system().lower()
        if system == "darwin":
            return Platform.MACOS
        elif system == "windows":
            return Platform.WINDOWS
        else:
            return Platform.LINUX

    @staticmethod
    def get_platform_version() -> str:
        """Get platform version.

        Returns:
            Version string
        """
        return platform.platform()

    @staticmethod
    def get_available_shells() -> List[str]:
        """Get available command shells.

        Returns:
            List of available shells
        """
        shells = []
        shell_paths = ["/bin/bash", "/bin/zsh", "/bin/sh"]

        for shell in shell_paths:
            if Path(shell).exists():
                shells.append(shell)

        return shells if shells else ["/bin/sh"]


class FileSystemManager:
    """Manages file system operations."""

    def __init__(self, base_path: Optional[Path] = None):
        """Initialize file system manager.

        Args:
            base_path: Base path for file operations
        """
        self.base_path = base_path or Path.home()

    async def read_file(self, path: Path) -> Optional[str]:
        """Read file contents.

        Args:
            path: File path

        Returns:
            File contents or None on error
        """
        try:
            full_path = self._resolve_path(path)
            logger.info(f"Reading file: {full_path}")
            with open(full_path, "r") as f:
                return f.read()
        except Exception as e:
            logger.error(f"Failed to read file {path}: {e}")
            return None

    async def write_file(self, path: Path, content: str, append: bool = False) -> bool:
        """Write to file.

        Args:
            path: File path
            content: Content to write
            append: Append to existing or overwrite

        Returns:
            True if successful
        """
        try:
            full_path = self._resolve_path(path)
            full_path.parent.mkdir(parents=True, exist_ok=True)
            mode = "a" if append else "w"
            logger.info(f"Writing to file: {full_path}")
            with open(full_path, mode) as f:
                f.write(content)
            return True
        except Exception as e:
            logger.error(f"Failed to write file {path}: {e}")
            return False

    async def list_directory(
        self, path: Path, recursive: bool = False
    ) -> Optional[List[Path]]:
        """List directory contents.

        Args:
            path: Directory path
            recursive: Recursively list subdirectories

        Returns:
            List of Path objects or None on error
        """
        try:
            full_path = self._resolve_path(path)
            logger.info(f"Listing directory: {full_path}")
            if recursive:
                return list(full_path.rglob("*"))
            else:
                return list(full_path.iterdir())
        except Exception as e:
            logger.error(f"Failed to list directory {path}: {e}")
            return None

    async def delete_file(self, path: Path) -> bool:
        """Delete a file.

        Args:
            path: File path

        Returns:
            True if successful
        """
        try:
            full_path = self._resolve_path(path)
            logger.info(f"Deleting file: {full_path}")
            full_path.unlink()
            return True
        except Exception as e:
            logger.error(f"Failed to delete file {path}: {e}")
            return False

    async def create_directory(self, path: Path) -> bool:
        """Create directory.

        Args:
            path: Directory path

        Returns:
            True if successful
        """
        try:
            full_path = self._resolve_path(path)
            logger.info(f"Creating directory: {full_path}")
            full_path.mkdir(parents=True, exist_ok=True)
            return True
        except Exception as e:
            logger.error(f"Failed to create directory {path}: {e}")
            return False

    def _resolve_path(self, path: Path) -> Path:
        """Resolve path relative to base path.

        Args:
            path: Path to resolve

        Returns:
            Resolved Path object
        """
        if path.is_absolute():
            return path
        return self.base_path / path


class ApplicationLauncher:
    """Launches and manages applications."""

    def __init__(self):
        """Initialize application launcher."""
        self.platform = PlatformDetector.get_platform()
        self.running_processes: Dict[str, subprocess.Popen] = {}

    async def launch_application(
        self,
        app_path: str,
        args: Optional[List[str]] = None,
        detach: bool = False,
    ) -> bool:
        """Launch an application.

        Args:
            app_path: Path or name of application to launch
            args: Command line arguments
            detach: Launch in background

        Returns:
            True if successful
        """
        try:
            cmd = [app_path] + (args or [])
            logger.info(f"Launching application: {app_path}")

            if detach:
                if self.platform == Platform.WINDOWS:
                    subprocess.Popen(cmd, creationflags=subprocess.CREATE_NEW_CONSOLE)
                else:
                    subprocess.Popen(cmd, start_new_session=True)
            else:
                process = subprocess.Popen(cmd)
                self.running_processes[app_path] = process
                process.wait()

            return True
        except Exception as e:
            logger.error(f"Failed to launch application {app_path}: {e}")
            return False

    async def open_file(self, file_path: Path) -> bool:
        """Open file with default application.

        Args:
            file_path: Path to file

        Returns:
            True if successful
        """
        try:
            logger.info(f"Opening file: {file_path}")

            if self.platform == Platform.MACOS:
                subprocess.run(["open", str(file_path)], check=True)
            elif self.platform == Platform.WINDOWS:
                subprocess.run(["start", str(file_path)], shell=True, check=True)
            else:  # Linux
                subprocess.run(["xdg-open", str(file_path)], check=True)

            return True
        except Exception as e:
            logger.error(f"Failed to open file {file_path}: {e}")
            return False

    async def terminate_application(self, app_identifier: str) -> bool:
        """Terminate a running application.

        Args:
            app_identifier: Application identifier or PID

        Returns:
            True if successful
        """
        try:
            logger.info(f"Terminating application: {app_identifier}")

            if app_identifier in self.running_processes:
                process = self.running_processes[app_identifier]
                process.terminate()
                process.wait(timeout=5)
                del self.running_processes[app_identifier]
            else:
                # Try to kill by name
                if self.platform == Platform.WINDOWS:
                    subprocess.run(["taskkill", "/IM", app_identifier], check=False)
                else:
                    subprocess.run(["pkill", "-f", app_identifier], check=False)

            return True
        except Exception as e:
            logger.error(f"Failed to terminate application {app_identifier}: {e}")
            return False


class NotificationManager:
    """Manages system notifications."""

    def __init__(self):
        """Initialize notification manager."""
        self.platform = PlatformDetector.get_platform()

    async def send_notification(
        self,
        title: str,
        message: str,
        notification_type: NotificationType = NotificationType.INFO,
    ) -> bool:
        """Send system notification.

        Args:
            title: Notification title
            message: Notification message
            notification_type: Type of notification

        Returns:
            True if successful
        """
        try:
            logger.info(f"Sending notification: {title}")

            if self.platform == Platform.MACOS:
                script = f'display notification "{message}" with title "{title}"'
                subprocess.run(["osascript", "-e", script], check=True)
            elif self.platform == Platform.WINDOWS:
                # PowerShell notification
                script = f'[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null\n[Windows.UI.Notifications.ToastNotification, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null\n[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null\n$APP_ID = "Luminaguard"\n$template = @"\n<toast>\n<visual>\n<binding template="ToastText02">\n<text id="1">{title}</text>\n<text id="2">{message}</text>\n</binding>\n</visual>\n</toast>\n"@\n$xml = New-Object Windows.Data.Xml.Dom.XmlDocument\n$xml.LoadXml($template)\n$toast = New-Object Windows.UI.Notifications.ToastNotification $xml\n[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($APP_ID).Show($toast)'
                subprocess.run(["powershell", "-Command", script], check=False)
            else:  # Linux
                subprocess.run(["notify-send", title, message], check=True)

            return True
        except Exception as e:
            logger.error(f"Failed to send notification: {e}")
            return False


class DesktopIntegration:
    """Main desktop integration interface."""

    def __init__(self, config: Optional[DesktopConfig] = None):
        """Initialize desktop integration.

        Args:
            config: DesktopConfig instance
        """
        self.config = config or DesktopConfig()
        self.platform = PlatformDetector.get_platform()
        self.filesystem = FileSystemManager()
        self.launcher = ApplicationLauncher()
        self.notifications = NotificationManager()

    def get_system_info(self) -> Dict[str, Any]:
        """Get system information.

        Returns:
            Dictionary of system information
        """
        return {
            "platform": self.platform.value,
            "version": PlatformDetector.get_platform_version(),
            "shells": PlatformDetector.get_available_shells(),
        }
