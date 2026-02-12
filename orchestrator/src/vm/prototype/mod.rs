// Firecracker Feasibility Prototype
//
// Quick 1-week test to validate Firecracker can boot VMs
//
// GOAL: Determine if Firecracker is viable for JIT Micro-VMs
// OUTCOME:
//   - If fails → Abandon JIT VM entirely (re-architect)
//   - If passes → Proceed to full Phase 3 validation
//
// This is EXPLORATORY code - not production ready

use std::process::Command;

pub mod resources;
pub mod spawn_test;

use resources::FirecrackerAssets;
use spawn_test::SpawnTestResult;

/// Feasibility test result
#[derive(Debug, Clone)]
pub struct FeasibilityResult {
    /// Firecracker binary found
    pub firecracker_available: bool,

    /// Can spawn VM (even if minimal)
    pub can_spawn: bool,

    /// Spawn time in milliseconds (None if spawn failed)
    pub spawn_time_ms: Option<f64>,

    /// Error message (if any)
    pub error: Option<String>,

    /// Recommendation
    pub recommendation: Recommendation,
}

/// Go/No-Go recommendation
#[derive(Debug, Clone, PartialEq)]
pub enum Recommendation {
    /// Proceed to full Phase 3 validation
    Proceed,

    /// Abandon JIT VMs - use alternative architecture
    Abandon,

    /// Needs investigation - partial success
    Investigate,
}

/// Check if Firecracker is installed
pub fn check_firecracker_installed() -> bool {
    let result = Command::new("firecracker").arg("--version").output();

    match result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Check if KVM is available (required for Firecracker)
pub fn check_kvm_available() -> bool {
    // Check if /dev/kvm exists (created when KVM module is loaded)
    std::path::Path::new("/dev/kvm").exists()
}

/// Run feasibility test
pub async fn run_feasibility_test() -> FeasibilityResult {
    tracing::info!("🔬 Starting Firecracker Feasibility Test");
    tracing::info!("═══════════════════════════════════════");

    // Step 1: Check prerequisites
    tracing::info!("Step 1: Checking prerequisites...");

    let fc_installed = check_firecracker_installed();
    tracing::info!(
        "  Firecracker binary: {}",
        if fc_installed {
            "✅ Found"
        } else {
            "❌ Not found"
        }
    );

    let kvm_available = check_kvm_available();
    tracing::info!(
        "  KVM module: {}",
        if kvm_available {
            "✅ Available"
        } else {
            "❌ Not available"
        }
    );

    if !fc_installed {
        return FeasibilityResult {
            firecracker_available: false,
            can_spawn: false,
            spawn_time_ms: None,
            error: Some("Firecracker binary not found. Install: https://github.com/firecracker-microvm/firecracker".to_string()),
            recommendation: Recommendation::Abandon,
        };
    }

    if !kvm_available {
        return FeasibilityResult {
            firecracker_available: true,
            can_spawn: false,
            spawn_time_ms: None,
            error: Some(
                "KVM not available. Firecracker requires hardware virtualization.".to_string(),
            ),
            recommendation: Recommendation::Abandon,
        };
    }

    // Step 2: Try to prepare test assets
    tracing::info!("Step 2: Preparing test assets...");

    let assets = match FirecrackerAssets::prepare().await {
        Ok(assets) => {
            tracing::info!("  ✅ Test assets ready");
            tracing::info!("     Kernel: {}", assets.kernel_path.display());
            tracing::info!("     Rootfs: {}", assets.rootfs_path.display());
            assets
        }
        Err(e) => {
            return FeasibilityResult {
                firecracker_available: true,
                can_spawn: false,
                spawn_time_ms: None,
                error: Some(format!("Failed to prepare assets: {}", e)),
                recommendation: Recommendation::Investigate,
            };
        }
    };

    // Step 3: Run spawn test
    tracing::info!("Step 3: Running spawn test...");

    let spawn_result = spawn_test::test_spawn(&assets).await;

    match spawn_result {
        SpawnTestResult::Success { spawn_time_ms } => {
            tracing::info!("  ✅ VM spawned successfully!");
            tracing::info!("     Spawn time: {:.2}ms", spawn_time_ms);

            // Cleanup
            let _ = assets.cleanup().await;

            // Make recommendation based on spawn time
            let recommendation = if spawn_time_ms < 500.0 {
                Recommendation::Proceed // Under 500ms - excellent
            } else if spawn_time_ms < 2000.0 {
                Recommendation::Proceed // Under 2s - acceptable for prototype
            } else {
                Recommendation::Investigate // Over 2s - needs optimization
            };

            FeasibilityResult {
                firecracker_available: true,
                can_spawn: true,
                spawn_time_ms: Some(spawn_time_ms),
                error: None,
                recommendation,
            }
        }
        SpawnTestResult::Failed { error } => {
            tracing::error!("  ❌ Spawn test failed: {}", error);

            let _ = assets.cleanup().await;

            FeasibilityResult {
                firecracker_available: true,
                can_spawn: false,
                spawn_time_ms: None,
                error: Some(error),
                recommendation: Recommendation::Abandon,
            }
        }
    }
}

/// Print feasibility report
pub fn print_report(result: &FeasibilityResult) {
    println!();
    println!("═══════════════════════════════════════");
    println!("🔬 Firecracker Feasibility Report");
    println!("═══════════════════════════════════════");
    println!();

    println!("Prerequisites:");
    println!(
        "  Firecracker: {}",
        if result.firecracker_available {
            "✅ Installed"
        } else {
            "❌ Not found"
        }
    );
    println!(
        "  Can Spawn: {}",
        if result.can_spawn {
            "✅ Yes"
        } else {
            "❌ No"
        }
    );
    println!();

    if let Some(spawn_time) = result.spawn_time_ms {
        println!("Performance:");
        println!("  Spawn Time: {:.2}ms", spawn_time);

        let vs_target = spawn_time / 200.0; // Target is 200ms
        println!(
            "  vs Target (200ms): {}x",
            if vs_target < 1.0 {
                format!("{:.2} (BETTER)", vs_target)
            } else {
                format!("{:.2}x", vs_target)
            }
        );
        println!();
    }

    if let Some(error) = &result.error {
        println!("Error:");
        println!("  {}", error);
        println!();
    }

    println!("Recommendation:");
    match result.recommendation {
        Recommendation::Proceed => {
            println!("  ✅ PROCEED to full Phase 3 validation");
            println!("     Firecracker is viable for JIT Micro-VMs");
            println!();
            println!("  Next steps:");
            println!("    1. Implement Snapshot Pool (H3)");
            println!("    2. Validate 10-50ms spawn time with snapshots");
            println!("    3. Full 12-week validation program");
        }
        Recommendation::Abandon => {
            println!("  ❌ ABANDON JIT VM approach");
            println!("     Firecracker is not viable on this system");
            println!();
            println!("  Alternative approaches:");
            println!("    1. Use container-based isolation (user namespaces)");
            println!("    2. Use WebAssembly (Wasmtime/Wasmer)");
            println!("    3. Accept host execution with approval cliff only");
        }
        Recommendation::Investigate => {
            println!("  ⚠️  INVESTIGATE further");
            println!("     Partial success - needs more work");
            println!();
            println!("  Issues to investigate:");
            println!("    1. Spawn time optimization");
            println!("    2. Kernel/rootfs configuration");
            println!("    3. System compatibility issues");
        }
    }
    println!("═══════════════════════════════════════");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_firecracker() {
        // This test just checks the function doesn't crash
        let result = check_firecracker_installed();
        println!("Firecracker installed: {}", result);
    }

    #[test]
    fn test_check_kvm() {
        // This test just checks the function doesn't crash
        let result = check_kvm_available();
        println!("KVM available: {}", result);
    }
}
