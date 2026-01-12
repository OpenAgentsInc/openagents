//! pylon doctor - Run diagnostics

use clap::Args;
use openagents_runtime::UnifiedIdentity;

use crate::config::PylonConfig;
use crate::provider::PylonProvider;

/// Arguments for the doctor command
#[derive(Args)]
pub struct DoctorArgs {
    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,
}

/// Run the doctor command
pub async fn run(args: DoctorArgs) -> anyhow::Result<()> {
    println!("Pylon Diagnostics");
    println!("=================\n");

    let config = PylonConfig::load()?;
    let mut provider = PylonProvider::new(config.clone()).await?;

    // Try to load identity from file
    let data_dir = config.data_path()?;
    let identity_file = data_dir.join("identity.mnemonic");
    if identity_file.exists() {
        let mnemonic = std::fs::read_to_string(&identity_file)?;
        if let Ok(identity) = UnifiedIdentity::from_mnemonic(mnemonic.trim(), "") {
            provider.set_identity(identity);
        }
    }
    let diag = provider.doctor().await;

    // Identity check
    print!("Identity:      ");
    if diag.identity_ok {
        println!("✅ OK");
        if let Some(ref npub) = diag.identity_npub {
            println!("               {}", npub);
        }
    } else {
        println!("❌ Not configured");
        println!("               Run 'pylon init' to create identity");
    }

    // Backend checks
    println!("\nBackends:");
    let mut any_backend = false;
    for (backend_id, available) in &diag.backends {
        let (endpoint, desc) = match backend_id.as_str() {
            "ollama" => ("localhost:11434", "Ollama"),
            "apple_fm" => ("localhost:11435", "Apple FM (fm-bridge)"),
            "llamacpp" => ("localhost:8080", "Llama.cpp / GPT-OSS"),
            other => (other, other),
        };

        if *available {
            println!("  ✅ {} ({})", desc, endpoint);
            any_backend = true;
        } else if args.verbose {
            println!("  ❌ {} ({}) - not responding", desc, endpoint);
        }
    }

    if !any_backend {
        println!("  ❌ No backends available");
        println!("\n  To fix:");
        println!("  - Install Ollama: https://ollama.ai");
        println!("  - Or start llama.cpp server on port 8080");
        if cfg!(target_os = "macos") {
            println!("  - Or run fm-bridge for Apple Foundation Models");
        }
    }

    // Relay checks
    println!("\nRelays:");
    for (url, _ok) in &diag.relays {
        // TODO: actually test connectivity
        println!("  📡 {}", url);
    }

    // Configuration
    if args.verbose {
        println!("\nConfiguration:");
        println!("  Config file: {:?}", PylonConfig::config_path()?);
        println!("  Data dir:    {:?}", config.data_path()?);
        println!(
            "  Min price:   {} msats ({} sats)",
            config.min_price_msats,
            config.min_price_msats / 1000
        );
        println!("  Default model: {}", config.default_model);
    }

    // Warnings
    if !diag.warnings.is_empty() {
        println!("\nWarnings:");
        for warning in &diag.warnings {
            println!("  ⚠️  {}", warning);
        }
    }

    // Summary
    println!();
    let all_ok = diag.identity_ok && any_backend;
    if all_ok {
        println!("✅ Ready to start earning! Run 'pylon start'");
    } else {
        println!("❌ Some issues need to be resolved before starting");
    }

    Ok(())
}
