//! pylon rlm - Run recursive language model queries across the swarm
//!
//! This command implements the RLM pattern: break down queries into sub-tasks,
//! fan out to multiple providers via NIP-90, and aggregate results.

use clap::Args;
use compute::backends::{BackendRegistry, CompletionRequest};
use compute::domain::identity::UnifiedIdentity;
use nostr::{JobInput, JobRequest, JobStatus};
use nostr::nip90::KIND_JOB_RLM_SUBQUERY;
use nostr_client::dvm::DvmClient;
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Arguments for the rlm command
#[derive(Args)]
pub struct RlmArgs {
    /// The query to run
    pub query: String,

    /// File to analyze (loaded as fragments)
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Maximum concurrent sub-queries (fanout)
    #[arg(long, default_value = "10")]
    pub fanout: usize,

    /// Maximum sats to spend
    #[arg(long, default_value = "1000")]
    pub budget: u64,

    /// Use local model only (no swarm)
    #[arg(long)]
    pub local_only: bool,

    /// Relay URLs (comma-separated)
    #[arg(long, default_value = "wss://nexus.openagents.com,wss://relay.damus.io,wss://nos.lol")]
    pub relay: String,

    /// Chunk size in characters (for file processing)
    #[arg(long, default_value = "2000")]
    pub chunk_size: usize,

    /// Timeout per sub-query in seconds
    #[arg(long, default_value = "60")]
    pub timeout: u64,
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

/// Chunk text into fragments of approximately chunk_size characters
fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for line in text.lines() {
        if current_chunk.len() + line.len() + 1 > chunk_size && !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
            current_chunk = String::new();
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    chunks
}

/// Run RLM query locally using available inference backends
async fn run_local(args: &RlmArgs) -> anyhow::Result<()> {
    println!("Running locally (no swarm)...\n");

    let registry = BackendRegistry::detect().await;
    let models = registry.list_all_models().await;

    if models.is_empty() {
        anyhow::bail!(
            "No local backends detected.\n\
             Install Ollama or use Apple Foundation Models on M-series Mac."
        );
    }

    let (backend_id, model_info) = models.first().unwrap();
    println!("Using backend: {} ({})", backend_id, model_info.id);

    let backend = registry
        .get(backend_id)
        .ok_or_else(|| anyhow::anyhow!("Backend not available"))?;

    // If file provided, chunk and process
    if let Some(file_path) = &args.file {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

        let chunks = chunk_text(&content, args.chunk_size);
        println!("Chunked file into {} fragments\n", chunks.len());

        let mut results = Vec::new();

        for (i, chunk) in chunks.iter().enumerate() {
            println!("Processing chunk {}/{}...", i + 1, chunks.len());

            let prompt = format!(
                "Given this context:\n\n{}\n\nAnswer: {}",
                chunk, args.query
            );

            let request = CompletionRequest::new(model_info.id.clone(), prompt);
            let response = backend.read().await.complete(request).await?;
            results.push(response.text);
        }

        // Aggregate results
        println!("\n--- Aggregated Results ---\n");
        for (i, result) in results.iter().enumerate() {
            println!("Fragment {}: {}\n", i + 1, result);
        }

        // Final synthesis
        if results.len() > 1 {
            println!("--- Synthesizing ---\n");
            let synthesis_prompt = format!(
                "Given these partial answers:\n\n{}\n\nProvide a final synthesized answer to: {}",
                results.join("\n\n---\n\n"),
                args.query
            );
            let request = CompletionRequest::new(model_info.id.clone(), synthesis_prompt);
            let response = backend.read().await.complete(request).await?;
            println!("Final Answer:\n{}", response.text);
        }
    } else {
        // Simple query
        let request = CompletionRequest::new(model_info.id.clone(), args.query.clone());
        let response = backend.read().await.complete(request).await?;
        println!("{}", response.text);
    }

    Ok(())
}

/// Run RLM query across the swarm
async fn run_swarm(args: &RlmArgs) -> anyhow::Result<()> {
    let client = create_dvm_client()?;
    let relays: Vec<&str> = args.relay.split(',').map(|s| s.trim()).collect();

    println!("RLM Query");
    println!("=========");
    println!("Query: {}", args.query);
    println!("Budget: {} sats", args.budget);
    println!("Relays: {}", relays.len());

    // Determine sub-queries
    let sub_queries: Vec<String> = if let Some(file_path) = &args.file {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

        let chunks = chunk_text(&content, args.chunk_size);
        println!("Fragments: {}", chunks.len());

        chunks
            .iter()
            .map(|chunk| {
                format!(
                    "Given this context:\n\n{}\n\nAnswer: {}",
                    chunk, args.query
                )
            })
            .collect()
    } else {
        // Single query
        vec![args.query.clone()]
    };

    let total_queries = sub_queries.len();
    let fanout = args.fanout.min(total_queries);
    println!("Sub-queries: {} (fanout: {})", total_queries, fanout);

    // Calculate bid per query
    let bid_per_query = (args.budget * 1000) / total_queries as u64; // convert to millisats
    println!("Bid per query: {} msats\n", bid_per_query);

    // Submit sub-queries (respecting fanout limit)
    let mut job_ids = Vec::new();
    let mut results: Vec<Option<String>> = vec![None; total_queries];

    println!("Submitting jobs to swarm...");

    for (i, query) in sub_queries.iter().enumerate() {
        // Build job request using kind:5940 (RLM sub-query)
        let mut request = JobRequest::new(KIND_JOB_RLM_SUBQUERY)?;
        request = request.add_input(JobInput::text(query));
        request = request.with_bid(bid_per_query);

        for relay_url in &relays {
            if !relay_url.is_empty() {
                request = request.add_relay(*relay_url);
            }
        }

        let submission = client.submit_job(request, &relays).await?;
        println!("  [{}/{}] Submitted: {}", i + 1, total_queries, &submission.event_id[..16]);
        job_ids.push((i, submission.event_id));
    }

    // Wait for results with auto-pay
    println!("\nWaiting for results...");
    let wallet = create_wallet().await?;
    let timeout = Duration::from_secs(args.timeout);
    let start = Instant::now();

    for (idx, job_id) in &job_ids {
        if start.elapsed() > timeout {
            println!("Timeout reached, {} jobs incomplete", total_queries - results.iter().filter(|r| r.is_some()).count());
            break;
        }

        // Subscribe to feedback for auto-pay
        let mut feedback_rx = client.subscribe_to_feedback(job_id).await?;

        // Wait for payment request
        let feedback_timeout = Duration::from_secs(15);
        let feedback_start = Instant::now();

        while feedback_start.elapsed() < feedback_timeout {
            match tokio::time::timeout(Duration::from_millis(500), feedback_rx.recv()).await {
                Ok(Some(feedback_event)) => {
                    if feedback_event.feedback.status == JobStatus::PaymentRequired {
                        if let Some(bolt11) = &feedback_event.feedback.bolt11 {
                            // Auto-pay
                            let prepare = wallet
                                .prepare_send_payment(bolt11, None)
                                .await
                                .map_err(|e| anyhow::anyhow!("Failed to prepare payment: {}", e))?;

                            wallet
                                .send_payment(prepare, None)
                                .await
                                .map_err(|e| anyhow::anyhow!("Payment failed: {}", e))?;

                            print!("$"); // payment indicator
                            std::io::Write::flush(&mut std::io::stdout())?;
                            break;
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        // Wait for result
        let remaining = timeout.saturating_sub(start.elapsed());
        match client.await_result(job_id, remaining.min(Duration::from_secs(30))).await {
            Ok(result) => {
                results[*idx] = Some(result.content);
                print!(".");
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            Err(e) => {
                eprintln!("\n  [{}/{}] Failed: {}", idx + 1, total_queries, e);
            }
        }
    }

    println!("\n");

    // Count successful results
    let successful = results.iter().filter(|r| r.is_some()).count();
    println!("Completed: {}/{} sub-queries\n", successful, total_queries);

    if successful == 0 {
        anyhow::bail!("No results received. Check that providers are running and handling kind:5940 jobs.");
    }

    // Display and aggregate results
    if total_queries == 1 {
        // Single query - just show result
        if let Some(result) = &results[0] {
            println!("{}", result);
        }
    } else {
        // Multiple queries - aggregate
        println!("--- Fragment Results ---\n");
        for (i, result) in results.iter().enumerate() {
            if let Some(text) = result {
                println!("Fragment {}: {}\n", i + 1, text);
            } else {
                println!("Fragment {}: (no result)\n", i + 1);
            }
        }

        // Simple aggregation - combine all results
        let combined: Vec<&str> = results
            .iter()
            .filter_map(|r| r.as_deref())
            .collect();

        println!("--- Combined Answer ---\n");
        println!("{}", combined.join("\n\n---\n\n"));
    }

    Ok(())
}

/// Execute the rlm command
pub async fn run(args: RlmArgs) -> anyhow::Result<()> {
    if args.local_only {
        run_local(&args).await
    } else {
        run_swarm(&args).await
    }
}
