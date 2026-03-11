#![allow(
    clippy::exit,
    reason = "CLI help flow exits immediately after printing usage."
)]
#![allow(
    clippy::print_stdout,
    reason = "CLI utility intentionally writes operational results to stdout."
)]
#![allow(
    clippy::print_stderr,
    reason = "CLI utility intentionally writes usage/errors to stderr."
)]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use keyring::Entry;
use nostr::identity_mnemonic_path;
use openagents_spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use serde_json::json;

const KEYCHAIN_SERVICE: &str = "com.openagents.autopilot.credentials";
const KEYCHAIN_ACCOUNT_SPARK_API_KEY: &str = "OPENAGENTS_SPARK_API_KEY";

#[derive(Debug, Clone, Copy)]
enum SparkApiKeySource {
    Flag,
    EnvOpenAgents,
    Keychain,
    EnvBreez,
    None,
}

impl SparkApiKeySource {
    fn label(self) -> &'static str {
        match self {
            SparkApiKeySource::Flag => "flag",
            SparkApiKeySource::EnvOpenAgents => "env:OPENAGENTS_SPARK_API_KEY",
            SparkApiKeySource::Keychain => "keychain:OPENAGENTS_SPARK_API_KEY",
            SparkApiKeySource::EnvBreez => "env:BREEZ_API_KEY",
            SparkApiKeySource::None => "none",
        }
    }
}

#[derive(Debug)]
enum Command {
    Status,
    SparkAddress,
    BitcoinAddress,
    Bolt11Invoice {
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    },
    CreateInvoice {
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u64>,
    },
    PayInvoice {
        payment_request: String,
        amount_sats: Option<u64>,
    },
}

#[derive(Debug)]
struct Cli {
    command: Command,
    network: Network,
    identity_path: PathBuf,
    storage_dir: PathBuf,
    api_key: Option<String>,
    api_key_source: SparkApiKeySource,
}

fn main() -> Result<()> {
    ensure_rustls_crypto_provider().context("failed to initialize rustls crypto provider")?;
    let cli = parse_args(std::env::args().skip(1).collect())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to initialize tokio runtime")?;
    runtime.block_on(run(cli))
}

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow!("failed to install rustls crypto provider: {error:?}"))
}

async fn run(cli: Cli) -> Result<()> {
    let mnemonic = read_mnemonic(cli.identity_path.as_path())?;
    let signer = SparkSigner::from_mnemonic(&mnemonic, "")
        .map_err(|error| anyhow!("failed to derive Spark signer: {error}"))?;
    std::fs::create_dir_all(cli.storage_dir.as_path()).with_context(|| {
        format!(
            "failed to create Spark storage dir {}",
            cli.storage_dir.display()
        )
    })?;

    let wallet = SparkWallet::new(
        signer,
        WalletConfig {
            network: cli.network,
            api_key: cli.api_key.clone(),
            storage_dir: cli.storage_dir.clone(),
        },
    )
    .await
    .context("failed to initialize Spark wallet")?;

    match cli.command {
        Command::Status => {
            let network_status = wallet.network_status().await;
            let balance = wallet
                .get_balance()
                .await
                .context("failed to fetch balance")?;
            let payments = wallet
                .list_payments(Some(20), None)
                .await
                .context("failed to list payments")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "network": network_label(cli.network),
                    "identityPath": cli.identity_path.display().to_string(),
                    "storageDir": cli.storage_dir.display().to_string(),
                    "apiKeySource": cli.api_key_source.label(),
                    "networkStatus": {
                        "status": format!("{:?}", network_status.status).to_ascii_lowercase(),
                        "detail": network_status.detail,
                    },
                    "balance": {
                        "sparkSats": balance.spark_sats,
                        "lightningSats": balance.lightning_sats,
                        "onchainSats": balance.onchain_sats,
                        "totalSats": balance.total_sats(),
                    },
                    "recentPayments": payments.into_iter().take(10).map(|payment| {
                        json!({
                            "id": payment.id,
                            "direction": payment.direction,
                            "status": payment.status,
                            "amountSats": payment.amount_sats,
                            "timestamp": payment.timestamp,
                        })
                    }).collect::<Vec<_>>(),
                }))?
            );
        }
        Command::SparkAddress => {
            let address = wallet
                .get_spark_address()
                .await
                .context("failed to generate Spark receive address")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "network": network_label(cli.network),
                    "sparkAddress": address,
                    "identityPath": cli.identity_path.display().to_string(),
                    "storageDir": cli.storage_dir.display().to_string(),
                }))?
            );
        }
        Command::BitcoinAddress => {
            let address = wallet
                .get_bitcoin_address()
                .await
                .context("failed to generate Bitcoin address")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "network": network_label(cli.network),
                    "bitcoinAddress": address,
                    "identityPath": cli.identity_path.display().to_string(),
                    "storageDir": cli.storage_dir.display().to_string(),
                }))?
            );
        }
        Command::Bolt11Invoice {
            amount_sats,
            description,
            expiry_seconds,
        } => {
            let invoice = wallet
                .create_bolt11_invoice(amount_sats, description.clone(), expiry_seconds)
                .await
                .context("failed to create Bolt11 invoice")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "network": network_label(cli.network),
                    "amountSats": amount_sats,
                    "description": description,
                    "expirySeconds": expiry_seconds,
                    "invoice": invoice,
                }))?
            );
        }
        Command::CreateInvoice {
            amount_sats,
            description,
            expiry_seconds,
        } => {
            let invoice = wallet
                .create_invoice(amount_sats, description.clone(), expiry_seconds)
                .await
                .context("failed to create Spark invoice")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "network": network_label(cli.network),
                    "amountSats": amount_sats,
                    "description": description,
                    "expirySeconds": expiry_seconds,
                    "invoice": invoice,
                }))?
            );
        }
        Command::PayInvoice {
            payment_request,
            amount_sats,
        } => {
            let payment_id = wallet
                .send_payment_simple(payment_request.as_str(), amount_sats)
                .await
                .context("failed to send Spark payment")?;
            let balance = wallet
                .get_balance()
                .await
                .context("failed to refresh balance")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "network": network_label(cli.network),
                    "paymentId": payment_id,
                    "amountSats": amount_sats,
                    "apiKeySource": cli.api_key_source.label(),
                    "postBalance": {
                        "sparkSats": balance.spark_sats,
                        "lightningSats": balance.lightning_sats,
                        "onchainSats": balance.onchain_sats,
                        "totalSats": balance.total_sats(),
                    }
                }))?
            );
        }
    }

    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<Cli> {
    if args.is_empty() {
        print_usage();
        bail!("missing command");
    }

    let mut index = 0usize;
    let mut network = Network::Mainnet;
    let mut identity_path_override: Option<PathBuf> = None;
    let mut storage_dir_override: Option<PathBuf> = None;
    let mut api_key_override: Option<String> = None;

    while index < args.len() {
        match args[index].as_str() {
            "--network" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --network"))?;
                network = parse_network(value)?;
                index += 1;
            }
            "--identity-path" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --identity-path"))?;
                identity_path_override = Some(PathBuf::from(value));
                index += 1;
            }
            "--storage-dir" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --storage-dir"))?;
                storage_dir_override = Some(PathBuf::from(value));
                index += 1;
            }
            "--api-key" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --api-key"))?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("--api-key cannot be empty");
                }
                api_key_override = Some(trimmed.to_string());
                index += 1;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => break,
        }
    }

    let command = parse_command(args.as_slice(), index)?;
    let identity_path = if let Some(path) = identity_path_override {
        path
    } else {
        identity_mnemonic_path().context("failed to resolve identity mnemonic path")?
    };
    let storage_dir = if let Some(path) = storage_dir_override {
        path
    } else {
        identity_path
            .parent()
            .map(|parent| parent.join("spark"))
            .unwrap_or_else(|| PathBuf::from(".openagents/spark"))
    };

    let (api_key, api_key_source) = resolve_api_key(api_key_override);

    Ok(Cli {
        command,
        network,
        identity_path,
        storage_dir,
        api_key,
        api_key_source,
    })
}

fn parse_command(args: &[String], start_index: usize) -> Result<Command> {
    let cmd = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing command"))?;
    match cmd.as_str() {
        "status" => {
            if start_index + 1 != args.len() {
                bail!("status does not accept positional arguments");
            }
            Ok(Command::Status)
        }
        "spark-address" => {
            if start_index + 1 != args.len() {
                bail!("spark-address does not accept positional arguments");
            }
            Ok(Command::SparkAddress)
        }
        "bitcoin-address" => {
            if start_index + 1 != args.len() {
                bail!("bitcoin-address does not accept positional arguments");
            }
            Ok(Command::BitcoinAddress)
        }
        "bolt11-invoice" => {
            let amount_raw = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <amount_sats> for bolt11-invoice"))?;
            let amount_sats = amount_raw
                .parse::<u64>()
                .map_err(|error| anyhow!("invalid amount '{}': {error}", amount_raw))?;
            if amount_sats == 0 {
                bail!("bolt11-invoice amount must be greater than 0");
            }
            let mut description: Option<String> = None;
            let mut expiry_seconds: Option<u32> = None;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--description" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --description"))?;
                        if value.trim().is_empty() {
                            bail!("--description cannot be empty");
                        }
                        description = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--expiry-seconds" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --expiry-seconds"))?;
                        let value = raw.parse::<u32>().map_err(|error| {
                            anyhow!("invalid --expiry-seconds '{}': {error}", raw)
                        })?;
                        if value == 0 {
                            bail!("--expiry-seconds must be greater than 0");
                        }
                        expiry_seconds = Some(value);
                        index += 1;
                    }
                    other => {
                        bail!("unexpected argument for bolt11-invoice: {other}");
                    }
                }
            }
            Ok(Command::Bolt11Invoice {
                amount_sats,
                description,
                expiry_seconds,
            })
        }
        "create-invoice" => {
            let amount_raw = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <amount_sats> for create-invoice"))?;
            let amount_sats = amount_raw
                .parse::<u64>()
                .map_err(|error| anyhow!("invalid amount '{}': {error}", amount_raw))?;
            if amount_sats == 0 {
                bail!("create-invoice amount must be greater than 0");
            }
            let mut description: Option<String> = None;
            let mut expiry_seconds: Option<u64> = None;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--description" => {
                        index += 1;
                        let value = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --description"))?;
                        if value.trim().is_empty() {
                            bail!("--description cannot be empty");
                        }
                        description = Some(value.trim().to_string());
                        index += 1;
                    }
                    "--expiry-seconds" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --expiry-seconds"))?;
                        let value = raw.parse::<u64>().map_err(|error| {
                            anyhow!("invalid --expiry-seconds '{}': {error}", raw)
                        })?;
                        if value == 0 {
                            bail!("--expiry-seconds must be greater than 0");
                        }
                        expiry_seconds = Some(value);
                        index += 1;
                    }
                    other => {
                        bail!("unexpected argument for create-invoice: {other}");
                    }
                }
            }
            Ok(Command::CreateInvoice {
                amount_sats,
                description,
                expiry_seconds,
            })
        }
        "pay-invoice" => {
            let request = args
                .get(start_index + 1)
                .ok_or_else(|| anyhow!("missing <payment_request> for pay-invoice"))?
                .trim()
                .to_string();
            if request.is_empty() {
                bail!("payment request cannot be empty");
            }
            let mut amount_sats = None;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--amount-sats" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --amount-sats"))?;
                        let value = raw
                            .parse::<u64>()
                            .map_err(|error| anyhow!("invalid --amount-sats '{}': {error}", raw))?;
                        if value == 0 {
                            bail!("--amount-sats must be greater than 0");
                        }
                        amount_sats = Some(value);
                        index += 1;
                    }
                    other => {
                        bail!("unexpected argument for pay-invoice: {other}");
                    }
                }
            }
            Ok(Command::PayInvoice {
                payment_request: request,
                amount_sats,
            })
        }
        other => {
            bail!("unsupported command '{other}'");
        }
    }
}

fn parse_network(raw: &str) -> Result<Network> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "mainnet" => Ok(Network::Mainnet),
        "regtest" => Ok(Network::Regtest),
        "testnet" | "signet" => {
            bail!("unsupported Spark network '{raw}' (supported: mainnet, regtest)")
        }
        _ => bail!("invalid --network '{raw}' (supported: mainnet, regtest)"),
    }
}

fn network_label(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Regtest => "regtest",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
    }
}

fn resolve_api_key(flag_value: Option<String>) -> (Option<String>, SparkApiKeySource) {
    if let Some(value) = flag_value {
        return (Some(value), SparkApiKeySource::Flag);
    }
    if let Some(value) = read_env_nonempty("OPENAGENTS_SPARK_API_KEY") {
        return (Some(value), SparkApiKeySource::EnvOpenAgents);
    }
    if let Some(value) = read_keychain_nonempty(KEYCHAIN_ACCOUNT_SPARK_API_KEY) {
        return (Some(value), SparkApiKeySource::Keychain);
    }
    if let Some(value) = read_env_nonempty("BREEZ_API_KEY") {
        return (Some(value), SparkApiKeySource::EnvBreez);
    }
    (None, SparkApiKeySource::None)
}

fn read_mnemonic(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read identity mnemonic {}", path.display()))?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        bail!("identity mnemonic is empty at {}", path.display());
    }
    Ok(trimmed.to_string())
}

fn read_env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_keychain_nonempty(account: &str) -> Option<String> {
    Entry::new(KEYCHAIN_SERVICE, account)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn print_usage() {
    eprintln!(
        "spark-wallet-cli\n\
         \n\
         Usage:\n\
           cargo run -p autopilot-desktop --bin spark-wallet-cli -- [options] status\n\
           cargo run -p autopilot-desktop --bin spark-wallet-cli -- [options] spark-address\n\
           cargo run -p autopilot-desktop --bin spark-wallet-cli -- [options] bitcoin-address\n\
           cargo run -p autopilot-desktop --bin spark-wallet-cli -- [options] bolt11-invoice <amount_sats> [--description <text>] [--expiry-seconds <n>]\n\
           cargo run -p autopilot-desktop --bin spark-wallet-cli -- [options] create-invoice <amount_sats> [--description <text>] [--expiry-seconds <n>]\n\
           cargo run -p autopilot-desktop --bin spark-wallet-cli -- [options] pay-invoice <payment_request> [--amount-sats <n>]\n\
         \n\
         Options:\n\
           --network <mainnet|regtest>      Spark network (default: mainnet)\n\
           --identity-path <path>           Override identity mnemonic path\n\
           --storage-dir <path>             Override Spark storage directory\n\
           --api-key <value>                Override Spark API key\n\
           --help, -h                       Show this help\n\
         \n\
         API key resolution order:\n\
           1) --api-key\n\
           2) OPENAGENTS_SPARK_API_KEY env\n\
           3) keychain service '{KEYCHAIN_SERVICE}' account '{KEYCHAIN_ACCOUNT_SPARK_API_KEY}'\n\
           4) BREEZ_API_KEY env\n"
    );
}
