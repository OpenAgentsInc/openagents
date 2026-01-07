//! Job CLI commands
//!
//! Commands for submitting and tracking NIP-90 jobs (buyer mode).

use crate::jobs::{JobRecord, JobStore};
use clap::{Parser, Subcommand};
use compute::domain::identity::UnifiedIdentity;
use nostr::{JobInput, JobRequest};
use nostr_client::dvm::DvmClient;
use std::path::PathBuf;
use std::time::Duration;

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
        /// Relay URL
        #[arg(long, default_value = "wss://relay.damus.io")]
        relay: String,
        /// Target provider pubkey (optional)
        #[arg(long)]
        provider: Option<String>,
        /// Wait for result (don't return immediately)
        #[arg(long)]
        wait: bool,
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
            timeout,
        } => {
            let client = create_dvm_client()?;
            let store = get_job_store()?;

            println!("Submitting job to {}...", relay);
            println!("Kind: {}", kind);
            println!("Prompt: {}", if prompt.len() > 50 { format!("{}...", &prompt[..50]) } else { prompt.clone() });

            // Build job request
            let mut request = JobRequest::new(kind)?;
            request = request.add_input(JobInput::text(&prompt));

            if let Some(model_name) = &model {
                request = request.add_param("model", model_name);
            }

            request = request.with_bid(bid);

            if let Some(provider_pk) = &provider {
                request = request.add_service_provider(provider_pk);
            }

            // Submit to relay
            let submission = client.submit_job(request, &[&relay]).await?;
            let job_id = submission.event_id.clone();

            println!("\nJob Submitted");
            println!("=============");
            println!("ID:     {}", job_id);
            println!("Pubkey: {}", client.pubkey());

            // Store in local DB
            let mut record = JobRecord::new(job_id.clone(), kind, prompt, relay);
            if let Some(p) = provider {
                record = record.with_provider(p);
            }
            record = record.with_bid(bid);
            store.insert(&record)?;

            // Optionally wait for result
            if wait {
                println!("\nWaiting for result ({}s timeout)...", timeout);

                match client
                    .await_result(&job_id, Duration::from_secs(timeout))
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

                        if let Some(bolt11) = &result.bolt11 {
                            println!("\nPayment Required");
                            println!("================");
                            println!("Amount: {} msats", result.amount.unwrap_or(0));
                            println!("Invoice: {}", bolt11);
                            println!("\nPay with: pylon wallet pay {}", bolt11);
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
