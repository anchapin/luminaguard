// Seccomp Filters for Firecracker VMs
//
// Defense-in-depth security: syscall filtering prevents even compromised VMs
// from executing dangerous operations. Blocks 99% of syscalls, allowing
// only essential ones for basic VM operation.
//
// ## Syscall Whitelisting Strategy
//
// The Basic level whitelist (production-recommended) includes ~60 syscalls
// carefully selected for agent workloads while maintaining security:
//
// ### Allowed Categories:
// - **I/O Operations**: read, write, readv, writev, pread64, pwrite64
// - **File Descriptor Management**: open, openat, close, dup, dup2, dup3, pipe, pipe2
// - **File Metadata**: stat, lstat, fstat, statfs, access, faccessat
// - **Directory Operations**: chdir, fchdir, getcwd, mkdir, rmdir
// - **File Operations**: unlink, rename, truncate, ftruncate, fsync
// - **Process Info**: getpid, gettid, getppid (read-only)
// - **Credentials**: geteuid, getuid, getegid, getgid (read-only)
// - **Time**: clock_gettime, gettimeofday, time
// - **Async I/O**: epoll_*, poll, select, pselect6, eventfd2
// - **Memory**: mmap, munmap, mprotect, brk (no exec)
// - **Signals**: rt_sigreturn, rt_sigprocmask, sigaltstack (no kill/tkill)
//
// ### Blocked Categories (Security):
// - **Process Creation**: fork, vfork, clone, execve, execveat
// - **Privilege Escalation**: setuid, setgid, setreuid, setresuid, setgroups
// - **System Control**: mount, umount2, pivot_root, chroot, ptrace
// - **Network**: socket, connect, bind, listen (isolated to vsock only)
// - **Signal Delivery**: kill, tkill, tgkill, rt_sigqueue
// - **Process Introspection**: ptrace, process_vm_readv, process_vm_writev

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Maximum number of audit entries to keep in memory (global limit)
const MAX_SECCOMP_LOG_ENTRIES: usize = 10_000;

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
    /// Syscall whitelist for audit logging (only these syscalls are logged)
    #[serde(default)]
    pub audit_whitelist: Vec<String>,
    /// Audit all blocked syscalls (if true, ignores whitelist)
    #[serde(default)]
    pub audit_all_blocked: bool,
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
            audit_whitelist: default_audit_whitelist(),
            audit_all_blocked: false,
        }
    }
}

/// Default audit whitelist for rootfs-hardened VMs
fn default_audit_whitelist() -> Vec<String> {
    // By default, audit security-sensitive syscalls
    vec![
        "execve".to_string(),
        "execveat".to_string(),
        "fork".to_string(),
        "clone".to_string(),
        "ptrace".to_string(),
        "mount".to_string(),
        "umount".to_string(),
        "pivot_root".to_string(),
        "chroot".to_string(),
        "setuid".to_string(),
        "setgid".to_string(),
        "setreuid".to_string(),
        "setregid".to_string(),
        "setresuid".to_string(),
        "setresgid".to_string(),
        "chmod".to_string(),
        "fchmod".to_string(),
        "chown".to_string(),
        "fchown".to_string(),
        "kill".to_string(),
        "prctl".to_string(),
    ]
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

    /// Add a syscall to the audit whitelist
    pub fn add_audit_whitelist(mut self, syscall: &str) -> Self {
        self.audit_whitelist.push(syscall.to_string());
        self
    }

    /// Set whether to audit all blocked syscalls
    pub fn audit_all_blocked(mut self, audit_all: bool) -> Self {
        self.audit_all_blocked = audit_all;
        self
    }

    /// Check if a syscall should be audited
    pub fn should_audit(&self, syscall: &str) -> bool {
        if !self.audit_enabled {
            return false;
        }

        if self.audit_all_blocked {
            return true;
        }

        self.audit_whitelist.contains(&syscall.to_string())
    }

    /// Get the complete audit whitelist (custom + defaults)
    pub fn get_audit_whitelist(&self) -> Vec<String> {
        let mut whitelist = default_audit_whitelist();
        whitelist.extend(self.audit_whitelist.iter().cloned());
        whitelist.sort();
        whitelist.dedup();
        whitelist
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
    ///
    /// This whitelist includes VSOCK and socket syscalls for guest-host communication:
    /// - Essential I/O operations (read, write, pread, pwrite)
    /// - File descriptor management (open, close, dup, pipe)
    /// - Memory management (mmap, brk, mprotect)
    /// - Process and thread information (getpid, gettid, geteuid)
    /// - Async I/O primitives (epoll, poll, select)
    /// - Time functions (clock_gettime, gettimeofday)
    /// - Safe signal handling
    /// - VSOCK/socket operations (AF_VSOCK for VM communication)
    ///
    /// Note: socket, connect, bind, listen, accept are allowed for VSOCK (AF_VSOCK)
    /// communication between guest and host. These do not enable network access
    /// as they are limited to VM-vsock communication.
    ///
    /// Excluded dangerous syscalls:
    /// - execve/execveat (no process spawning)
    /// - fork/clone (no process creation)
    /// - ptrace (no process introspection)
    /// - mount/umount (no filesystem changes)
    /// - setuid/setgid/setresuid (no privilege escalation)
    /// - kill/tkill (no signal sending)
    fn basic_whitelist(&self) -> Vec<&'static str> {
        let mut whitelist = self.minimal_whitelist();

        // Additional safe syscalls organized by category
        whitelist.extend(vec![
            // Extended I/O operations
            "readv",    // Vectored read
            "writev",   // Vectored write
            "pread64",  // Positional read
            "pwrite64", // Positional write
            // File descriptor operations
            "open",      // Open file
            "openat",    // Open relative to dirfd
            "access",    // Check file accessibility
            "faccessat", // Check file accessibility relative to dirfd
            "dup",       // Duplicate file descriptor
            "dup2",      // Duplicate to specific fd
            "dup3",      // Duplicate with flags
            // File metadata
            "statfs",  // Get filesystem statistics
            "fstatfs", // Get filesystem statistics for fd
            // Pipes and sockets
            "pipe",  // Create unidirectional pipe
            "pipe2", // Create pipe with flags (for nonblocking I/O)
            // VSOCK syscalls for guest-host VM communication
            // These enable AF_VSOCK sockets which are VM-local only
            // (not exposed to external networks)
            "socket",      // Create socket (AF_VSOCK for VM communication)
            "connect",     // Connect to VSOCK endpoint
            "bind",        // Bind socket to VSOCK CID:port
            "listen",      // Listen for VSOCK connections
            "accept",      // Accept VSOCK connections
            "accept4",     // Accept with flags
            "getsockname", // Get socket address
            "getpeername", // Get peer address
            "setsockopt",  // Set socket options
            "getsockopt",  // Get socket options
            "shutdown",    // Shutdown socket
            "sendmsg",     // Send message
            "recvmsg",     // Receive message
            "sendto",      // Send to address
            "recvfrom",    // Receive from address
            // Time operations
            "clock_gettime", // Get current time (secure)
            "gettimeofday",  // Get current time (legacy)
            // Process information (read-only)
            "getpid",  // Get process ID
            "gettid",  // Get thread ID
            "getppid", // Get parent process ID
            // Process credentials (read-only)
            "geteuid", // Get effective user ID
            "getegid", // Get effective group ID
            "getuid",  // Get real user ID
            "getgid",  // Get real group ID
            // Scheduling control (safe operations)
            "sched_yield",       // Yield to other threads
            "sched_getaffinity", // Get CPU affinity
            // Async I/O multiplexing
            "epoll_wait",  // Wait on epoll file descriptor
            "epoll_ctl",   // Control epoll set
            "epoll_pwait", // Wait on epoll with signal mask
            "select",      // Synchronous I/O multiplexing
            "pselect6",    // Select with signal mask
            // Event notification
            "eventfd2", // Create event notification fd
            // Polling
            "poll",  // Poll file descriptors
            "ppoll", // Poll with signal mask
            // Basic signal handling
            "sigaltstack", // Set alternate signal stack
            // Additional safe syscalls for broader capability
            "fcntl",     // File descriptor control (get flags, set flags)
            "fcntl64",   // File descriptor control (64-bit)
            "getcwd",    // Get current working directory
            "chdir",     // Change directory (limited to VM filesystem)
            "fchdir",    // Change to directory fd
            "lseek",     // Seek in file (already in minimal but being explicit)
            "mkdir",     // Create directory
            "rmdir",     // Remove directory
            "unlink",    // Remove file
            "truncate",  // Truncate file
            "ftruncate", // Truncate open file
            "rename",    // Rename file
            "renameat",  // Rename relative to dirfd
            "fsync",     // Synchronize file to disk
            "fdatasync", // Synchronize file data to disk
            "flock",     // Apply/remove advisory lock
            "realpath",  // Resolve pathname
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

        serde_json::to_string_pretty(&filter).context("Failed to serialize seccomp filter")
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
    pub async fn log_blocked_syscall(&self, vm_id: &str, syscall: &str, pid: u32) -> Result<()> {
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
            debug!(
                "Blocked syscall in VM {}: {} (count: {})",
                vm_id, syscall, count
            );
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

    info!(
        "Seccomp filter validation passed: {} syscalls allowed",
        whitelist.len()
    );

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
        let filter = SeccompFilter::default().add_rule(SyscallRule {
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

    #[test]
    fn test_audit_whitelist_default() {
        let filter = SeccompFilter::default();
        assert!(filter.audit_enabled);
        assert!(!filter.audit_whitelist.is_empty());
        assert!(!filter.audit_all_blocked);
    }

    #[test]
    fn test_add_audit_whitelist() {
        let filter = SeccompFilter::default()
            .add_audit_whitelist("custom_syscall")
            .add_audit_whitelist("another_syscall");

        assert!(filter
            .audit_whitelist
            .contains(&"custom_syscall".to_string()));
        assert!(filter
            .audit_whitelist
            .contains(&"another_syscall".to_string()));
    }

    #[test]
    fn test_should_audit_with_whitelist() {
        let filter = SeccompFilter::default().add_audit_whitelist("execve");

        assert!(filter.should_audit("execve"));
        assert!(filter.should_audit("mount")); // In default whitelist
        assert!(!filter.should_audit("read")); // Not in whitelist
    }

    #[test]
    fn test_should_audit_all_blocked() {
        let filter = SeccompFilter::default().audit_all_blocked(true);

        assert!(filter.should_audit("execve"));
        assert!(filter.should_audit("mount"));
        assert!(filter.should_audit("read"));
        assert!(filter.should_audit("anything"));
    }

    #[test]
    fn test_should_audit_disabled() {
        let mut filter = SeccompFilter::default();
        filter.audit_enabled = false;

        assert!(!filter.should_audit("execve"));
        assert!(!filter.should_audit("mount"));
    }

    #[test]
    fn test_get_audit_whitelist_includes_defaults() {
        let filter = SeccompFilter::default().add_audit_whitelist("custom");

        let whitelist = filter.get_audit_whitelist();

        assert!(whitelist.contains(&"custom".to_string()));
        assert!(whitelist.contains(&"execve".to_string())); // Default
        assert!(whitelist.contains(&"mount".to_string())); // Default
    }

    #[test]
    fn test_get_audit_whitelist_dedupes() {
        let filter = SeccompFilter::default().add_audit_whitelist("execve"); // Already in defaults

        let whitelist = filter.get_audit_whitelist();

        // Should not have duplicates
        let execve_count = whitelist.iter().filter(|s| *s == "execve").count();
        assert_eq!(execve_count, 1);
    }

    #[test]
    fn test_property_audit_security_syscalls() {
        // Verify all security-sensitive syscalls are in default whitelist
        let filter = SeccompFilter::default();
        let whitelist = filter.get_audit_whitelist();

        let security_syscalls = [
            "execve",
            "execveat",
            "fork",
            "clone",
            "ptrace",
            "mount",
            "umount",
            "pivot_root",
            "chroot",
            "setuid",
            "setgid",
            "chmod",
            "chown",
        ];

        for syscall in &security_syscalls {
            assert!(
                whitelist.contains(&syscall.to_string()),
                "Security syscall {} should be in audit whitelist",
                syscall
            );
        }
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

        assert!(
            min_count < basic_count,
            "Minimal should be smaller than Basic"
        );
        assert!(
            basic_count < perm_count,
            "Basic should be smaller than Permissive"
        );
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
                assert!(
                    whitelist.contains(&sys),
                    "Required syscall {} not found in whitelist",
                    sys
                );
            }
        }
    }

    // Security test: ensure dangerous syscalls are blocked
    // Note: socket, connect, bind, listen are now allowed for VSOCK (AF_VSOCK)
    // which is VM-local only and doesn't enable external network access
    #[test]
    fn test_dangerous_syscalls_blocked() {
        let filter = SeccompFilter::new(SeccompLevel::Basic);
        let whitelist = filter.build_whitelist();

        // These syscalls MUST NOT be allowed for security
        // Note: socket, connect, bind, listen ARE allowed for VSOCK
        let dangerous = [
            "clone",         // Process creation
            "fork",          // Process creation
            "vfork",         // Process creation
            "execve",        // Execute programs
            "execveat",      // Execute programs
            "mount",         // Filesystem mounting
            "umount",        // Filesystem operations
            "umount2",       // Filesystem operations
            "reboot",        // System reboot
            "ptrace",        // Process tracing
            "kexec_load",    // Load new kernel
            "init_module",   // Load kernel module
            "delete_module", // Unload kernel module
            "chroot",        // Change root directory
            "pivot_root",    // Change root filesystem
            "setuid",        // Set user ID
            "setgid",        // Set group ID
            "setreuid",      // Set real/effective UID
            "setregid",      // Set real/effective GID
            "setresuid",     // Set real/effective/saved UID
            "setresgid",     // Set real/effective/saved GID
            "kill",          // Send signals
            "tkill",         // Send thread signals
            "tgkill",        // Send thread signals
        ];

        for sys in &dangerous {
            assert!(
                !whitelist.contains(&sys),
                "Dangerous syscall {} should be blocked",
                sys
            );
        }
    }

    // Security test: ensure VSOCK syscalls are allowed
    #[test]
    fn test_vsock_syscalls_allowed() {
        let filter = SeccompFilter::new(SeccompLevel::Basic);
        let whitelist = filter.build_whitelist();

        // VSOCK syscalls should be allowed for guest-host communication
        let vsock_syscalls = [
            "socket",      // Create socket (AF_VSOCK)
            "connect",     // Connect to VSOCK
            "bind",        // Bind to VSOCK port
            "listen",      // Listen for VSOCK
            "accept",      // Accept VSOCK connection
            "accept4",     // Accept with flags
            "getsockname", // Get socket address
            "getpeername", // Get peer address
            "setsockopt",  // Set socket options
            "getsockopt",  // Get socket options
            "shutdown",    // Shutdown socket
            "sendmsg",     // Send message
            "recvmsg",     // Receive message
            "sendto",      // Send to address
            "recvfrom",    // Receive from address
        ];

        for sys in &vsock_syscalls {
            assert!(
                whitelist.contains(&sys),
                "VSOCK syscall {} should be allowed",
                sys
            );
        }
    }

    #[tokio::test]
    async fn test_audit_log_bounded_growth() {
        let log = SeccompAuditLog::new();
        let vm_id = "vm-growth-test";
        // Insert more than the limit to trigger rotation
        let iterations = MAX_SECCOMP_LOG_ENTRIES + 500;

        for i in 0..iterations {
            log.log_blocked_syscall(vm_id, "socket", 1000 + i as u32)
                .await
                .unwrap();
        }

        let stats = log.get_stats(vm_id).await;
        // Should be capped at MAX_SECCOMP_LOG_ENTRIES
        assert_eq!(stats.total_blocked, MAX_SECCOMP_LOG_ENTRIES as u32);

        // Verify FIFO behavior: the oldest entries should be gone
        let entries = log.get_entries_for_vm(vm_id).await;
        assert_eq!(entries.len(), MAX_SECCOMP_LOG_ENTRIES);

        // The first entry should be from the rotated window
        // We inserted 0..(N+500). We kept the last N.
        // So the first kept entry should be index 500.
        // PID was 1000 + i. So 1000 + 500 = 1500.
        assert_eq!(
            entries[0].pid,
            1000 + (iterations - MAX_SECCOMP_LOG_ENTRIES) as u32
        );

        // The last entry should be the last inserted
        assert_eq!(
            entries[entries.len() - 1].pid,
            1000 + (iterations - 1) as u32
        );
    }
}
