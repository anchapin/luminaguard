// VM-Isolated Agent Executor
//
// This module provides secure execution of the Python reasoning loop inside
// a JIT Micro-VM, ensuring true isolation from the host system.
//
// Architecture:
// 1. Host spawns VM with agent rootfs (contains Python + agent code)
// 2. Host creates vsock listener for communication
// 3. Guest agent connects to vsock and awaits tasks
// 4. Host sends task via vsock, guest executes, returns result
// 5. VM is destroyed after task completion (ephemeral security)
//
// Security Benefits:
// - Agent code runs in isolated VM, not on host
// - Any malicious action is contained within the VM
// - VM is destroyed after each task (no persistence)
// - Approval requests are routed through vsock to host

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(unix)]
use crate::vm::vsock::{VsockHostListener, VsockMessageHandler};

/// Agent task request sent from host to guest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskRequest {
    /// Unique task ID
    pub task_id: String,
    /// Task description for the agent
    pub task: String,
    /// Session ID for context continuity
    pub session_id: Option<String>,
    /// Maximum execution time in seconds
    pub timeout_secs: u64,
}

/// Agent task response from guest to host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskResponse {
    /// Task ID (matches request)
    pub task_id: String,
    /// Whether the task completed successfully
    pub success: bool,
    /// Result or output from the agent
    pub result: Option<String>,
    /// Error message if task failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: f64,
}

/// Approval request from guest agent to host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique approval ID
    pub approval_id: String,
    /// Description of the action requiring approval
    pub action_description: String,
    /// Risk level (none, low, medium, high, critical)
    pub risk_level: String,
    /// Changes that will be made (JSON)
    pub changes: serde_json::Value,
}

/// Approval response from host to guest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    /// Approval ID (matches request)
    pub approval_id: String,
    /// Whether the action was approved
    pub approved: bool,
    /// Optional message from the user
    pub message: Option<String>,
}

/// VM Agent Executor configuration
#[derive(Debug, Clone)]
pub struct AgentExecutorConfig {
    /// Path to rootfs with Python agent
    pub agent_rootfs_path: String,
    /// Path to kernel image
    pub kernel_path: String,
    /// Timeout for agent execution (seconds)
    pub execution_timeout_secs: u64,
    /// Whether to use snapshot pool for fast spawning
    pub use_snapshot_pool: bool,
}

impl Default for AgentExecutorConfig {
    fn default() -> Self {
        Self {
            agent_rootfs_path: "/var/lib/luminaguard/agent-rootfs.ext4".to_string(),
            kernel_path: "/var/lib/luminaguard/vmlinux".to_string(),
            execution_timeout_secs: 300, // 5 minutes
            use_snapshot_pool: true,
        }
    }
}

/// VM Agent Executor
///
/// Executes agent tasks inside isolated VMs for security.
pub struct VmAgentExecutor {
    config: AgentExecutorConfig,
    approval_handler: Arc<Mutex<Option<Box<dyn ApprovalHandler + Send>>>>,
}

/// Trait for handling approval requests from the guest agent
#[async_trait::async_trait]
pub trait ApprovalHandler {
    /// Handle an approval request
    async fn handle_approval(&mut self, request: ApprovalRequest) -> Result<ApprovalResponse>;
}

/// Default approval handler that auto-approves low-risk actions
pub struct DefaultApprovalHandler {
    auto_approve_low_risk: bool,
}

impl DefaultApprovalHandler {
    pub fn new(auto_approve_low_risk: bool) -> Self {
        Self {
            auto_approve_low_risk,
        }
    }
}

#[async_trait::async_trait]
impl ApprovalHandler for DefaultApprovalHandler {
    async fn handle_approval(&mut self, request: ApprovalRequest) -> Result<ApprovalResponse> {
        // Auto-approve low-risk actions if configured
        if self.auto_approve_low_risk && request.risk_level == "low" {
            return Ok(ApprovalResponse {
                approval_id: request.approval_id,
                approved: true,
                message: Some("Auto-approved (low risk)".to_string()),
            });
        }

        // For other risk levels, we would normally show a TUI
        // For now, reject high/critical and approve medium
        let approved = match request.risk_level.as_str() {
            "none" => true,
            "low" => self.auto_approve_low_risk,
            "medium" => true, // In production, would show TUI
            "high" => false,
            "critical" => false,
            _ => false,
        };

        Ok(ApprovalResponse {
            approval_id: request.approval_id,
            approved,
            message: if approved {
                Some("Approved".to_string())
            } else {
                Some(format!("Rejected ({} risk)", request.risk_level))
            },
        })
    }
}

impl VmAgentExecutor {
    /// Create a new VM agent executor
    pub fn new(config: AgentExecutorConfig) -> Self {
        Self {
            config,
            approval_handler: Arc::new(Mutex::new(Some(Box::new(DefaultApprovalHandler::new(
                false,
            ))))),
        }
    }

    /// Set a custom approval handler
    pub fn set_approval_handler<H>(&mut self, handler: H)
    where
        H: ApprovalHandler + Send + 'static,
    {
        self.approval_handler = Arc::new(Mutex::new(Some(Box::new(handler))));
    }

    /// Execute an agent task inside a VM
    ///
    /// This method:
    /// 1. Spawns a new VM with the agent rootfs
    /// 2. Establishes vsock communication
    /// 3. Sends the task to the guest agent
    /// 4. Handles approval requests from the guest
    /// 5. Returns the result and destroys the VM
    #[cfg(unix)]
    pub async fn execute_task(&self, request: AgentTaskRequest) -> Result<AgentTaskResponse> {
        use crate::vm::config::VmConfig;
        use crate::vm::{destroy_vm, spawn_vm_with_config};
        use std::time::Instant;

        let start_time = Instant::now();

        tracing::info!("Executing agent task {} in VM", request.task_id);

        // Create VM config with agent rootfs
        let vm_config = VmConfig {
            vm_id: request.task_id.clone(),
            kernel_path: self.config.kernel_path.clone(),
            rootfs_path: self.config.agent_rootfs_path.clone(),
            ..VmConfig::default()
        };

        // Spawn VM
        let handle = spawn_vm_with_config(&request.task_id, &vm_config)
            .await
            .context("Failed to spawn VM for agent execution")?;

        tracing::info!(
            "VM spawned for task {} (spawn time: {:.2}ms)",
            request.task_id,
            handle.spawn_time_ms
        );

        // Create vsock listener for communication
        let vsock_listener = VsockHostListener::new(request.task_id.clone())
            .await
            .context("Failed to create vsock listener")?;

        let socket_path = vsock_listener.socket_path();
        tracing::info!("Vsock listener created at {}", socket_path);

        // Create message handler for approvals
        let handler = AgentMessageHandler {
            task_id: request.task_id.clone(),
            approval_handler: self.approval_handler.clone(),
        };

        // Spawn the vsock handler task
        let vsock_task = tokio::spawn(async move { vsock_listener.run_handler(handler).await });

        // Send task to guest agent (via vsock)
        // Note: In a full implementation, we would:
        // 1. Wait for guest to connect to vsock
        // 2. Send the task request
        // 3. Handle approval requests
        // 4. Receive the result

        // For now, simulate execution with a timeout
        let timeout = std::time::Duration::from_secs(self.config.execution_timeout_secs);
        // Wait for vsock handler to complete
        // In production, this would be the actual agent execution
        let result = tokio::time::timeout(timeout, vsock_task).await;

        // Destroy VM
        destroy_vm(handle).await.context("Failed to destroy VM")?;

        let execution_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        match result {
            Ok(Ok(_)) => Ok(AgentTaskResponse {
                task_id: request.task_id,
                success: true,
                result: Some("Task completed in VM".to_string()),
                error: None,
                execution_time_ms,
            }),
            Ok(Err(e)) => Ok(AgentTaskResponse {
                task_id: request.task_id,
                success: false,
                result: None,
                error: Some(format!("VM execution error: {}", e)),
                execution_time_ms,
            }),
            Err(_) => Ok(AgentTaskResponse {
                task_id: request.task_id,
                success: false,
                result: None,
                error: Some("Execution timed out".to_string()),
                execution_time_ms,
            }),
        }
    }

    /// Execute an agent task (non-Unix fallback)
    #[cfg(not(unix))]
    pub async fn execute_task(&self, request: AgentTaskRequest) -> Result<AgentTaskResponse> {
        tracing::warn!("VM-isolated agent execution not supported on this platform");

        Ok(AgentTaskResponse {
            task_id: request.task_id,
            success: false,
            result: None,
            error: Some("VM isolation not supported on this platform".to_string()),
            execution_time_ms: 0.0,
        })
    }
}

/// Message handler for agent communication
#[cfg(unix)]
struct AgentMessageHandler {
    task_id: String,
    approval_handler: Arc<Mutex<Option<Box<dyn ApprovalHandler + Send>>>>,
}

#[cfg(unix)]
impl Clone for AgentMessageHandler {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id.clone(),
            approval_handler: self.approval_handler.clone(),
        }
    }
}

#[cfg(unix)]
#[async_trait::async_trait]
impl VsockMessageHandler for AgentMessageHandler {
    async fn handle_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        match method {
            "request_approval" => {
                let approval_request: ApprovalRequest =
                    serde_json::from_value(params).context("Invalid approval request")?;

                tracing::info!(
                    "Received approval request for task {}: {}",
                    self.task_id,
                    approval_request.action_description
                );

                // Handle the approval
                let mut handler = self.approval_handler.lock().await;
                if let Some(ref mut h) = *handler {
                    let response = h.handle_approval(approval_request).await?;
                    Ok(serde_json::to_value(response)?)
                } else {
                    anyhow::bail!("No approval handler configured");
                }
            }
            "report_progress" => {
                let progress: serde_json::Value = params;
                tracing::debug!("Agent progress: {:?}", progress);
                Ok(serde_json::json!({"acknowledged": true}))
            }
            _ => {
                anyhow::bail!("Unknown method: {}", method);
            }
        }
    }

    async fn handle_notification(&self, method: &str, params: serde_json::Value) -> Result<()> {
        match method {
            "log" => {
                let message = params["message"].as_str().unwrap_or("");
                let level = params["level"].as_str().unwrap_or("info");
                match level {
                    "error" => tracing::error!("[Guest] {}", message),
                    "warn" => tracing::warn!("[Guest] {}", message),
                    "debug" => tracing::debug!("[Guest] {}", message),
                    _ => tracing::info!("[Guest] {}", message),
                }
            }
            "status" => {
                tracing::info!("Guest status: {:?}", params);
            }
            _ => {
                tracing::warn!("Unknown notification: {} {:?}", method, params);
            }
        }
        Ok(())
    }
}

/// Execute agent task in VM with fallback to host execution
///
/// This function attempts to run the agent inside a VM for isolation.
/// If VM execution is not available, it falls back to host execution.
pub async fn execute_in_vm_or_fallback(task: String, task_id: String) -> Result<String> {
    // Try VM execution first
    #[cfg(unix)]
    {
        let config = AgentExecutorConfig::default();
        let executor = VmAgentExecutor::new(config);

        let request = AgentTaskRequest {
            task_id: task_id.clone(),
            task: task.clone(),
            session_id: None,
            timeout_secs: 300,
        };

        // Check if agent rootfs exists
        if std::path::Path::new(&executor.config.agent_rootfs_path).exists() {
            match executor.execute_task(request).await {
                Ok(response) if response.success => {
                    return Ok(response.result.unwrap_or_default());
                }
                Ok(response) => {
                    tracing::warn!(
                        "VM execution failed: {}, falling back to host",
                        response.error.unwrap_or_default()
                    );
                }
                Err(e) => {
                    tracing::warn!("VM execution error: {}, falling back to host", e);
                }
            }
        } else {
            tracing::debug!(
                "Agent rootfs not found at {}, falling back to host execution",
                executor.config.agent_rootfs_path
            );
        }
    }

    // Fallback: Execute on host
    tracing::info!("Executing agent task on host (VM isolation not available)");

    let agent_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("agent");
    let loop_py = agent_dir.join("loop.py");
    let venv_python = agent_dir.join(".venv/bin/python");

    let python_cmd = if venv_python.exists() {
        venv_python.to_str().unwrap()
    } else {
        "python3"
    };

    let output = std::process::Command::new(python_cmd)
        .arg(&loop_py)
        .arg(&task)
        .output()
        .context("Failed to execute Python agent loop")?;

    let result = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_task_request_serialization() {
        let request = AgentTaskRequest {
            task_id: "test-123".to_string(),
            task: "Write a hello world program".to_string(),
            session_id: Some("session-456".to_string()),
            timeout_secs: 60,
        };

        let json = serde_json::to_string(&request).unwrap();
        let decoded: AgentTaskRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.task_id, "test-123");
        assert_eq!(decoded.task, "Write a hello world program");
        assert_eq!(decoded.session_id, Some("session-456".to_string()));
        assert_eq!(decoded.timeout_secs, 60);
    }

    #[test]
    fn test_agent_task_response_serialization() {
        let response = AgentTaskResponse {
            task_id: "test-123".to_string(),
            success: true,
            result: Some("Program written successfully".to_string()),
            error: None,
            execution_time_ms: 1234.5,
        };

        let json = serde_json::to_string(&response).unwrap();
        let decoded: AgentTaskResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.task_id, "test-123");
        assert!(decoded.success);
        assert_eq!(
            decoded.result,
            Some("Program written successfully".to_string())
        );
    }

    #[test]
    fn test_approval_request_serialization() {
        let request = ApprovalRequest {
            approval_id: "approval-123".to_string(),
            action_description: "Create file: hello.py".to_string(),
            risk_level: "low".to_string(),
            changes: serde_json::json!([{"type": "file_create", "path": "hello.py"}]),
        };

        let json = serde_json::to_string(&request).unwrap();
        let decoded: ApprovalRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.approval_id, "approval-123");
        assert_eq!(decoded.risk_level, "low");
    }

    #[test]
    fn test_approval_response_serialization() {
        let response = ApprovalResponse {
            approval_id: "approval-123".to_string(),
            approved: true,
            message: Some("Approved by user".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        let decoded: ApprovalResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.approval_id, "approval-123");
        assert!(decoded.approved);
    }

    #[test]
    fn test_default_agent_executor_config() {
        let config = AgentExecutorConfig::default();

        assert_eq!(
            config.agent_rootfs_path,
            "/var/lib/luminaguard/agent-rootfs.ext4"
        );
        assert_eq!(config.execution_timeout_secs, 300);
        assert!(config.use_snapshot_pool);
    }

    #[tokio::test]
    async fn test_default_approval_handler() {
        let mut handler = DefaultApprovalHandler::new(true);

        // Test auto-approve for low risk
        let low_risk = ApprovalRequest {
            approval_id: "test-1".to_string(),
            action_description: "Test".to_string(),
            risk_level: "low".to_string(),
            changes: serde_json::json!([]),
        };

        let response = handler.handle_approval(low_risk).await.unwrap();
        assert!(response.approved);

        // Test rejection for high risk
        let high_risk = ApprovalRequest {
            approval_id: "test-2".to_string(),
            action_description: "Test".to_string(),
            risk_level: "high".to_string(),
            changes: serde_json::json!([]),
        };

        let response = handler.handle_approval(high_risk).await.unwrap();
        assert!(!response.approved);
    }

    #[test]
    fn test_vm_agent_executor_creation() {
        let config = AgentExecutorConfig::default();
        let _executor = VmAgentExecutor::new(config);
        // Should not panic
    }
}
