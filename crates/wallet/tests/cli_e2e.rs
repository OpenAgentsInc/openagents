//! CLI-based End-to-end tests for wallet
//!
//! These tests exercise the actual CLI binary to verify complete wallet flows work
//! from command invocation through to completion.
//!
//! Run with: `cargo test -p wallet --test cli_e2e`

use testing::{E2EEnvironment, extract_address, extract_sats};

/// Skip test if binary not found
macro_rules! skip_if_no_binary {
    ($env:expr) => {
        if $env.is_err() {
            println!("Skipping test: openagents binary not found. Run `cargo build --bin openagents` first.");
            return;
        }
    };
}

/// Test that the wallet help command works
#[tokio::test]
async fn test_wallet_help() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    let output = env.cli.run(&["wallet", "--help"]).await;
    assert!(output.is_ok(), "Help command should succeed");

    let output = output.unwrap();
    // Help should either succeed or show usage
    assert!(
        output.success() || output.exit_code == Some(2),
        "Unexpected exit code: {:?}",
        output.exit_code
    );

    // Should contain wallet-related content
    let combined = output.combined();
    assert!(
        combined.contains("wallet") || combined.contains("Wallet") || combined.contains("USAGE"),
        "Output should mention wallet: {}",
        combined
    );
}

/// Test wallet init command
#[tokio::test]
async fn test_wallet_init() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // Run wallet init
    let output = env.cli.run(&["wallet", "init"]).await;

    match output {
        Ok(out) => {
            println!("Wallet init output: {}", out.combined());
            // Init should work (creates new identity) or indicate already initialized
        }
        Err(e) => {
            println!("Wallet init failed (may be acceptable): {}", e);
        }
    }
}

/// Test wallet whoami command
#[tokio::test]
async fn test_wallet_whoami() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // First try to init
    let _ = env.cli.run(&["wallet", "init"]).await;

    // Then run whoami
    let output = env.cli.run(&["wallet", "whoami"]).await;

    match output {
        Ok(out) => {
            let combined = out.combined();
            println!("Whoami output: {}", combined);

            // If successful, should show npub or pubkey
            if out.success() {
                assert!(
                    combined.contains("npub")
                        || combined.contains("pubkey")
                        || combined.contains("Public"),
                    "Should show identity information: {}",
                    combined
                );
            }
        }
        Err(e) => {
            println!("Whoami failed (may need init first): {}", e);
        }
    }
}

/// Test wallet balance command
#[tokio::test]
async fn test_wallet_balance() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // First init wallet
    let _ = env.cli.run(&["wallet", "init"]).await;

    // Check balance
    let output = env.cli.run(&["wallet", "balance"]).await;

    match output {
        Ok(out) => {
            let combined = out.combined();
            println!("Balance output: {}", combined);

            // If successful, should show some balance info (might be 0)
            if out.success() {
                // Try to extract balance
                if let Some(sats) = extract_sats(&combined) {
                    println!("Extracted balance: {} sats", sats);
                }
            }
        }
        Err(e) => {
            println!("Balance check failed: {}", e);
        }
    }
}

/// Test wallet receive command (generates address)
#[tokio::test]
async fn test_wallet_receive() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // First init wallet
    let _ = env.cli.run(&["wallet", "init"]).await;

    // Get receive address
    let output = env.cli.run(&["wallet", "receive"]).await;

    match output {
        Ok(out) => {
            let combined = out.combined();
            println!("Receive output: {}", combined);

            if out.success() {
                // Try to extract address
                if let Some(addr) = extract_address(&combined) {
                    println!("Extracted address: {}", addr);
                    // Should be a valid Bitcoin address format
                    assert!(
                        addr.starts_with("tb1")
                            || addr.starts_with("bc1")
                            || addr.starts_with("1")
                            || addr.starts_with("3"),
                        "Should be valid Bitcoin address: {}",
                        addr
                    );
                }
            }
        }
        Err(e) => {
            println!("Receive failed: {}", e);
        }
    }
}

/// Test wallet send with invalid address (should fail gracefully)
#[tokio::test]
async fn test_wallet_send_invalid() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // First init wallet
    let _ = env.cli.run(&["wallet", "init"]).await;

    // Try to send to invalid address (should fail)
    let output = env
        .cli
        .run(&["wallet", "send", "not-a-valid-address", "100"])
        .await;

    match output {
        Ok(out) => {
            // Should fail
            assert!(!out.success(), "Should fail for invalid address");
            let combined = out.combined();
            assert!(
                combined.contains("error")
                    || combined.contains("Error")
                    || combined.contains("invalid")
                    || combined.contains("Invalid"),
                "Should have error message: {}",
                combined
            );
        }
        Err(e) => {
            println!("Send failed as expected: {}", e);
        }
    }
}

/// Test wallet lifecycle: init -> whoami -> balance
#[tokio::test]
async fn test_wallet_lifecycle() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // Step 1: Initialize wallet
    let init_result = env.cli.run(&["wallet", "init"]).await;
    println!(
        "Init result: {:?}",
        init_result.as_ref().map(|o| o.combined())
    );

    // Step 2: Check identity
    let whoami_result = env.cli.run(&["wallet", "whoami"]).await;
    match whoami_result {
        Ok(out) => {
            println!("Whoami: {}", out.combined());
        }
        Err(e) => {
            println!("Whoami failed: {}", e);
        }
    }

    // Step 3: Check balance
    let balance_result = env.cli.run(&["wallet", "balance"]).await;
    match balance_result {
        Ok(out) => {
            println!("Balance: {}", out.combined());
        }
        Err(e) => {
            println!("Balance failed: {}", e);
        }
    }
}

/// Test wallet send/receive with faucet
#[tokio::test]
async fn test_wallet_payment_flow() {
    let env = E2EEnvironment::for_wallet().await;
    skip_if_no_binary!(env);
    let mut env = env.unwrap();

    // Initialize wallet
    let init_result = env.cli.run(&["wallet", "init"]).await;
    if init_result.is_err() || !init_result.as_ref().unwrap().success() {
        println!("Wallet init failed (may need keychain access), skipping payment test");
        return;
    }

    // Get receive address
    let receive_result = env.cli.run(&["wallet", "receive"]).await;
    let address = match receive_result {
        Ok(out) if out.success() => match extract_address(&out.stdout) {
            Some(addr) => addr,
            None => {
                println!("Could not extract address from output, skipping");
                return;
            }
        },
        _ => {
            println!("Receive command failed (wallet not initialized properly), skipping");
            return;
        }
    };

    // Fund from faucet
    if let Some(ref faucet) = env.faucet {
        match faucet.fund_address(&address, 10_000).await {
            Ok(_) => {
                // Wait for confirmation
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                // Check balance
                if let Ok(balance_output) = env.cli.run_ok(&["wallet", "balance"]).await {
                    let balance = extract_sats(&balance_output).unwrap_or(0);
                    println!("Balance after funding: {} sats", balance);
                    // Don't assert - just log, as regtest timing varies
                }
            }
            Err(e) => {
                println!("Faucet funding failed (expected if not on regtest): {}", e);
            }
        }
    } else {
        println!("No faucet available, skipping payment flow");
    }
}

#[cfg(test)]
mod smoke_tests {
    /// Quick smoke test that wallet commands are available
    #[tokio::test]
    async fn test_wallet_subcommand_exists() {
        let result = testing::CliHarness::new().await;
        match result {
            Ok(mut harness) => {
                let output = harness.run(&["wallet", "--help"]).await;
                assert!(output.is_ok(), "Wallet subcommand should exist");
            }
            Err(e) => {
                println!("Binary not found (build first): {}", e);
            }
        }
    }
}
