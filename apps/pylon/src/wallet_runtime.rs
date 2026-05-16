use std::path::Path;

use anyhow::{Result, anyhow, bail};
use serde::Serialize;

use crate::{
    PylonWalletCreditSummary, PylonWalletInvoiceRecord, PylonWalletPaymentRecord,
    ensure_local_setup, load_ledger, mutate_ledger, now_epoch_ms,
};

const LDK_EXTERNAL_WALLET_DETAIL: &str =
    "Pylon no longer opens a local Spark wallet. Configure an LDK payout destination for earnings.";

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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletCreditSummaryReport {
    pub runtime: WalletRuntimeSurface,
    pub credits: PylonWalletCreditSummary,
}

#[derive(Clone, Debug)]
struct WalletRuntimeContext {
    runtime: WalletRuntimeSurface,
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
    load_wallet_status_report_internal(config_path, true).await
}

pub async fn load_wallet_balance_status_report(config_path: &Path) -> Result<WalletStatusReport> {
    load_wallet_status_report_internal(config_path, false).await
}

async fn load_wallet_status_report_internal(
    config_path: &Path,
    include_recent_payments: bool,
) -> Result<WalletStatusReport> {
    let context = prepare_wallet_context(config_path)?;
    let ledger = load_ledger(config_path)?;
    let balance = WalletBalanceSnapshot {
        total_sats: ledger.wallet.last_balance_sats.unwrap_or_default(),
        ..WalletBalanceSnapshot::default()
    };
    let recent_payments = if include_recent_payments {
        ledger.wallet.payments.iter().take(10).cloned().collect()
    } else {
        Vec::new()
    };
    let report = WalletStatusReport {
        runtime: context.runtime.clone(),
        runtime_status: "external_ldk_target".to_string(),
        runtime_detail: Some(LDK_EXTERNAL_WALLET_DETAIL.to_string()),
        balance,
        recent_payments,
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

pub async fn create_wallet_address_report(config_path: &Path) -> Result<WalletAddressReport> {
    let context = prepare_wallet_context(config_path)?;
    sync_wallet_error(
        config_path,
        &context.runtime,
        LDK_EXTERNAL_WALLET_DETAIL.to_string(),
    )?;
    bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
}

pub async fn create_wallet_invoice_report(
    config_path: &Path,
    amount_sats: u64,
    description: Option<String>,
    expiry_seconds: Option<u32>,
) -> Result<WalletInvoiceReport> {
    let context = prepare_wallet_context(config_path)?;
    let _ = (amount_sats, description, expiry_seconds);
    sync_wallet_error(
        config_path,
        &context.runtime,
        LDK_EXTERNAL_WALLET_DETAIL.to_string(),
    )?;
    bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
}

pub async fn pay_wallet_invoice_report(
    config_path: &Path,
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<WalletPayReport> {
    let context = prepare_wallet_context(config_path)?;
    let _ = (payment_request, amount_sats);
    sync_wallet_error(
        config_path,
        &context.runtime,
        LDK_EXTERNAL_WALLET_DETAIL.to_string(),
    )?;
    bail!("{LDK_EXTERNAL_WALLET_DETAIL}")
}

pub async fn load_wallet_history_report(
    config_path: &Path,
    limit: Option<u32>,
) -> Result<WalletHistoryReport> {
    let context = prepare_wallet_context(config_path)?;
    let limit = limit.unwrap_or(20) as usize;
    let records = load_ledger(config_path)?
        .wallet
        .payments
        .into_iter()
        .take(limit)
        .collect::<Vec<_>>();
    Ok(WalletHistoryReport {
        runtime: context.runtime,
        payments: records,
    })
}

pub async fn load_wallet_credit_summary_report(
    config_path: &Path,
) -> Result<WalletCreditSummaryReport> {
    let context = prepare_wallet_context(config_path)?;
    let records = load_ledger(config_path)?.wallet.payments;
    let credits = compute_wallet_credit_summary(records.as_slice(), now_epoch_ms() as u64);
    sync_wallet_credit_summary(config_path, &credits)?;
    Ok(WalletCreditSummaryReport {
        runtime: context.runtime,
        credits,
    })
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
    let config = ensure_local_setup(config_path)?;
    Ok(WalletRuntimeContext {
        runtime: WalletRuntimeSurface {
            network: "ldk-external".to_string(),
            identity_path: config.identity_path.display().to_string(),
            storage_dir: config.wallet_storage_dir.display().to_string(),
            api_key_env: config.wallet_api_key_env.clone(),
            api_key_source: "none:ldk-external".to_string(),
        },
    })
}

fn compute_wallet_credit_summary(
    payments: &[PylonWalletPaymentRecord],
    now_ms: u64,
) -> PylonWalletCreditSummary {
    let current_day = now_ms / 86_400_000;
    let mut credits = PylonWalletCreditSummary::default();
    for payment in payments {
        if !wallet_payment_counts_as_credit(payment) {
            continue;
        }
        credits.credited_lifetime_sats = credits
            .credited_lifetime_sats
            .saturating_add(payment.amount_sats);
        if payment.created_at_ms / 86_400_000 == current_day {
            credits.credited_today_sats = credits
                .credited_today_sats
                .saturating_add(payment.amount_sats);
            credits.credited_today_count = credits.credited_today_count.saturating_add(1);
        }
        credits.last_credit_at_ms = Some(
            credits
                .last_credit_at_ms
                .unwrap_or(0)
                .max(payment.created_at_ms),
        );
    }
    credits.last_full_sync_at_ms = Some(now_ms);
    credits
}

fn wallet_payment_counts_as_credit(payment: &PylonWalletPaymentRecord) -> bool {
    payment.direction.eq_ignore_ascii_case("receive")
        && matches!(
            payment.status.to_ascii_lowercase().as_str(),
            "succeeded" | "success" | "settled" | "completed" | "confirmed"
        )
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

fn sync_wallet_credit_summary(
    config_path: &Path,
    credits: &PylonWalletCreditSummary,
) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        ledger.wallet.credits = credits.clone();
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
        WalletSubcommand, compute_wallet_credit_summary, parse_wallet_command,
    };
    use crate::PylonWalletPaymentRecord;

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
    fn wallet_credit_summary_uses_created_at_for_today_and_counts_all_history() {
        let now_ms = 1_762_580_000_000u64;
        let current_day = now_ms / 86_400_000;
        let today_created_at_ms = current_day * 86_400_000 + 1_000;
        let yesterday_created_at_ms = today_created_at_ms.saturating_sub(86_400_000);
        let credits = compute_wallet_credit_summary(
            &[
                PylonWalletPaymentRecord {
                    payment_id: "credit-today".to_string(),
                    direction: "receive".to_string(),
                    status: "settled".to_string(),
                    amount_sats: 21,
                    fees_sats: 0,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    created_at_ms: today_created_at_ms,
                    updated_at_ms: yesterday_created_at_ms,
                },
                PylonWalletPaymentRecord {
                    payment_id: "credit-old".to_string(),
                    direction: "receive".to_string(),
                    status: "settled".to_string(),
                    amount_sats: 34,
                    fees_sats: 0,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    created_at_ms: yesterday_created_at_ms,
                    updated_at_ms: now_ms,
                },
                PylonWalletPaymentRecord {
                    payment_id: "send".to_string(),
                    direction: "send".to_string(),
                    status: "settled".to_string(),
                    amount_sats: 13,
                    fees_sats: 1,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    created_at_ms: today_created_at_ms,
                    updated_at_ms: today_created_at_ms,
                },
            ],
            now_ms,
        );

        assert_eq!(credits.credited_lifetime_sats, 55);
        assert_eq!(credits.credited_today_sats, 21);
        assert_eq!(credits.credited_today_count, 1);
        assert_eq!(credits.last_credit_at_ms, Some(today_created_at_ms));
        assert_eq!(credits.last_full_sync_at_ms, Some(now_ms));
    }
}
