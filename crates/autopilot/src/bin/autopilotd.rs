//! Autopilot Daemon - Supervisor for autopilot worker processes
//!
//! This daemon spawns, monitors, and restarts autopilot workers, handling
//! memory pressure and crash recovery.

use anyhow::Result;
use autopilot::daemon::{
    config::DaemonConfig,
    control::{ControlClient, ControlServer},
    supervisor::{DaemonMetrics, SharedMetrics, WorkerSupervisor},
};
use autopilot::deprecation;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Guard to ensure PID file and socket are cleaned up on error/panic
struct CleanupGuard {
    pid_file: PathBuf,
    socket_path: PathBuf,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.pid_file);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[derive(Parser)]
#[command(name = "autopilotd")]
#[command(about = "Autopilot supervisor daemon")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Config file path
    #[arg(short, long, default_value = "~/.autopilot/daemon.toml")]
    config: String,

    /// Working directory for the worker
    #[arg(short, long)]
    workdir: Option<PathBuf>,

    /// Project to run
    #[arg(short, long)]
    project: Option<String>,

    /// Model to use (sonnet, opus, haiku)
    #[arg(long, default_value = "sonnet")]
    model: String,

    /// Maximum budget in USD (0 = no constraint)
    #[arg(long, default_value = "0")]
    max_budget: f64,

    /// Maximum turns
    #[arg(long, default_value = "99999")]
    max_turns: u32,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon (default)
    Start,
    /// Stop the daemon
    Stop,
    /// Get daemon status
    Status,
    /// Restart the worker process (not the daemon)
    RestartWorker,
    /// Stop the worker process (not the daemon)
    StopWorker,
}

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("{}", deprecation::autopilotd_warning());

    let cli = Cli::parse();

    // Expand ~ in config path
    let config_path = shellexpand::tilde(&cli.config).to_string();
    let config_path = PathBuf::from(config_path);

    // Load config or use defaults
    let mut config = if config_path.exists() {
        match DaemonConfig::load_from_file(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: Failed to load config: {}", e);
                DaemonConfig::default()
            }
        }
    } else {
        DaemonConfig::default()
    };

    // Override with CLI args
    if let Some(workdir) = cli.workdir {
        config.working_dir = workdir;
    }
    if let Some(project) = cli.project {
        config.project = Some(project);
    }
    config.model = cli.model;
    config.max_budget = cli.max_budget;
    config.max_turns = cli.max_turns;

    match cli.command.unwrap_or(Commands::Start) {
        Commands::Start => run_daemon(config).await,
        Commands::Stop => send_shutdown(&config).await,
        Commands::Status => show_status(&config).await,
        Commands::RestartWorker => restart_worker(&config).await,
        Commands::StopWorker => stop_worker(&config).await,
    }
}

/// Run the daemon
async fn run_daemon(config: DaemonConfig) -> Result<()> {
    // Ensure directories exist
    config.ensure_dirs()?;

    // Check if daemon is already running
    if config.pid_file.exists() {
        let pid_str = std::fs::read_to_string(&config.pid_file)?;
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            // Check if process is actually running
            let proc_path = format!("/proc/{}", pid);
            if std::path::Path::new(&proc_path).exists() {
                anyhow::bail!("Daemon already running with PID {}", pid);
            }
        }
        // Stale PID file, remove it
        std::fs::remove_file(&config.pid_file)?;
    }

    // Write PID file
    std::fs::write(&config.pid_file, std::process::id().to_string())?;

    // Cleanup guard to remove PID file on error/panic
    let pid_file_guard = CleanupGuard {
        pid_file: config.pid_file.clone(),
        socket_path: config.socket_path.clone(),
    };

    eprintln!("Starting autopilotd...");
    eprintln!("  Working dir: {:?}", config.working_dir);
    eprintln!("  Project: {:?}", config.project);
    eprintln!("  Model: {}", config.model);
    if config.max_budget > 0.0 {
        eprintln!("  Max budget: ${:.2}", config.max_budget);
    }
    eprintln!("  Socket: {:?}", config.socket_path);

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

    // Create supervisor
    let supervisor = Arc::new(Mutex::new(WorkerSupervisor::new(config.clone())));

    // Create shared metrics for control socket to read without blocking supervisor
    let shared_metrics: SharedMetrics = Arc::new(std::sync::RwLock::new(DaemonMetrics::default()));

    // Setup signal handlers and store the task handle
    let shutdown_tx_signal = shutdown_tx.clone();
    let signal_handle = tokio::spawn(async move {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to create SIGTERM handler");
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("Failed to create SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                eprintln!("Received SIGTERM");
            }
            _ = sigint.recv() => {
                eprintln!("Received SIGINT");
            }
        }

        let _ = shutdown_tx_signal.send(()).await;
    });

    // Start control socket server and store the task handle
    let control_server = ControlServer::new(&config.socket_path);
    let control_supervisor = supervisor.clone();
    let control_metrics = shared_metrics.clone();
    let control_shutdown_tx = shutdown_tx.clone();
    let control_handle = tokio::spawn(async move {
        if let Err(e) = control_server
            .run(control_supervisor, control_metrics, control_shutdown_tx)
            .await
        {
            eprintln!("Control server error: {}", e);
        }
    });

    // Run supervisor
    {
        let mut guard = supervisor.lock().await;
        if let Err(e) = guard.run(shutdown_rx, Some(shared_metrics)).await {
            eprintln!("Supervisor error: {}", e);
        }
    }

    // Await background tasks to ensure clean shutdown
    signal_handle.abort();
    let _ = signal_handle.await;

    control_handle.abort();
    let _ = control_handle.await;

    // Cleanup - the guard will also clean up on drop, but we do it explicitly
    // to ensure it happens even if we forget the guard
    let _ = std::fs::remove_file(&config.pid_file);
    let _ = std::fs::remove_file(&config.socket_path);

    // Prevent double cleanup by forgetting the guard
    std::mem::forget(pid_file_guard);

    eprintln!("Daemon stopped");
    Ok(())
}

/// Send shutdown command to running daemon
async fn send_shutdown(config: &DaemonConfig) -> Result<()> {
    let client = ControlClient::new(&config.socket_path);
    match client.shutdown().await {
        Ok(_) => {
            println!("Shutdown signal sent");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to connect to daemon: {}", e);
            eprintln!("Is the daemon running?");
            Err(e)
        }
    }
}

/// Show daemon status
async fn show_status(config: &DaemonConfig) -> Result<()> {
    let client = ControlClient::new(&config.socket_path);
    match client.status().await {
        Ok(metrics) => {
            println!("Autopilot Daemon Status");
            println!("=======================");
            println!("Worker status:  {}", metrics.worker_status);
            if let Some(pid) = metrics.worker_pid {
                println!("Worker PID:     {}", pid);
            }
            println!("Uptime:         {} seconds", metrics.uptime_seconds);
            println!("Total restarts: {}", metrics.total_restarts);
            println!("Failures:       {}", metrics.consecutive_failures);
            println!(
                "Memory:         {:.1} GB / {:.1} GB available",
                metrics.memory_available_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                metrics.memory_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to connect to daemon: {}", e);
            eprintln!("Is the daemon running?");
            Err(e)
        }
    }
}

/// Restart the worker
async fn restart_worker(config: &DaemonConfig) -> Result<()> {
    let client = ControlClient::new(&config.socket_path);
    match client.restart_worker().await {
        Ok(_) => {
            println!("Worker restart initiated");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to restart worker: {}", e);
            Err(e)
        }
    }
}

/// Stop the worker
async fn stop_worker(config: &DaemonConfig) -> Result<()> {
    let client = ControlClient::new(&config.socket_path);
    match client.stop_worker().await {
        Ok(_) => {
            println!("Worker stopped");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to stop worker: {}", e);
            Err(e)
        }
    }
}
