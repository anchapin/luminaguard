//! VM Startup Optimization Module
//!
//! This module provides profiling and optimization for VM startup time.
//! Target: <100ms VM startup baseline
//!
//! # Optimization Strategies
//!
//! 1. **Pre-warmed Snapshot Pool**: Maintain ready-to-use VM snapshots
//! 2. **Lazy Initialization**: Defer non-critical initialization
//! 3. **Parallel Resource Setup**: Concurrent kernel/rootfs loading
//! 4. **Memory Pre-allocation**: Pre-allocate VM memory regions
//! 5. **Socket Optimization**: Use faster Unix socket patterns
//!
//! # Performance Targets
//!
//! - Cold boot: <200ms (current: ~110ms)
//! - Snapshot load: <50ms (target: <20ms)
//! - Pool acquire: <10ms (target: <5ms)

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Performance profile for a VM startup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupProfile {
    /// Unique profile ID
    pub id: String,

    /// Total startup time in milliseconds
    pub total_time_ms: f64,

    /// Breakdown of startup phases
    pub phases: StartupPhases,

    /// Configuration used
    pub config_summary: ConfigSummary,

    /// Whether this was a cold boot or snapshot load
    pub startup_type: StartupType,

    /// Timestamp of the profile
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Breakdown of startup phases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupPhases {
    /// Time to validate configuration
    pub config_validation_ms: f64,

    /// Time to prepare resources (kernel, rootfs)
    pub resource_preparation_ms: f64,

    /// Time to spawn Firecracker process
    pub process_spawn_ms: f64,

    /// Time to wait for socket availability
    pub socket_ready_ms: f64,

    /// Time to configure VM via API
    pub api_configuration_ms: f64,

    /// Time to start the instance
    pub instance_start_ms: f64,
}

impl StartupPhases {
    /// Create empty phases
    pub fn new() -> Self {
        Self {
            config_validation_ms: 0.0,
            resource_preparation_ms: 0.0,
            process_spawn_ms: 0.0,
            socket_ready_ms: 0.0,
            api_configuration_ms: 0.0,
            instance_start_ms: 0.0,
        }
    }

    /// Get total time from phases
    pub fn total_ms(&self) -> f64 {
        self.config_validation_ms
            + self.resource_preparation_ms
            + self.process_spawn_ms
            + self.socket_ready_ms
            + self.api_configuration_ms
            + self.instance_start_ms
    }

    /// Get the slowest phase
    pub fn slowest_phase(&self) -> (&'static str, f64) {
        let phases = [
            ("config_validation", self.config_validation_ms),
            ("resource_preparation", self.resource_preparation_ms),
            ("process_spawn", self.process_spawn_ms),
            ("socket_ready", self.socket_ready_ms),
            ("api_configuration", self.api_configuration_ms),
            ("instance_start", self.instance_start_ms),
        ];

        phases
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|&(name, time)| (name, time))
            .unwrap_or(("unknown", 0.0))
    }
}

impl Default for StartupPhases {
    fn default() -> Self {
        Self::new()
    }
}

/// Type of startup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupType {
    /// Cold boot from scratch
    ColdBoot,
    /// Load from snapshot
    SnapshotLoad,
    /// Acquire from pre-warmed pool
    PoolAcquire,
}

/// Summary of VM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    /// Number of vCPUs
    pub vcpu_count: u8,

    /// Memory in MB
    pub memory_mb: u32,

    /// Whether seccomp is enabled
    pub seccomp_enabled: bool,
}

/// Startup profiler for measuring VM startup performance
#[derive(Debug)]
pub struct StartupProfiler {
    /// Start time
    start: Instant,

    /// Current phase start time
    phase_start: Instant,

    /// Phases recorded
    phases: StartupPhases,

    /// Profile ID
    id: String,

    /// Startup type
    startup_type: StartupType,
}

impl StartupProfiler {
    /// Create a new profiler for a cold boot
    pub fn cold_boot() -> Self {
        Self::new(StartupType::ColdBoot)
    }

    /// Create a new profiler for a snapshot load
    pub fn snapshot_load() -> Self {
        Self::new(StartupType::SnapshotLoad)
    }

    /// Create a new profiler for a pool acquire
    pub fn pool_acquire() -> Self {
        Self::new(StartupType::PoolAcquire)
    }

    /// Create a new profiler
    fn new(startup_type: StartupType) -> Self {
        Self {
            start: Instant::now(),
            phase_start: Instant::now(),
            phases: StartupPhases::new(),
            id: format!("profile-{}", uuid::Uuid::new_v4()),
            startup_type,
        }
    }

    /// Record config validation phase
    pub fn record_config_validation(&mut self) {
        self.phases.config_validation_ms = self.phase_start.elapsed().as_secs_f64() * 1000.0;
        self.phase_start = Instant::now();
    }

    /// Record resource preparation phase
    pub fn record_resource_preparation(&mut self) {
        self.phases.resource_preparation_ms = self.phase_start.elapsed().as_secs_f64() * 1000.0;
        self.phase_start = Instant::now();
    }

    /// Record process spawn phase
    pub fn record_process_spawn(&mut self) {
        self.phases.process_spawn_ms = self.phase_start.elapsed().as_secs_f64() * 1000.0;
        self.phase_start = Instant::now();
    }

    /// Record socket ready phase
    pub fn record_socket_ready(&mut self) {
        self.phases.socket_ready_ms = self.phase_start.elapsed().as_secs_f64() * 1000.0;
        self.phase_start = Instant::now();
    }

    /// Record API configuration phase
    pub fn record_api_configuration(&mut self) {
        self.phases.api_configuration_ms = self.phase_start.elapsed().as_secs_f64() * 1000.0;
        self.phase_start = Instant::now();
    }

    /// Record instance start phase
    pub fn record_instance_start(&mut self) {
        self.phases.instance_start_ms = self.phase_start.elapsed().as_secs_f64() * 1000.0;
    }

    /// Finish profiling and return the profile
    pub fn finish(self, vcpu_count: u8, memory_mb: u32, seccomp_enabled: bool) -> StartupProfile {
        let total_time_ms = self.start.elapsed().as_secs_f64() * 1000.0;

        StartupProfile {
            id: self.id,
            total_time_ms,
            phases: self.phases,
            config_summary: ConfigSummary {
                vcpu_count,
                memory_mb,
                seccomp_enabled,
            },
            startup_type: self.startup_type,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Performance statistics for VM startup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupStats {
    /// Number of profiles recorded
    pub sample_count: usize,

    /// Average startup time
    pub avg_time_ms: f64,

    /// Minimum startup time
    pub min_time_ms: f64,

    /// Maximum startup time
    pub max_time_ms: f64,

    /// 95th percentile startup time
    pub p95_time_ms: f64,

    /// Average time by phase
    pub avg_phases: StartupPhases,

    /// Startup type
    pub startup_type: StartupType,
}

/// Performance tracker for aggregating startup profiles
#[derive(Debug, Clone)]
pub struct PerformanceTracker {
    /// Profiles by startup type
    profiles: Arc<RwLock<Vec<StartupProfile>>>,
}

impl PerformanceTracker {
    /// Create a new performance tracker
    pub fn new() -> Self {
        Self {
            profiles: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a startup profile
    pub async fn record(&self, profile: StartupProfile) {
        let mut profiles = self.profiles.write().await;
        profiles.push(profile);

        // Keep only last 1000 profiles
        if profiles.len() > 1000 {
            let excess = profiles.len() - 1000;
            profiles.drain(0..excess);
        }
    }

    /// Get statistics for a startup type
    pub async fn get_stats(&self, startup_type: StartupType) -> Option<StartupStats> {
        let profiles = self.profiles.read().await;

        let filtered: Vec<&StartupProfile> = profiles
            .iter()
            .filter(|p| p.startup_type == startup_type)
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let times: Vec<f64> = filtered.iter().map(|p| p.total_time_ms).collect();
        let mut sorted_times = times.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let sample_count = filtered.len();
        let avg_time_ms = times.iter().sum::<f64>() / sample_count as f64;
        let min_time_ms = sorted_times.first().copied().unwrap_or(0.0);
        let max_time_ms = sorted_times.last().copied().unwrap_or(0.0);
        let p95_index = ((sample_count as f64) * 0.95) as usize;
        let p95_time_ms = sorted_times
            .get(p95_index.min(sample_count - 1))
            .copied()
            .unwrap_or(0.0);

        // Calculate average phases
        let mut avg_phases = StartupPhases::new();
        for profile in &filtered {
            avg_phases.config_validation_ms += profile.phases.config_validation_ms;
            avg_phases.resource_preparation_ms += profile.phases.resource_preparation_ms;
            avg_phases.process_spawn_ms += profile.phases.process_spawn_ms;
            avg_phases.socket_ready_ms += profile.phases.socket_ready_ms;
            avg_phases.api_configuration_ms += profile.phases.api_configuration_ms;
            avg_phases.instance_start_ms += profile.phases.instance_start_ms;
        }

        let count = sample_count as f64;
        avg_phases.config_validation_ms /= count;
        avg_phases.resource_preparation_ms /= count;
        avg_phases.process_spawn_ms /= count;
        avg_phases.socket_ready_ms /= count;
        avg_phases.api_configuration_ms /= count;
        avg_phases.instance_start_ms /= count;

        Some(StartupStats {
            sample_count,
            avg_time_ms,
            min_time_ms,
            max_time_ms,
            p95_time_ms,
            avg_phases,
            startup_type,
        })
    }

    /// Get all profiles
    pub async fn get_profiles(&self) -> Vec<StartupProfile> {
        self.profiles.read().await.clone()
    }

    /// Clear all profiles
    pub async fn clear(&self) {
        self.profiles.write().await.clear();
    }
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Optimization recommendations based on profiling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendations {
    /// Current average startup time
    pub current_avg_ms: f64,

    /// Target startup time
    pub target_ms: f64,

    /// List of recommendations
    pub recommendations: Vec<Recommendation>,

    /// Potential improvement in milliseconds
    pub potential_improvement_ms: f64,
}

/// A single optimization recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Phase to optimize
    pub phase: String,

    /// Current time for this phase
    pub current_ms: f64,

    /// Recommended action
    pub action: String,

    /// Expected improvement
    pub expected_improvement_ms: f64,
}

/// Generate optimization recommendations from stats
pub fn generate_recommendations(stats: &StartupStats) -> OptimizationRecommendations {
    let target_ms = match stats.startup_type {
        StartupType::ColdBoot => 100.0,
        StartupType::SnapshotLoad => 20.0,
        StartupType::PoolAcquire => 5.0,
    };

    let mut recommendations = Vec::new();
    let mut potential_improvement = 0.0;

    // Analyze each phase
    if stats.avg_phases.socket_ready_ms > 20.0 {
        recommendations.push(Recommendation {
            phase: "socket_ready".to_string(),
            current_ms: stats.avg_phases.socket_ready_ms,
            action: "Use inotify or epoll for faster socket detection".to_string(),
            expected_improvement_ms: stats.avg_phases.socket_ready_ms - 10.0,
        });
        potential_improvement += stats.avg_phases.socket_ready_ms - 10.0;
    }

    if stats.avg_phases.api_configuration_ms > 30.0 {
        recommendations.push(Recommendation {
            phase: "api_configuration".to_string(),
            current_ms: stats.avg_phases.api_configuration_ms,
            action: "Batch API requests or use connection pooling".to_string(),
            expected_improvement_ms: stats.avg_phases.api_configuration_ms - 15.0,
        });
        potential_improvement += stats.avg_phases.api_configuration_ms - 15.0;
    }

    if stats.avg_phases.resource_preparation_ms > 15.0 {
        recommendations.push(Recommendation {
            phase: "resource_preparation".to_string(),
            current_ms: stats.avg_phases.resource_preparation_ms,
            action: "Pre-validate resources at startup or use cached validation".to_string(),
            expected_improvement_ms: stats.avg_phases.resource_preparation_ms - 5.0,
        });
        potential_improvement += stats.avg_phases.resource_preparation_ms - 5.0;
    }

    if stats.avg_phases.process_spawn_ms > 25.0 {
        recommendations.push(Recommendation {
            phase: "process_spawn".to_string(),
            current_ms: stats.avg_phases.process_spawn_ms,
            action: "Use process pool or pre-spawn Firecracker processes".to_string(),
            expected_improvement_ms: stats.avg_phases.process_spawn_ms - 10.0,
        });
        potential_improvement += stats.avg_phases.process_spawn_ms - 10.0;
    }

    OptimizationRecommendations {
        current_avg_ms: stats.avg_time_ms,
        target_ms,
        recommendations,
        potential_improvement_ms: potential_improvement,
    }
}

/// Global performance tracker
static TRACKER: std::sync::OnceLock<PerformanceTracker> = std::sync::OnceLock::new();

/// Get the global performance tracker
pub fn global_tracker() -> &'static PerformanceTracker {
    TRACKER.get_or_init(PerformanceTracker::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_startup_phases_total() {
        let phases = StartupPhases {
            config_validation_ms: 5.0,
            resource_preparation_ms: 10.0,
            process_spawn_ms: 20.0,
            socket_ready_ms: 30.0,
            api_configuration_ms: 25.0,
            instance_start_ms: 15.0,
        };

        assert_eq!(phases.total_ms(), 105.0);
    }

    #[test]
    fn test_startup_phases_slowest() {
        let phases = StartupPhases {
            config_validation_ms: 5.0,
            resource_preparation_ms: 10.0,
            process_spawn_ms: 20.0,
            socket_ready_ms: 30.0,
            api_configuration_ms: 25.0,
            instance_start_ms: 15.0,
        };

        let (name, time) = phases.slowest_phase();
        assert_eq!(name, "socket_ready");
        assert_eq!(time, 30.0);
    }

    #[tokio::test]
    async fn test_profiler_cold_boot() {
        let mut profiler = StartupProfiler::cold_boot();

        // Simulate phases
        tokio::time::sleep(Duration::from_millis(1)).await;
        profiler.record_config_validation();

        tokio::time::sleep(Duration::from_millis(1)).await;
        profiler.record_resource_preparation();

        tokio::time::sleep(Duration::from_millis(1)).await;
        profiler.record_process_spawn();

        tokio::time::sleep(Duration::from_millis(1)).await;
        profiler.record_socket_ready();

        tokio::time::sleep(Duration::from_millis(1)).await;
        profiler.record_api_configuration();

        tokio::time::sleep(Duration::from_millis(1)).await;
        profiler.record_instance_start();

        let profile = profiler.finish(1, 128, true);

        assert_eq!(profile.startup_type, StartupType::ColdBoot);
        assert!(profile.total_time_ms > 0.0);
        assert!(profile.phases.config_validation_ms > 0.0);
    }

    #[tokio::test]
    async fn test_performance_tracker() {
        let tracker = PerformanceTracker::new();

        // Record a profile
        let profile = StartupProfile {
            id: "test-1".to_string(),
            total_time_ms: 100.0,
            phases: StartupPhases::new(),
            config_summary: ConfigSummary {
                vcpu_count: 1,
                memory_mb: 128,
                seccomp_enabled: true,
            },
            startup_type: StartupType::ColdBoot,
            timestamp: chrono::Utc::now(),
        };

        tracker.record(profile).await;

        let stats = tracker.get_stats(StartupType::ColdBoot).await;
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.sample_count, 1);
        assert_eq!(stats.avg_time_ms, 100.0);
    }

    #[tokio::test]
    async fn test_performance_tracker_multiple_profiles() {
        let tracker = PerformanceTracker::new();

        for i in 0..10 {
            let profile = StartupProfile {
                id: format!("test-{}", i),
                total_time_ms: 100.0 + i as f64 * 10.0,
                phases: StartupPhases::new(),
                config_summary: ConfigSummary {
                    vcpu_count: 1,
                    memory_mb: 128,
                    seccomp_enabled: true,
                },
                startup_type: StartupType::ColdBoot,
                timestamp: chrono::Utc::now(),
            };
            tracker.record(profile).await;
        }

        let stats = tracker.get_stats(StartupType::ColdBoot).await.unwrap();
        assert_eq!(stats.sample_count, 10);
        assert_eq!(stats.min_time_ms, 100.0);
        assert_eq!(stats.max_time_ms, 190.0);
    }

    #[test]
    fn test_generate_recommendations() {
        let stats = StartupStats {
            sample_count: 10,
            avg_time_ms: 150.0,
            min_time_ms: 100.0,
            max_time_ms: 200.0,
            p95_time_ms: 180.0,
            avg_phases: StartupPhases {
                config_validation_ms: 5.0,
                resource_preparation_ms: 20.0,
                process_spawn_ms: 30.0,
                socket_ready_ms: 40.0,
                api_configuration_ms: 35.0,
                instance_start_ms: 20.0,
            },
            startup_type: StartupType::ColdBoot,
        };

        let recs = generate_recommendations(&stats);

        assert_eq!(recs.target_ms, 100.0);
        assert!(!recs.recommendations.is_empty());
        assert!(recs.potential_improvement_ms > 0.0);
    }

    #[test]
    fn test_global_tracker() {
        let tracker1 = global_tracker();
        let tracker2 = global_tracker();
        // Both should point to the same tracker instance
        assert!(std::ptr::eq(tracker1 as *const _, tracker2 as *const _));
    }
}
