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
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("spawn-vm")
        .assert()
        .success()
        .stdout(predicate::str::contains("Spawning JIT Micro-VM..."));
}
