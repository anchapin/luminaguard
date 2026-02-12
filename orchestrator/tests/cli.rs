use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("ironclaw 0.1.0"));
}

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Secure agentic AI runtime with JIT Micro-VMs",
        ));
}

#[test]
fn test_cli_run_missing_task() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("run")
        .assert()
        .failure() // Should fail because 'task' argument is required
        .stderr(predicate::str::contains(
            "required arguments were not provided",
        ));
}

#[test]
fn test_cli_spawn_vm() {
    // Skip test if not running as root (required for firewall operations)
    #[cfg(unix)]
    {
        let output = std::process::Command::new("id").arg("-u").output();
        if let Ok(output) = output {
            let uid = String::from_utf8_lossy(&output.stdout);
            // If not root (uid 0), skip the test
            if !uid.starts_with("uid=0") {
                println!("Skipping test_cli_spawn_vm: requires root privileges");
                return;
            }
        }
    }

    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("spawn-vm")
        .assert()
        .success()
        .stdout(predicate::str::contains("Spawning JIT Micro-VM..."));
}
