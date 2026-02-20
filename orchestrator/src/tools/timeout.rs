//! Execution Timeout Management
//!
//! This module provides timeout handling for subprocess execution.

use std::time::Duration;
use tokio::time;

/// Execution timeout configuration
///
/// Timeouts are enforced to prevent commands from hanging indefinitely.
#[derive(Debug, Clone)]
pub struct ExecutionTimeout {
    /// The timeout duration
    duration: Duration,
}

impl Default for ExecutionTimeout {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

impl ExecutionTimeout {
    /// Create a new execution timeout
    ///
    /// # Arguments
    ///
    /// * `duration` - The timeout duration
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    /// use luminaguard_orchestrator::tools::ExecutionTimeout;
    ///
    /// let timeout = ExecutionTimeout::new(Duration::from_secs(30));
    /// ```
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }

    /// Get the timeout duration
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Create a timeout for long-running operations (5 minutes)
    pub fn long() -> Self {
        Self::new(Duration::from_secs(300))
    }

    /// Create a timeout for medium operations (1 minute, default)
    pub fn medium() -> Self {
        Self::default()
    }

    /// Create a timeout for quick operations (10 seconds)
    pub fn short() -> Self {
        Self::new(Duration::from_secs(10))
    }

    /// Create a timeout from seconds
    pub fn from_secs(secs: u64) -> Self {
        Self::new(Duration::from_secs(secs))
    }

    /// Execute a future with a timeout
    ///
    /// # Arguments
    ///
    /// * `future` - The future to execute
    ///
    /// # Returns
    ///
    /// Returns the result of the future if it completes before the timeout,
    /// or an error if the timeout expires.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use luminaguard_orchestrator::tools::ExecutionTimeout;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let timeout = ExecutionTimeout::short();
    ///     let result = timeout.run(async {
    ///         // Do some work
    ///         Ok::<(), anyhow::Error>(())
    ///     }).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn run<F, T>(&self, future: F) -> anyhow::Result<T>
    where
        F: std::future::Future<Output = anyhow::Result<T>>,
    {
        match time::timeout(self.duration, future).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "Command execution timed out after {:?}",
                self.duration
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_default() {
        let timeout = ExecutionTimeout::default();
        assert_eq!(timeout.duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_timeout_new() {
        let timeout = ExecutionTimeout::new(Duration::from_secs(30));
        assert_eq!(timeout.duration(), Duration::from_secs(30));
    }

    #[test]
    fn test_timeout_long() {
        let timeout = ExecutionTimeout::long();
        assert_eq!(timeout.duration(), Duration::from_secs(300));
    }

    #[test]
    fn test_timeout_medium() {
        let timeout = ExecutionTimeout::medium();
        assert_eq!(timeout.duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_timeout_short() {
        let timeout = ExecutionTimeout::short();
        assert_eq!(timeout.duration(), Duration::from_secs(10));
    }

    #[test]
    fn test_timeout_from_secs() {
        let timeout = ExecutionTimeout::from_secs(45);
        assert_eq!(timeout.duration(), Duration::from_secs(45));
    }

    #[tokio::test]
    async fn test_timeout_run_success() {
        let timeout = ExecutionTimeout::short();

        let result = timeout
            .run(async {
                // Immediate success
                Ok::<(), anyhow::Error>(())
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeout_run_timeout() {
        let timeout = ExecutionTimeout::from_secs(1);

        let result = timeout
            .run(async {
                // Sleep longer than timeout
                tokio::time::sleep(Duration::from_secs(2)).await;
                Ok::<(), anyhow::Error>(())
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("timed out"));
    }

    #[tokio::test]
    async fn test_timeout_run_future_error() {
        let timeout = ExecutionTimeout::short();

        let result = timeout
            .run(async {
                // Future returns an error
                Err::<(), anyhow::Error>(anyhow::anyhow!("Test error"))
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Test error"));
    }

    #[tokio::test]
    async fn test_timeout_run_just_in_time() {
        let timeout = ExecutionTimeout::from_secs(1);

        let result = timeout
            .run(async {
                // Sleep just under the timeout
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok::<(), anyhow::Error>(())
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeout_run_returns_value() {
        let timeout = ExecutionTimeout::short();

        let result = timeout
            .run(async {
                // Return a value
                Ok::<String, anyhow::Error>("test value".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test value");
    }
}
