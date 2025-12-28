//! pylon status - Show daemon and provider status

use clap::Args;

use crate::config::PylonConfig;
use crate::daemon::{
    is_daemon_running, pid_path, socket_path, ControlClient, DaemonCommand, DaemonResponse,
    PidFile,
};
use crate::provider::PylonProvider;

/// Arguments for the status command
#[derive(Args)]
pub struct StatusArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Run the status command
pub async fn run(args: StatusArgs) -> anyhow::Result<()> {
    let config = PylonConfig::load()?;
    let daemon_running = is_daemon_running();

    // Try to get live status from daemon if running
    let daemon_status = if daemon_running {
        let client = ControlClient::new(socket_path()?);
        match client.send(DaemonCommand::Status) {
            Ok(DaemonResponse::Status {
                uptime_secs,
                provider_active,
                host_active,
                jobs_completed,
                earnings_msats,
                ..
            }) => Some((uptime_secs, provider_active, host_active, jobs_completed, earnings_msats)),
            _ => None,
        }
    } else {
        None
    };

    // Get backend status (works whether daemon is running or not)
    let provider = PylonProvider::new(config.clone()).await?;
    let backend_status = provider.status().await;

    if args.json {
        let pid = if daemon_running {
            PidFile::new(pid_path()?).read().ok()
        } else {
            None
        };

        let json = serde_json::json!({
            "daemon": {
                "running": daemon_running,
                "pid": pid,
                "uptime_secs": daemon_status.as_ref().map(|s| s.0),
                "provider_active": daemon_status.as_ref().map(|s| s.1),
                "host_active": daemon_status.as_ref().map(|s| s.2),
            },
            "stats": {
                "jobs_completed": daemon_status.as_ref().map(|s| s.3).unwrap_or(0),
                "earnings_msats": daemon_status.as_ref().map(|s| s.4).unwrap_or(0),
            },
            "backends": backend_status.backends,
            "default_backend": backend_status.default_backend,
            "relays": config.relays,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Pylon Status");
        println!("============\n");

        // Daemon status
        if daemon_running {
            let pid = PidFile::new(pid_path()?).read().unwrap_or(0);
            println!("Daemon: Running (PID: {})", pid);

            if let Some((uptime, provider, host, jobs, earnings)) = daemon_status {
                let hours = uptime / 3600;
                let mins = (uptime % 3600) / 60;
                let secs = uptime % 60;
                println!("Uptime: {}h {}m {}s", hours, mins, secs);

                let modes: Vec<&str> = [
                    if provider { Some("provider") } else { None },
                    if host { Some("host") } else { None },
                ]
                .iter()
                .filter_map(|&m| m)
                .collect();

                if !modes.is_empty() {
                    println!("Modes:  {}", modes.join(", "));
                }

                println!("\nSession Stats:");
                println!("  Jobs completed: {}", jobs);
                println!("  Earnings: {} sats ({} msats)", earnings / 1000, earnings);
            } else {
                println!("  (could not query daemon status)");
            }
        } else {
            println!("Daemon: Stopped");
            println!("\n  Run 'pylon start' to start the daemon.");
        }

        // Identity
        let data_dir = config.data_path()?;
        let identity_file = data_dir.join("identity.mnemonic");
        println!("\nIdentity:");
        if identity_file.exists() {
            println!("  Configured");
        } else {
            println!("  Not initialized (run 'pylon init')");
        }

        // Backends
        println!("\nBackends:");
        if backend_status.backends.is_empty() {
            println!("  None available");
            println!("  Install Ollama or start a llama.cpp server.");
        } else {
            for backend in &backend_status.backends {
                let is_default = backend_status.default_backend.as_ref() == Some(backend);
                let marker = if is_default { " (default)" } else { "" };
                println!("  {} {}{}", "Available:", backend, marker);
            }
        }

        // Relays
        println!("\nRelays:");
        for relay in &config.relays {
            println!("  {}", relay);
        }
    }

    Ok(())
}
