#![allow(
    clippy::print_stdout,
    reason = "headless CLI intentionally prints operational endpoints and summaries."
)]

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use autopilot_desktop::headless_compute::{
    HEADLESS_BUY_MODE_BUDGET_SATS, HEADLESS_BUY_MODE_INTERVAL_SECONDS, HEADLESS_BUY_MODE_PROMPT,
    HEADLESS_BUY_MODE_REQUEST_TYPE, HEADLESS_BUY_MODE_TIMEOUT_SECONDS, HeadlessBuyerConfig,
    HeadlessIdentitySummary, HeadlessNip28SeedConfig, HeadlessNip28SeedSummary,
    HeadlessProviderBackend, HeadlessProviderConfig, HeadlessRelayConfig, identity_summary,
    run_headless_buyer, run_headless_provider, run_headless_relay,
    seed_headless_nip28_main_channel,
};
use autopilot_desktop::logging;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "autopilot-headless-compute")]
#[command(about = "Headless buyer/provider/relay runtime for OpenAgents compute handshake")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Relay {
        #[arg(long, default_value = "127.0.0.1:18490")]
        listen: SocketAddr,
        #[arg(long, default_value_t = 1024)]
        event_capacity: usize,
    },
    Provider {
        #[arg(long = "relay", required = true)]
        relay_urls: Vec<String>,
        #[arg(long)]
        identity_path: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = ProviderBackendArg::Auto)]
        backend: ProviderBackendArg,
        #[arg(long)]
        max_settled_jobs: Option<usize>,
        #[arg(long, default_value_t = 3600)]
        invoice_expiry_seconds: u32,
    },
    Buyer {
        #[arg(long = "relay", required = true)]
        relay_urls: Vec<String>,
        #[arg(long)]
        identity_path: Option<PathBuf>,
        #[arg(long, default_value = HEADLESS_BUY_MODE_REQUEST_TYPE)]
        request_type: String,
        #[arg(long, default_value = HEADLESS_BUY_MODE_PROMPT)]
        prompt: String,
        #[arg(long, default_value_t = HEADLESS_BUY_MODE_BUDGET_SATS)]
        budget_sats: u64,
        #[arg(long, default_value_t = HEADLESS_BUY_MODE_TIMEOUT_SECONDS)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = HEADLESS_BUY_MODE_INTERVAL_SECONDS)]
        interval_seconds: u64,
        #[arg(long = "target-provider-pubkey")]
        target_provider_pubkeys: Vec<String>,
        #[arg(long)]
        max_settled_requests: Option<usize>,
        #[arg(long)]
        fail_fast: bool,
    },
    Identity {
        #[arg(long)]
        identity_path: Option<PathBuf>,
    },
    SeedNip28Main {
        #[arg(long = "relay", required = true)]
        relay_urls: Vec<String>,
        #[arg(long)]
        identity_path: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProviderBackendArg {
    Auto,
    AppleFm,
    Canned,
}

impl ProviderBackendArg {
    fn resolve(self) -> HeadlessProviderBackend {
        match self {
            Self::Auto => {
                if cfg!(target_os = "macos") {
                    HeadlessProviderBackend::AppleFoundationModels
                } else {
                    HeadlessProviderBackend::Canned
                }
            }
            Self::AppleFm => HeadlessProviderBackend::AppleFoundationModels,
            Self::Canned => HeadlessProviderBackend::Canned,
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    logging::init();
    let cli = Cli::parse();
    match cli.command {
        Command::Relay {
            listen,
            event_capacity,
        } => {
            println!("headless relay listening on ws://{listen}");
            run_headless_relay(HeadlessRelayConfig {
                listen_addr: listen,
                event_capacity,
            })
            .await
        }
        Command::Provider {
            relay_urls,
            identity_path,
            backend,
            max_settled_jobs,
            invoice_expiry_seconds,
        } => run_headless_provider(HeadlessProviderConfig {
            relay_urls,
            identity_path,
            backend: backend.resolve(),
            max_settled_jobs,
            invoice_expiry_seconds,
        }),
        Command::Buyer {
            relay_urls,
            identity_path,
            request_type,
            prompt,
            budget_sats,
            timeout_seconds,
            interval_seconds,
            target_provider_pubkeys,
            max_settled_requests,
            fail_fast,
        } => run_headless_buyer(HeadlessBuyerConfig {
            relay_urls,
            identity_path,
            request_type,
            prompt,
            budget_sats,
            timeout_seconds,
            interval_seconds,
            target_provider_pubkeys,
            max_settled_requests,
            fail_fast,
        }),
        Command::Identity { identity_path } => {
            let summary = identity_summary(identity_path)?;
            print_identity_summary(&summary);
            Ok(())
        }
        Command::SeedNip28Main {
            relay_urls,
            identity_path,
        } => {
            let summary = seed_headless_nip28_main_channel(HeadlessNip28SeedConfig {
                relay_urls,
                identity_path,
            })
            .await?;
            print_nip28_seed_summary(&summary);
            Ok(())
        }
    }
}

fn print_identity_summary(summary: &HeadlessIdentitySummary) {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "identityPath": summary.identity_path.display().to_string(),
            "npub": summary.npub,
            "publicKeyHex": summary.public_key_hex,
        }))
        .unwrap_or_else(|error| format!(
            "{{\"error\":\"{}\"}}",
            escape_json_string(error.to_string().as_str())
        ))
    );
}

fn print_nip28_seed_summary(summary: &HeadlessNip28SeedSummary) {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "identityPath": summary.identity_path.display().to_string(),
            "npub": summary.npub,
            "publicKeyHex": summary.public_key_hex,
            "relayUrls": summary.relay_urls,
            "groupId": summary.group_id,
            "groupMetadataEventId": summary.group_metadata_event_id,
            "channelId": summary.channel_id,
        }))
        .unwrap_or_else(|error| format!(
            "{{\"error\":\"{}\"}}",
            escape_json_string(error.to_string().as_str())
        ))
    );
}

fn escape_json_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
