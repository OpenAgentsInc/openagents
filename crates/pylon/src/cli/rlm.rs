//! pylon rlm - Run recursive language model queries across the swarm
//!
//! This command implements the RLM pattern: break down queries into sub-tasks,
//! fan out to multiple providers via NIP-90, and aggregate results.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use clap::{Args, Subcommand};
use compute::backends::{BackendRegistry, CompletionRequest};
use compute::domain::identity::UnifiedIdentity;
use frlm::conductor::{FrlmConductor, LocalExecutor, SubQuerySubmitter};
use frlm::error::{FrlmError, Result as FrlmResult};
use frlm::policy::FrlmPolicy;
use frlm::trace_db::TraceDbWriter;
use frlm::types::{Fragment, FrlmProgram, SubQuery, SubQueryResult, Venue};
use nostr::nip90::KIND_JOB_RLM_SUBQUERY;
use nostr::{JobInput, JobRequest, JobStatus};
use nostr_client::dvm::DvmClient;
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use tokio::sync::mpsc as tokio_mpsc;

use crate::db::rlm::RlmStore;

/// Arguments for the rlm command
#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct RlmArgs {
    #[command(subcommand)]
    pub command: Option<RlmCommand>,

    /// The query to run
    #[arg(value_name = "query", required_unless_present = "command")]
    pub query: Option<String>,

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

    /// Log trace events to local SQLite
    #[arg(long, default_value_t = true)]
    pub log: bool,
}

/// RLM subcommands
#[derive(Subcommand)]
pub enum RlmCommand {
    /// List recent RLM runs
    History {
        /// Number of runs to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },
}

/// Get pylon data directory
fn data_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".openagents").join("pylon"))
}

/// Get the RLM SQLite database path
fn rlm_db_path() -> anyhow::Result<PathBuf> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("rlm.db"))
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

/// Build fragments for FRLM execution.
fn build_fragments(args: &RlmArgs) -> anyhow::Result<Vec<Fragment>> {
    if let Some(file_path) = &args.file {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

        let chunks = chunk_text(&content, args.chunk_size);
        let fragments = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| Fragment::new(format!("fragment-{}", i + 1), chunk))
            .collect();
        Ok(fragments)
    } else {
        Ok(Vec::new())
    }
}

/// Local backend executor for FRLM fallback.
struct LocalBackendExecutor {
    backend: Arc<tokio::sync::RwLock<dyn compute::backends::InferenceBackend + Send + Sync>>,
    model_id: String,
}

impl LocalBackendExecutor {
    async fn new() -> anyhow::Result<Self> {
        let registry = BackendRegistry::detect().await;
        let models = registry.list_all_models().await;

        if models.is_empty() {
            anyhow::bail!(
                "No local backends detected.\n\
                 Install Ollama or use Apple Foundation Models on M-series Mac."
            );
        }

        let (backend_id, model_info) = models
            .first()
            .ok_or_else(|| anyhow::anyhow!("No local models available"))?;
        let backend = registry
            .get(backend_id)
            .ok_or_else(|| anyhow::anyhow!("Backend not available"))?;

        println!("Using local backend: {} ({})", backend_id, model_info.id);

        Ok(Self {
            backend,
            model_id: model_info.id.clone(),
        })
    }
}

#[async_trait]
impl LocalExecutor for LocalBackendExecutor {
    async fn execute(&self, query: &str) -> FrlmResult<String> {
        let request = CompletionRequest::new(self.model_id.clone(), query.to_string());
        let response = self
            .backend
            .read()
            .await
            .complete(request)
            .await
            .map_err(|e| FrlmError::Internal(format!("local completion failed: {}", e)))?;

        Ok(response.text)
    }
}

/// Submitter that runs all sub-queries locally.
struct LocalSubmitter<L: LocalExecutor + Send + Sync + 'static> {
    executor: Arc<L>,
    result_tx: tokio_mpsc::Sender<SubQueryResult>,
}

impl<L: LocalExecutor + Send + Sync + 'static> LocalSubmitter<L> {
    fn new(executor: Arc<L>, result_tx: tokio_mpsc::Sender<SubQueryResult>) -> Self {
        Self { executor, result_tx }
    }
}

#[async_trait]
impl<L: LocalExecutor + Send + Sync + 'static> SubQuerySubmitter for LocalSubmitter<L> {
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> FrlmResult<Vec<(String, String)>> {
        for query in &queries {
            let result_tx = self.result_tx.clone();
            let executor = Arc::clone(&self.executor);
            let query_id = query.id.clone();
            let prompt = query.prompt.clone();

            tokio::spawn(async move {
                let start = Instant::now();
                let result = executor.execute(&prompt).await;
                let duration_ms = start.elapsed().as_millis() as u64;

                let subquery_result = match result {
                    Ok(output) => SubQueryResult {
                        query_id,
                        content: output,
                        provider_id: None,
                        venue: Venue::Local,
                        duration_ms,
                        cost_sats: 0,
                        success: true,
                        error: None,
                        metadata: HashMap::new(),
                    },
                    Err(err) => SubQueryResult {
                        query_id,
                        content: String::new(),
                        provider_id: None,
                        venue: Venue::Local,
                        duration_ms,
                        cost_sats: 0,
                        success: false,
                        error: Some(err.to_string()),
                        metadata: HashMap::new(),
                    },
                };

                let _ = result_tx.send(subquery_result).await;
            });
        }

        Ok(queries
            .iter()
            .map(|q| (q.id.clone(), q.id.clone()))
            .collect())
    }

    async fn is_available(&self) -> bool {
        true
    }
}

/// Submitter that sends sub-queries to the NIP-90 swarm.
struct SwarmSubmitter {
    client: Arc<DvmClient>,
    relays: Vec<String>,
    bid_msats: u64,
    timeout: Duration,
    wallet: Arc<SparkWallet>,
    result_tx: tokio_mpsc::Sender<SubQueryResult>,
}

impl SwarmSubmitter {
    fn new(
        client: Arc<DvmClient>,
        relays: Vec<String>,
        bid_msats: u64,
        timeout: Duration,
        wallet: Arc<SparkWallet>,
        result_tx: tokio_mpsc::Sender<SubQueryResult>,
    ) -> Self {
        Self {
            client,
            relays,
            bid_msats,
            timeout,
            wallet,
            result_tx,
        }
    }
}

#[async_trait]
impl SubQuerySubmitter for SwarmSubmitter {
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> FrlmResult<Vec<(String, String)>> {
        let mut mappings = Vec::new();

        for query in &queries {
            let mut request = JobRequest::new(KIND_JOB_RLM_SUBQUERY)
                .map_err(|e| FrlmError::NostrError(e.to_string()))?;
            request = request.add_input(JobInput::text(query.prompt.clone()));
            request = request.with_bid(self.bid_msats);

            for relay_url in &self.relays {
                if !relay_url.is_empty() {
                    request = request.add_relay(relay_url);
                }
            }

            let relay_refs: Vec<&str> = self.relays.iter().map(|s| s.as_str()).collect();
            let submission = self
                .client
                .submit_job(request, &relay_refs)
                .await
                .map_err(|e| FrlmError::NostrError(e.to_string()))?;

            let query_id = query.id.clone();
            let job_id = submission.event_id.clone();
            mappings.push((query_id.clone(), job_id.clone()));

            let client = Arc::clone(&self.client);
            let wallet = Arc::clone(&self.wallet);
            let result_tx = self.result_tx.clone();
            let timeout = self.timeout;

            tokio::spawn(async move {
                let start = Instant::now();

                if let Ok(mut feedback_rx) = client.subscribe_to_feedback(&job_id).await {
                    let feedback_timeout = Duration::from_secs(15);
                    let feedback_start = Instant::now();

                    while feedback_start.elapsed() < feedback_timeout {
                        match tokio::time::timeout(Duration::from_millis(500), feedback_rx.recv()).await {
                            Ok(Some(feedback_event)) => {
                                if feedback_event.feedback.status == JobStatus::PaymentRequired {
                                    if let Some(bolt11) = &feedback_event.feedback.bolt11 {
                                        let prepare = match wallet.prepare_send_payment(bolt11, None).await {
                                            Ok(prepare) => prepare,
                                            Err(err) => {
                                                eprintln!("Payment prepare failed: {}", err);
                                                break;
                                            }
                                        };

                                        if let Err(err) = wallet.send_payment(prepare, None).await {
                                            eprintln!("Payment failed: {}", err);
                                        }
                                        break;
                                    }
                                }
                            }
                            Ok(None) => break,
                            Err(_) => continue,
                        }
                    }
                }

                let result = client.await_result(&job_id, timeout).await;
                let duration_ms = start.elapsed().as_millis() as u64;

                let subquery_result = match result {
                    Ok(result) => {
                        let cost_sats = result.amount.unwrap_or(0) / 1000;
                        SubQueryResult {
                            query_id,
                            content: result.content,
                            provider_id: None,
                            venue: Venue::Swarm,
                            duration_ms,
                            cost_sats,
                            success: true,
                            error: None,
                            metadata: HashMap::new(),
                        }
                    }
                    Err(err) => SubQueryResult {
                        query_id,
                        content: String::new(),
                        provider_id: None,
                        venue: Venue::Swarm,
                        duration_ms,
                        cost_sats: 0,
                        success: false,
                        error: Some(err.to_string()),
                        metadata: HashMap::new(),
                    },
                };

                let _ = result_tx.send(subquery_result).await;
            });
        }

        Ok(mappings)
    }

    async fn is_available(&self) -> bool {
        !self.relays.is_empty()
    }
}

async fn run_history(limit: u32) -> anyhow::Result<()> {
    let db_path = rlm_db_path()?;
    let store = RlmStore::new(&db_path)?;
    let runs = store.list_runs(limit)?;

    if runs.is_empty() {
        println!("No RLM runs found.");
        return Ok(());
    }

    println!("Recent RLM Runs");
    println!("================");
    println!("{:<12} {:<10} {:>8} {:>10} {}", "RUN ID", "STATUS", "COST", "DURATION", "QUERY");
    println!("{}", "-".repeat(72));

    for run in runs {
        let short_id = truncate_id(&run.id, 10);
        let cost = format!("{}", run.total_cost_sats);
        let duration = format_duration_ms(run.total_duration_ms);
        let query = truncate_text(&run.query, 60);

        println!(
            "{:<12} {:<10} {:>8} {:>10} {}",
            short_id, run.status, cost, duration, query
        );
    }

    Ok(())
}

/// Execute the rlm command
pub async fn run(args: RlmArgs) -> anyhow::Result<()> {
    if let Some(command) = args.command {
        return match command {
            RlmCommand::History { limit } => run_history(limit).await,
        };
    }

    let query = args
        .query
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Query is required"))?;

    let fragments = build_fragments(&args)?;
    let total_queries = std::cmp::max(1, fragments.len());

    let mut program = FrlmProgram::new(query.clone());
    if !fragments.is_empty() {
        program = program.with_fragments(fragments.clone());
    }
    let run_id = program.run_id.clone();

    let per_query_timeout = Duration::from_secs(args.timeout);
    let total_timeout = Duration::from_secs(args.timeout.saturating_mul(total_queries as u64));

    let policy = FrlmPolicy::default()
        .with_budget_sats(args.budget)
        .with_per_query_timeout(per_query_timeout)
        .with_timeout(total_timeout);

    println!("RLM Query");
    println!("=========");
    println!("Query: {}", query);
    println!("Budget: {} sats", args.budget);
    let relay_list: Vec<String> = args
        .relay
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !args.local_only {
        println!("Relays: {}", relay_list.len());
    }
    if !fragments.is_empty() {
        println!("Fragments: {}", fragments.len());
    }

    let db_path = rlm_db_path()?;
    let store = if args.log {
        Some(RlmStore::new(&db_path)?)
    } else {
        None
    };

    if let Some(store) = &store {
        store.insert_run(&run_id, &program.query, fragments.len(), args.budget)?;
    }

    let mut conductor = FrlmConductor::new(policy);
    let trace_handle = if args.log {
        if let Some(trace_rx) = conductor.take_trace_receiver() {
            let mut writer = TraceDbWriter::open(&db_path)
                .map_err(|e| anyhow::anyhow!("Trace DB init failed: {}", e))?;
            Some(std::thread::spawn(move || writer.drain(trace_rx)))
        } else {
            None
        }
    } else {
        None
    };

    let local_executor = if args.local_only {
        Some(Arc::new(LocalBackendExecutor::new().await?))
    } else {
        match LocalBackendExecutor::new().await {
            Ok(executor) => Some(Arc::new(executor)),
            Err(err) => {
                eprintln!("Local fallback unavailable: {}", err);
                None
            }
        }
    };

    let run_result = if args.local_only {
        let executor = local_executor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Local execution requested but no backend available"))?;
        let submitter = LocalSubmitter::new(Arc::clone(executor), conductor.result_sender());
        conductor.run(program, &submitter, Some(executor.as_ref())).await
    } else {
        let client = Arc::new(create_dvm_client()?);
        let wallet = Arc::new(create_wallet().await?);
        let bid_msats = (args.budget * 1000) / total_queries as u64;
        let submitter = SwarmSubmitter::new(
            client,
            relay_list,
            bid_msats,
            per_query_timeout,
            wallet,
            conductor.result_sender(),
        );

        let local_ref = local_executor.as_deref();
        conductor.run(program, &submitter, local_ref).await
    };

    if let Some(handle) = trace_handle {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => eprintln!("Trace logging failed: {}", err),
            Err(_) => eprintln!("Trace logging thread panicked"),
        }
    }

    match run_result {
        Ok(result) => {
            if let Some(store) = &store {
                store.mark_completed(
                    &result.run_id,
                    &result.output,
                    result.total_cost_sats,
                    result.total_duration_ms,
                )?;
            }

            if result.sub_query_results.len() <= 1 {
                println!("{}", result.output);
            } else {
                println!("\n--- Fragment Results ---\n");
                for (i, subquery) in result.sub_query_results.iter().enumerate() {
                    if subquery.success {
                        println!("Fragment {}: {}\n", i + 1, subquery.content);
                    } else {
                        println!(
                            "Fragment {}: (failed) {}\n",
                            i + 1,
                            subquery.error.as_deref().unwrap_or("unknown error")
                        );
                    }
                }

                println!("--- Combined Answer ---\n");
                println!("{}", result.output);
            }

            Ok(())
        }
        Err(err) => {
            if let Some(store) = &store {
                store.mark_failed(&run_id, &err.to_string())?;
            }
            Err(anyhow::anyhow!(err))
        }
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}

fn truncate_id(id: &str, max_len: usize) -> String {
    if id.len() <= max_len {
        id.to_string()
    } else {
        id[..max_len].to_string()
    }
}

fn format_duration_ms(duration_ms: i64) -> String {
    if duration_ms < 1000 {
        format!("{}ms", duration_ms)
    } else {
        format!("{:.2}s", duration_ms as f64 / 1000.0)
    }
}
