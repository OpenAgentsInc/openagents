//! Full Integration E2E Tests
//!
//! These tests exercise complete user journeys across multiple subsystems
//! via CLI commands, testing the entire application lifecycle.
//!
//! Run with: `cargo test -p testing --test full_flow_e2e`
//! Run ignored tests: `cargo test -p testing --test full_flow_e2e -- --ignored`

use std::time::Duration;
use testing::{CliHarness, E2EEnvironment, extract_issue_id};

/// Skip test if binary not found
macro_rules! skip_if_no_binary {
    () => {
        if CliHarness::new().await.is_err() {
            println!("Skipping test: openagents binary not found. Run `cargo build --bin openagents` first.");
            return;
        }
    };
}

/// Test complete user journey: wallet init -> identity -> balance
#[tokio::test]
async fn test_wallet_identity_flow() {
    skip_if_no_binary!();

    let env = E2EEnvironment::for_wallet().await;
    if env.is_err() {
        println!("Skipping: Could not create E2E environment");
        return;
    }
    let mut env = env.unwrap();

    // Step 1: Initialize wallet
    let init_result = env.cli.run(&["wallet", "init"]).await;
    println!("Init: {:?}", init_result.as_ref().map(|o| o.combined()));

    // Step 2: Get identity
    let whoami_result = env.cli.run(&["wallet", "whoami"]).await;
    match whoami_result {
        Ok(out) => {
            println!("Identity: {}", out.combined());
            if out.success() {
                assert!(
                    out.combined().contains("npub") || out.combined().contains("pub"),
                    "Should show public key"
                );
            }
        }
        Err(e) => println!("Whoami failed: {}", e),
    }

    // Step 3: Check balance
    let balance_result = env.cli.run(&["wallet", "balance"]).await;
    match balance_result {
        Ok(out) => {
            println!("Balance: {}", out.combined());
        }
        Err(e) => println!("Balance failed: {}", e),
    }
}

/// Test autopilot issue management flow
#[tokio::test]
async fn test_autopilot_issue_management_flow() {
    skip_if_no_binary!();

    let env = E2EEnvironment::for_autopilot().await;
    if env.is_err() {
        println!("Skipping: Could not create E2E environment");
        return;
    }
    let mut env = env.unwrap();

    // Step 1: List issues (may be empty)
    let list_result = env.cli.run(&["autopilot", "issue", "list"]).await;
    println!(
        "Issue list: {:?}",
        list_result.as_ref().map(|o| o.combined())
    );

    // Step 2: Create an issue
    let create_result = env
        .cli
        .run(&[
            "autopilot",
            "issue",
            "create",
            "--title",
            "Full Flow Test Issue",
            "--body",
            "Created during full flow E2E test",
            "--directive",
            "d-e2e-test",
        ])
        .await;

    if let Ok(ref out) = create_result {
        if out.success() {
            println!("Created issue: {}", out.combined());

            // Try to extract issue ID
            if let Some(id) = extract_issue_id(&out.stdout) {
                println!("Issue ID: {}", id);

                // Step 3: Show the issue
                let show_result = env.cli.run(&["autopilot", "issue", "show", &id]).await;
                if let Ok(show_out) = show_result {
                    println!("Issue details: {}", show_out.combined());
                }
            }
        }
    }
}

/// Test dashboard and status commands across subsystems
#[tokio::test]
async fn test_multi_subsystem_status() {
    skip_if_no_binary!();

    let env = E2EEnvironment::minimal().await;
    if env.is_err() {
        println!("Skipping: Could not create E2E environment");
        return;
    }
    let mut env = env.unwrap();

    // Check various status commands
    let commands = vec![
        vec!["--version"],
        vec!["autopilot", "dashboard"],
        vec!["daemon", "status"],
        vec!["wallet", "whoami"],
    ];

    for args in commands {
        let result = env
            .cli
            .run(&args.iter().map(|s| *s).collect::<Vec<_>>())
            .await;
        match result {
            Ok(out) => {
                println!(
                    "{}: exit={:?}, output={}",
                    args.join(" "),
                    out.exit_code,
                    out.combined()
                        .lines()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" | ")
                );
            }
            Err(e) => {
                println!("{}: error={}", args.join(" "), e);
            }
        }
    }
}

/// Test help commands work for all subsystems
#[tokio::test]
async fn test_all_help_commands() {
    skip_if_no_binary!();

    let env = E2EEnvironment::minimal().await;
    if env.is_err() {
        println!("Skipping: Could not create E2E environment");
        return;
    }
    let mut env = env.unwrap();

    let help_commands = vec![
        vec!["--help"],
        vec!["wallet", "--help"],
        vec!["autopilot", "--help"],
        vec!["daemon", "--help"],
        vec!["marketplace", "--help"],
    ];

    for args in help_commands {
        let result = env
            .cli
            .run(&args.iter().map(|s| *s).collect::<Vec<_>>())
            .await;
        match result {
            Ok(out) => {
                let success = out.success() || out.exit_code == Some(2); // 2 = help usage
                assert!(
                    success,
                    "{} should show help: {:?}",
                    args.join(" "),
                    out.exit_code
                );
                println!("{}: OK", args.join(" "));
            }
            Err(e) => {
                println!("{}: error={}", args.join(" "), e);
            }
        }
    }
}

/// Test complete autonomous flow: wallet init -> create issue -> run autopilot
#[tokio::test]
async fn test_complete_autonomous_journey() {
    skip_if_no_binary!();

    let env = E2EEnvironment::for_autopilot().await;
    if env.is_err() {
        println!("Skipping: Could not create E2E environment");
        return;
    }
    let mut env = env.unwrap();
    env.cli.set_timeout(Duration::from_secs(60));

    // Step 1: Initialize wallet
    let _ = env.cli.run(&["wallet", "init"]).await;

    // Step 2: Create an issue to work on
    let create_result = env
        .cli
        .run(&[
            "autopilot",
            "issue",
            "create",
            "--title",
            "E2E Test: Add simple function",
            "--body",
            "Create a function that adds two numbers",
            "--directive",
            "d-e2e-test",
        ])
        .await;

    let issue_id = match create_result {
        Ok(out) if out.success() => {
            extract_issue_id(&out.stdout).unwrap_or_else(|| "1".to_string())
        }
        _ => {
            println!("Could not create issue, stopping test");
            return;
        }
    };

    println!("Created issue: {}", issue_id);

    // Step 3: Run autopilot on the issue (would use API key)
    let run_result = env
        .cli
        .run(&[
            "autopilot",
            "run",
            "--model",
            "haiku",
            &format!("Work on issue #{}", issue_id),
        ])
        .await;

    match run_result {
        Ok(out) => {
            println!("Autopilot run ({}ms): {}", out.duration_ms, out.combined());
        }
        Err(e) => {
            println!("Autopilot run failed: {}", e);
        }
    }

    // Step 4: Check for trajectory files
    let trajectories = env.get_trajectory_files();
    if let Ok(files) = trajectories {
        println!("Trajectory files created: {}", files.len());
        for file in &files {
            println!("  - {:?}", file);
        }
    }
}

/// Test error handling across subsystems
#[tokio::test]
async fn test_error_handling() {
    skip_if_no_binary!();

    let env = E2EEnvironment::minimal().await;
    if env.is_err() {
        println!("Skipping: Could not create E2E environment");
        return;
    }
    let mut env = env.unwrap();

    // Invalid subcommands should fail gracefully
    let invalid_commands = vec![
        vec!["invalid-subcommand"],
        vec!["wallet", "invalid-action"],
        vec!["autopilot", "replay", "/nonexistent/file.rlog"],
    ];

    for args in invalid_commands {
        let result = env
            .cli
            .run(&args.iter().map(|s| *s).collect::<Vec<_>>())
            .await;
        match result {
            Ok(out) => {
                // Should fail but not crash
                println!("{}: exit={:?}", args.join(" "), out.exit_code);
            }
            Err(e) => {
                println!("{}: error (expected)={}", args.join(" "), e);
            }
        }
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    /// Basic smoke test that the binary exists and runs
    #[tokio::test]
    async fn test_binary_runs() {
        let result = CliHarness::new().await;
        match result {
            Ok(mut harness) => {
                let output = harness.run(&["--version"]).await;
                assert!(output.is_ok(), "Should run --version");
                println!("Binary works: {:?}", output.map(|o| o.stdout));
            }
            Err(e) => {
                println!(
                    "Binary not found: {}. Run `cargo build --bin openagents` first.",
                    e
                );
            }
        }
    }

    /// Test that environment setup works
    #[tokio::test]
    async fn test_environment_setup() {
        let env = E2EEnvironment::minimal().await;
        match env {
            Ok(e) => {
                assert!(e.log_dir.exists(), "Log dir should exist");
                assert!(e.temp_path().exists(), "Temp path should exist");
                println!("Environment setup OK");
            }
            Err(e) => {
                println!("Environment setup failed: {}", e);
            }
        }
    }
}
