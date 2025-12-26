//! Daemon CLI subcommands
//!
//! Wraps autopilotd daemon commands for unified binary.

use clap::Subcommand;
use std::path::PathBuf;
use std::process::Command;
use which::which;

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

        /// Agent to use (claude, codex, gpt-oss, fm-bridge)
        #[arg(long, default_value = "claude")]
        agent: String,

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

fn resolve_autopilotd_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("OPENAGENTS_AUTOPILOTD_BIN") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    which("autopilotd").ok()
}

fn run_autopilotd_bin(args: &[String]) -> anyhow::Result<()> {
    let bin = resolve_autopilotd_bin().ok_or_else(|| {
        anyhow::anyhow!(
            "autopilotd binary not found. Set OPENAGENTS_AUTOPILOTD_BIN or install the autopilotd binary."
        )
    })?;

    let status = Command::new(bin).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("autopilotd exited with status {}", status)
    }
}

pub fn run(cmd: DaemonCommands) -> anyhow::Result<()> {
    use autopilot::daemon::config::DaemonConfig;
    use autopilot::daemon::control::ControlClient;

    let runtime = tokio::runtime::Runtime::new()?;

    // Load default config
    let config_path = shellexpand::tilde("~/.autopilot/daemon.toml").to_string();
    let config_path = PathBuf::from(config_path);
    let config = if config_path.exists() {
        match DaemonConfig::load_from_file(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Error loading daemon config from {:?}: {}", config_path, e);
                eprintln!("Using default configuration instead.");
                DaemonConfig::default()
            }
        }
    } else {
        DaemonConfig::default()
    };

    match cmd {
        DaemonCommands::Start {
            workdir,
            project,
            agent,
            model,
            max_budget,
        } => {
            let mut args = vec!["start".to_string()];
            if let Some(workdir) = workdir {
                args.push("--workdir".to_string());
                args.push(workdir.display().to_string());
            }
            if let Some(project) = project {
                args.push("--project".to_string());
                args.push(project);
            }
            args.push("--agent".to_string());
            args.push(agent);
            args.push("--model".to_string());
            args.push(model);
            args.push("--max-budget".to_string());
            args.push(max_budget.to_string());

            run_autopilotd_bin(&args)
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
