// Snapshot Pool Demo
//
// Demonstrates the snapshot pool functionality for fast VM spawning.

use ironclaw_orchestrator::vm;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Snapshot Pool Demo ===\n");

    // Set custom snapshot path for demo
    let temp_dir = std::env::temp_dir().join("ironclaw-demo");
    std::env::set_var("IRONCLAW_SNAPSHOT_PATH", temp_dir.to_str().unwrap());
    std::env::set_var("IRONCLAW_POOL_SIZE", "3");

    println!("1. Warming up snapshot pool...");
    let warmup_start = Instant::now();
    vm::warmup_pool().await?;
    println!("   ✓ Pool warmed up in {:?}\n", warmup_start.elapsed());

    // Get pool statistics
    let stats = vm::pool_stats().await?;
    println!("2. Pool Statistics:");
    println!("   - Current size: {}/{}", stats.current_size, stats.max_size);
    if let Some(age) = stats.oldest_snapshot_age_secs {
        println!("   - Oldest snapshot: {}s old", age);
    }
    if let Some(age) = stats.newest_snapshot_age_secs {
        println!("   - Newest snapshot: {}s old", age);
    }
    println!();

    // Spawn VMs from pool
    println!("3. Spawning VMs from pool:");
    for i in 1..=5 {
        let task_id = format!("demo-task-{}", i);
        let spawn_start = Instant::now();

        let handle = vm::spawn_vm(&task_id).await?;

        let elapsed = spawn_start.elapsed();
        println!("   - VM {}: {} (spawned in {:?})", i, handle.id, elapsed);
    }
    println!();

    // Cleanup
    std::env::remove_var("IRONCLAW_SNAPSHOT_PATH");
    std::env::remove_var("IRONCLAW_POOL_SIZE");

    println!("=== Demo Complete ===");
    println!("\nNote: This is a Phase 1 prototype with placeholder snapshots.");
    println!("Phase 2 will integrate real Firecracker snapshot API for 10-50ms spawn times.");

    Ok(())
}
