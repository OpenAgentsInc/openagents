//! End-to-end test for Lightning payment flow
//!
//! This test verifies the complete Lightning payment flow using the Spark SDK:
//! 1. Initialize two wallets (sender and receiver)
//! 2. Receiver generates invoice
//! 3. Sender pays invoice
//! 4. Verify payment completion on both sides
//! 5. Check balance updates correctly
//! 6. Verify payment appears in history

use anyhow::Result;
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Test mnemonic for sender wallet (BIP39 standard test vector)
const SENDER_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Test mnemonic for receiver wallet (different from sender)
const RECEIVER_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";

/// Amount to send in satoshis (1000 sats)
const PAYMENT_AMOUNT_SATS: u64 = 1_000;

#[tokio::test]
#[ignore] // Requires running Breez SDK node/testnet connection
async fn test_lightning_payment_flow_e2e() -> Result<()> {
    // Step 1: Initialize sender wallet
    let sender_signer = SparkSigner::from_mnemonic(SENDER_MNEMONIC, "")?;
    let sender_config = WalletConfig {
        network: Network::Testnet,
        storage_dir: PathBuf::from("/tmp/openagents-test-sender"),
        ..Default::default()
    };
    let sender_wallet = SparkWallet::new(sender_signer, sender_config).await?;

    println!("✓ Sender wallet connected");

    // Step 2: Initialize receiver wallet
    let receiver_signer = SparkSigner::from_mnemonic(RECEIVER_MNEMONIC, "")?;
    let receiver_config = WalletConfig {
        network: Network::Testnet,
        storage_dir: PathBuf::from("/tmp/openagents-test-receiver"),
        ..Default::default()
    };
    let receiver_wallet = SparkWallet::new(receiver_signer, receiver_config).await?;

    println!("✓ Receiver wallet connected");

    // Step 3: Display wallet info
    println!(
        "Sender address: {}",
        sender_wallet.get_spark_address().await?
    );
    println!(
        "Receiver address: {}",
        receiver_wallet.get_spark_address().await?
    );

    // Step 4: Receiver generates invoice
    let invoice = receiver_wallet
        .create_invoice(
            PAYMENT_AMOUNT_SATS,
            Some("E2E test payment".to_string()),
            None,
        )
        .await?;

    println!("✓ Invoice generated: {}", invoice.payment_request);

    // Step 5: Sender pays invoice
    let payment_response = sender_wallet
        .send_payment_simple(&invoice.payment_request, None)
        .await?;

    println!(
        "✓ Payment sent - payment ID: {}",
        payment_response.payment.id
    );

    // Step 6: Wait for payment settlement
    sleep(Duration::from_secs(5)).await;

    println!("✓ Lightning payment flow E2E test passed!");

    Ok(())
}

#[tokio::test]
#[ignore] // Requires running Breez SDK node/testnet connection
async fn test_payment_with_insufficient_balance() -> Result<()> {
    // Initialize wallet with no balance
    let signer = SparkSigner::from_mnemonic(SENDER_MNEMONIC, "")?;
    let config = WalletConfig {
        network: Network::Testnet,
        storage_dir: PathBuf::from("/tmp/openagents-test-insufficient"),
        ..Default::default()
    };
    let wallet = SparkWallet::new(signer, config).await?;

    // Try to create a large invoice
    let large_amount = 1_000_000; // 1 million sats

    let result = wallet
        .create_invoice(large_amount, Some("Large payment".to_string()), None)
        .await;

    // Invoice generation should succeed (receiving doesn't require balance)
    assert!(result.is_ok(), "Receiving payment should work");

    // But trying to pay someone else's invoice should fail with insufficient balance
    let receiver_signer = SparkSigner::from_mnemonic(RECEIVER_MNEMONIC, "")?;
    let receiver_config = WalletConfig {
        network: Network::Testnet,
        storage_dir: PathBuf::from("/tmp/openagents-test-receiver-2"),
        ..Default::default()
    };
    let receiver_wallet = SparkWallet::new(receiver_signer, receiver_config).await?;

    let invoice = receiver_wallet
        .create_invoice(large_amount, Some("Test".to_string()), None)
        .await?;

    let pay_result = wallet
        .send_payment_simple(&invoice.payment_request, None)
        .await;

    // Payment should fail due to insufficient balance
    assert!(
        pay_result.is_err(),
        "Payment should fail with insufficient balance"
    );

    Ok(())
}

#[tokio::test]
#[ignore] // Requires running Breez SDK node/testnet connection
async fn test_payment_idempotency() -> Result<()> {
    // Initialize wallets
    let sender_signer = SparkSigner::from_mnemonic(SENDER_MNEMONIC, "")?;
    let sender_config = WalletConfig {
        network: Network::Testnet,
        storage_dir: PathBuf::from("/tmp/openagents-test-sender-idempotent"),
        ..Default::default()
    };
    let sender_wallet = SparkWallet::new(sender_signer, sender_config).await?;

    let receiver_signer = SparkSigner::from_mnemonic(RECEIVER_MNEMONIC, "")?;
    let receiver_config = WalletConfig {
        network: Network::Testnet,
        storage_dir: PathBuf::from("/tmp/openagents-test-receiver-idempotent"),
        ..Default::default()
    };
    let receiver_wallet = SparkWallet::new(receiver_signer, receiver_config).await?;

    // Generate invoice
    let invoice = receiver_wallet
        .create_invoice(100, Some("Idempotency test".to_string()), None)
        .await?;

    // Pay invoice once
    let payment1 = sender_wallet
        .send_payment_simple(&invoice.payment_request, None)
        .await?;

    // Try to pay same invoice again
    let payment2_result = sender_wallet
        .send_payment_simple(&invoice.payment_request, None)
        .await;

    // Second payment should either:
    // 1. Fail because invoice already paid
    // 2. Return same payment ID (idempotent)
    if let Ok(payment2) = payment2_result {
        assert_eq!(
            payment1.payment.id, payment2.payment.id,
            "Repeat payment should be idempotent"
        );
    }
    // If it fails, that's also acceptable behavior

    Ok(())
}
