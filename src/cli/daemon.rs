//! Daemon CLI subcommands
//!
//! Wraps autopilotd daemon commands for unified binary.

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DaemonCommands {
    /// Start the daemon
    Start {
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
    },

    /// Stop the daemon
    Stop,

    /// Get daemon status
    Status,

    /// Restart the worker process (not the daemon)
    RestartWorker,

    /// Stop the worker process (not the daemon)
    StopWorker,
}

pub fn run(cmd: DaemonCommands) -> anyhow::Result<()> {
    use autopilot::daemon::config::DaemonConfig;
    use autopilot::daemon::control::ControlClient;

    let runtime = tokio::runtime::Runtime::new()?;

    // Load default config
    let config_path = shellexpand::tilde("~/.autopilot/daemon.toml").to_string();
    let config_path = PathBuf::from(config_path);
    let mut config = if config_path.exists() {
        DaemonConfig::load_from_file(&config_path).unwrap_or_default()
    } else {
        DaemonConfig::default()
    };

    match cmd {
        DaemonCommands::Start {
            workdir,
            project,
            model,
            max_budget,
        } => {
            // Override with CLI args
            if let Some(workdir) = workdir {
                config.working_dir = workdir;
            }
            if let Some(project) = project {
                config.project = Some(project);
            }
            config.model = model;
            config.max_budget = max_budget;

            runtime.block_on(async {
                // Daemon start requires process forking which is not yet integrated
                anyhow::bail!("Daemon start requires the standalone daemon binary. Use: cargo run --bin daemon -- start --workdir {:?} --project {:?}",
                    config.working_dir,
                    config.project.as_deref().unwrap_or("(none)")
                )
            })
        }
        DaemonCommands::Stop => {
            runtime.block_on(async {
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
            })
        }
        DaemonCommands::Status => {
            runtime.block_on(async {
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
            })
        }
        DaemonCommands::RestartWorker => {
            runtime.block_on(async {
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
            })
        }
        DaemonCommands::StopWorker => {
            runtime.block_on(async {
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
            })
        }
    }
}
