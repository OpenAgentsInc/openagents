//! Job CLI commands
//!
//! Commands for submitting and tracking NIP-90 jobs (buyer mode).

use crate::jobs::{JobRecord, JobStore};
use clap::{Parser, Subcommand};
use compute::domain::identity::UnifiedIdentity;
use nostr::{JobInput, JobRequest, JobStatus};
use nostr_client::dvm::DvmClient;
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Job management commands (buyer mode)
#[derive(Parser)]
pub struct JobArgs {
    #[command(subcommand)]
    pub command: JobCommand,
}

/// Available job commands
#[derive(Subcommand)]
pub enum JobCommand {
    /// Submit a job to a DVM provider
    Submit {
        /// Prompt or input text
        prompt: String,
        /// Job kind (default: 5050 for text generation)
        #[arg(long, default_value = "5050")]
        kind: u16,
        /// Model to use (optional)
        #[arg(long)]
        model: Option<String>,
        /// Bid amount in millisats
        #[arg(long, default_value = "1000")]
        bid: u64,
        /// Relay URLs (comma-separated, or specify multiple times)
        #[arg(long, default_value = "wss://nexus.openagents.com,wss://relay.damus.io,wss://nos.lol")]
        relay: String,
        /// Target provider pubkey (optional)
        #[arg(long)]
        provider: Option<String>,
        /// Wait for result (don't return immediately)
        #[arg(long)]
        wait: bool,
        /// Auto-pay invoice when provider requests payment
        #[arg(long)]
        auto_pay: bool,
        /// Timeout in seconds when waiting
        #[arg(long, default_value = "60")]
        timeout: u64,
    },
    /// Check job status
    Status {
        /// Job event ID
        job_id: String,
    },
    /// Get job results (waits if not complete)
    Results {
        /// Job event ID
        job_id: String,
        /// Timeout in seconds
        #[arg(long, default_value = "60")]
        timeout: u64,
    },
    /// List submitted jobs
    List {
        /// Number of jobs to show
        #[arg(long, default_value = "10")]
        limit: u32,
    },
}

/// Get pylon data directory
fn data_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".openagents").join("pylon"))
}

/// Load mnemonic from identity file
fn load_mnemonic() -> anyhow::Result<String> {
    let identity_file = data_dir()?.join("identity.mnemonic");
    if !identity_file.exists() {
        anyhow::bail!(
            "No identity found. Run 'pylon init' first.\n  Expected: {:?}",
            identity_file
        );
    }
    let mnemonic = std::fs::read_to_string(&identity_file)?;
    Ok(mnemonic.trim().to_string())
}

/// Get or create the job store
fn get_job_store() -> anyhow::Result<JobStore> {
    let db_path = data_dir()?.join("jobs.db");
    JobStore::new(&db_path)
}

/// Create a DVM client from stored identity
fn create_dvm_client() -> anyhow::Result<DvmClient> {
    let mnemonic = load_mnemonic()?;
    let identity = UnifiedIdentity::from_mnemonic(&mnemonic, "")
        .map_err(|e| anyhow::anyhow!("Failed to create identity: {}", e))?;

    let private_key = *identity.private_key_bytes();
    DvmClient::new(private_key).map_err(|e| anyhow::anyhow!("Failed to create DVM client: {}", e))
}

/// Create a Spark wallet from the stored identity
async fn create_wallet() -> anyhow::Result<SparkWallet> {
    let mnemonic = load_mnemonic()?;

    let signer = SparkSigner::from_mnemonic(&mnemonic, "")
        .map_err(|e| anyhow::anyhow!("Failed to create signer: {}", e))?;

    let config = WalletConfig {
        network: Network::Regtest,
        api_key: None,
        storage_dir: data_dir()?.join("spark"),
    };

    let wallet = SparkWallet::new(signer, config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize wallet: {}", e))?;

    Ok(wallet)
}

/// Execute a job command
pub async fn run(args: JobArgs) -> anyhow::Result<()> {
    match args.command {
        JobCommand::Submit {
            prompt,
            kind,
            model,
            bid,
            relay,
            provider,
            wait,
            auto_pay,
            timeout,
        } => {
            let client = create_dvm_client()?;
            let store = get_job_store()?;

            // Parse comma-separated relays
            let relays: Vec<&str> = relay.split(',').map(|s| s.trim()).collect();

            println!("Submitting job to {} relays...", relays.len());
            for r in &relays {
                println!("  - {}", r);
            }
            println!("Kind: {}", kind);
            println!("Prompt: {}", if prompt.len() > 50 { format!("{}...", &prompt[..50]) } else { prompt.clone() });
            if auto_pay {
                println!("Auto-pay: enabled");
            }

            // Build job request
            let mut request = JobRequest::new(kind)?;
            request = request.add_input(JobInput::text(&prompt));

            if let Some(model_name) = &model {
                request = request.add_param("model", model_name);
            }

            request = request.with_bid(bid);

            for relay_url in &relays {
                if !relay_url.is_empty() {
                    request = request.add_relay(*relay_url);
                }
            }

            if let Some(provider_pk) = &provider {
                request = request.add_service_provider(provider_pk);
            }

            // Submit to relays
            let submission = client.submit_job(request, &relays).await?;
            let job_id = submission.event_id.clone();

            println!("\nJob Submitted");
            println!("=============");
            println!("ID:     {}", job_id);
            println!("Pubkey: {}", client.pubkey());

            // Store in local DB (use first relay for record)
            let mut record = JobRecord::new(job_id.clone(), kind, prompt, relays.first().copied().unwrap_or("unknown").to_string());
            if let Some(p) = provider {
                record = record.with_provider(p);
            }
            record = record.with_bid(bid);
            store.insert(&record)?;

            // Optionally wait for result (with optional auto-pay)
            if wait || auto_pay {
                // Subscribe to feedback to catch payment-required
                let mut feedback_rx = client.subscribe_to_feedback(&job_id).await?;

                let total_timeout = Duration::from_secs(timeout);
                let start = Instant::now();
                let mut payment_made = false;

                // Wait for payment-required feedback if auto_pay is enabled
                if auto_pay {
                    println!("\nWaiting for payment request...");

                    let feedback_timeout = Duration::from_secs(30);
                    let feedback_start = Instant::now();

                    while feedback_start.elapsed() < feedback_timeout {
                        match tokio::time::timeout(Duration::from_millis(500), feedback_rx.recv()).await {
                            Ok(Some(feedback_event)) => {
                                println!("Received feedback: {:?}", feedback_event.feedback.status);

                                if feedback_event.feedback.status == JobStatus::PaymentRequired {
                                    if let Some(bolt11) = &feedback_event.feedback.bolt11 {
                                        let amount = feedback_event.feedback.amount.unwrap_or(0);
                                        println!("\nPayment Required");
                                        println!("================");
                                        println!("Amount: {} msats", amount);

                                        // Connect to Spark wallet and pay
                                        println!("\nConnecting to Spark wallet...");
                                        let wallet = create_wallet().await?;

                                        println!("Preparing payment...");
                                        let prepare = wallet
                                            .prepare_send_payment(bolt11, None)
                                            .await
                                            .map_err(|e| anyhow::anyhow!("Failed to prepare payment: {}", e))?;

                                        println!("Sending payment...");
                                        let response = wallet
                                            .send_payment(prepare, None)
                                            .await
                                            .map_err(|e| anyhow::anyhow!("Payment failed: {}", e))?;

                                        println!("Payment sent! ID: {}", response.payment.id);
                                        payment_made = true;
                                        break;
                                    } else {
                                        println!("Warning: payment-required but no bolt11 invoice");
                                    }
                                } else if feedback_event.feedback.status == JobStatus::Error {
                                    let error_msg = feedback_event.feedback.status_extra
                                        .unwrap_or_else(|| "Unknown error".to_string());
                                    anyhow::bail!("Job failed: {}", error_msg);
                                }
                            }
                            Ok(None) => {
                                // Channel closed
                                break;
                            }
                            Err(_) => {
                                // Timeout, continue waiting
                                continue;
                            }
                        }
                    }

                    if !payment_made && auto_pay {
                        println!("\nNo payment request received within 30s.");
                        println!("The provider may not have seen the job or may offer free service.");
                    }
                }

                // Now wait for the result
                let remaining_timeout = total_timeout.saturating_sub(start.elapsed());
                if remaining_timeout.is_zero() {
                    anyhow::bail!("Timeout waiting for result");
                }

                println!("\nWaiting for result ({:.0}s remaining)...", remaining_timeout.as_secs_f64());

                match client
                    .await_result(&job_id, remaining_timeout)
                    .await
                {
                    Ok(result) => {
                        println!("\nResult Received");
                        println!("===============");
                        println!("{}", result.content);

                        // Update local store
                        store.update_result(
                            &job_id,
                            &result.content,
                            result.bolt11.as_deref(),
                            result.amount,
                        )?;

                        if !payment_made {
                            if let Some(bolt11) = &result.bolt11 {
                                println!("\nPayment Required");
                                println!("================");
                                println!("Amount: {} msats", result.amount.unwrap_or(0));
                                println!("Invoice: {}", bolt11);
                                println!("\nPay with: pylon wallet pay {}", bolt11);
                            }
                        }
                    }
                    Err(e) => {
                        println!("\nFailed to get result: {}", e);
                        store.mark_failed(&job_id, &e.to_string())?;
                    }
                }
            } else {
                println!("\nJob submitted. Check status with:");
                println!("  pylon job status {}", job_id);
                println!("  pylon job results {}", job_id);
            }
        }

        JobCommand::Status { job_id } => {
            let store = get_job_store()?;

            match store.get(&job_id)? {
                Some(job) => {
                    println!("\nJob Status");
                    println!("==========");
                    println!("ID:      {}", job.id);
                    println!("Kind:    {}", job.kind);
                    println!("Status:  {}", job.status);
                    println!("Relay:   {}", job.relay);

                    if let Some(provider) = &job.provider {
                        println!("Provider: {}", provider);
                    }

                    if let Some(bid) = job.bid_msats {
                        println!("Bid:     {} msats", bid);
                    }

                    let created = chrono::DateTime::from_timestamp(job.created_at, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    println!("Created: {}", created);

                    if job.status == "completed" {
                        if let Some(result) = &job.result {
                            println!("\nResult:");
                            println!("{}", result);
                        }
                    }
                }
                None => {
                    println!("Job not found in local database: {}", job_id);
                    println!("\nThe job may have been submitted from another device.");
                    println!("Try: pylon job results {} --timeout 60", job_id);
                }
            }
        }

        JobCommand::Results { job_id, timeout } => {
            let store = get_job_store()?;

            // Check local store first
            if let Some(job) = store.get(&job_id)? {
                if job.status == "completed" {
                    if let Some(result) = &job.result {
                        println!("\nJob Result (cached)");
                        println!("===================");
                        println!("{}", result);

                        if let Some(bolt11) = &job.bolt11 {
                            println!("\nPayment");
                            println!("=======");
                            println!("Amount: {} msats", job.amount_msats.unwrap_or(0));
                            println!("Invoice: {}", bolt11);
                        }
                        return Ok(());
                    }
                }
            }

            // Not in cache or not complete - wait for result from relay
            println!("Waiting for result ({}s timeout)...", timeout);

            let client = create_dvm_client()?;

            match client
                .await_result(&job_id, Duration::from_secs(timeout))
                .await
            {
                Ok(result) => {
                    println!("\nJob Result");
                    println!("==========");
                    println!("{}", result.content);

                    // Update local store if we have the job
                    let _ = store.update_result(
                        &job_id,
                        &result.content,
                        result.bolt11.as_deref(),
                        result.amount,
                    );

                    if let Some(bolt11) = &result.bolt11 {
                        println!("\nPayment Required");
                        println!("================");
                        println!("Amount: {} msats", result.amount.unwrap_or(0));
                        println!("Invoice: {}", bolt11);
                        println!("\nPay with: pylon wallet pay {}", bolt11);
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to get result: {}", e);
                }
            }
        }

        JobCommand::List { limit } => {
            let store = get_job_store()?;
            let jobs = store.list(limit)?;

            println!("\nSubmitted Jobs");
            println!("==============");

            if jobs.is_empty() {
                println!("No jobs found.");
            } else {
                for job in jobs {
                    let status_icon = match job.status.as_str() {
                        "completed" => "[done]",
                        "pending" => "[pend]",
                        "processing" => "[proc]",
                        "failed" => "[fail]",
                        _ => "[????]",
                    };

                    let time = chrono::DateTime::from_timestamp(job.created_at, 0)
                        .map(|dt| dt.format("%m/%d %H:%M").to_string())
                        .unwrap_or_else(|| "??/??".to_string());

                    let prompt_preview = if job.prompt.len() > 30 {
                        format!("{}...", &job.prompt[..30])
                    } else {
                        job.prompt.clone()
                    };

                    println!(
                        "{} {} {} \"{}\"",
                        status_icon,
                        time,
                        &job.id[..16],
                        prompt_preview
                    );
                }
            }
        }
    }

    Ok(())
}
