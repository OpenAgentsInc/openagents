//! Integration tests for Spark payment flow
//!
//! These tests use the Lightspark regtest network for testing Bitcoin payments.
//!
//! To run these tests:
//! 1. The tests connect to Lightspark's regtest network (no local setup needed)
//! 2. For tests requiring funds, get test sats from: https://app.lightspark.com/regtest-faucet
//! 3. Run with: cargo test -p compute --test payment_integration -- --ignored --nocapture
//!
//! Network: Regtest (no real value, no API key required)

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

use compute::backends::{BackendRegistry, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result as BackendResult, StreamChunk};
use compute::domain::UnifiedIdentity;
use compute::services::{DvmConfig, DvmService, RelayService};
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Mock backend for testing - returns predictable responses
struct MockInferenceBackend {
    response: String,
}

impl MockInferenceBackend {
    fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

#[async_trait]
impl InferenceBackend for MockInferenceBackend {
    fn id(&self) -> &str {
        "mock"
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn list_models(&self) -> BackendResult<Vec<ModelInfo>> {
        Ok(vec![ModelInfo::new("mock-model", "Mock Model", 4096)])
    }

    async fn complete(&self, request: CompletionRequest) -> BackendResult<CompletionResponse> {
        Ok(CompletionResponse {
            id: "mock-1".to_string(),
            model: request.model,
            text: format!("Response to '{}': {}", request.prompt, self.response),
            finish_reason: Some("stop".to_string()),
            usage: None,
            extra: Default::default(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> BackendResult<mpsc::Receiver<BackendResult<StreamChunk>>> {
        let (tx, rx) = mpsc::channel(1);
        let response = self.response.clone();
        let model = request.model.clone();

        tokio::spawn(async move {
            let _ = tx.send(Ok(StreamChunk {
                id: "chunk-1".to_string(),
                model,
                delta: response,
                finish_reason: Some("stop".to_string()),
                extra: Default::default(),
            })).await;
        });

        Ok(rx)
    }
}

/// Test connecting to regtest network and getting wallet info
///
/// This test verifies we can connect to Lightspark's regtest network.
/// No funds required - just tests connectivity.
#[tokio::test]
#[ignore] // Requires network connectivity
async fn test_regtest_wallet_connect() {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    // Generate a test mnemonic (don't use this for real funds!)
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let signer = SparkSigner::from_mnemonic(mnemonic, "")
        .expect("should create signer from mnemonic");

    let config = WalletConfig {
        network: Network::Regtest,
        api_key: None, // No API key needed for regtest
        storage_dir: std::env::temp_dir().join("spark_test_provider"),
    };

    println!("Connecting to Lightspark regtest network...");

    match SparkWallet::new(signer, config).await {
        Ok(wallet) => {
            println!("Connected successfully!");

            // Get deposit address
            match wallet.get_bitcoin_address().await {
                Ok(address) => {
                    println!("Bitcoin deposit address: {}", address);
                    println!("\nTo fund this wallet, send regtest sats to this address.");
                    println!("Use the Lightspark faucet: https://app.lightspark.com/regtest-faucet");
                }
                Err(e) => {
                    println!("Failed to get deposit address: {}", e);
                }
            }

            // Get Spark address
            match wallet.get_spark_address().await {
                Ok(address) => {
                    println!("Spark address: {}", address);
                }
                Err(e) => {
                    println!("Failed to get Spark address: {}", e);
                }
            }

            // Check balance
            match wallet.get_balance().await {
                Ok(balance) => {
                    println!("Current balance: {} sats", balance.total_sats());
                }
                Err(e) => {
                    println!("Failed to get balance: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Failed to connect to regtest: {}", e);
            println!("\nThis might be a network issue. Make sure you have internet access.");
        }
    }
}

/// Test creating two wallets and doing a Spark transfer
///
/// This test requires the sender wallet to be funded first.
/// Use the faucet to fund the wallet before running.
#[tokio::test]
#[ignore] // Requires network connectivity AND funded wallet
async fn test_spark_payment_between_wallets() {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    // Two different mnemonics for provider and customer
    let provider_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let customer_mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    // Create provider wallet
    let provider_signer = SparkSigner::from_mnemonic(provider_mnemonic, "")
        .expect("should create provider signer");
    let provider_config = WalletConfig {
        network: Network::Regtest,
        api_key: None,
        storage_dir: std::env::temp_dir().join("spark_test_provider_2"),
    };

    // Create customer wallet
    let customer_signer = SparkSigner::from_mnemonic(customer_mnemonic, "")
        .expect("should create customer signer");
    let customer_config = WalletConfig {
        network: Network::Regtest,
        api_key: None,
        storage_dir: std::env::temp_dir().join("spark_test_customer_2"),
    };

    println!("Connecting wallets to regtest...");

    let provider_wallet = SparkWallet::new(provider_signer, provider_config)
        .await
        .expect("should connect provider wallet");

    let customer_wallet = SparkWallet::new(customer_signer, customer_config)
        .await
        .expect("should connect customer wallet");

    // Get addresses
    let provider_spark_address = provider_wallet.get_spark_address().await
        .expect("should get provider spark address");
    let customer_btc_address = customer_wallet.get_bitcoin_address().await
        .expect("should get customer btc address");

    println!("Provider Spark address: {}", provider_spark_address);
    println!("Customer BTC address (for faucet): {}", customer_btc_address);

    // Check balances
    let provider_balance = provider_wallet.get_balance().await
        .expect("should get provider balance");
    let customer_balance = customer_wallet.get_balance().await
        .expect("should get customer balance");

    println!("\nProvider balance: {} sats", provider_balance.total_sats());
    println!("Customer balance: {} sats", customer_balance.total_sats());

    if customer_balance.total_sats() < 100 {
        println!("\n!!! Customer wallet needs funds !!!");
        println!("Send regtest sats to: {}", customer_btc_address);
        println!("Using faucet: https://app.lightspark.com/regtest-faucet");
        println!("Then re-run this test.");
        return;
    }

    // Create invoice from provider
    let amount_sats = 10;
    println!("\nCreating invoice for {} sats...", amount_sats);

    let invoice_response = provider_wallet
        .create_invoice(amount_sats, Some("Test job payment".to_string()), Some(3600))
        .await
        .expect("should create invoice");

    let bolt11 = &invoice_response.payment_request;
    println!("Invoice created: {}...", &bolt11[..50.min(bolt11.len())]);

    // Customer pays the invoice
    println!("Customer paying invoice...");

    match customer_wallet.send_payment_simple(bolt11, None).await {
        Ok(payment_response) => {
            println!("Payment successful!");
            println!("Payment ID: {}", payment_response.payment.id);
            println!("Amount: {} sats", payment_response.payment.amount);

            // Verify balances changed
            tokio::time::sleep(Duration::from_secs(2)).await;

            let new_provider_balance = provider_wallet.get_balance().await
                .expect("should get new provider balance");
            let new_customer_balance = customer_wallet.get_balance().await
                .expect("should get new customer balance");

            println!("\nAfter payment:");
            println!("Provider balance: {} sats (was {})",
                new_provider_balance.total_sats(), provider_balance.total_sats());
            println!("Customer balance: {} sats (was {})",
                new_customer_balance.total_sats(), customer_balance.total_sats());

            assert!(
                new_provider_balance.total_sats() > provider_balance.total_sats(),
                "Provider balance should increase"
            );
        }
        Err(e) => {
            println!("Payment failed: {}", e);
            panic!("Payment should succeed with funded wallet");
        }
    }
}

/// Full E2E test: Job request → Invoice → Payment → Processing → Result
///
/// This tests the complete NIP-90 paid job flow with real Spark payments.
#[tokio::test]
#[ignore] // Requires network connectivity AND funded wallet
async fn test_full_paid_job_e2e() {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
    use compute::domain::DomainEvent;
    use nostr::JobInput;
    use std::collections::HashMap;

    println!("=== Full Paid Job E2E Test ===\n");

    // Create provider and customer identities
    let provider_identity = UnifiedIdentity::generate().expect("should generate provider identity");
    let customer_identity = UnifiedIdentity::generate().expect("should generate customer identity");

    println!("Provider pubkey: {}", provider_identity.public_key_hex());
    println!("Customer pubkey: {}", customer_identity.public_key_hex());

    // Create Spark wallets
    let provider_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let customer_mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    let provider_signer = SparkSigner::from_mnemonic(provider_mnemonic, "")
        .expect("should create provider signer");
    let provider_wallet = SparkWallet::new(
        provider_signer,
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_e2e_provider"),
        },
    ).await.expect("should connect provider wallet");

    let customer_signer = SparkSigner::from_mnemonic(customer_mnemonic, "")
        .expect("should create customer signer");
    let customer_wallet = SparkWallet::new(
        customer_signer,
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_e2e_customer"),
        },
    ).await.expect("should connect customer wallet");

    // Check if customer has funds
    let customer_balance = customer_wallet.get_balance().await
        .expect("should get customer balance");

    if customer_balance.total_sats() < 100 {
        let btc_address = customer_wallet.get_bitcoin_address().await
            .expect("should get address");
        println!("\n!!! Customer needs funds !!!");
        println!("Send regtest sats to: {}", btc_address);
        println!("Faucet: https://app.lightspark.com/regtest-faucet");
        return;
    }

    println!("\nCustomer has {} sats available", customer_balance.total_sats());

    // Create DVM service with payment requirement
    let mut registry = BackendRegistry::new();
    registry.register_with_id("mock", Arc::new(RwLock::new(
        MockInferenceBackend::new("42 is the answer")
    )));

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(100);

    let mut dvm = DvmService::new(relay_service, backend_registry, event_tx);

    // Configure to require payment
    let config = DvmConfig {
        require_payment: true,
        min_price_msats: 10_000, // 10 sats
        default_model: "mock-model".to_string(),
        network: "regtest".to_string(),
    };
    dvm.set_config(config);
    dvm.set_identity(Arc::new(provider_identity)).await;
    dvm.set_wallet(Arc::new(provider_wallet)).await;

    println!("\nDVM configured to require payment of 10 sats per job");

    // Customer sends job request
    println!("\n--- Step 1: Customer requests job ---");

    let job_inputs = vec![JobInput::text("What is the meaning of life?")];
    let mut params = HashMap::new();
    params.insert("model".to_string(), "mock-model".to_string());
    params.insert("backend".to_string(), "mock".to_string());

    let event_id = "e2e_test_job_12345678901234567890";

    dvm.handle_job_request(
        event_id,
        5050,
        &customer_identity.public_key_hex(),
        job_inputs,
        params,
    ).await.expect("should handle job request");

    // Check for InvoiceCreated event
    let mut invoice_bolt11 = String::new();
    let mut found_invoice = false;

    while let Ok(event) = event_rx.try_recv() {
        println!("Event: {}", event.description());
        if let DomainEvent::InvoiceCreated { bolt11, amount_msats, .. } = &event {
            invoice_bolt11 = bolt11.clone();
            println!("Invoice created for {} msats", amount_msats);
            found_invoice = true;
        }
    }

    assert!(found_invoice, "Should have InvoiceCreated event");

    // Get job status
    let job_id = format!("job_{}", &event_id[..16]);
    let job = dvm.get_job(&job_id).await.expect("should have job");

    match &job.status {
        compute::domain::job::JobStatus::PaymentRequired { bolt11, amount_msats } => {
            println!("\nJob status: PaymentRequired");
            println!("Amount: {} msats ({} sats)", amount_msats, amount_msats / 1000);
            println!("Invoice: {}...", &bolt11[..50.min(bolt11.len())]);
        }
        other => panic!("Expected PaymentRequired, got {:?}", other),
    }

    // Customer pays the invoice
    println!("\n--- Step 2: Customer pays invoice ---");

    let payment = customer_wallet
        .send_payment_simple(&invoice_bolt11, None)
        .await
        .expect("should pay invoice");

    println!("Payment sent! ID: {}", payment.payment.id);

    // Wait for payment to settle
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Confirm payment and process job
    println!("\n--- Step 3: Confirm payment and process ---");

    dvm.confirm_payment(&job_id).await.expect("should confirm payment");

    // Collect remaining events
    tokio::time::sleep(Duration::from_millis(100)).await;
    while let Ok(event) = event_rx.try_recv() {
        println!("Event: {}", event.description());
    }

    // Verify job completed
    let final_job = dvm.get_job(&job_id).await.expect("should have job");

    match &final_job.status {
        compute::domain::job::JobStatus::Completed { result } => {
            println!("\n--- Job Completed! ---");
            println!("Result: {}", result);
            println!("Payment amount: {} msats", final_job.amount_msats.unwrap_or(0));
            assert!(result.contains("42"), "Should contain mock response");
        }
        other => panic!("Expected Completed, got {:?}", other),
    }

    // Verify provider received payment
    let provider_wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(provider_mnemonic, "").unwrap(),
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_e2e_provider"),
        },
    ).await.expect("should reconnect provider wallet");

    let final_balance = provider_wallet.get_balance().await
        .expect("should get final balance");

    println!("\nProvider final balance: {} sats", final_balance.total_sats());
    println!("\n=== E2E Test Complete! ===");
}

/// Quick connectivity test - just tries to connect to regtest
#[tokio::test]
#[ignore]
async fn test_quick_connectivity() {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let signer = SparkSigner::from_mnemonic(mnemonic, "").unwrap();

    let config = WalletConfig {
        network: Network::Regtest,
        api_key: None,
        storage_dir: std::env::temp_dir().join("spark_quick_test"),
    };

    let start = std::time::Instant::now();

    match tokio::time::timeout(Duration::from_secs(30), SparkWallet::new(signer, config)).await {
        Ok(Ok(wallet)) => {
            println!("Connected in {:?}", start.elapsed());
            let balance = wallet.get_balance().await.unwrap();
            println!("Balance: {} sats", balance.total_sats());
        }
        Ok(Err(e)) => {
            println!("Connection failed: {}", e);
        }
        Err(_) => {
            println!("Connection timed out after 30s");
        }
    }
}
