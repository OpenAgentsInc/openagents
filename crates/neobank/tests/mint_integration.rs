//! Integration tests for neobank mint connectivity
//!
//! These tests connect to real Cashu mints to verify the wallet implementation.

use neobank::{CashuWallet, Currency, MintConfig};
use tempfile::tempdir;

#[tokio::test]
async fn test_connect_to_btc_mint() {
    let mint = MintConfig::default_btc_mint();
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("wallet.redb");
    let seed = [0u8; 32]; // Test seed

    let wallet = CashuWallet::new(mint.url, Currency::Btc, &seed, &db_path)
        .await
        .expect("Failed to create wallet");

    // New wallet should have zero balance
    let balance = wallet.balance().await.expect("Failed to get balance");
    assert_eq!(balance.currency, Currency::Btc);
    assert_eq!(balance.value, 0);

    // Verify wallet metadata
    assert_eq!(wallet.currency(), Currency::Btc);
    assert!(wallet.mint_url().as_str().contains("minibits"));
}

#[tokio::test]
async fn test_connect_to_usd_mint() {
    let mint = MintConfig::default_usd_mint();
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("wallet.redb");
    let seed = [1u8; 32]; // Different seed

    let wallet = CashuWallet::new(mint.url, Currency::Usd, &seed, &db_path)
        .await
        .expect("Failed to create wallet");

    let balance = wallet.balance().await.expect("Failed to get balance");
    assert_eq!(balance.currency, Currency::Usd);
    assert_eq!(balance.value, 0);

    assert_eq!(wallet.currency(), Currency::Usd);
    assert!(wallet.mint_url().as_str().contains("stablenut"));
}

/// Test creating a mint quote (requires mint to support v1 API)
/// This test may fail if the mint is unavailable or has API changes
#[tokio::test]
#[ignore = "requires live mint with v1 API support"]
async fn test_create_mint_quote() {
    let mint = MintConfig::default_btc_mint();
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("wallet.redb");
    let seed = [2u8; 32];

    let wallet = CashuWallet::new(mint.url, Currency::Btc, &seed, &db_path)
        .await
        .expect("Failed to create wallet");

    // Create a quote for 1000 sats
    let quote = wallet
        .create_mint_quote(1000)
        .await
        .expect("Failed to create mint quote");

    // Verify quote
    assert!(!quote.id.is_empty(), "Quote ID should not be empty");
    assert!(!quote.bolt11.is_empty(), "Invoice should not be empty");
    assert!(
        quote.bolt11.starts_with("lnbc") || quote.bolt11.starts_with("lntb"),
        "Invoice should be a Lightning invoice"
    );
    assert_eq!(quote.amount, 1000);
}

#[tokio::test]
async fn test_proof_count_empty_wallet() {
    let mint = MintConfig::default_btc_mint();
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("wallet.redb");
    let seed = [3u8; 32];

    let wallet = CashuWallet::new(mint.url, Currency::Btc, &seed, &db_path)
        .await
        .expect("Failed to create wallet");

    let count = wallet.proof_count().await.expect("Failed to get proof count");
    assert_eq!(count, 0, "New wallet should have no proofs");
}

#[tokio::test]
async fn test_alt_btc_mint() {
    let mint = MintConfig::alt_btc_mint();
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("wallet.redb");
    let seed = [4u8; 32];

    let wallet = CashuWallet::new(mint.url, Currency::Btc, &seed, &db_path)
        .await
        .expect("Failed to create wallet");

    assert_eq!(wallet.currency(), Currency::Btc);
    assert!(wallet.mint_url().as_str().contains("8333"));
}
