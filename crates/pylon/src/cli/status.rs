//! pylon status - Show provider status

use clap::Args;

use crate::config::PylonConfig;
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

    // Create provider to check status
    let provider = PylonProvider::new(config.clone()).await?;
    let status = provider.status().await;

    if args.json {
        let json = serde_json::json!({
            "running": status.running,
            "backends": status.backends,
            "default_backend": status.default_backend,
            "relays": status.relays,
            "jobs_processed": status.jobs_processed,
            "total_earnings_msats": status.total_earnings_msats,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Pylon Provider Status");
        println!("=====================\n");

        // Running status
        let running_status = if status.running { "🟢 Running" } else { "⚪ Stopped" };
        println!("Status: {}", running_status);

        // Identity
        let data_dir = config.data_path()?;
        let identity_file = data_dir.join("identity.enc");
        if identity_file.exists() {
            println!("Identity: ✅ Configured");
        } else {
            println!("Identity: ❌ Not initialized (run 'pylon init')");
        }

        // Backends
        println!("\nBackends:");
        if status.backends.is_empty() {
            println!("  ❌ No backends available");
        } else {
            for backend in &status.backends {
                let is_default = status.default_backend.as_ref() == Some(backend);
                let marker = if is_default { " (default)" } else { "" };
                println!("  ✅ {}{}", backend, marker);
            }
        }

        // Relays
        println!("\nRelays:");
        if status.relays.is_empty() && !status.running {
            println!("  (will connect when started)");
            for relay in &config.relays {
                println!("  - {}", relay);
            }
        } else if status.relays.is_empty() {
            println!("  ❌ Not connected");
        } else {
            for relay in &status.relays {
                println!("  🟢 {}", relay);
            }
        }

        // Stats
        println!("\nStats:");
        println!("  Jobs processed: {}", status.jobs_processed);
        let earnings_sats = status.total_earnings_msats / 1000;
        println!("  Total earnings: {} sats", earnings_sats);
    }

    Ok(())
}
