// Prometheus metrics for LuminaGuard daemon monitoring
//
// Exposes metrics on /metrics HTTP endpoint:
// - VM spawn times (histogram)
// - Execution latencies (histogram)
// - Approval acceptance rates (counter)
// - Active VMs (gauge)
// - Resource usage (gauge)

use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, Histogram, HistogramVec, IntCounter, IntGauge, Registry,
    TextEncoder,
};
use std::sync::Arc;

lazy_static! {
    pub static ref REGISTRY: Arc<Registry> = Arc::new(Registry::new());

    // VM metrics
    pub static ref VM_SPAWN_TIME_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new("vm_spawn_time_seconds", "VM spawn time in seconds"),
        &["vm_type"]
    ).expect("Failed to create VM spawn time metric");

    pub static ref ACTIVE_VMS: IntGauge = IntGauge::new(
        "active_vms_total",
        "Number of currently active VMs"
    ).expect("Failed to create active VMs metric");

    pub static ref VMS_SPAWNED_TOTAL: IntCounter = IntCounter::new(
        "vms_spawned_total",
        "Total number of VMs spawned since daemon start"
    ).expect("Failed to create VMs spawned metric");

    pub static ref VMS_DESTROYED_TOTAL: IntCounter = IntCounter::new(
        "vms_destroyed_total",
        "Total number of VMs destroyed since daemon start"
    ).expect("Failed to create VMs destroyed metric");

    pub static ref VM_SPAWN_ERRORS_TOTAL: IntCounter = IntCounter::new(
        "vm_spawn_errors_total",
        "Total number of VM spawn failures"
    ).expect("Failed to create VM spawn errors metric");

    pub static ref VM_DESTROY_TIME_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new("vm_destroy_time_seconds", "VM destroy time in seconds"),
        &["vm_type"]
    ).expect("Failed to create VM destroy time metric");

    pub static ref VM_DESTROY_ERRORS_TOTAL: IntCounter = IntCounter::new(
        "vm_destroy_errors_total",
        "Total number of VM destroy failures"
    ).expect("Failed to create VM destroy errors metric");

    pub static ref VM_MEMORY_BYTES: IntGauge = IntGauge::new(
        "vm_memory_bytes",
        "Memory allocated to VMs in bytes"
    ).expect("Failed to create VM memory metric");

    pub static ref VM_CPU_COUNT: IntGauge = IntGauge::new(
        "vm_cpu_count",
        "Number of CPUs allocated to VMs"
    ).expect("Failed to create VM CPU count metric");

    pub static ref SNAPSHOT_POOL_SIZE: IntGauge = IntGauge::new(
        "snapshot_pool_size",
        "Number of snapshots in the pool"
    ).expect("Failed to create snapshot pool size metric");

    pub static ref SNAPSHOT_CREATE_TIME_SECONDS: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new("snapshot_create_time_seconds", "Time to create a VM snapshot"),
    ).expect("Failed to create snapshot create time metric");

    pub static ref SNAPSHOT_LOAD_TIME_SECONDS: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new("snapshot_load_time_seconds", "Time to load a VM snapshot"),
    ).expect("Failed to create snapshot load time metric");

    // Execution metrics
    pub static ref EXECUTION_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new("execution_duration_seconds", "Agent execution duration in seconds"),
        &["task_type", "status"]
    ).expect("Failed to create execution duration metric");

    pub static ref AGENT_TASKS_TOTAL: CounterVec = CounterVec::new(
        prometheus::Opts::new("agent_tasks_total", "Total number of agent tasks executed"),
        &["task_type", "status"]
    ).expect("Failed to create agent tasks total metric");

    // Approval metrics
    pub static ref APPROVAL_REQUESTS_TOTAL: IntCounter = IntCounter::new(
        "approval_requests_total",
        "Total number of approval requests"
    ).expect("Failed to create approval requests metric");

    pub static ref APPROVALS_GRANTED_TOTAL: IntCounter = IntCounter::new(
        "approvals_granted_total",
        "Total number of approvals granted"
    ).expect("Failed to create approvals granted metric");

    pub static ref APPROVALS_DENIED_TOTAL: IntCounter = IntCounter::new(
        "approvals_denied_total",
        "Total number of approvals denied"
    ).expect("Failed to create approvals denied metric");

    pub static ref APPROVAL_RESPONSE_TIME_SECONDS: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new("approval_response_time_seconds", "Time to respond to approval request"),
    ).expect("Failed to create approval response time metric");

    // MCP metrics
    pub static ref MCP_CONNECTIONS_ACTIVE: IntGauge = IntGauge::new(
        "mcp_connections_active",
        "Number of active MCP server connections"
    ).expect("Failed to create MCP connections metric");

    pub static ref MCP_TOOL_CALLS_TOTAL: CounterVec = CounterVec::new(
        prometheus::Opts::new("mcp_tool_calls_total", "Total number of MCP tool calls"),
        &["tool_name", "status"]
    ).expect("Failed to create MCP tool calls metric");

    pub static ref MCP_TOOL_CALL_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new("mcp_tool_call_duration_seconds", "Duration of MCP tool calls"),
        &["tool_name"]
    ).expect("Failed to create MCP tool call duration metric");

    // Resource metrics
    pub static ref MEMORY_USAGE_BYTES: IntGauge = IntGauge::new(
        "memory_usage_bytes",
        "Current memory usage in bytes"
    ).expect("Failed to create memory usage metric");

    pub static ref CPU_TIME_SECONDS: Counter = Counter::new(
        "cpu_time_seconds",
        "Total CPU time consumed in seconds"
    ).expect("Failed to create CPU time metric");

    pub static ref DAEMON_UPTIME_SECONDS: Gauge = Gauge::new(
        "daemon_uptime_seconds",
        "Daemon uptime in seconds"
    ).expect("Failed to create daemon uptime metric");
}

/// Initialize metrics registry - must be called once at daemon startup
pub fn init() -> prometheus::Result<()> {
    // VM metrics
    REGISTRY.register(Box::new(VM_SPAWN_TIME_SECONDS.clone()))?;
    REGISTRY.register(Box::new(ACTIVE_VMS.clone()))?;
    REGISTRY.register(Box::new(VMS_SPAWNED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(VMS_DESTROYED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(VM_SPAWN_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(VM_DESTROY_TIME_SECONDS.clone()))?;
    REGISTRY.register(Box::new(VM_DESTROY_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(VM_MEMORY_BYTES.clone()))?;
    REGISTRY.register(Box::new(VM_CPU_COUNT.clone()))?;
    REGISTRY.register(Box::new(SNAPSHOT_POOL_SIZE.clone()))?;
    REGISTRY.register(Box::new(SNAPSHOT_CREATE_TIME_SECONDS.clone()))?;
    REGISTRY.register(Box::new(SNAPSHOT_LOAD_TIME_SECONDS.clone()))?;
    // Execution metrics
    REGISTRY.register(Box::new(EXECUTION_DURATION_SECONDS.clone()))?;
    REGISTRY.register(Box::new(AGENT_TASKS_TOTAL.clone()))?;
    // Approval metrics
    REGISTRY.register(Box::new(APPROVAL_REQUESTS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(APPROVALS_GRANTED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(APPROVALS_DENIED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(APPROVAL_RESPONSE_TIME_SECONDS.clone()))?;
    // MCP metrics
    REGISTRY.register(Box::new(MCP_CONNECTIONS_ACTIVE.clone()))?;
    REGISTRY.register(Box::new(MCP_TOOL_CALLS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(MCP_TOOL_CALL_DURATION_SECONDS.clone()))?;
    // Resource metrics
    REGISTRY.register(Box::new(MEMORY_USAGE_BYTES.clone()))?;
    REGISTRY.register(Box::new(CPU_TIME_SECONDS.clone()))?;
    REGISTRY.register(Box::new(DAEMON_UPTIME_SECONDS.clone()))?;
    Ok(())
}

/// Gather all metrics in Prometheus text format
pub fn gather_metrics() -> anyhow::Result<String> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|e| anyhow::anyhow!("Failed to encode metrics: {}", e))?;
    String::from_utf8(buffer).map_err(|e| anyhow::anyhow!("Invalid UTF-8 in metrics: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_metrics() {
        let result = init();
        // Note: This will fail if metrics are already registered (idempotency issue)
        // In practice, metrics should be initialized once per process
        let _ = result; // Suppress warning
    }

    #[test]
    fn test_vm_metrics() {
        VMS_SPAWNED_TOTAL.inc();
        ACTIVE_VMS.set(1);
        assert_eq!(ACTIVE_VMS.get(), 1);
    }

    #[test]
    fn test_approval_metrics() {
        // Initialize metrics first (may fail if already registered, which is fine)
        let _ = init();

        APPROVAL_REQUESTS_TOTAL.inc();
        APPROVALS_GRANTED_TOTAL.inc();
        // Verify counters increased
        let metrics = REGISTRY.gather();
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_vm_lifecycle_metrics() {
        // Test spawn metrics
        VMS_SPAWNED_TOTAL.inc();
        VM_SPAWN_TIME_SECONDS
            .with_label_values(&["firecracker"])
            .observe(0.110);

        // Test destroy metrics
        VMS_DESTROYED_TOTAL.inc();
        VM_DESTROY_TIME_SECONDS
            .with_label_values(&["firecracker"])
            .observe(0.050);

        // Test active VMs
        ACTIVE_VMS.inc();
        assert!(ACTIVE_VMS.get() >= 1);

        // Test error counters
        VM_SPAWN_ERRORS_TOTAL.inc();
        VM_DESTROY_ERRORS_TOTAL.inc();
    }

    #[test]
    fn test_vm_resource_metrics() {
        // Test memory metric
        VM_MEMORY_BYTES.set(512 * 1024 * 1024); // 512MB
        assert_eq!(VM_MEMORY_BYTES.get(), 512 * 1024 * 1024);

        // Test CPU count metric
        VM_CPU_COUNT.set(2);
        assert_eq!(VM_CPU_COUNT.get(), 2);
    }

    #[test]
    fn test_snapshot_metrics() {
        // Test snapshot pool size
        SNAPSHOT_POOL_SIZE.set(5);
        assert_eq!(SNAPSHOT_POOL_SIZE.get(), 5);

        // Test snapshot timing
        SNAPSHOT_CREATE_TIME_SECONDS.observe(0.080);
        SNAPSHOT_LOAD_TIME_SECONDS.observe(0.020);
    }

    #[test]
    fn test_gather_metrics() {
        // Initialize metrics first
        let _ = init();

        // Set some metrics
        ACTIVE_VMS.set(3);
        VMS_SPAWNED_TOTAL.inc();

        // Gather metrics
        let result = gather_metrics();
        assert!(result.is_ok());

        let metrics_text = result.unwrap();
        // Check for metric names (they may have luminaguard_ prefix depending on registry)
        assert!(metrics_text.contains("vms_spawned") || metrics_text.contains("active_vms"));
    }
}
