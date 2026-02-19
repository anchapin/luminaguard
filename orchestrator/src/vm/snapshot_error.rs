// Snapshot Error Types
//
// This module provides comprehensive error handling for VM snapshot operations
// with proper error types, recovery strategies, and retry logic.
//
// Issue: #496

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// Result type alias for snapshot operations
pub type SnapshotResult<T> = Result<T, SnapshotError>;

/// Error types for snapshot operations
#[derive(Debug)]
pub enum SnapshotError {
    /// Snapshot not found at the specified path
    NotFound {
        snapshot_id: String,
        path: PathBuf,
    },

    /// Failed to create snapshot directory
    DirectoryCreationFailed {
        path: PathBuf,
        source: io::Error,
    },

    /// Failed to read snapshot file
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },

    /// Failed to write snapshot file
    WriteFailed {
        path: PathBuf,
        source: io::Error,
    },

    /// Failed to parse snapshot metadata
    MetadataParseFailed {
        path: PathBuf,
        source: serde_json::Error,
    },

    /// Failed to serialize snapshot metadata
    MetadataSerializeFailed {
        source: serde_json::Error,
    },

    /// Failed to connect to Firecracker API
    ApiConnectionFailed {
        socket_path: String,
        message: String,
    },

    /// Firecracker API returned an error
    ApiError {
        status_code: u16,
        message: String,
    },

    /// Snapshot operation timed out
    Timeout {
        operation: String,
        duration: Duration,
    },

    /// Snapshot is corrupted or invalid
    Corrupted {
        snapshot_id: String,
        reason: String,
    },

    /// Insufficient disk space for snapshot
    InsufficientSpace {
        required_bytes: u64,
        available_bytes: u64,
    },

    /// Snapshot version mismatch
    VersionMismatch {
        expected: u32,
        actual: u32,
    },

    /// Concurrent access conflict
    ConcurrentAccess {
        snapshot_id: String,
        message: String,
    },

    /// VM is not in a state suitable for snapshotting
    InvalidVmState {
        vm_id: String,
        current_state: String,
        required_state: String,
    },

    /// Retry exhausted
    RetryExhausted {
        operation: String,
        attempts: u32,
        last_error: Box<SnapshotError>,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { snapshot_id, path } => {
                write!(
                    f,
                    "Snapshot '{}' not found at path '{}'",
                    snapshot_id,
                    path.display()
                )
            }
            Self::DirectoryCreationFailed { path, source } => {
                write!(
                    f,
                    "Failed to create snapshot directory '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    f,
                    "Failed to read snapshot file '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::WriteFailed { path, source } => {
                write!(
                    f,
                    "Failed to write snapshot file '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::MetadataParseFailed { path, source } => {
                write!(
                    f,
                    "Failed to parse snapshot metadata from '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::MetadataSerializeFailed { source } => {
                write!(f, "Failed to serialize snapshot metadata: {}", source)
            }
            Self::ApiConnectionFailed { socket_path, message } => {
                write!(
                    f,
                    "Failed to connect to Firecracker API at '{}': {}",
                    socket_path, message
                )
            }
            Self::ApiError { status_code, message } => {
                write!(
                    f,
                    "Firecracker API error (status {}): {}",
                    status_code, message
                )
            }
            Self::Timeout { operation, duration } => {
                write!(
                    f,
                    "Snapshot operation '{}' timed out after {:.2}s",
                    operation,
                    duration.as_secs_f64()
                )
            }
            Self::Corrupted { snapshot_id, reason } => {
                write!(
                    f,
                    "Snapshot '{}' is corrupted: {}",
                    snapshot_id, reason
                )
            }
            Self::InsufficientSpace {
                required_bytes,
                available_bytes,
            } => {
                write!(
                    f,
                    "Insufficient disk space: required {} bytes, available {} bytes",
                    required_bytes, available_bytes
                )
            }
            Self::VersionMismatch { expected, actual } => {
                write!(
                    f,
                    "Snapshot version mismatch: expected {}, found {}",
                    expected, actual
                )
            }
            Self::ConcurrentAccess {
                snapshot_id,
                message,
            } => {
                write!(
                    f,
                    "Concurrent access conflict for snapshot '{}': {}",
                    snapshot_id, message
                )
            }
            Self::InvalidVmState {
                vm_id,
                current_state,
                required_state,
            } => {
                write!(
                    f,
                    "VM '{}' is in '{}' state, but '{}' is required for snapshotting",
                    vm_id, current_state, required_state
                )
            }
            Self::RetryExhausted {
                operation,
                attempts,
                last_error,
            } => {
                write!(
                    f,
                    "Retry exhausted for '{}' after {} attempts. Last error: {}",
                    operation, attempts, last_error
                )
            }
        }
    }
}

impl std::error::Error for SnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectoryCreationFailed { source, .. } => Some(source),
            Self::ReadFailed { source, .. } => Some(source),
            Self::WriteFailed { source, .. } => Some(source),
            Self::MetadataParseFailed { source, .. } => Some(source),
            Self::MetadataSerializeFailed { source } => Some(source),
            Self::RetryExhausted { last_error, .. } => Some(last_error.as_ref()),
            _ => None,
        }
    }
}

impl SnapshotError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ApiConnectionFailed { .. } => true,
            Self::ApiError { status_code, .. } => *status_code >= 500,
            Self::Timeout { .. } => true,
            Self::ConcurrentAccess { .. } => true,
            Self::ReadFailed { .. } => true,
            Self::WriteFailed { .. } => true,
            _ => false,
        }
    }

    /// Get recommended retry delay for this error
    pub fn retry_delay(&self) -> Duration {
        match self {
            Self::ApiConnectionFailed { .. } => Duration::from_millis(100),
            Self::ApiError { status_code, .. } if *status_code >= 500 => {
                Duration::from_millis(200)
            }
            Self::Timeout { .. } => Duration::from_millis(500),
            Self::ConcurrentAccess { .. } => Duration::from_millis(50),
            _ => Duration::from_millis(100),
        }
    }

    /// Create a not found error
    pub fn not_found(snapshot_id: impl Into<String>, path: PathBuf) -> Self {
        Self::NotFound {
            snapshot_id: snapshot_id.into(),
            path,
        }
    }

    /// Create a corrupted error
    pub fn corrupted(snapshot_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Corrupted {
            snapshot_id: snapshot_id.into(),
            reason: reason.into(),
        }
    }

    /// Create an API error
    pub fn api_error(status_code: u16, message: impl Into<String>) -> Self {
        Self::ApiError {
            status_code,
            message: message.into(),
        }
    }
}

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with custom max attempts
    pub fn with_max_attempts(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }

    /// Calculate delay for a given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32);
        let delay_ms = delay_ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(delay_ms as u64)
    }
}

/// Execute an operation with retry logic
pub async fn with_retry<T, F, Fut>(
    config: &RetryConfig,
    operation_name: &str,
    mut operation: F,
) -> SnapshotResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = SnapshotResult<T>>,
{
    let mut last_error = None;
    let mut attempt = 0;

    while attempt < config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !e.is_retryable() {
                    return Err(e);
                }

                last_error = Some(e);
                attempt += 1;

                if attempt < config.max_attempts {
                    let delay = config.delay_for_attempt(attempt - 1);
                    tracing::warn!(
                        "Snapshot operation '{}' failed (attempt {}/{}), retrying in {:.2}ms: {}",
                        operation_name,
                        attempt,
                        config.max_attempts,
                        delay.as_secs_f64() * 1000.0,
                        last_error.as_ref().unwrap()
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(SnapshotError::RetryExhausted {
        operation: operation_name.to_string(),
        attempts: config.max_attempts,
        last_error: Box::new(last_error.unwrap_or_else(|| SnapshotError::api_error(0, "Unknown error"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_error_display() {
        let err = SnapshotError::not_found("test-snapshot", PathBuf::from("/path/to/snapshot"));
        assert!(err.to_string().contains("test-snapshot"));
        assert!(err.to_string().contains("/path/to/snapshot"));
    }

    #[test]
    fn test_is_retryable() {
        assert!(SnapshotError::ApiConnectionFailed {
            socket_path: "/tmp/socket".to_string(),
            message: "Connection refused".to_string(),
        }
        .is_retryable());

        assert!(SnapshotError::Timeout {
            operation: "create".to_string(),
            duration: Duration::from_secs(5),
        }
        .is_retryable());

        assert!(!SnapshotError::NotFound {
            snapshot_id: "test".to_string(),
            path: PathBuf::from("/tmp"),
        }
        .is_retryable());
    }

    #[test]
    fn test_retry_delay() {
        let config = RetryConfig::default();

        // First retry should have initial delay
        let delay0 = config.delay_for_attempt(0);
        assert_eq!(delay0, Duration::from_millis(100));

        // Second retry should be doubled
        let delay1 = config.delay_for_attempt(1);
        assert_eq!(delay1, Duration::from_millis(200));

        // Third retry should be quadrupled
        let delay2 = config.delay_for_attempt(2);
        assert_eq!(delay2, Duration::from_millis(400));
    }

    #[test]
    fn test_retry_delay_capped() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 10.0,
        };

        // Large multiplier should hit the cap
        let delay = config.delay_for_attempt(5);
        assert!(delay <= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_with_retry_success_on_first_attempt() {
        let config = RetryConfig::default();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = with_retry(&config, "test", move || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Ok::<_, SnapshotError>(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_with_retry_success_on_second_attempt() {
        let config = RetryConfig::default();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = with_retry(&config, "test", move || {
            let attempts = attempts_clone.clone();
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    Err(SnapshotError::ApiConnectionFailed {
                        socket_path: "/tmp".to_string(),
                        message: "retry".to_string(),
                    })
                } else {
                    Ok::<_, SnapshotError>(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_with_retry_exhausted() {
        let config = RetryConfig::with_max_attempts(2);
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = with_retry(&config, "test", move || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(SnapshotError::ApiConnectionFailed {
                    socket_path: "/tmp".to_string(),
                    message: "always fails".to_string(),
                })
            }
        })
        .await;

        assert!(matches!(result, Err(SnapshotError::RetryExhausted { .. })));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_with_retry_non_retryable_error() {
        let config = RetryConfig::default();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = with_retry(&config, "test", move || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(SnapshotError::NotFound {
                    snapshot_id: "test".to_string(),
                    path: PathBuf::from("/tmp"),
                })
            }
        })
        .await;

        assert!(matches!(result, Err(SnapshotError::NotFound { .. })));
        assert_eq!(attempts.load(Ordering::SeqCst), 1); // Should not retry non-retryable errors
    }
}
