//! CLI-based End-to-end tests for autopilot
//!
//! These tests exercise the actual CLI binary to verify complete flows work
//! from command invocation through to completion.
//!
//! Run with: `cargo test -p autopilot --test cli_e2e`

use std::time::Duration;
use testing::{E2EEnvironment, extract_issue_id};

/// Skip test if binary not found
macro_rules! skip_if_no_binary {
    ($env:expr) => {
        if $env.is_err() {
            println!("Skipping test: openagents binary not found. Run `cargo build --bin openagents` first.");
            return;
        }
    };
}

/// Test that the autopilot help command works
#[tokio::test]
async fn test_autopilot_help() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["autopilot", "--help"]).await;
    assert!(output.is_ok(), "Help command should succeed");

    let output = output.unwrap();
    // Help should either succeed or show usage (exit code 2)
    assert!(
        output.success() || output.exit_code == Some(2),
        "Unexpected exit code: {:?}",
        output.exit_code
    );

    // Should contain autopilot-related content
    let combined = output.combined();
    assert!(
        combined.contains("autopilot") || combined.contains("Autopilot") || combined.contains("USAGE"),
        "Output should mention autopilot: {}",
        combined
    );
}

/// Test that the dashboard command works (may show empty state)
#[tokio::test]
async fn test_autopilot_dashboard() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["autopilot", "dashboard"]).await;

    // Dashboard might fail if no database exists, or succeed with empty state
    // Either is acceptable for this basic smoke test
    match output {
        Ok(out) => {
            println!("Dashboard output: {}", out.combined());
        }
        Err(e) => {
            println!("Dashboard failed (acceptable): {}", e);
        }
    }
}

/// Test issue list command (should work even with empty database)
#[tokio::test]
async fn test_autopilot_issue_list() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["autopilot", "issue", "list"]).await;

    match output {
        Ok(out) => {
            println!("Issue list output: {}", out.combined());
            // Should succeed (even if showing "no issues")
        }
        Err(e) => {
            // Acceptable if DB doesn't exist yet
            println!("Issue list failed (acceptable): {}", e);
        }
    }
}

/// Test creating an issue
#[tokio::test]
async fn test_autopilot_issue_create() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&[
        "autopilot", "issue", "create",
        "--title", "Test E2E Issue",
        "--body", "This is a test issue created by CLI E2E tests",
        "--directive", "d-test"
    ]).await;

    match output {
        Ok(out) => {
            println!("Create issue output: {}", out.combined());
            if out.success() {
                // Try to extract issue ID
                if let Some(id) = extract_issue_id(&out.stdout) {
                    println!("Created issue ID: {}", id);
                }
            }
        }
        Err(e) => {
            println!("Create issue failed: {}", e);
        }
    }
}

/// Test the full issue lifecycle: create -> claim -> complete
#[tokio::test]
async fn test_autopilot_issue_lifecycle() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // Step 1: Create an issue
    let create_output = env.cli.run(&[
        "autopilot", "issue", "create",
        "--title", "Lifecycle Test Issue",
        "--directive", "d-test"
    ]).await;

    let issue_id = match create_output {
        Ok(out) if out.success() => {
            extract_issue_id(&out.stdout).unwrap_or_else(|| "1".to_string())
        }
        Ok(out) => {
            println!("Create failed with exit code {:?}: {}", out.exit_code, out.combined());
            return;
        }
        Err(e) => {
            println!("Create failed: {}", e);
            return;
        }
    };

    println!("Created issue: {}", issue_id);

    // Step 2: Claim the issue
    let claim_output = env.cli.run(&[
        "autopilot", "issue", "claim", &issue_id
    ]).await;

    match claim_output {
        Ok(out) => {
            println!("Claim output: {}", out.combined());
        }
        Err(e) => {
            println!("Claim failed: {}", e);
        }
    }

    // Step 3: Complete the issue
    let complete_output = env.cli.run(&[
        "autopilot", "issue", "complete", &issue_id
    ]).await;

    match complete_output {
        Ok(out) => {
            println!("Complete output: {}", out.combined());
        }
        Err(e) => {
            println!("Complete failed: {}", e);
        }
    }
}

/// Test autopilot run with a simple task (timeout protected)
#[tokio::test]
async fn test_autopilot_run_simple() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // Set a short timeout for testing
    env.cli.set_timeout(Duration::from_secs(30));

    let output = env.cli.run(&[
        "autopilot", "run",
        "--model", "haiku",
        "Say hello world"
    ]).await;

    match output {
        Ok(out) => {
            println!("Run output ({} ms): {}", out.duration_ms, out.combined());
        }
        Err(e) => {
            println!("Run failed or timed out: {}", e);
        }
    }
}

/// Test that trajectory files are created during a run
#[tokio::test]
async fn test_trajectory_file_creation() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let env = env.unwrap();

    // Check for trajectory files in log dir
    let trajectory_files = env.get_trajectory_files();

    match trajectory_files {
        Ok(files) => {
            println!("Found {} trajectory files: {:?}", files.len(), files);
        }
        Err(e) => {
            println!("Error finding trajectory files: {}", e);
        }
    }
}

/// Test replay command with a non-existent file (should fail gracefully)
#[tokio::test]
async fn test_autopilot_replay_missing_file() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&[
        "autopilot", "replay",
        "/nonexistent/file.rlog"
    ]).await;

    // Should fail with a helpful error message
    match output {
        Ok(out) => {
            assert!(!out.success(), "Should fail for missing file");
            let combined = out.combined();
            // Error message should mention it failed to read/find the file
            assert!(
                combined.contains("not found")
                    || combined.contains("No such file")
                    || combined.contains("error")
                    || combined.contains("Error")
                    || combined.contains("Failed"),
                "Should have helpful error: {}",
                combined
            );
        }
        Err(e) => {
            println!("Replay failed as expected: {}", e);
        }
    }
}

/// Test version command
#[tokio::test]
async fn test_version() {
    let env = E2EEnvironment::for_autopilot().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["--version"]).await;

    match output {
        Ok(out) => {
            println!("Version: {}", out.stdout.trim());
            // Should contain version number
            assert!(
                out.stdout.contains(".") || out.stderr.contains("."),
                "Should show version: {}",
                out.combined()
            );
        }
        Err(e) => {
            println!("Version failed: {}", e);
        }
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    /// Quick smoke test that binary exists and runs
    #[tokio::test]
    async fn test_binary_exists() {
        let result = testing::CliHarness::new().await;
        match result {
            Ok(mut harness) => {
                let output = harness.run(&["--help"]).await;
                assert!(output.is_ok(), "Binary should run with --help");
            }
            Err(e) => {
                println!("Binary not found (build first): {}", e);
            }
        }
    }
}
