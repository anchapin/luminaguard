use assert_cmd::Command;

#[test]
fn test_help() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Secure agentic AI runtime with JIT Micro-VMs",
        ));
}

#[test]
fn test_version() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains("ironclaw 0.1.0"));
}

#[test]
fn test_run_command_missing_task() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("run")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "required arguments were not provided",
        ));
}

#[test]
fn test_run_command_success() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("run").arg("test task").assert().success();
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("nonexistent-command")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unrecognized subcommand"));
}

#[test]
fn test_test_mcp_command_parsing() {
    // Just verify argument parsing works, even if execution fails
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();
    cmd.arg("test-mcp")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Test MCP connection"));
}

#[test]
fn test_spawn_vm_command_execution() {
    // This test verifies that the command runs and attempts to spawn a VM.
    // In the CI environment, it is expected to fail due to missing Firecracker/resources.
    let mut cmd = Command::cargo_bin("ironclaw").unwrap();

    // We check that it at least started the attempt
    let assert = cmd.arg("spawn-vm").assert();

    // It might succeed (if env is perfect) or fail (most likely)
    // We just want to ensure it didn't panic or crash unexpectedly with weird code.
    // If it fails, it should be because of missing resources/binary.

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);

    if output.status.success() {
        // If it somehow succeeded
        assert!(stdout.contains("Spawning JIT Micro-VM"));
    } else {
        // If it failed, it should be a known error
        let error_msg = format!("{}{}", stdout, stderr);
        assert!(
            error_msg.contains("Kernel image not found")
                || error_msg.contains("Firecracker API socket")
                || error_msg.contains("Failed to spawn firecracker process")
                || error_msg.contains("No such file or directory")
                || error_msg.contains("failed to run"), // catch-all for command spawn fail
            "Unexpected error: {}",
            error_msg
        );
    }
}
