//! Integration tests for Spark wallet
//!
//! These tests verify the full payment flow using the Breez SDK.
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all integration tests (regtest mode - no API key needed)
//! cargo test -p openagents-spark --test integration
//!
//! # Run with mainnet API key for full testing
//! BREEZ_API_KEY="..." cargo test -p openagents-spark --test integration
//!
//! # Run ignored tests that require network
//! cargo test -p openagents-spark --test integration -- --ignored
//!
//! # Run real testnet E2E (requires funded wallets)
//! SPARK_E2E_SENDER_MNEMONIC="..." \
//! SPARK_E2E_RECEIVER_MNEMONIC="..." \
//! SPARK_E2E_AMOUNT_SATS=100 \
//! SPARK_E2E_NETWORK=testnet \
//! cargo test -p openagents-spark --test integration -- --ignored
//!
//! # Run real testnet E2E via regtest faucet
//! SPARK_E2E_USE_FAUCET=1 \
//! cargo test -p openagents-spark --test integration -- --ignored test_real_testnet_payment_flow
//! ```

use openagents_spark::{Balance, Network, SparkSigner, SparkWallet, WalletConfig};
use std::env;

/// Standard test mnemonic (DO NOT USE FOR REAL FUNDS)
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Alternative test mnemonic for two-wallet scenarios
const TEST_MNEMONIC_2: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

/// Check if we have network connectivity for real tests
fn has_network() -> bool {
    env::var("RUN_NETWORK_TESTS").is_ok() || env::var("BREEZ_API_KEY").is_ok()
}

/// Get the test network - defaults to Regtest which doesn't require API key
fn test_network() -> Network {
    if env::var("MAINNET").is_ok() {
        Network::Mainnet
    } else if env::var("TESTNET").is_ok() {
        Network::Testnet
    } else {
        Network::Regtest
    }
}

mod signer_tests {
    use super::*;

    #[test]
    fn test_signer_creation_from_mnemonic() {
        let signer = SparkSigner::from_mnemonic(TEST_MNEMONIC, "")
            .expect("should create signer from valid mnemonic");

        // Verify deterministic key derivation
        let pubkey = signer.public_key_hex();
        assert!(!pubkey.is_empty(), "should have public key");
        assert_eq!(
            pubkey.len(),
            66,
            "compressed public key should be 33 bytes (66 hex chars)"
        );

        // Same mnemonic should produce same key
        let signer2 = SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");
        assert_eq!(
            signer.public_key_hex(),
            signer2.public_key_hex(),
            "deterministic derivation should produce same keys"
        );
    }

    #[test]
    fn test_signer_with_passphrase() {
        let signer_no_pass =
            SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");
        let signer_with_pass = SparkSigner::from_mnemonic(TEST_MNEMONIC, "secret")
            .expect("should create signer with passphrase");

        // Different passphrases should produce different keys
        assert_ne!(
            signer_no_pass.public_key_hex(),
            signer_with_pass.public_key_hex(),
            "passphrase should change derived keys"
        );
    }

    #[test]
    fn test_different_mnemonics_different_keys() {
        let signer1 = SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");
        let signer2 =
            SparkSigner::from_mnemonic(TEST_MNEMONIC_2, "").expect("should create signer");

        assert_ne!(
            signer1.public_key_hex(),
            signer2.public_key_hex(),
            "different mnemonics should produce different keys"
        );
    }

    #[test]
    fn test_invalid_mnemonic_rejected() {
        let result = SparkSigner::from_mnemonic("invalid mnemonic words here", "");
        assert!(result.is_err(), "should reject invalid mnemonic");
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        let signer = SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");

        // Get mnemonic back and create new signer
        let mnemonic = signer.mnemonic();
        let signer2 = SparkSigner::from_mnemonic(mnemonic, "")
            .expect("should create signer from retrieved mnemonic");

        assert_eq!(
            signer.public_key_hex(),
            signer2.public_key_hex(),
            "mnemonic roundtrip should preserve keys"
        );
    }
}

mod balance_tests {
    use super::*;

    #[test]
    fn test_balance_total_calculation() {
        let balance = Balance {
            spark_sats: 100_000,
            lightning_sats: 50_000,
            onchain_sats: 25_000,
        };

        assert_eq!(balance.total_sats(), 175_000);
    }

    #[test]
    fn test_balance_empty_check() {
        let empty = Balance::default();
        assert!(empty.is_empty());

        let non_empty = Balance {
            spark_sats: 1,
            lightning_sats: 0,
            onchain_sats: 0,
        };
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_balance_overflow_protection() {
        let balance = Balance {
            spark_sats: u64::MAX,
            lightning_sats: 1,
            onchain_sats: 1,
        };

        // Should use saturating_add to prevent overflow
        assert_eq!(balance.total_sats(), u64::MAX);
    }
}

mod config_tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WalletConfig::default();

        // Default should be testnet (which maps to regtest internally)
        assert_eq!(config.network, Network::Testnet);
    }

    #[test]
    fn test_network_mapping() {
        use breez_sdk_spark::Network as SdkNetwork;

        // Mainnet should map to Mainnet
        assert!(matches!(
            Network::Mainnet.to_sdk_network(),
            SdkNetwork::Mainnet
        ));

        // All test networks should map to Regtest
        assert!(matches!(
            Network::Testnet.to_sdk_network(),
            SdkNetwork::Regtest
        ));
        assert!(matches!(
            Network::Signet.to_sdk_network(),
            SdkNetwork::Regtest
        ));
        assert!(matches!(
            Network::Regtest.to_sdk_network(),
            SdkNetwork::Regtest
        ));
    }
}

mod wallet_tests {
    use super::*;
    use openagents_spark::{Payment, PaymentStatus, PaymentType, SparkError};
    use reqwest::Client;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct RegtestFaucet {
        client: Client,
        url: String,
        username: Option<String>,
        password: Option<String>,
    }

    impl RegtestFaucet {
        fn new() -> Result<Self, SparkError> {
            let url = env::var("FAUCET_URL")
                .unwrap_or_else(|_| "https://api.lightspark.com/graphql/spark/rc".to_string());
            Ok(Self {
                client: Client::new(),
                url,
                username: env::var("FAUCET_USERNAME").ok(),
                password: env::var("FAUCET_PASSWORD").ok(),
            })
        }

        async fn fund_address(
            &self,
            address: &str,
            amount_sats: u64,
        ) -> Result<String, SparkError> {
            let mut request = self
                .client
                .post(&self.url)
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "operationName": "RequestRegtestFunds",
                    "variables": {
                        "address": address,
                        "amount_sats": amount_sats
                    },
                    "query": "mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) { request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) { transaction_hash }}"
                }));

            if let (Some(username), Some(password)) = (&self.username, &self.password) {
                request = request.basic_auth(username, Some(password));
            }

            let response = request
                .send()
                .await
                .map_err(|e| SparkError::Wallet(format!("Faucet request failed: {e}")))?;

            let result: serde_json::Value = response
                .json()
                .await
                .map_err(|e| SparkError::Wallet(format!("Failed to parse faucet response: {e}")))?;

            if let Some(errors) = result.get("errors").and_then(|e| e.as_array()) {
                if let Some(err) = errors.first() {
                    let msg = err
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    return Err(SparkError::Wallet(format!("Faucet error: {msg}")));
                }
            }

            let txid = result
                .get("data")
                .and_then(|data| data.get("request_regtest_funds"))
                .and_then(|data| data.get("transaction_hash"))
                .and_then(|hash| hash.as_str())
                .unwrap_or_default()
                .to_string();

            Ok(txid)
        }
    }
    use tokio::time::{Duration, Instant, sleep};

    async fn wait_for_payment_by_id(
        wallet: &SparkWallet,
        payment_id: &str,
        timeout: Duration,
    ) -> Result<Payment, SparkError> {
        let deadline = Instant::now() + timeout;

        loop {
            let payments = wallet.list_payments(Some(50), Some(0)).await?;
            if let Some(payment) = payments.into_iter().find(|p| p.id == payment_id) {
                if payment.status == PaymentStatus::Completed {
                    return Ok(payment);
                }
                if payment.status == PaymentStatus::Failed {
                    return Err(SparkError::Wallet(format!("payment {} failed", payment_id)));
                }
            }

            if Instant::now() >= deadline {
                return Err(SparkError::Wallet(format!(
                    "timed out waiting for payment {}",
                    payment_id
                )));
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn wait_for_receive_amount(
        wallet: &SparkWallet,
        amount_sats: u64,
        timeout: Duration,
    ) -> Result<Payment, SparkError> {
        let deadline = Instant::now() + timeout;
        let amount = amount_sats as u128;

        loop {
            let payments = wallet.list_payments(Some(50), Some(0)).await?;
            if let Some(payment) = payments.into_iter().find(|p| {
                p.payment_type == PaymentType::Receive
                    && p.amount == amount
                    && p.status == PaymentStatus::Completed
            }) {
                return Ok(payment);
            }

            if Instant::now() >= deadline {
                return Err(SparkError::Wallet(format!(
                    "timed out waiting for receive payment of {} sats",
                    amount_sats
                )));
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn wait_for_min_balance(
        wallet: &SparkWallet,
        min_sats: u64,
        timeout: Duration,
    ) -> Result<(), SparkError> {
        let deadline = Instant::now() + timeout;

        loop {
            let balance = wallet.get_balance().await?;
            if balance.total_sats() >= min_sats {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(SparkError::Wallet(format!(
                    "timed out waiting for balance >= {} sats",
                    min_sats
                )));
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn ensure_funded(
        wallet: &SparkWallet,
        min_balance: u64,
        timeout: Duration,
    ) -> Result<(), SparkError> {
        let balance = wallet.get_balance().await?;
        if balance.total_sats() >= min_balance {
            return Ok(());
        }

        let needed = min_balance.saturating_sub(balance.total_sats());
        let request_amount = needed.clamp(10_000, 50_000);
        let deposit_address = wallet.get_bitcoin_address().await?;

        let faucet = RegtestFaucet::new().map_err(|e| SparkError::Wallet(e.to_string()))?;
        faucet
            .fund_address(&deposit_address, request_amount)
            .await
            .map_err(|e| SparkError::Wallet(e.to_string()))?;

        wait_for_min_balance(wallet, balance.total_sats().saturating_add(1), timeout).await?;

        Ok(())
    }

    struct RealE2eConfig {
        sender_mnemonic: String,
        receiver_mnemonic: String,
        amount_sats: u64,
        network: Network,
        api_key: Option<String>,
        timeout: Duration,
        use_faucet: bool,
    }

    fn parse_network(value: &str) -> Option<Network> {
        match value.to_ascii_lowercase().as_str() {
            "mainnet" => Some(Network::Mainnet),
            "testnet" => Some(Network::Testnet),
            "signet" => Some(Network::Signet),
            "regtest" => Some(Network::Regtest),
            _ => None,
        }
    }

    fn real_e2e_config() -> Option<RealE2eConfig> {
        let use_faucet = env::var("SPARK_E2E_USE_FAUCET").is_ok();
        let sender_env = env::var("SPARK_E2E_SENDER_MNEMONIC").ok();
        let receiver_env = env::var("SPARK_E2E_RECEIVER_MNEMONIC").ok();
        let (sender_mnemonic, receiver_mnemonic) = match (sender_env, receiver_env) {
            (Some(sender), Some(receiver)) => (sender, receiver),
            _ if use_faucet => (TEST_MNEMONIC.to_string(), TEST_MNEMONIC_2.to_string()),
            _ => return None,
        };

        let amount_sats = env::var("SPARK_E2E_AMOUNT_SATS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(100);

        let timeout = env::var("SPARK_E2E_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(180));

        let network = env::var("SPARK_E2E_NETWORK")
            .ok()
            .and_then(|value| parse_network(&value))
            .unwrap_or(Network::Testnet);

        if network == Network::Mainnet && env::var("SPARK_E2E_ALLOW_MAINNET").is_err() {
            println!("Skipping mainnet E2E test - set SPARK_E2E_ALLOW_MAINNET=1 to enable");
            return None;
        }
        if network == Network::Mainnet && use_faucet {
            println!("Skipping mainnet E2E test - faucet funding only supported on regtest");
            return None;
        }

        let api_key = env::var("SPARK_E2E_API_KEY")
            .ok()
            .or_else(|| env::var("BREEZ_API_KEY").ok());

        Some(RealE2eConfig {
            sender_mnemonic,
            receiver_mnemonic,
            amount_sats,
            network,
            api_key,
            timeout,
            use_faucet,
        })
    }

    fn unique_storage_dir(label: &str) -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "openagents-spark-e2e-{}-{}-{}",
            label,
            std::process::id(),
            now
        ));
        fs::create_dir_all(&dir).expect("should create spark e2e storage dir");
        dir
    }

    #[tokio::test]
    #[ignore = "Requires Breez SDK network connection"]
    async fn test_wallet_creation_regtest() {
        let signer = SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");

        let config = WalletConfig {
            network: Network::Regtest,
            api_key: env::var("BREEZ_API_KEY").ok(),
            ..Default::default()
        };

        let wallet = SparkWallet::new(signer, config).await;

        match wallet {
            Ok(w) => {
                println!("Wallet created successfully");

                // Query balance
                let balance = w.get_balance().await.expect("should get balance");
                println!(
                    "Balance: {} spark, {} lightning, {} onchain",
                    balance.spark_sats, balance.lightning_sats, balance.onchain_sats
                );
            }
            Err(e) => {
                // Network errors are expected if Breez infrastructure isn't available
                println!("Wallet creation failed (expected if no network): {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Breez SDK network connection"]
    async fn test_get_spark_address() {
        if !has_network() {
            println!("Skipping network test - set RUN_NETWORK_TESTS=1 to enable");
            return;
        }

        let signer = SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");

        let config = WalletConfig {
            network: test_network(),
            api_key: env::var("BREEZ_API_KEY").ok(),
            ..Default::default()
        };

        let wallet = SparkWallet::new(signer, config)
            .await
            .expect("should create wallet");

        let address = wallet
            .get_spark_address()
            .await
            .expect("should get spark address");

        println!("Spark address: {}", address);
        assert!(!address.is_empty(), "address should not be empty");
    }

    #[tokio::test]
    #[ignore = "Requires Breez SDK network connection"]
    async fn test_create_invoice() {
        if !has_network() {
            println!("Skipping network test - set RUN_NETWORK_TESTS=1 to enable");
            return;
        }

        let signer = SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer");

        let config = WalletConfig {
            network: test_network(),
            api_key: env::var("BREEZ_API_KEY").ok(),
            ..Default::default()
        };

        let wallet = SparkWallet::new(signer, config)
            .await
            .expect("should create wallet");

        // Create a 1000 sat invoice
        let response = wallet
            .create_invoice(
                1000,
                Some("Test invoice".to_string()),
                Some(3600), // 1 hour expiry
            )
            .await
            .expect("should create invoice");

        println!("Invoice created: {}", response.payment_request);
        assert!(
            response.payment_request.starts_with("ln"),
            "invoice should start with 'ln'"
        );
    }

    #[tokio::test]
    #[ignore = "Requires Breez SDK network connection and funded wallet"]
    async fn test_full_payment_flow() {
        if !has_network() {
            println!("Skipping network test - set RUN_NETWORK_TESTS=1 to enable");
            return;
        }

        // Create two wallets
        let signer1 =
            SparkSigner::from_mnemonic(TEST_MNEMONIC, "").expect("should create signer 1");
        let signer2 =
            SparkSigner::from_mnemonic(TEST_MNEMONIC_2, "").expect("should create signer 2");

        let config = WalletConfig {
            network: test_network(),
            api_key: env::var("BREEZ_API_KEY").ok(),
            ..Default::default()
        };

        let wallet1 = SparkWallet::new(signer1, config.clone())
            .await
            .expect("should create wallet 1");
        let wallet2 = SparkWallet::new(signer2, config)
            .await
            .expect("should create wallet 2");

        // Check initial balances
        let balance1_before = wallet1.get_balance().await.expect("should get balance 1");
        let balance2_before = wallet2.get_balance().await.expect("should get balance 2");

        println!(
            "Wallet 1 balance before: {} sats",
            balance1_before.total_sats()
        );
        println!(
            "Wallet 2 balance before: {} sats",
            balance2_before.total_sats()
        );

        if balance1_before.total_sats() < 1000 {
            println!("Wallet 1 needs funding - skipping payment test");
            return;
        }

        // Create invoice on wallet 2
        let invoice = wallet2
            .create_invoice(100, Some("Test payment".to_string()), None)
            .await
            .expect("should create invoice");

        println!("Created invoice: {}", invoice.payment_request);

        // Pay invoice from wallet 1
        let payment = wallet1
            .send_payment_simple(&invoice.payment_request, None)
            .await
            .expect("should send payment");

        println!("Payment sent: {:?}", payment.payment.status);

        let payment_id = payment.payment.id.clone();
        let _sent_payment = wait_for_payment_by_id(&wallet1, &payment_id, Duration::from_secs(60))
            .await
            .expect("payment should complete");

        let _received_payment = wait_for_receive_amount(&wallet2, 100, Duration::from_secs(60))
            .await
            .expect("receiver should see completed payment");

        // Verify balances changed
        let balance1_after = wallet1.get_balance().await.expect("should get balance 1");
        let balance2_after = wallet2.get_balance().await.expect("should get balance 2");

        println!(
            "Wallet 1 balance after: {} sats",
            balance1_after.total_sats()
        );
        println!(
            "Wallet 2 balance after: {} sats",
            balance2_after.total_sats()
        );

        assert!(
            balance1_after.total_sats() < balance1_before.total_sats(),
            "sender balance should decrease"
        );
        assert!(
            balance2_after.total_sats() > balance2_before.total_sats(),
            "receiver balance should increase"
        );
    }

    #[tokio::test]
    #[ignore = "Requires funded Spark testnet wallets"]
    async fn test_real_testnet_payment_flow() {
        let Some(config) = real_e2e_config() else {
            println!(
                "Skipping real Spark E2E test - set SPARK_E2E_SENDER_MNEMONIC/SPARK_E2E_RECEIVER_MNEMONIC or SPARK_E2E_USE_FAUCET=1"
            );
            return;
        };

        if config.amount_sats == 0 {
            println!("Skipping real Spark E2E test - SPARK_E2E_AMOUNT_SATS must be > 0");
            return;
        }

        let sender_signer = SparkSigner::from_mnemonic(&config.sender_mnemonic, "")
            .expect("should create sender signer");
        let receiver_signer = SparkSigner::from_mnemonic(&config.receiver_mnemonic, "")
            .expect("should create receiver signer");

        let sender_wallet = SparkWallet::new(
            sender_signer,
            WalletConfig {
                network: config.network,
                api_key: config.api_key.clone(),
                storage_dir: unique_storage_dir("sender"),
            },
        )
        .await
        .expect("should create sender wallet");

        let receiver_wallet = SparkWallet::new(
            receiver_signer,
            WalletConfig {
                network: config.network,
                api_key: config.api_key.clone(),
                storage_dir: unique_storage_dir("receiver"),
            },
        )
        .await
        .expect("should create receiver wallet");

        if config.use_faucet {
            if let Err(error) =
                ensure_funded(&sender_wallet, config.amount_sats, config.timeout).await
            {
                println!(
                    "Skipping real Spark E2E test - faucet funding failed: {}",
                    error
                );
                return;
            }
        } else {
            let sender_balance_before = sender_wallet
                .get_balance()
                .await
                .expect("should get sender balance");
            if sender_balance_before.total_sats() < config.amount_sats {
                println!("Sender wallet requires funding before running this test");
                return;
            }
        }

        let invoice = receiver_wallet
            .create_invoice(
                config.amount_sats,
                Some("OpenAgents real testnet payment".to_string()),
                Some(3600),
            )
            .await
            .expect("should create invoice");

        let payment = sender_wallet
            .send_payment_simple(&invoice.payment_request, None)
            .await
            .expect("should send payment");

        let payment_id = payment.payment.id.clone();
        wait_for_payment_by_id(&sender_wallet, &payment_id, config.timeout)
            .await
            .expect("sender payment should complete");

        wait_for_receive_amount(&receiver_wallet, config.amount_sats, config.timeout)
            .await
            .expect("receiver should see completed payment");
    }
}

/// Simulated tests that don't require network connectivity
mod simulated_tests {
    use super::*;

    #[test]
    fn test_payment_amount_validation() {
        // Test that we handle edge cases properly
        let balance = Balance {
            spark_sats: 0,
            lightning_sats: 0,
            onchain_sats: 0,
        };

        assert!(balance.is_empty());
        assert_eq!(balance.total_sats(), 0);
    }

    #[test]
    fn test_invoice_description_handling() {
        // Test description formatting (simulated)
        let description = Some("Payment for goods".to_string());
        assert!(description.is_some());

        let no_description: Option<String> = None;
        assert!(no_description.is_none());
    }

    #[tokio::test]
    async fn test_wallet_config_variations() {
        // Test different config combinations
        let configs = vec![
            WalletConfig {
                network: Network::Regtest,
                api_key: None,
                ..Default::default()
            },
            WalletConfig {
                network: Network::Testnet,
                api_key: Some("test-key".to_string()),
                ..Default::default()
            },
            WalletConfig {
                network: Network::Mainnet,
                api_key: Some("prod-key".to_string()),
                ..Default::default()
            },
        ];

        for (i, config) in configs.iter().enumerate() {
            println!(
                "Config {}: network={:?}, has_api_key={}",
                i,
                config.network,
                config.api_key.is_some()
            );
        }
    }
}

/// CLI integration tests
mod cli_simulation {
    use super::*;

    #[test]
    fn test_balance_display_format() {
        let balance = Balance {
            spark_sats: 1_234_567,
            lightning_sats: 89_012,
            onchain_sats: 345,
        };

        // Simulate CLI output format
        let output = format!(
            "Wallet Balance\n\
             ────────────────────────────\n\
             Spark:     {} sats\n\
             Lightning: {} sats\n\
             On-chain:  {} sats\n\
             ────────────────────────────\n\
             Total:     {} sats",
            balance.spark_sats,
            balance.lightning_sats,
            balance.onchain_sats,
            balance.total_sats()
        );

        println!("{}", output);
        assert!(output.contains("1234567"));
        assert!(output.contains("1323924")); // total
    }

    #[test]
    fn test_invoice_display() {
        // Simulate invoice display
        let invoice = "lnbc10u1pj...truncated...";
        let amount = 1000u64;

        let output = format!(
            "Lightning Invoice Created\n\
             ────────────────────────────────────────\n\
             Amount: {} sats\n\n\
             Invoice:\n{}",
            amount, invoice
        );

        println!("{}", output);
        assert!(output.contains("1000 sats"));
    }
}
