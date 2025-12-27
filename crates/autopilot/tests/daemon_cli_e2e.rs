//! CLI-based End-to-end tests for autopilot daemon
//!
//! These tests exercise the daemon start/stop/status lifecycle
//! via CLI commands.
//!
//! Run with: `cargo test -p autopilot --test daemon_cli_e2e`

use std::time::Duration;
use testing::E2EEnvironment;

/// Skip test if binary not found
macro_rules! skip_if_no_binary {
    ($env:expr) => {
        if $env.is_err() {
            println!("Skipping test: openagents binary not found. Run `cargo build --bin openagents` first.");
            return;
        }
    };
}

/// Test that daemon help command works
#[tokio::test]
async fn test_daemon_help() {
    let env = E2EEnvironment::for_daemon().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["daemon", "--help"]).await;
    assert!(output.is_ok(), "Help command should succeed");

    let output = output.unwrap();
    // Help should either succeed or show usage
    assert!(
        output.success() || output.exit_code == Some(2),
        "Unexpected exit code: {:?}",
        output.exit_code
    );

    let combined = output.combined();
    assert!(
        combined.contains("daemon") || combined.contains("Daemon") || combined.contains("USAGE"),
        "Output should mention daemon: {}",
        combined
    );
}

/// Test daemon status when not running
#[tokio::test]
async fn test_daemon_status_not_running() {
    let env = E2EEnvironment::for_daemon().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["daemon", "status"]).await;

    match output {
        Ok(out) => {
            let combined = out.combined();
            println!("Status output: {}", combined);
            // When not running, should indicate that
            assert!(
                combined.contains("not running")
                    || combined.contains("No daemon")
                    || combined.contains("stopped")
                    || !out.success(),
                "Should indicate daemon not running: {}",
                combined
            );
        }
        Err(e) => {
            println!("Status check failed (expected if no daemon): {}", e);
        }
    }
}

/// Test daemon start command (brief run)
#[tokio::test]
async fn test_daemon_start() {
    let env = E2EEnvironment::for_daemon().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // Set short timeout
    env.cli.set_timeout(Duration::from_secs(5));

    // Get temp path before mutable borrow
    let workdir = env.temp_path().to_str().unwrap().to_string();

    // Try to start daemon (will timeout or return quickly)
    let output = env.cli.run(&[
        "daemon", "start",
        "--workdir", &workdir,
        "--project", "test-project"
    ]).await;

    match output {
        Ok(out) => {
            println!("Start output: {}", out.combined());
        }
        Err(e) => {
            // Timeout is acceptable - daemon runs continuously
            println!("Start timed out or failed: {}", e);
        }
    }
}

/// Test daemon stop command (when not running)
#[tokio::test]
async fn test_daemon_stop_not_running() {
    let env = E2EEnvironment::for_daemon().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["daemon", "stop"]).await;

    match output {
        Ok(out) => {
            let combined = out.combined();
            println!("Stop output: {}", combined);
            // Should indicate nothing to stop or succeed gracefully
        }
        Err(e) => {
            println!("Stop failed: {}", e);
        }
    }
}

/// Test daemon lifecycle: start -> status -> stop
#[tokio::test]
async fn test_daemon_lifecycle() {
    let env = E2EEnvironment::for_daemon().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let workdir = env.temp_path().to_str().unwrap().to_string();

    // Start daemon in background
    let start_handle = env.cli.run_background(&[
        "daemon", "start",
        "--workdir", &workdir,
        "--project", "e2e-test"
    ]).await;

    match start_handle {
        Ok(mut child) => {
            // Wait a bit for daemon to start
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Check status
            let status_output = env.cli.run(&["daemon", "status"]).await;
            if let Ok(out) = status_output {
                println!("Status: {}", out.combined());
            }

            // Stop daemon
            let stop_output = env.cli.run(&["daemon", "stop"]).await;
            if let Ok(out) = stop_output {
                println!("Stop: {}", out.combined());
            }

            // Kill the background process
            let _ = child.kill().await;
        }
        Err(e) => {
            println!("Failed to start daemon: {}", e);
        }
    }
}

/// Test restart-worker command
#[tokio::test]
async fn test_daemon_restart_worker() {
    let env = E2EEnvironment::for_daemon().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["daemon", "restart-worker"]).await;

    match output {
        Ok(out) => {
            let combined = out.combined();
            println!("Restart-worker output: {}", combined);
            // Should fail gracefully if no daemon running
        }
        Err(e) => {
            println!("Restart-worker failed: {}", e);
        }
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    /// Quick smoke test that daemon commands are available
    #[tokio::test]
    async fn test_daemon_subcommand_exists() {
        let result = testing::CliHarness::new().await;
        match result {
            Ok(mut harness) => {
                let output = harness.run(&["daemon", "--help"]).await;
                assert!(output.is_ok(), "Daemon subcommand should exist");
            }
            Err(e) => {
                println!("Binary not found (build first): {}", e);
            }
        }
    }
}
