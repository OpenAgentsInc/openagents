use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use openagents_spark::{
    Balance as SparkBalance, Network as SparkNetwork, PaymentSummary, SparkSigner, SparkWallet,
    WalletConfig,
};
use serde::Serialize;

use crate::{
    PylonWalletInvoiceRecord, PylonWalletPaymentRecord, ensure_local_setup, mutate_ledger,
    now_epoch_ms,
};

// Release fallback so standalone Pylon boots without requiring shell env injection.
const DEFAULT_OPENAGENTS_SPARK_API_KEY: &str = "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0=";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WalletSubcommand {
    Status {
        json: bool,
    },
    Balance {
        json: bool,
    },
    Address {
        json: bool,
    },
    Invoice {
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
        json: bool,
    },
    Pay {
        payment_request: String,
        amount_sats: Option<u64>,
        json: bool,
    },
    History {
        limit: Option<u32>,
        json: bool,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletRuntimeSurface {
    pub network: String,
    pub identity_path: String,
    pub storage_dir: String,
    pub api_key_env: Option<String>,
    pub api_key_source: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletBalanceSnapshot {
    pub spark_sats: u64,
    pub lightning_sats: u64,
    pub onchain_sats: u64,
    pub total_sats: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletStatusReport {
    pub runtime: WalletRuntimeSurface,
    pub runtime_status: String,
    pub runtime_detail: Option<String>,
    pub balance: WalletBalanceSnapshot,
    pub recent_payments: Vec<PylonWalletPaymentRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletAddressReport {
    pub runtime: WalletRuntimeSurface,
    pub spark_address: String,
    pub bitcoin_address: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletInvoiceReport {
    pub runtime: WalletRuntimeSurface,
    pub invoice: PylonWalletInvoiceRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletPayReport {
    pub runtime: WalletRuntimeSurface,
    pub payment_id: String,
    pub payment: PylonWalletPaymentRecord,
    pub post_balance: WalletBalanceSnapshot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletHistoryReport {
    pub runtime: WalletRuntimeSurface,
    pub payments: Vec<PylonWalletPaymentRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WalletApiKeySource {
    ConfigEnv,
    OpenAgentsEnv,
    BreezEnv,
    EmbeddedDefault,
}

impl WalletApiKeySource {
    fn label(self, config_env: Option<&str>) -> String {
        match self {
            Self::ConfigEnv => format!(
                "env:{}",
                config_env.unwrap_or("OPENAGENTS_SPARK_API_KEY").trim()
            ),
            Self::OpenAgentsEnv => "env:OPENAGENTS_SPARK_API_KEY".to_string(),
            Self::BreezEnv => "env:BREEZ_API_KEY".to_string(),
            Self::EmbeddedDefault => "embedded:openagents-default".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct WalletRuntimeContext {
    runtime: WalletRuntimeSurface,
    network: SparkNetwork,
    identity_path: PathBuf,
    storage_dir: PathBuf,
    api_key: Option<String>,
}

pub async fn run_wallet_command(config_path: &Path, command: &WalletSubcommand) -> Result<String> {
    match command {
        WalletSubcommand::Status { json } => {
            let report = load_wallet_status_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_status_report(&report))
        }
        WalletSubcommand::Balance { json } => {
            let report = load_wallet_status_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report.balance)?);
            }
            Ok(render_wallet_balance_report(&report))
        }
        WalletSubcommand::Address { json } => {
            let report = create_wallet_address_report(config_path).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_address_report(&report))
        }
        WalletSubcommand::Invoice {
            amount_sats,
            description,
            expiry_seconds,
            json,
        } => {
            let report = create_wallet_invoice_report(
                config_path,
                *amount_sats,
                description.clone(),
                *expiry_seconds,
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_invoice_report(&report))
        }
        WalletSubcommand::Pay {
            payment_request,
            amount_sats,
            json,
        } => {
            let report =
                pay_wallet_invoice_report(config_path, payment_request.as_str(), *amount_sats)
                    .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_pay_report(&report))
        }
        WalletSubcommand::History { limit, json } => {
            let report = load_wallet_history_report(config_path, *limit).await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_wallet_history_report(&report))
        }
    }
}

pub fn parse_wallet_command(args: &[String], start_index: usize) -> Result<WalletSubcommand> {
    let subcommand = args
        .get(start_index + 1)
        .ok_or_else(|| anyhow!("missing wallet subcommand"))?;
    match subcommand.as_str() {
        "status" => Ok(WalletSubcommand::Status {
            json: parse_json_only(args, start_index + 2, "wallet status")?,
        }),
        "balance" => Ok(WalletSubcommand::Balance {
            json: parse_json_only(args, start_index + 2, "wallet balance")?,
        }),
        "address" => Ok(WalletSubcommand::Address {
            json: parse_json_only(args, start_index + 2, "wallet address")?,
        }),
        "invoice" => {
            let amount_raw = args
                .get(start_index + 2)
                .ok_or_else(|| anyhow!("missing <amount_sats> for wallet invoice"))?;
            let amount_sats = amount_raw
                .parse::<u64>()
                .map_err(|error| anyhow!("invalid amount '{}': {error}", amount_raw))?;
            if amount_sats == 0 {
                bail!("wallet invoice amount must be greater than 0");
            }
            let mut description = None;
            let mut expiry_seconds = None;
            let mut json = false;
            let mut index = start_index + 3;
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
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet invoice: {other}"),
                }
            }
            Ok(WalletSubcommand::Invoice {
                amount_sats,
                description,
                expiry_seconds,
                json,
            })
        }
        "pay" => {
            let payment_request = args
                .get(start_index + 2)
                .ok_or_else(|| anyhow!("missing <payment_request> for wallet pay"))?
                .trim()
                .to_string();
            if payment_request.is_empty() {
                bail!("payment request cannot be empty");
            }
            let mut amount_sats = None;
            let mut json = false;
            let mut index = start_index + 3;
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
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet pay: {other}"),
                }
            }
            Ok(WalletSubcommand::Pay {
                payment_request,
                amount_sats,
                json,
            })
        }
        "history" => {
            let mut limit = None;
            let mut json = false;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--limit" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --limit"))?;
                        let value = raw
                            .parse::<u32>()
                            .map_err(|error| anyhow!("invalid --limit '{}': {error}", raw))?;
                        if value == 0 {
                            bail!("--limit must be greater than 0");
                        }
                        limit = Some(value);
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for wallet history: {other}"),
                }
            }
            Ok(WalletSubcommand::History { limit, json })
        }
        other => bail!("unsupported wallet subcommand '{other}'"),
    }
}

pub async fn load_wallet_status_report(config_path: &Path) -> Result<WalletStatusReport> {
    let context = prepare_wallet_context(config_path)?;
    let runtime = context.runtime.clone();
    let result: Result<WalletStatusReport> = async {
        let wallet = open_wallet(&context).await?;
        let network_status = wallet.network_status().await;
        let balance = wallet
            .get_balance()
            .await
            .context("failed to fetch Spark balance")?;
        let payments = wallet
            .list_payments(Some(10), None)
            .await
            .context("failed to list Spark payments")?;
        let report = WalletStatusReport {
            runtime,
            runtime_status: network_status_label(&network_status),
            runtime_detail: network_status.detail.clone(),
            balance: balance_snapshot(&balance),
            recent_payments: payments
                .iter()
                .map(payment_record_from_summary)
                .collect::<Vec<_>>(),
        };
        sync_wallet_status(
            config_path,
            &report.runtime,
            report.runtime_status.as_str(),
            report.runtime_detail.clone(),
            Some(&report.balance),
            None,
            None,
            report.recent_payments.as_slice(),
        )?;
        Ok(report)
    }
    .await;
    if let Err(error) = result.as_ref() {
        sync_wallet_error(config_path, &context.runtime, error.to_string())?;
    }
    result
}

pub async fn create_wallet_address_report(config_path: &Path) -> Result<WalletAddressReport> {
    let context = prepare_wallet_context(config_path)?;
    let runtime = context.runtime.clone();
    let result: Result<WalletAddressReport> = async {
        let wallet = open_wallet(&context).await?;
        let spark_address = wallet
            .get_spark_address()
            .await
            .context("failed to create Spark receive address")?;
        let bitcoin_address = wallet
            .get_bitcoin_address()
            .await
            .context("failed to create Bitcoin receive address")?;
        let report = WalletAddressReport {
            runtime,
            spark_address,
            bitcoin_address,
        };
        sync_wallet_status(
            config_path,
            &report.runtime,
            "connected",
            None,
            None,
            Some(report.spark_address.as_str()),
            Some(report.bitcoin_address.as_str()),
            &[],
        )?;
        Ok(report)
    }
    .await;
    if let Err(error) = result.as_ref() {
        sync_wallet_error(config_path, &context.runtime, error.to_string())?;
    }
    result
}

pub async fn create_wallet_invoice_report(
    config_path: &Path,
    amount_sats: u64,
    description: Option<String>,
    expiry_seconds: Option<u32>,
) -> Result<WalletInvoiceReport> {
    let context = prepare_wallet_context(config_path)?;
    let runtime = context.runtime.clone();
    let result: Result<WalletInvoiceReport> = async {
        let wallet = open_wallet(&context).await?;
        let payment_request = wallet
            .create_bolt11_invoice(amount_sats, description.clone(), expiry_seconds)
            .await
            .context("failed to create Bolt11 invoice")?;
        let invoice = PylonWalletInvoiceRecord {
            invoice_id: format!("bolt11-{}", now_epoch_ms()),
            amount_sats,
            status: "created".to_string(),
            payment_request,
            description,
            created_at_ms: now_epoch_ms() as u64,
            updated_at_ms: now_epoch_ms() as u64,
        };
        mutate_ledger(config_path, |ledger| {
            ledger.wallet.runtime_status = Some("connected".to_string());
            ledger.wallet.last_error = None;
            ledger.wallet.network = Some(runtime.network.clone());
            ledger.upsert_wallet_invoice(invoice.clone());
            Ok(())
        })?;
        Ok(WalletInvoiceReport { runtime, invoice })
    }
    .await;
    if let Err(error) = result.as_ref() {
        sync_wallet_error(config_path, &context.runtime, error.to_string())?;
    }
    result
}

pub async fn pay_wallet_invoice_report(
    config_path: &Path,
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<WalletPayReport> {
    let context = prepare_wallet_context(config_path)?;
    let runtime = context.runtime.clone();
    let request = payment_request.trim().to_string();
    let result: Result<WalletPayReport> = async {
        let wallet = open_wallet(&context).await?;
        let payment_id = wallet
            .send_payment_simple(request.as_str(), amount_sats)
            .await
            .context("failed to send Spark payment")?;
        let balance = wallet
            .get_balance()
            .await
            .context("failed to refresh Spark balance after send")?;
        let payments = wallet
            .list_payments(Some(20), None)
            .await
            .context("failed to refresh Spark payment history after send")?;
        let payment = payments
            .iter()
            .find(|payment| payment.id == payment_id)
            .map(payment_record_from_summary)
            .unwrap_or_else(|| PylonWalletPaymentRecord {
                payment_id: payment_id.clone(),
                direction: "send".to_string(),
                status: "submitted".to_string(),
                amount_sats: amount_sats.unwrap_or(0),
                fees_sats: 0,
                method: "spark".to_string(),
                description: None,
                invoice: Some(request.clone()),
                created_at_ms: now_epoch_ms() as u64,
                updated_at_ms: now_epoch_ms() as u64,
            });
        let post_balance = balance_snapshot(&balance);
        sync_wallet_status(
            config_path,
            &runtime,
            "connected",
            None,
            Some(&post_balance),
            None,
            None,
            std::slice::from_ref(&payment),
        )?;
        Ok(WalletPayReport {
            runtime,
            payment_id,
            payment,
            post_balance,
        })
    }
    .await;
    if let Err(error) = result.as_ref() {
        sync_wallet_error(config_path, &context.runtime, error.to_string())?;
    }
    result
}

pub async fn load_wallet_history_report(
    config_path: &Path,
    limit: Option<u32>,
) -> Result<WalletHistoryReport> {
    let context = prepare_wallet_context(config_path)?;
    let runtime = context.runtime.clone();
    let result: Result<WalletHistoryReport> = async {
        let wallet = open_wallet(&context).await?;
        let payments = wallet
            .list_payments(limit.or(Some(20)), None)
            .await
            .context("failed to list Spark payments")?;
        let records = payments
            .iter()
            .map(payment_record_from_summary)
            .collect::<Vec<_>>();
        sync_wallet_status(
            config_path,
            &runtime,
            "connected",
            None,
            None,
            None,
            None,
            records.as_slice(),
        )?;
        Ok(WalletHistoryReport {
            runtime,
            payments: records,
        })
    }
    .await;
    if let Err(error) = result.as_ref() {
        sync_wallet_error(config_path, &context.runtime, error.to_string())?;
    }
    result
}

pub fn render_wallet_status_report(report: &WalletStatusReport) -> String {
    let mut lines = vec![
        format!("runtime_status: {}", report.runtime_status),
        format!("network: {}", report.runtime.network),
        format!("api_key_source: {}", report.runtime.api_key_source),
        format!("identity_path: {}", report.runtime.identity_path),
        format!("storage_dir: {}", report.runtime.storage_dir),
        format!("spark_sats: {}", report.balance.spark_sats),
        format!("lightning_sats: {}", report.balance.lightning_sats),
        format!("onchain_sats: {}", report.balance.onchain_sats),
        format!("total_sats: {}", report.balance.total_sats),
    ];
    if let Some(detail) = report.runtime_detail.as_deref() {
        lines.push(format!("runtime_detail: {detail}"));
    }
    if report.recent_payments.is_empty() {
        lines.push(String::new());
        lines.push("recent_payments: none".to_string());
        return lines.join("\n");
    }
    for payment in &report.recent_payments {
        lines.push(String::new());
        lines.push(format!("payment_id: {}", payment.payment_id));
        lines.push(format!("direction: {}", payment.direction));
        lines.push(format!("status: {}", payment.status));
        lines.push(format!("amount_sats: {}", payment.amount_sats));
        lines.push(format!("fees_sats: {}", payment.fees_sats));
        lines.push(format!("method: {}", payment.method));
        if let Some(description) = payment.description.as_deref() {
            lines.push(format!("description: {description}"));
        }
        if let Some(invoice) = payment.invoice.as_deref() {
            lines.push(format!("invoice: {invoice}"));
        }
    }
    lines.join("\n")
}

pub fn render_wallet_balance_report(report: &WalletStatusReport) -> String {
    let mut lines = vec![
        format!("network: {}", report.runtime.network),
        format!("runtime_status: {}", report.runtime_status),
        format!("spark_sats: {}", report.balance.spark_sats),
        format!("lightning_sats: {}", report.balance.lightning_sats),
        format!("onchain_sats: {}", report.balance.onchain_sats),
        format!("total_sats: {}", report.balance.total_sats),
    ];
    if let Some(detail) = report.runtime_detail.as_deref() {
        lines.push(format!("runtime_detail: {detail}"));
    }
    lines.join("\n")
}

pub fn render_wallet_address_report(report: &WalletAddressReport) -> String {
    [
        format!("network: {}", report.runtime.network),
        format!("spark_address: {}", report.spark_address),
        format!("bitcoin_address: {}", report.bitcoin_address),
    ]
    .join("\n")
}

pub fn render_wallet_invoice_report(report: &WalletInvoiceReport) -> String {
    let mut lines = vec![
        format!("network: {}", report.runtime.network),
        format!("invoice_id: {}", report.invoice.invoice_id),
        format!("amount_sats: {}", report.invoice.amount_sats),
        format!("status: {}", report.invoice.status),
        format!("payment_request: {}", report.invoice.payment_request),
    ];
    if let Some(description) = report.invoice.description.as_deref() {
        lines.push(format!("description: {description}"));
    }
    lines.join("\n")
}

pub fn render_wallet_pay_report(report: &WalletPayReport) -> String {
    let mut lines = vec![
        format!("network: {}", report.runtime.network),
        format!("payment_id: {}", report.payment_id),
        format!("status: {}", report.payment.status),
        format!("amount_sats: {}", report.payment.amount_sats),
        format!("fees_sats: {}", report.payment.fees_sats),
        format!("total_sats: {}", report.post_balance.total_sats),
    ];
    if let Some(description) = report.payment.description.as_deref() {
        lines.push(format!("description: {description}"));
    }
    if let Some(invoice) = report.payment.invoice.as_deref() {
        lines.push(format!("invoice: {invoice}"));
    }
    lines.join("\n")
}

pub fn render_wallet_history_report(report: &WalletHistoryReport) -> String {
    let mut lines = vec![
        format!("network: {}", report.runtime.network),
        format!("payments: {}", report.payments.len()),
    ];
    if report.payments.is_empty() {
        lines.push(String::new());
        lines.push("history: none".to_string());
        return lines.join("\n");
    }
    for payment in &report.payments {
        lines.push(String::new());
        lines.push(format!("payment_id: {}", payment.payment_id));
        lines.push(format!("direction: {}", payment.direction));
        lines.push(format!("status: {}", payment.status));
        lines.push(format!("amount_sats: {}", payment.amount_sats));
        lines.push(format!("fees_sats: {}", payment.fees_sats));
        lines.push(format!("method: {}", payment.method));
        if let Some(description) = payment.description.as_deref() {
            lines.push(format!("description: {description}"));
        }
        if let Some(invoice) = payment.invoice.as_deref() {
            lines.push(format!("invoice: {invoice}"));
        }
    }
    lines.join("\n")
}

fn parse_json_only(args: &[String], start_index: usize, label: &str) -> Result<bool> {
    let mut json = false;
    let mut index = start_index;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            other => bail!("unexpected argument for {label}: {other}"),
        }
    }
    Ok(json)
}

fn prepare_wallet_context(config_path: &Path) -> Result<WalletRuntimeContext> {
    ensure_rustls_crypto_provider()?;
    let config = ensure_local_setup(config_path)?;
    let network = parse_wallet_network(config.wallet_network.as_str())?;
    let (api_key, api_key_source) = resolve_wallet_api_key(config.wallet_api_key_env.as_deref());
    std::fs::create_dir_all(config.wallet_storage_dir.as_path()).with_context(|| {
        format!(
            "failed to create wallet storage dir {}",
            config.wallet_storage_dir.display()
        )
    })?;
    Ok(WalletRuntimeContext {
        runtime: WalletRuntimeSurface {
            network: wallet_network_label(network).to_string(),
            identity_path: config.identity_path.display().to_string(),
            storage_dir: config.wallet_storage_dir.display().to_string(),
            api_key_env: config.wallet_api_key_env.clone(),
            api_key_source: api_key_source.label(config.wallet_api_key_env.as_deref()),
        },
        network,
        identity_path: config.identity_path.clone(),
        storage_dir: config.wallet_storage_dir.clone(),
        api_key,
    })
}

async fn open_wallet(context: &WalletRuntimeContext) -> Result<SparkWallet> {
    let mnemonic = std::fs::read_to_string(context.identity_path.as_path()).with_context(|| {
        format!(
            "failed to read identity mnemonic {}",
            context.identity_path.display()
        )
    })?;
    let mnemonic = mnemonic.trim();
    if mnemonic.is_empty() {
        bail!(
            "identity mnemonic is empty at {}",
            context.identity_path.display()
        );
    }
    let signer = SparkSigner::from_mnemonic(mnemonic, "")
        .map_err(|error| anyhow!("failed to derive Spark signer: {error}"))?;
    SparkWallet::new(
        signer,
        WalletConfig {
            network: context.network,
            api_key: context.api_key.clone(),
            storage_dir: context.storage_dir.clone(),
        },
    )
    .await
    .context("failed to initialize Spark wallet")
}

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow!("failed to install rustls crypto provider: {error:?}"))
}

fn parse_wallet_network(raw: &str) -> Result<SparkNetwork> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "mainnet" => Ok(SparkNetwork::Mainnet),
        "regtest" => Ok(SparkNetwork::Regtest),
        "testnet" | "signet" => {
            bail!("unsupported Spark network '{raw}' (supported: mainnet, regtest)")
        }
        _ => bail!("invalid wallet_network '{raw}' (supported: mainnet, regtest)"),
    }
}

fn wallet_network_label(network: SparkNetwork) -> &'static str {
    match network {
        SparkNetwork::Mainnet => "mainnet",
        SparkNetwork::Regtest => "regtest",
        SparkNetwork::Testnet => "testnet",
        SparkNetwork::Signet => "signet",
    }
}

fn resolve_wallet_api_key(config_env: Option<&str>) -> (Option<String>, WalletApiKeySource) {
    if let Some(name) = config_env
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "OPENAGENTS_SPARK_API_KEY")
        && let Some(value) = read_env_nonempty(name)
    {
        return (Some(value), WalletApiKeySource::ConfigEnv);
    }
    if let Some(value) = read_env_nonempty("OPENAGENTS_SPARK_API_KEY") {
        return (Some(value), WalletApiKeySource::OpenAgentsEnv);
    }
    if let Some(value) = read_env_nonempty("BREEZ_API_KEY") {
        return (Some(value), WalletApiKeySource::BreezEnv);
    }
    (
        Some(DEFAULT_OPENAGENTS_SPARK_API_KEY.to_string()),
        WalletApiKeySource::EmbeddedDefault,
    )
}

fn read_env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn network_status_label(report: &openagents_spark::NetworkStatusReport) -> String {
    match report.status {
        openagents_spark::NetworkStatus::Connected => "connected".to_string(),
        openagents_spark::NetworkStatus::Disconnected => "disconnected".to_string(),
    }
}

fn balance_snapshot(balance: &SparkBalance) -> WalletBalanceSnapshot {
    WalletBalanceSnapshot {
        spark_sats: balance.spark_sats,
        lightning_sats: balance.lightning_sats,
        onchain_sats: balance.onchain_sats,
        total_sats: balance.total_sats(),
    }
}

fn payment_record_from_summary(payment: &PaymentSummary) -> PylonWalletPaymentRecord {
    PylonWalletPaymentRecord {
        payment_id: payment.id.clone(),
        direction: payment.direction.clone(),
        status: payment.status.clone(),
        amount_sats: payment.amount_sats,
        fees_sats: payment.fees_sats,
        method: payment.method.clone(),
        description: payment.description.clone(),
        invoice: payment.invoice.clone(),
        created_at_ms: payment.timestamp.saturating_mul(1000),
        updated_at_ms: payment.timestamp.saturating_mul(1000),
    }
}

fn sync_wallet_status(
    config_path: &Path,
    runtime: &WalletRuntimeSurface,
    runtime_status: &str,
    runtime_detail: Option<String>,
    balance: Option<&WalletBalanceSnapshot>,
    spark_address: Option<&str>,
    bitcoin_address: Option<&str>,
    payments: &[PylonWalletPaymentRecord],
) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        ledger.wallet.runtime_status = Some(runtime_status.to_string());
        ledger.wallet.last_error = runtime_detail;
        ledger.wallet.network = Some(runtime.network.clone());
        if let Some(balance) = balance {
            ledger.wallet.last_balance_sats = Some(balance.total_sats);
            ledger.wallet.last_balance_at_ms = Some(now_epoch_ms() as u64);
        }
        if let Some(spark_address) = spark_address {
            ledger.wallet.spark_address = Some(spark_address.to_string());
        }
        if let Some(bitcoin_address) = bitcoin_address {
            ledger.wallet.bitcoin_address = Some(bitcoin_address.to_string());
        }
        for payment in payments {
            ledger.upsert_wallet_payment(payment.clone());
        }
        Ok(())
    })
}

fn sync_wallet_error(
    config_path: &Path,
    runtime: &WalletRuntimeSurface,
    error: String,
) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        ledger.wallet.runtime_status = Some("error".to_string());
        ledger.wallet.last_error = Some(error);
        ledger.wallet.network = Some(runtime.network.clone());
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{
        WalletSubcommand, parse_wallet_command, parse_wallet_network, resolve_wallet_api_key,
    };

    #[test]
    fn parse_wallet_command_supports_balance_and_history() {
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("balance"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet balance should parse"),
            WalletSubcommand::Balance { json: true }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("history"),
                    String::from("--limit"),
                    String::from("5"),
                ],
                0,
            )
            .expect("wallet history should parse"),
            WalletSubcommand::History {
                limit: Some(5),
                json: false,
            }
        );
    }

    #[test]
    fn parse_wallet_command_supports_invoice_and_pay() {
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("invoice"),
                    String::from("21"),
                    String::from("--description"),
                    String::from("earn"),
                ],
                0,
            )
            .expect("wallet invoice should parse"),
            WalletSubcommand::Invoice {
                amount_sats: 21,
                description: Some(String::from("earn")),
                expiry_seconds: None,
                json: false,
            }
        );
        assert_eq!(
            parse_wallet_command(
                &[
                    String::from("wallet"),
                    String::from("pay"),
                    String::from("lnbc1example"),
                    String::from("--amount-sats"),
                    String::from("8"),
                    String::from("--json"),
                ],
                0,
            )
            .expect("wallet pay should parse"),
            WalletSubcommand::Pay {
                payment_request: String::from("lnbc1example"),
                amount_sats: Some(8),
                json: true,
            }
        );
    }

    #[test]
    fn parse_wallet_network_accepts_retained_values() {
        assert!(matches!(
            parse_wallet_network("mainnet"),
            Ok(openagents_spark::Network::Mainnet)
        ));
        assert!(matches!(
            parse_wallet_network("regtest"),
            Ok(openagents_spark::Network::Regtest)
        ));
        assert!(parse_wallet_network("signet").is_err());
    }

    #[test]
    fn resolve_wallet_api_key_prefers_explicit_env_name() {
        unsafe {
            std::env::set_var("PYLON_TEST_SPARK_KEY", "abc123");
            std::env::remove_var("OPENAGENTS_SPARK_API_KEY");
            std::env::remove_var("BREEZ_API_KEY");
        }
        let (api_key, source) = resolve_wallet_api_key(Some("PYLON_TEST_SPARK_KEY"));
        assert_eq!(api_key.as_deref(), Some("abc123"));
        assert_eq!(
            source.label(Some("PYLON_TEST_SPARK_KEY")),
            "env:PYLON_TEST_SPARK_KEY"
        );
        unsafe {
            std::env::remove_var("PYLON_TEST_SPARK_KEY");
        }
    }

    #[test]
    fn resolve_wallet_api_key_defaults_to_embedded_release_key() {
        unsafe {
            std::env::remove_var("OPENAGENTS_SPARK_API_KEY");
            std::env::remove_var("BREEZ_API_KEY");
        }
        let (api_key, source) = resolve_wallet_api_key(Some("OPENAGENTS_SPARK_API_KEY"));
        assert_eq!(api_key.as_deref(), Some(DEFAULT_OPENAGENTS_SPARK_API_KEY));
        assert_eq!(source, WalletApiKeySource::EmbeddedDefault);
        assert_eq!(
            source.label(Some("OPENAGENTS_SPARK_API_KEY")),
            "embedded:openagents-default"
        );
    }
}
