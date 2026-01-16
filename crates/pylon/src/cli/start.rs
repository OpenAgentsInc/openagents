//! pylon start - Start the Pylon daemon
//!
//! Runs Pylon in host mode, provider mode, or both.

use clap::{Args, ValueEnum};
use compute::domain::DomainEvent;
use openagents_runtime::UnifiedIdentity;
use std::time::Instant;
use tokio::sync::broadcast;

use crate::config::PylonConfig;
use crate::daemon::{
    ControlSocket, DaemonCommand, DaemonResponse, PidFile, daemonize, db_path, is_daemon_running,
    pid_path, socket_path,
};
use crate::db::PylonDb;
use crate::db::jobs::{Job, JobStatus};
use crate::host::AgentRunner;
use crate::local_bridge::{LocalBridgeConfig, PylonBridgeInfo, start_local_bridge};
use crate::provider::PylonProvider;
use std::sync::Arc;

/// Operating mode for Pylon
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum PylonMode {
    /// Run only as a provider (earn sats)
    Provider,
    /// Run only as a host (run your agents)
    Host,
    /// Run both modes simultaneously
    #[default]
    Both,
}

/// Arguments for the start command
#[derive(Args)]
pub struct StartArgs {
    /// Run in foreground (don't daemonize)
    #[arg(long, short = 'f')]
    pub foreground: bool,

    /// Operating mode
    #[arg(long, short = 'm', value_enum, default_value = "both")]
    pub mode: PylonMode,

    /// Config file path (default: ~/.config/pylon/config.toml)
    #[arg(long, short)]
    pub config: Option<String>,
}

/// Run the start command
pub async fn run(args: StartArgs) -> anyhow::Result<()> {
    // Check if daemon is already running
    if is_daemon_running() {
        println!("Pylon daemon is already running.");
        println!("Use 'pylon stop' to stop it first.");
        return Ok(());
    }

    // Load config
    let config = if let Some(ref path) = args.config {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content)?
    } else {
        PylonConfig::load()?
    };

    // Load identity
    let data_dir = config.data_path()?;
    let identity_file = data_dir.join("identity.mnemonic");

    if !identity_file.exists() {
        println!("No identity found. Run 'pylon init' first.");
        return Err(anyhow::anyhow!("Identity not initialized"));
    }

    let mnemonic = std::fs::read_to_string(&identity_file)?;
    let mnemonic = mnemonic.trim();

    let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")
        .map_err(|e| anyhow::anyhow!("Failed to load identity: {}", e))?;

    let npub = identity
        .npub()
        .map_err(|e| anyhow::anyhow!("Failed to get npub: {}", e))?;

    if args.foreground {
        println!("Starting Pylon in foreground mode...");
        println!("Identity: {}", npub);
        println!("Mode: {:?}", args.mode);
        run_daemon(config, identity, args.mode).await
    } else {
        // Daemonize
        println!("Starting Pylon daemon...");
        println!("Identity: {}", npub);
        println!("Mode: {:?}", args.mode);

        match daemonize()? {
            false => {
                // Parent process - daemon was forked successfully
                // Wait a moment for the daemon to initialize
                std::thread::sleep(std::time::Duration::from_millis(500));

                if is_daemon_running() {
                    println!("Pylon daemon started successfully.");
                    println!("Use 'pylon status' to check status.");
                    println!("Use 'pylon stop' to stop.");
                } else {
                    println!("Warning: Daemon may have failed to start.");
                    println!("Check logs or try 'pylon start -f' to see errors.");
                }
                Ok(())
            }
            true => {
                // Child process - we are the daemon
                run_daemon(config, identity, args.mode).await
            }
        }
    }
}

/// Run the daemon (called in either foreground or after daemonize)
async fn run_daemon(
    config: PylonConfig,
    identity: UnifiedIdentity,
    mode: PylonMode,
) -> anyhow::Result<()> {
    let start_time = Instant::now();

    // Write PID file (for foreground mode, daemonize already writes it)
    let pid_file = PidFile::new(pid_path()?);
    if !pid_file.exists() {
        pid_file.write()?;
    }

    // Open database
    let db = PylonDb::open(db_path()?)?;
    tracing::info!("Database opened at {:?}", db_path()?);

    // Load previous stats from database
    let summary = db.get_earnings_summary()?;
    let job_counts = db.count_jobs_by_status()?;
    let mut jobs_completed: u64 = job_counts.get(&JobStatus::Completed).copied().unwrap_or(0);
    let mut earnings_msats: u64 = summary.total_msats;

    tracing::info!(
        "Loaded stats: {} jobs completed, {} sats earned",
        jobs_completed,
        earnings_msats / 1000
    );

    // Open control socket
    let control_socket = ControlSocket::new(socket_path()?)?;

    // Provider state
    let mut provider: Option<PylonProvider> = None;
    let mut provider_events: Option<broadcast::Receiver<DomainEvent>> = None;
    let provider_enabled = matches!(mode, PylonMode::Provider | PylonMode::Both);
    let host_active = matches!(mode, PylonMode::Host | PylonMode::Both);
    let mut provider_active = provider_enabled;

    // Initialize provider if needed
    if provider_enabled {
        let mut p = PylonProvider::new(config.clone()).await?;
        p.init_with_identity(identity.clone()).await?;

        let status = p.status().await;
        if status.backends.is_empty() {
            tracing::warn!("No inference backends detected for provider mode");
            tracing::info!("Provider mode disabled; continuing without provider backends");
            provider_active = false;
        } else {
            tracing::info!("Provider backends: {}", status.backends.join(", "));
            p.start().await?;
            provider_events = Some(p.events());
            provider = Some(p);
            tracing::info!("Provider mode started");
        }
    }

    // Initialize host mode if needed
    let agent_runner: Option<AgentRunner> = if host_active {
        let relay_url = config
            .relays
            .first()
            .cloned()
            .unwrap_or_else(|| "wss://nexus.openagents.com".to_string());

        match AgentRunner::new(Arc::new(PylonDb::open(db_path()?)?), relay_url) {
            Ok(runner) => {
                // Start all active agents
                if let Err(e) = runner.start_all_active().await {
                    tracing::warn!("Failed to start some agents: {}", e);
                }

                let running = runner.list_running().await;
                if running.is_empty() {
                    tracing::info!("Host mode started (no active agents)");
                } else {
                    tracing::info!("Host mode started ({} agents running)", running.len());
                }

                Some(runner)
            }
            Err(e) => {
                tracing::warn!("Failed to initialize agent runner: {}", e);
                tracing::info!("Host mode started (agent runner unavailable)");
                None
            }
        }
    } else {
        None
    };

    let npub = identity.npub().ok();
    let bridge_config = LocalBridgeConfig::for_pylon(
        PylonBridgeInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            npub,
            host_active,
            provider_active,
            network: config.network.clone(),
        },
        config.codex.clone(),
    );
    let bridge_handle = match start_local_bridge(bridge_config).await {
        Ok(handle) => Some(handle),
        Err(err) => {
            tracing::warn!("Failed to start local bridge: {}", err);
            None
        }
    };

    tracing::info!("Pylon daemon running");

    // Main event loop
    let mut shutdown_requested = false;

    loop {
        // Check for control socket commands
        if let Some(mut conn) = control_socket.try_accept() {
            match conn.read_command() {
                Ok(cmd) => {
                    let response = match cmd {
                        DaemonCommand::Ping => DaemonResponse::Pong,
                        DaemonCommand::Status => DaemonResponse::Status {
                            running: true,
                            uptime_secs: start_time.elapsed().as_secs(),
                            provider_active,
                            host_active,
                            jobs_completed,
                            earnings_msats,
                        },
                        DaemonCommand::Shutdown => {
                            shutdown_requested = true;
                            DaemonResponse::Ok
                        }
                    };
                    let _ = conn.write_response(&response);
                }
                Err(e) => {
                    tracing::warn!("Failed to read control command: {}", e);
                }
            }
        }

        if shutdown_requested {
            tracing::info!("Shutdown requested via control socket");
            break;
        }

        // Process provider events
        if let Some(ref mut events) = provider_events {
            match events.try_recv() {
                Ok(event) => {
                    let desc = event.description();
                    tracing::info!("Provider event: {}", desc);

                    // Track job completions and earnings from event type
                    match &event {
                        DomainEvent::JobReceived {
                            job_id,
                            customer_pubkey,
                            kind,
                            ..
                        } => {
                            // Create job record in database
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();

                            let job = Job {
                                id: job_id.clone(),
                                kind: *kind,
                                customer_pubkey: customer_pubkey.clone(),
                                status: JobStatus::Processing,
                                price_msats: 0,
                                input_hash: None,
                                output_hash: None,
                                error_message: None,
                                started_at: now,
                                completed_at: None,
                                created_at: now,
                            };

                            if let Err(e) = db.create_job(&job) {
                                tracing::warn!("Failed to record job: {}", e);
                            }
                        }
                        DomainEvent::JobCompleted {
                            job_id,
                            amount_msats,
                            ..
                        } => {
                            jobs_completed += 1;

                            // Complete job in database
                            let price = amount_msats.unwrap_or(0);
                            if let Err(e) = db.complete_job(job_id, None, price) {
                                tracing::warn!("Failed to complete job: {}", e);
                            }

                            // Record earning if payment was received
                            if let Some(amount) = amount_msats {
                                earnings_msats += amount;
                                if let Err(e) = db.record_job_earning(job_id, *amount, None, None) {
                                    tracing::warn!("Failed to record earning: {}", e);
                                }
                            }
                        }
                        DomainEvent::JobFailed { job_id, error, .. } => {
                            if let Err(e) = db.fail_job(job_id, error) {
                                tracing::warn!("Failed to mark job as failed: {}", e);
                            }
                        }
                        DomainEvent::PaymentReceived {
                            job_id,
                            amount_msats,
                            ..
                        } => {
                            earnings_msats += amount_msats;
                            // Mark the invoice as paid
                            if let Err(e) = db.mark_invoice_paid(job_id, *amount_msats) {
                                tracing::warn!("Failed to mark invoice paid: {}", e);
                            }
                        }
                        DomainEvent::JobStarted { job_id, .. } => {
                            // Update job status to processing
                            if let Err(e) = db.update_job_status(job_id, JobStatus::Processing) {
                                tracing::warn!("Failed to update job status: {}", e);
                            }
                        }
                        DomainEvent::InvoiceCreated {
                            job_id,
                            bolt11,
                            amount_msats,
                            ..
                        } => {
                            // Record the invoice in the database
                            if let Err(e) = db.record_invoice(job_id, bolt11, *amount_msats) {
                                tracing::warn!("Failed to record invoice: {}", e);
                            }
                        }
                        _ => {}
                    }
                }
                Err(broadcast::error::TryRecvError::Empty) => {}
                Err(broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("Dropped {} provider events", n);
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    tracing::error!("Provider event channel closed");
                    break;
                }
            }
        }

        // Check for signals
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received SIGINT, shutting down...");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                // Continue loop
            }
        }
    }

    // Cleanup
    tracing::info!("Shutting down Pylon daemon...");

    // Stop agents
    if let Some(runner) = agent_runner {
        runner.stop_all().await;
    }

    // Stop provider
    if let Some(mut p) = provider {
        p.stop().await?;
    }

    if let Some(handle) = bridge_handle {
        handle.shutdown().await;
    }

    pid_file.remove()?;

    tracing::info!("Pylon daemon stopped");
    Ok(())
}
