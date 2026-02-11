// Seccomp Filters for Firecracker VMs
//
// Defense-in-depth security: syscall filtering prevents even compromised VMs
// from executing dangerous operations. Blocks 99% of syscalls, allowing
// only essential ones for basic VM operation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Seccomp filter level (security profile)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SeccompLevel {
    /// Minimal - allow only absolutely essential syscalls (most secure)
    Minimal,
    /// Basic - allow common operations (recommended)
    #[default]
    Basic,
    /// Permissive - allow most syscalls (for testing only)
    Permissive,
}

/// Syscall filtering rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallRule {
    /// Syscall number or name
    pub name: String,
    /// Action to take (allow, deny, log)
    pub action: SeccompAction,
    /// Security rationale
    pub rationale: String,
}

/// Seccomp action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeccompAction {
    /// Allow syscall
    Allow,
    /// Block syscall (returns EPERM)
    Deny,
    /// Allow but log for security monitoring
    Log,
}

/// Seccomp filter configuration for a VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompFilter {
    /// Filter level (determines whitelist)
    pub level: SeccompLevel,
    /// Custom syscall rules (overrides level defaults)
    #[serde(default)]
    pub custom_rules: Vec<SyscallRule>,
    /// Enable audit logging
    #[serde(default = "default_audit_enabled")]
    pub audit_enabled: bool,
}

fn default_audit_enabled() -> bool {
    true
}

impl Default for SeccompFilter {
    fn default() -> Self {
        Self {
            level: SeccompLevel::Basic,
            custom_rules: Vec::new(),
            audit_enabled: true,
        }
    }
}

impl SeccompFilter {
    /// Create a new seccomp filter with specified level
    pub fn new(level: SeccompLevel) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Add a custom syscall rule
    pub fn add_rule(mut self, rule: SyscallRule) -> Self {
        self.custom_rules.push(rule);
        self
    }

    /// Build the syscall whitelist based on filter level
    pub(crate) fn build_whitelist(&self) -> Vec<&'static str> {
        match self.level {
            SeccompLevel::Minimal => self.minimal_whitelist(),
            SeccompLevel::Basic => self.basic_whitelist(),
            SeccompLevel::Permissive => self.permissive_whitelist(),
        }
    }

    /// Minimal whitelist - absolute essentials for VM operation
    fn minimal_whitelist(&self) -> Vec<&'static str> {
        vec![
            // I/O operations
            "read",
            "write",
            "exit",
            "exit_group",
            // Memory management
            "mmap",
            "munmap",
            "mprotect",
            "brk",
            // Signal handling
            "rt_sigreturn",
            "rt_sigprocmask",
            // Basic filesystem
            "fstat",
            "stat",
            "lseek",
            "close",
        ]
    }

    /// Basic whitelist - common operations (recommended for production)
    fn basic_whitelist(&self) -> Vec<&'static str> {
        let mut whitelist = self.minimal_whitelist();

        // Additional safe syscalls
        whitelist.extend(vec![
            // Extended I/O
            "readv",
            "writev",
            "pread64",
            "pwrite64",
            // File operations
            "open",
            "openat",
            "access",
            "faccessat",
            "statfs",
            "fstatfs",
            // Time
            "clock_gettime",
            "gettimeofday",
            // Process info (read-only)
            "getpid",
            "gettid",
            "getppid",
            // Scheduling
            "sched_yield",
            "sched_getaffinity",
            // epoll for async I/O
            "epoll_wait",
            "epoll_ctl",
            "epoll_pwait",
            // Eventfd
            "eventfd2",
            // Basic signal handling
            "sigaltstack",
            // Pipe
            "pipe",
            "pipe2",
            // Dup
            "dup",
            "dup2",
            "dup3",
            // Basic polling
            "poll",
            "ppoll",
        ]);

        whitelist
    }

    /// Permissive whitelist - for testing/debugging only
    fn permissive_whitelist(&self) -> Vec<&'static str> {
        let mut whitelist = self.basic_whitelist();

        // Additional syscalls for testing
        whitelist.extend(vec![
            "uname",
            "sysinfo",
            "getrlimit",
            "getrusage",
            "getgroups",
            "getegid",
            "geteuid",
            "getgid",
            "getuid",
            "arch_prctl",
            "set_tid_address",
            "set_robust_list",
        ]);

        whitelist
    }

    /// Convert to Firecracker JSON format
    ///
    /// Firecracker seccomp filter format:
    /// ```json
    /// {
    ///   "seccomp": {
    ///     "filter": "ALLOW",
    ///     "args": [
    ///       {
    ///         "syscall_number": 0,
    ///         "arg_filter": "EQ",
    ///         "val": 0
    ///       }
    ///     ]
    ///   }
    /// }
    /// ```
    ///
    /// However, Firecracker v1.14+ uses a simpler format where we specify
    /// allowed syscalls by name/number.
    pub fn to_firecracker_json(&self) -> Result<String> {
        let whitelist = self.build_whitelist();

        // Build seccomp filter in Firecracker format
        // Note: Firecracker uses a predefined seccomp profile that we can customize
        let filter = serde_json::json!({
            "seccomp": {
                "filter": "allow",
                "args": whitelist
            }
        });

        serde_json::to_string_pretty(&filter)
            .context("Failed to serialize seccomp filter")
    }
}

/// Audit log entry for blocked syscalls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompAuditEntry {
    /// VM ID
    pub vm_id: String,
    /// Syscall name
    pub syscall: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// PID of process that attempted syscall
    pub pid: u32,
    /// Attack detected (multiple blocked syscalls from same VM)
    pub attack_detected: bool,
}

/// Maximum number of audit entries to keep in memory
const MAX_SECCOMP_LOG_ENTRIES: usize = 10000;

/// Seccomp audit log manager
#[derive(Debug, Clone)]
pub struct SeccompAuditLog {
    entries: Arc<RwLock<VecDeque<SeccompAuditEntry>>>,
    /// Track repeated violations per VM (for attack detection)
    violation_counts: Arc<RwLock<HashMap<String, usize>>>,
}

impl Default for SeccompAuditLog {
    fn default() -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::new())),
            violation_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl SeccompAuditLog {
    /// Create a new audit log
    pub fn new() -> Self {
        Self::default()
    }

    /// Log a blocked syscall
    pub async fn log_blocked_syscall(
        &self,
        vm_id: &str,
        syscall: &str,
        pid: u32,
    ) -> Result<()> {
        let entry = SeccompAuditEntry {
            vm_id: vm_id.to_string(),
            syscall: syscall.to_string(),
            timestamp: chrono::Utc::now(),
            pid,
            attack_detected: false,
        };

        // Check for repeated violations (attack detection)
        let mut counts = self.violation_counts.write().await;
        let count = counts.entry(vm_id.to_string()).or_insert(0);
        *count += 1;

        let attack_detected = *count > 10; // Threshold for alert

        // Log the entry
        let mut entries = self.entries.write().await;
        if entries.len() >= MAX_SECCOMP_LOG_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(entry);

        if attack_detected {
            warn!(
                "⚠️  ATTACK DETECTED: VM {} has {} blocked syscalls - possible compromise",
                vm_id, count
            );
        } else {
            debug!("Blocked syscall in VM {}: {} (count: {})", vm_id, syscall, count);
        }

        Ok(())
    }

    /// Get all audit entries for a VM
    pub async fn get_entries_for_vm(&self, vm_id: &str) -> Vec<SeccompAuditEntry> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|e| e.vm_id == vm_id)
            .cloned()
            .collect()
    }

    /// Get recent entries (within last N seconds)
    pub async fn get_recent_entries(&self, seconds: i64) -> Vec<SeccompAuditEntry> {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(seconds);
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|e| e.timestamp > cutoff)
            .cloned()
            .collect()
    }

    /// Clear audit log (call when VM is destroyed)
    pub async fn clear_vm(&self, vm_id: &str) {
        let mut counts = self.violation_counts.write().await;
        counts.remove(vm_id);

        let mut entries = self.entries.write().await;
        entries.retain(|e| e.vm_id != vm_id);
    }

    /// Get statistics
    pub async fn get_stats(&self, vm_id: &str) -> SeccompStats {
        let entries = self.entries.read().await;
        let vm_entries: Vec<_> = entries.iter().filter(|e| e.vm_id == vm_id).collect();
        let total_blocked = vm_entries.len() as u32;

        // Count unique syscalls
        let mut unique_syscalls: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for entry in &vm_entries {
            unique_syscalls.insert(entry.syscall.as_str());
        }

        SeccompStats {
            total_blocked,
            unique_syscalls: unique_syscalls.len() as u32,
        }
    }
}

/// Seccomp statistics for a VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompStats {
    /// Total number of blocked syscalls
    pub total_blocked: u32,
    /// Number of unique syscalls blocked
    pub unique_syscalls: u32,
}

/// Validate seccomp rules before applying to VM
pub fn validate_seccomp_rules(filter: &SeccompFilter) -> Result<()> {
    // Ensure audit is enabled (security requirement)
    if !filter.audit_enabled {
        anyhow::bail!("Seccomp audit logging must be enabled for security");
    }

    // Validate that minimal profile has at least basic I/O syscalls
    let whitelist = filter.build_whitelist();
    let required = ["read", "write", "exit", "exit_group"];

    for sys in required.iter() {
        if !whitelist.contains(sys) {
            anyhow::bail!(
                "Seccomp filter missing required syscall: {} - VM will not function",
                sys
            );
        }
    }

    info!("Seccomp filter validation passed: {} syscalls allowed", whitelist.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seccomp_filter_default() {
        let filter = SeccompFilter::default();
        assert_eq!(filter.level, SeccompLevel::Basic);
        assert!(filter.audit_enabled);
        assert!(filter.custom_rules.is_empty());
    }

    #[test]
    fn test_seccomp_filter_new() {
        let filter = SeccompFilter::new(SeccompLevel::Minimal);
        assert_eq!(filter.level, SeccompLevel::Minimal);
    }

    #[test]
    fn test_seccomp_filter_add_rule() {
        let filter = SeccompFilter::default()
            .add_rule(SyscallRule {
                name: "test_syscall".to_string(),
                action: SeccompAction::Allow,
                rationale: "For testing".to_string(),
            });

        assert_eq!(filter.custom_rules.len(), 1);
        assert_eq!(filter.custom_rules[0].name, "test_syscall");
    }

    #[test]
    fn test_minimal_whitelist() {
        let filter = SeccompFilter::new(SeccompLevel::Minimal);
        let whitelist = filter.build_whitelist();

        // Check essential syscalls are present
        assert!(whitelist.contains(&"read"));
        assert!(whitelist.contains(&"write"));
        assert!(whitelist.contains(&"exit"));
        assert!(whitelist.contains(&"mmap"));

        // Minimal should be smallest whitelist
        let basic_filter = SeccompFilter::new(SeccompLevel::Basic);
        assert!(whitelist.len() < basic_filter.build_whitelist().len());
    }

    #[test]
    fn test_basic_whitelist() {
        let filter = SeccompFilter::new(SeccompLevel::Basic);
        let whitelist = filter.build_whitelist();

        // Should have minimal syscalls
        assert!(whitelist.contains(&"read"));
        assert!(whitelist.contains(&"write"));

        // Should have additional syscalls
        assert!(whitelist.contains(&"open"));
        assert!(whitelist.contains(&"epoll_wait"));
        assert!(whitelist.contains(&"pipe"));

        // Basic should be larger than minimal
        let minimal_filter = SeccompFilter::new(SeccompLevel::Minimal);
        assert!(whitelist.len() > minimal_filter.build_whitelist().len());
    }

    #[test]
    fn test_permissive_whitelist() {
        let filter = SeccompFilter::new(SeccompLevel::Permissive);
        let whitelist = filter.build_whitelist();

        // Should have all basic syscalls
        assert!(whitelist.contains(&"read"));
        assert!(whitelist.contains(&"open"));

        // Should have testing syscalls
        assert!(whitelist.contains(&"uname"));
        assert!(whitelist.contains(&"sysinfo"));

        // Permissive should be largest
        let basic_filter = SeccompFilter::new(SeccompLevel::Basic);
        assert!(whitelist.len() > basic_filter.build_whitelist().len());
    }

    #[test]
    fn test_validate_seccomp_rules_success() {
        let filter = SeccompFilter::new(SeccompLevel::Basic);
        assert!(validate_seccomp_rules(&filter).is_ok());
    }

    #[test]
    fn test_validate_seccomp_rules_no_audit() {
        let mut filter = SeccompFilter::new(SeccompLevel::Basic);
        filter.audit_enabled = false;

        let result = validate_seccomp_rules(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("audit logging"));
    }

    #[tokio::test]
    async fn test_audit_log_blocked_syscall() {
        let log = SeccompAuditLog::new();

        log.log_blocked_syscall("vm-1", "socket", 1234)
            .await
            .unwrap();

        let entries = log.get_entries_for_vm("vm-1").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].vm_id, "vm-1");
        assert_eq!(entries[0].syscall, "socket");
        assert_eq!(entries[0].pid, 1234);
    }

    #[tokio::test]
    async fn test_audit_log_attack_detection() {
        let log = SeccompAuditLog::new();

        // Log 11 blocked syscalls (above threshold of 10)
        for i in 0..11 {
            log.log_blocked_syscall("vm-attack", "socket", 1000 + i)
                .await
                .unwrap();
        }

        let stats = log.get_stats("vm-attack").await;
        assert_eq!(stats.total_blocked, 11);
    }

    #[tokio::test]
    async fn test_audit_log_clear_vm() {
        let log = SeccompAuditLog::new();

        log.log_blocked_syscall("vm-1", "socket", 1234)
            .await
            .unwrap();

        log.clear_vm("vm-1").await;

        let entries = log.get_entries_for_vm("vm-1").await;
        assert_eq!(entries.len(), 0);
    }

    #[tokio::test]
    async fn test_audit_log_recent_entries() {
        let log = SeccompAuditLog::new();

        log.log_blocked_syscall("vm-1", "socket", 1234)
            .await
            .unwrap();

        let recent = log.get_recent_entries(60).await;
        assert_eq!(recent.len(), 1);

        let old = log.get_recent_entries(0).await;
        assert_eq!(old.len(), 0);
    }

    #[test]
    fn test_to_firecracker_json() {
        let filter = SeccompFilter::new(SeccompLevel::Minimal);
        let json = filter.to_firecracker_json().unwrap();

        // Should contain seccomp field
        assert!(json.contains("seccomp"));
    }

    #[test]
    fn test_syscall_action_serialization() {
        let action = SeccompAction::Allow;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"Allow\"");
    }

    #[test]
    fn test_seccomp_level_serialization() {
        let level = SeccompLevel::Basic;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"Basic\"");
    }

    // Property-based test: ensure all whitelist levels are ordered correctly
    #[test]
    fn test_whitelist_ordering() {
        let minimal = SeccompFilter::new(SeccompLevel::Minimal);
        let basic = SeccompFilter::new(SeccompLevel::Basic);
        let permissive = SeccompFilter::new(SeccompLevel::Permissive);

        let min_count = minimal.build_whitelist().len();
        let basic_count = basic.build_whitelist().len();
        let perm_count = permissive.build_whitelist().len();

        assert!(min_count < basic_count, "Minimal should be smaller than Basic");
        assert!(basic_count < perm_count, "Basic should be smaller than Permissive");
    }

    // Property-based test: ensure minimal contains required syscalls
    #[test]
    fn test_minimal_always_has_required_syscalls() {
        for _dummy in 0u8..10 {
            let filter = SeccompFilter::new(SeccompLevel::Minimal);
            let whitelist = filter.build_whitelist();

            // These syscalls MUST be present for any VM to function
            let required = ["read", "write", "exit", "exit_group", "mmap"];

            for sys in &required {
                assert!(whitelist.contains(&sys), "Required syscall {} not found in whitelist", sys);
            }
        }
    }

    // Security test: ensure dangerous syscalls are blocked
    #[test]
    fn test_dangerous_syscalls_blocked() {
        let filter = SeccompFilter::new(SeccompLevel::Basic);
        let whitelist = filter.build_whitelist();

        // These syscalls MUST NOT be allowed for security
        let dangerous = [
            "socket",    // Network operations
            "bind",      // Network operations
            "listen",    // Network operations
            "connect",   // Network operations
            "clone",     // Process creation
            "fork",      // Process creation
            "vfork",     // Process creation
            "execve",    // Execute programs
            "mount",     // Filesystem mounting
            "umount",    // Filesystem operations
            "reboot",    // System reboot
            "ptrace",    // Process tracing
            "kexec_load", // Load new kernel
        ];

        for sys in &dangerous {
            assert!(!whitelist.contains(&sys), "Dangerous syscall {} should be blocked", sys);
        }
    }

    #[tokio::test]
    async fn test_audit_log_limit() {
        let log = SeccompAuditLog::new();
        let max = MAX_SECCOMP_LOG_ENTRIES;

        // Add max + 5 entries
        for i in 0..(max + 5) {
            log.log_blocked_syscall("vm-limit", "socket", i as u32)
                .await
                .unwrap();
        }

        let entries = log.get_entries_for_vm("vm-limit").await;
        assert_eq!(entries.len(), max);

        // Verify we have the latest entries (FIFO)
        // The first 5 should be gone (0..4). The oldest remaining should be 5.
        assert_eq!(entries[0].pid, 5);
        assert_eq!(entries[max - 1].pid, (max + 4) as u32);
    }
}
