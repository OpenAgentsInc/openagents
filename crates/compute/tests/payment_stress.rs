//! Stress tests for payment throughput
//!
//! Measures maximum jobs/payments per second the system can handle.
//!
//! Run with: cargo test -p compute --test payment_stress -- --ignored --nocapture

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock, Semaphore};

use compute::backends::{BackendRegistry, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result as BackendResult, StreamChunk};
use compute::domain::UnifiedIdentity;
use compute::services::{DvmConfig, DvmService, RelayService};
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Ultra-fast mock backend for stress testing
struct FastMockBackend {
    response_delay_us: u64,
}

impl FastMockBackend {
    fn new(response_delay_us: u64) -> Self {
        Self { response_delay_us }
    }
}

#[async_trait]
impl InferenceBackend for FastMockBackend {
    fn id(&self) -> &str {
        "fast-mock"
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn list_models(&self) -> BackendResult<Vec<ModelInfo>> {
        Ok(vec![ModelInfo::new("fast-model", "Fast Model", 4096)])
    }

    async fn complete(&self, request: CompletionRequest) -> BackendResult<CompletionResponse> {
        if self.response_delay_us > 0 {
            tokio::time::sleep(Duration::from_micros(self.response_delay_us)).await;
        }
        Ok(CompletionResponse {
            id: "fast-1".to_string(),
            model: request.model,
            text: "done".to_string(),
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
        let delay = self.response_delay_us;
        let model = request.model.clone();

        tokio::spawn(async move {
            if delay > 0 {
                tokio::time::sleep(Duration::from_micros(delay)).await;
            }
            let _ = tx.send(Ok(StreamChunk {
                id: "chunk-1".to_string(),
                model,
                delta: "done".to_string(),
                finish_reason: Some("stop".to_string()),
                extra: Default::default(),
            })).await;
        });

        Ok(rx)
    }
}

/// Stress test: measure job processing throughput without payments
#[tokio::test]
#[ignore]
async fn stress_test_job_throughput_no_payment() {
    use nostr::JobInput;
    use std::collections::HashMap;

    println!("\n=== Job Throughput Stress Test (No Payment) ===\n");

    let provider_identity = UnifiedIdentity::generate().expect("generate identity");

    // Setup DVM with fast mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id("fast-mock", Arc::new(RwLock::new(FastMockBackend::new(0))));

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, _event_rx) = broadcast::channel(10000);

    let dvm = Arc::new(RwLock::new(DvmService::new(relay_service, backend_registry, event_tx)));

    // Configure for no payment required
    {
        let mut dvm = dvm.write().await;
        dvm.set_config(DvmConfig {
            require_payment: false,
            min_price_msats: 0,
            default_model: "fast-model".to_string(),
            network: "regtest".to_string(),
        });
        dvm.set_identity(Arc::new(provider_identity)).await;
    }

    // Test parameters
    let test_durations = vec![1, 5, 10];
    let concurrency_levels = vec![1, 10, 50, 100, 500];

    for duration_secs in &test_durations {
        println!("--- Test duration: {}s ---", duration_secs);

        for concurrency in &concurrency_levels {
            let completed = Arc::new(AtomicU64::new(0));
            let errors = Arc::new(AtomicU64::new(0));
            let semaphore = Arc::new(Semaphore::new(*concurrency));

            let start = Instant::now();
            let deadline = start + Duration::from_secs(*duration_secs);

            let mut handles = vec![];

            // Spawn worker tasks
            for _ in 0..*concurrency {
                let dvm = dvm.clone();
                let completed = completed.clone();
                let errors = errors.clone();
                let sem = semaphore.clone();

                let handle = tokio::spawn(async move {
                    let customer_pubkey = format!("{:064x}", rand::random::<u64>());

                    while Instant::now() < deadline {
                        let _permit = sem.acquire().await.unwrap();

                        let job_inputs = vec![JobInput::text("test")];
                        let mut params = HashMap::new();
                        params.insert("model".to_string(), "fast-model".to_string());
                        params.insert("backend".to_string(), "fast-mock".to_string());

                        let event_id = format!("stress_{:016x}", rand::random::<u64>());

                        let result = {
                            let mut dvm = dvm.write().await;
                            dvm.handle_job_request(
                                &event_id,
                                5050,
                                &customer_pubkey,
                                job_inputs,
                                params,
                            ).await
                        };

                        match result {
                            Ok(_) => {
                                completed.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                errors.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                });

                handles.push(handle);
            }

            // Wait for all workers
            for handle in handles {
                let _ = handle.await;
            }

            let elapsed = start.elapsed();
            let total_completed = completed.load(Ordering::Relaxed);
            let total_errors = errors.load(Ordering::Relaxed);
            let throughput = total_completed as f64 / elapsed.as_secs_f64();

            println!(
                "  Concurrency {:4}: {:6} jobs in {:5.2}s = {:8.1} jobs/sec (errors: {})",
                concurrency, total_completed, elapsed.as_secs_f64(), throughput, total_errors
            );
        }
        println!();
    }
}

/// Stress test: measure payment processing throughput
#[tokio::test]
#[ignore]
async fn stress_test_payment_throughput() {
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

    println!("\n=== Payment Throughput Stress Test ===\n");

    // Create wallets
    let provider_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let customer_mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    println!("Connecting wallets...");

    let provider_signer = SparkSigner::from_mnemonic(provider_mnemonic, "")
        .expect("create provider signer");
    let provider_wallet = match SparkWallet::new(
        provider_signer,
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_stress_provider"),
        },
    ).await {
        Ok(w) => w,
        Err(e) => {
            println!("Failed to connect provider wallet: {}", e);
            println!("Skipping payment stress test (requires network)");
            return;
        }
    };

    let customer_signer = SparkSigner::from_mnemonic(customer_mnemonic, "")
        .expect("create customer signer");
    let customer_wallet = match SparkWallet::new(
        customer_signer,
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_stress_customer"),
        },
    ).await {
        Ok(w) => w,
        Err(e) => {
            println!("Failed to connect customer wallet: {}", e);
            return;
        }
    };

    // Check balances
    let provider_balance = provider_wallet.get_balance().await
        .expect("get provider balance");
    let customer_balance = customer_wallet.get_balance().await
        .expect("get customer balance");

    println!("Provider balance: {} sats", provider_balance.total_sats());
    println!("Customer balance: {} sats", customer_balance.total_sats());

    if customer_balance.total_sats() < 1000 {
        let btc_address = customer_wallet.get_bitcoin_address().await
            .expect("get address");
        println!("\n!!! Customer needs more funds for stress test !!!");
        println!("Send regtest sats to: {}", btc_address);
        println!("Faucet: https://app.lightspark.com/regtest-faucet");
        return;
    }

    // Test invoice creation throughput
    println!("\n--- Invoice Creation Throughput ---");

    let provider_wallet = Arc::new(provider_wallet);
    let customer_wallet = Arc::new(customer_wallet);

    for batch_size in [1, 5, 10, 20] {
        let start = Instant::now();
        let mut invoices = vec![];

        for i in 0..batch_size {
            let invoice = provider_wallet
                .create_invoice(1, Some(format!("stress test {}", i)), Some(300))
                .await
                .expect("create invoice");
            invoices.push(invoice);
        }

        let elapsed = start.elapsed();
        let rate = batch_size as f64 / elapsed.as_secs_f64();
        println!(
            "  {} invoices in {:6.3}s = {:6.1} invoices/sec",
            batch_size, elapsed.as_secs_f64(), rate
        );
    }

    // Test payment throughput (sequential - Lightning is serial per channel)
    println!("\n--- Payment Throughput (Sequential) ---");

    let num_payments = 5;
    let amount_per_payment = 1; // 1 sat each

    // Create invoices first
    let mut invoices = vec![];
    for i in 0..num_payments {
        let invoice = provider_wallet
            .create_invoice(amount_per_payment, Some(format!("payment {}", i)), Some(300))
            .await
            .expect("create invoice");
        invoices.push(invoice.payment_request);
    }

    println!("Created {} invoices, now paying...", num_payments);

    let start = Instant::now();
    let mut successful = 0;
    let mut failed = 0;

    for bolt11 in invoices {
        match customer_wallet.send_payment_simple(&bolt11, None).await {
            Ok(_) => successful += 1,
            Err(e) => {
                failed += 1;
                println!("  Payment failed: {}", e);
            }
        }
    }

    let elapsed = start.elapsed();
    let rate = successful as f64 / elapsed.as_secs_f64();
    println!(
        "  {} payments in {:6.3}s = {:6.2} payments/sec (failed: {})",
        successful, elapsed.as_secs_f64(), rate, failed
    );

    // Final balance check
    let final_provider = provider_wallet.get_balance().await.expect("final provider balance");
    let final_customer = customer_wallet.get_balance().await.expect("final customer balance");

    println!("\n--- Final Balances ---");
    println!("Provider: {} sats (was {})", final_provider.total_sats(), provider_balance.total_sats());
    println!("Customer: {} sats (was {})", final_customer.total_sats(), customer_balance.total_sats());
}

/// Stress test: full E2E job + payment throughput
#[tokio::test]
#[ignore]
async fn stress_test_full_e2e_throughput() {
    use nostr::JobInput;
    use std::collections::HashMap;
    use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
    use compute::domain::DomainEvent;

    println!("\n=== Full E2E Throughput Stress Test ===\n");

    // Connect wallets
    let provider_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let customer_mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    let provider_signer = SparkSigner::from_mnemonic(provider_mnemonic, "")
        .expect("create provider signer");
    let provider_wallet = match SparkWallet::new(
        provider_signer,
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_e2e_stress_provider"),
        },
    ).await {
        Ok(w) => Arc::new(w),
        Err(e) => {
            println!("Failed to connect provider wallet: {}", e);
            println!("Skipping E2E stress test (requires network)");
            return;
        }
    };

    let customer_signer = SparkSigner::from_mnemonic(customer_mnemonic, "")
        .expect("create customer signer");
    let customer_wallet = match SparkWallet::new(
        customer_signer,
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: std::env::temp_dir().join("spark_e2e_stress_customer"),
        },
    ).await {
        Ok(w) => Arc::new(w),
        Err(e) => {
            println!("Failed to connect customer wallet: {}", e);
            return;
        }
    };

    let customer_balance = customer_wallet.get_balance().await
        .expect("get customer balance");
    println!("Customer balance: {} sats", customer_balance.total_sats());

    if customer_balance.total_sats() < 100 {
        let btc_address = customer_wallet.get_bitcoin_address().await
            .expect("get address");
        println!("\n!!! Customer needs funds !!!");
        println!("Send regtest sats to: {}", btc_address);
        return;
    }

    // Setup DVM
    let provider_identity = UnifiedIdentity::generate().expect("generate identity");

    let mut registry = BackendRegistry::new();
    registry.register_with_id("fast-mock", Arc::new(RwLock::new(FastMockBackend::new(100))));

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(1000);

    let mut dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_config(DvmConfig {
        require_payment: true,
        min_price_msats: 1000, // 1 sat
        default_model: "fast-model".to_string(),
        network: "regtest".to_string(),
    });
    dvm.set_identity(Arc::new(provider_identity.clone())).await;
    dvm.set_wallet(provider_wallet.clone()).await;

    let dvm = Arc::new(RwLock::new(dvm));

    // Run E2E jobs
    let num_jobs = 5;
    let customer_pubkey = UnifiedIdentity::generate().expect("gen").public_key_hex();

    println!("\nRunning {} E2E paid jobs...", num_jobs);

    let start = Instant::now();
    let mut completed = 0;

    for i in 0..num_jobs {
        let event_id = format!("e2e_stress_{:016x}_{}", rand::random::<u64>(), i);
        let job_id = format!("job_{}", &event_id[..16]);

        // 1. Submit job request
        {
            let mut dvm = dvm.write().await;
            let job_inputs = vec![JobInput::text("stress test")];
            let mut params = HashMap::new();
            params.insert("model".to_string(), "fast-model".to_string());
            params.insert("backend".to_string(), "fast-mock".to_string());

            dvm.handle_job_request(&event_id, 5050, &customer_pubkey, job_inputs, params)
                .await
                .expect("handle job request");
        }

        // 2. Get invoice from events
        let mut invoice_bolt11 = String::new();
        while let Ok(event) = event_rx.try_recv() {
            if let DomainEvent::InvoiceCreated { bolt11, .. } = event {
                invoice_bolt11 = bolt11;
                break;
            }
        }

        if invoice_bolt11.is_empty() {
            // Check job status for invoice
            let dvm = dvm.read().await;
            if let Some(job) = dvm.get_job(&job_id).await {
                if let compute::domain::job::JobStatus::PaymentRequired { bolt11, .. } = &job.status {
                    invoice_bolt11 = bolt11.clone();
                }
            }
        }

        if invoice_bolt11.is_empty() {
            println!("  Job {}: No invoice generated", i);
            continue;
        }

        // 3. Pay invoice
        match customer_wallet.send_payment_simple(&invoice_bolt11, None).await {
            Ok(_) => {}
            Err(e) => {
                println!("  Job {}: Payment failed: {}", i, e);
                continue;
            }
        }

        // 4. Confirm and process
        {
            let mut dvm = dvm.write().await;
            if let Err(e) = dvm.confirm_payment(&job_id).await {
                println!("  Job {}: Confirm failed: {}", i, e);
                continue;
            }
        }

        completed += 1;
    }

    let elapsed = start.elapsed();
    let rate = completed as f64 / elapsed.as_secs_f64();

    println!("\n--- Results ---");
    println!("{} E2E jobs completed in {:.2}s", completed, elapsed.as_secs_f64());
    println!("Throughput: {:.2} jobs/sec", rate);
    println!("Average latency: {:.0}ms/job", elapsed.as_millis() as f64 / completed as f64);
}

/// Quick throughput benchmark
#[tokio::test]
#[ignore]
async fn quick_throughput_benchmark() {
    use nostr::JobInput;
    use std::collections::HashMap;

    println!("\n=== Quick Throughput Benchmark ===\n");

    let provider_identity = UnifiedIdentity::generate().expect("generate identity");

    let mut registry = BackendRegistry::new();
    registry.register_with_id("fast-mock", Arc::new(RwLock::new(FastMockBackend::new(0))));

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, _) = broadcast::channel(10000);

    let mut dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_config(DvmConfig {
        require_payment: false,
        min_price_msats: 0,
        default_model: "fast-model".to_string(),
        network: "regtest".to_string(),
    });
    dvm.set_identity(Arc::new(provider_identity)).await;

    let customer_pubkey = format!("{:064x}", rand::random::<u64>());

    // Warmup
    for _ in 0..100 {
        let job_inputs = vec![JobInput::text("warmup")];
        let mut params = HashMap::new();
        params.insert("model".to_string(), "fast-model".to_string());
        params.insert("backend".to_string(), "fast-mock".to_string());
        let event_id = format!("warmup_{:016x}", rand::random::<u64>());

        let _ = dvm.handle_job_request(&event_id, 5050, &customer_pubkey, job_inputs, params).await;
    }

    // Benchmark
    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        let job_inputs = vec![JobInput::text("benchmark")];
        let mut params = HashMap::new();
        params.insert("model".to_string(), "fast-model".to_string());
        params.insert("backend".to_string(), "fast-mock".to_string());
        let event_id = format!("bench_{:016x}_{}", rand::random::<u64>(), i);

        dvm.handle_job_request(&event_id, 5050, &customer_pubkey, job_inputs, params)
            .await
            .expect("job request");
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();
    let latency_us = elapsed.as_micros() as f64 / iterations as f64;

    println!("Iterations: {}", iterations);
    println!("Total time: {:.3}s", elapsed.as_secs_f64());
    println!("Throughput: {:.1} jobs/sec", rate);
    println!("Avg latency: {:.1}µs/job", latency_us);
}
