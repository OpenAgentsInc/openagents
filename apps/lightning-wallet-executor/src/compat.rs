use std::collections::HashMap;

use rand::RngCore;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::config::{SparkNetwork, WalletExecutorConfig};
use crate::gateway::{extract_preimage_hex, to_paid_at_ms};

#[derive(Debug, Clone)]
struct CompatWalletRecord {
    wallet_id: String,
    mnemonic: String,
    spark_address: String,
    lightning_address: String,
    identity_pubkey: String,
    balance_sats: u64,
}

#[derive(Debug, Clone)]
struct CompatInvoiceRecord {
    wallet_id: String,
    amount_msats: u64,
}

#[derive(Debug, Default)]
struct CompatMockState {
    wallets: HashMap<String, CompatWalletRecord>,
    wallet_by_spark_address: HashMap<String, String>,
    invoices: HashMap<String, CompatInvoiceRecord>,
}

#[derive(Debug)]
pub struct WalletCompatHttpError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

impl WalletCompatHttpError {
    fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CompatHttpResult {
    pub ok: bool,
    pub result: Value,
}

pub struct WalletCompatService {
    config: WalletExecutorConfig,
    mock_state: Mutex<CompatMockState>,
}

impl WalletCompatService {
    pub fn new(config: WalletExecutorConfig) -> Self {
        Self {
            config,
            mock_state: Mutex::new(CompatMockState::default()),
        }
    }

    pub async fn wallets_create(
        &self,
        wallet_id: String,
        mnemonic: Option<String>,
    ) -> Result<CompatHttpResult, WalletCompatHttpError> {
        let requested_mnemonic = normalize_optional_string(mnemonic);
        let normalized_mnemonic = requested_mnemonic.unwrap_or_else(default_mnemonic);

        let wallet = if is_spark_mode(&self.config) {
            spark_wallet_snapshot(&self.config, &wallet_id, &normalized_mnemonic).await?
        } else {
            self.ensure_mock_wallet(wallet_id.clone(), normalized_mnemonic)
                .await
        };

        Ok(CompatHttpResult {
            ok: true,
            result: wallet_to_value(&wallet),
        })
    }

    pub async fn wallets_status(
        &self,
        wallet_id: String,
        mnemonic: String,
    ) -> Result<CompatHttpResult, WalletCompatHttpError> {
        let normalized_mnemonic = normalize_required_string(Some(mnemonic), "mnemonic")?;

        let wallet = if is_spark_mode(&self.config) {
            spark_wallet_snapshot(&self.config, &wallet_id, &normalized_mnemonic).await?
        } else {
            self.ensure_mock_wallet(wallet_id.clone(), normalized_mnemonic)
                .await
        };

        Ok(CompatHttpResult {
            ok: true,
            result: wallet_to_value(&wallet),
        })
    }

    pub async fn wallets_create_invoice(
        &self,
        wallet_id: String,
        mnemonic: String,
        amount_sats: u64,
        description: Option<String>,
    ) -> Result<CompatHttpResult, WalletCompatHttpError> {
        let normalized_mnemonic = normalize_required_string(Some(mnemonic), "mnemonic")?;
        let description = normalize_optional_string(description)
            .unwrap_or_else(|| format!("OpenAgents wallet invoice ({wallet_id})"));

        if is_spark_mode(&self.config) {
            let invoice = spark_create_invoice(
                &self.config,
                &wallet_id,
                &normalized_mnemonic,
                amount_sats,
                &description,
            )
            .await?;

            return Ok(CompatHttpResult {
                ok: true,
                result: json!({
                    "walletId": wallet_id,
                    "paymentRequest": invoice,
                    "invoice": invoice,
                    "amountSats": amount_sats,
                    "description": description,
                    "expiresAt": Value::Null,
                }),
            });
        }

        let wallet = self
            .ensure_mock_wallet(wallet_id.clone(), normalized_mnemonic)
            .await;
        let invoice = self
            .mock_create_invoice(&wallet.wallet_id, amount_sats)
            .await;

        Ok(CompatHttpResult {
            ok: true,
            result: json!({
                "walletId": wallet.wallet_id,
                "paymentRequest": invoice,
                "invoice": invoice,
                "amountSats": amount_sats,
                "description": description,
                "expiresAt": Value::Null,
            }),
        })
    }

    pub async fn wallets_pay_bolt11(
        &self,
        wallet_id: String,
        mnemonic: String,
        invoice: String,
        max_amount_msats: u64,
        timeout_ms: Option<u64>,
        host: Option<String>,
    ) -> Result<CompatHttpResult, WalletCompatHttpError> {
        let normalized_mnemonic = normalize_required_string(Some(mnemonic), "mnemonic")?;
        let normalized_host =
            normalize_optional_string(host).map(|value| value.to_ascii_lowercase());
        allow_host_or_throw(&self.config, normalized_host.as_deref())?;

        if is_spark_mode(&self.config) {
            let timeout_ms = timeout_ms.unwrap_or(12_000).max(1_000);
            let payment = spark_pay_bolt11(
                &self.config,
                &wallet_id,
                &normalized_mnemonic,
                &invoice,
                max_amount_msats,
                timeout_ms,
            )
            .await?;

            return Ok(CompatHttpResult {
                ok: true,
                result: json!({
                    "walletId": wallet_id,
                    "paymentId": payment.payment_id,
                    "preimage": payment.preimage,
                    "status": payment.status,
                    "amountMsats": payment.amount_msats,
                    "paidAtMs": payment.paid_at_ms,
                }),
            });
        }

        let outcome = self
            .mock_pay_invoice(&wallet_id, &invoice, max_amount_msats)
            .await?;

        Ok(CompatHttpResult {
            ok: true,
            result: json!({
                "walletId": wallet_id,
                "paymentId": outcome.payment_id,
                "preimage": outcome.preimage,
                "status": "completed",
                "amountMsats": outcome.amount_msats,
                "paidAtMs": outcome.paid_at_ms,
            }),
        })
    }

    pub async fn wallets_send_spark(
        &self,
        wallet_id: String,
        mnemonic: String,
        spark_address: String,
        amount_sats: u64,
        timeout_ms: Option<u64>,
    ) -> Result<CompatHttpResult, WalletCompatHttpError> {
        let normalized_mnemonic = normalize_required_string(Some(mnemonic), "mnemonic")?;

        if is_spark_mode(&self.config) {
            let timeout_ms = timeout_ms.unwrap_or(12_000).max(1_000);
            let payment = spark_send_spark(
                &self.config,
                &wallet_id,
                &normalized_mnemonic,
                &spark_address,
                amount_sats,
                timeout_ms,
            )
            .await?;

            return Ok(CompatHttpResult {
                ok: true,
                result: json!({
                    "walletId": wallet_id,
                    "paymentId": payment.payment_id,
                    "status": payment.status,
                    "amountSats": amount_sats,
                    "amountMsats": payment.amount_msats,
                    "paidAtMs": payment.paid_at_ms,
                }),
            });
        }

        let outcome = self
            .mock_send_spark(
                &wallet_id,
                &normalized_mnemonic,
                &spark_address,
                amount_sats,
            )
            .await?;

        Ok(CompatHttpResult {
            ok: true,
            result: json!({
                "walletId": wallet_id,
                "paymentId": outcome.payment_id,
                "status": "completed",
                "amountSats": amount_sats,
                "amountMsats": amount_sats.saturating_mul(1_000),
                "paidAtMs": outcome.paid_at_ms,
            }),
        })
    }

    async fn ensure_mock_wallet(&self, wallet_id: String, mnemonic: String) -> CompatWalletRecord {
        let mut state = self.mock_state.lock().await;
        if let Some(existing) = state.wallets.get_mut(&wallet_id) {
            if !mnemonic.trim().is_empty() {
                existing.mnemonic = normalize_mnemonic(&mnemonic);
            }
            return existing.clone();
        }

        let created = CompatWalletRecord {
            wallet_id: wallet_id.clone(),
            mnemonic: normalize_mnemonic(&mnemonic),
            spark_address: mock_lightning_address(&wallet_id),
            lightning_address: mock_lightning_address(&wallet_id),
            identity_pubkey: mock_identity(&wallet_id),
            balance_sats: 1_000,
        };

        let _ = state.wallet_by_spark_address.insert(
            created.spark_address.to_ascii_lowercase(),
            wallet_id.clone(),
        );
        let _ = state.wallets.insert(wallet_id, created.clone());
        created
    }

    async fn mock_create_invoice(&self, wallet_id: &str, amount_sats: u64) -> String {
        let mut random = [0_u8; 8];
        rand::rng().fill_bytes(&mut random);
        let invoice = format!(
            "lnmock{}{}",
            chrono::Utc::now().timestamp_millis(),
            hex::encode(random)
        );

        let mut state = self.mock_state.lock().await;
        let _ = state.invoices.insert(
            invoice.clone(),
            CompatInvoiceRecord {
                wallet_id: wallet_id.to_string(),
                amount_msats: amount_sats.saturating_mul(1_000),
            },
        );

        invoice
    }

    async fn mock_pay_invoice(
        &self,
        payer_wallet_id: &str,
        invoice: &str,
        max_amount_msats: u64,
    ) -> Result<MockPayOutcome, WalletCompatHttpError> {
        let mut state = self.mock_state.lock().await;
        let Some(quoted) = state.invoices.get(invoice).cloned() else {
            return Err(WalletCompatHttpError::new(
                404,
                "invoice_not_found",
                "invoice not found in mock ledger",
            ));
        };

        if quoted.amount_msats > max_amount_msats {
            let mut error = WalletCompatHttpError::new(
                422,
                "quoted_amount_exceeds_cap",
                "quoted amount exceeds maxAmountMsats",
            );
            error.details = Some(json!({
                "quotedAmountMsats": quoted.amount_msats,
                "maxAmountMsats": max_amount_msats,
            }));
            return Err(error);
        }

        let payer_balance_sats = state
            .wallets
            .get(payer_wallet_id)
            .map(|wallet| wallet.balance_sats)
            .ok_or_else(|| {
                WalletCompatHttpError::new(404, "wallet_not_found", "wallet not found")
            })?;

        if payer_balance_sats.saturating_mul(1_000) < quoted.amount_msats {
            let mut error = WalletCompatHttpError::new(
                402,
                "insufficient_balance",
                "mock wallet has insufficient balance",
            );
            error.details = Some(json!({
                "walletId": payer_wallet_id,
                "requiredMsats": quoted.amount_msats,
                "availableMsats": payer_balance_sats.saturating_mul(1000),
            }));
            return Err(error);
        }

        if !state.wallets.contains_key(&quoted.wallet_id) {
            return Err(WalletCompatHttpError::new(
                404,
                "wallet_not_found",
                "wallet not found",
            ));
        }

        let debit_sats = div_ceil(quoted.amount_msats, 1_000);
        if let Some(payer) = state.wallets.get_mut(payer_wallet_id) {
            payer.balance_sats = payer.balance_sats.saturating_sub(debit_sats);
        }

        let credit_sats = div_ceil(quoted.amount_msats, 1_000);
        let receiver = state.wallets.get_mut(&quoted.wallet_id).ok_or_else(|| {
            WalletCompatHttpError::new(404, "wallet_not_found", "wallet not found")
        })?;
        receiver.balance_sats = receiver.balance_sats.saturating_add(credit_sats);

        let _ = state.invoices.remove(invoice);

        Ok(MockPayOutcome {
            payment_id: format!("mock-pay-{}", uuid::Uuid::new_v4()),
            preimage: hex::encode({
                let mut random = [0_u8; 32];
                rand::rng().fill_bytes(&mut random);
                random
            }),
            amount_msats: quoted.amount_msats,
            paid_at_ms: chrono::Utc::now().timestamp_millis(),
        })
    }

    async fn mock_send_spark(
        &self,
        wallet_id: &str,
        mnemonic: &str,
        spark_address: &str,
        amount_sats: u64,
    ) -> Result<MockSendSparkOutcome, WalletCompatHttpError> {
        let _ = self
            .ensure_mock_wallet(wallet_id.to_string(), mnemonic.to_string())
            .await;

        let mut state = self.mock_state.lock().await;
        let Some(recipient_wallet_id) = state
            .wallet_by_spark_address
            .get(&spark_address.to_ascii_lowercase())
            .cloned()
        else {
            return Err(WalletCompatHttpError::new(
                404,
                "recipient_not_found",
                "mock recipient spark address not found",
            ));
        };

        let sender = state.wallets.get(wallet_id).cloned().ok_or_else(|| {
            WalletCompatHttpError::new(404, "wallet_not_found", "wallet not found")
        })?;

        if sender.balance_sats < amount_sats {
            return Err(WalletCompatHttpError::new(
                402,
                "insufficient_balance",
                "mock wallet has insufficient balance",
            ));
        }

        if let Some(sender_mut) = state.wallets.get_mut(wallet_id) {
            sender_mut.balance_sats = sender_mut.balance_sats.saturating_sub(amount_sats);
        }

        let recipient = state.wallets.get_mut(&recipient_wallet_id).ok_or_else(|| {
            WalletCompatHttpError::new(404, "wallet_not_found", "wallet not found")
        })?;
        recipient.balance_sats = recipient.balance_sats.saturating_add(amount_sats);

        Ok(MockSendSparkOutcome {
            payment_id: format!("mock-spark-{}", uuid::Uuid::new_v4()),
            paid_at_ms: chrono::Utc::now().timestamp_millis(),
        })
    }
}

struct MockPayOutcome {
    payment_id: String,
    preimage: String,
    amount_msats: u64,
    paid_at_ms: i64,
}

struct MockSendSparkOutcome {
    payment_id: String,
    paid_at_ms: i64,
}

fn is_spark_mode(config: &WalletExecutorConfig) -> bool {
    matches!(config.mode, crate::config::ExecutorMode::Spark)
}

fn default_mnemonic() -> String {
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        .to_string()
}

fn normalize_mnemonic(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn normalize_required_string(
    value: Option<String>,
    field: &str,
) -> Result<String, WalletCompatHttpError> {
    let normalized = normalize_optional_string(value).unwrap_or_default();
    if normalized.is_empty() {
        return Err(WalletCompatHttpError::new(
            400,
            "invalid_request",
            format!("{field} is required"),
        ));
    }
    Ok(normalized)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn mock_identity(wallet_id: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(wallet_id.as_bytes());
    format!("mock-{}", &hex::encode(hasher.finalize())[..16])
}

fn mock_lightning_address(wallet_id: &str) -> String {
    let sanitized = wallet_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("{sanitized}@spark.mock")
}

fn wallet_to_value(wallet: &CompatWalletRecord) -> Value {
    json!({
        "walletId": wallet.wallet_id,
        "mnemonic": wallet.mnemonic,
        "sparkAddress": wallet.spark_address,
        "lightningAddress": wallet.lightning_address,
        "identityPubkey": wallet.identity_pubkey,
        "balanceSats": wallet.balance_sats,
        "status": "active",
    })
}

fn allow_host_or_throw(
    config: &WalletExecutorConfig,
    host: Option<&str>,
) -> Result<(), WalletCompatHttpError> {
    let Some(host) = host.map(|value| value.trim().to_ascii_lowercase()) else {
        return Ok(());
    };

    if config.allowed_hosts.is_empty() || config.allowed_hosts.contains(&host) {
        return Ok(());
    }

    let mut error = WalletCompatHttpError::new(
        403,
        "host_not_allowed",
        format!("host {host} is not in the wallet allowlist"),
    );
    error.details = Some(json!({
        "host": host,
        "allowlistHosts": config.allowed_hosts,
    }));
    Err(error)
}

fn div_ceil(value: u64, divisor: u64) -> u64 {
    if divisor == 0 {
        return value;
    }
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder == 0 {
        quotient
    } else {
        quotient.saturating_add(1)
    }
}

async fn build_spark_wallet(
    config: &WalletExecutorConfig,
    wallet_id: &str,
    mnemonic: &str,
) -> Result<spark::SparkWallet, WalletCompatHttpError> {
    let Some(api_key) = config.spark_api_key.as_ref() else {
        return Err(WalletCompatHttpError::new(
            500,
            "config_error",
            "spark api key is missing",
        ));
    };

    let signer = spark::SparkSigner::from_mnemonic(mnemonic, "").map_err(|error| {
        WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
    })?;

    let wallet_config = spark::WalletConfig {
        network: match config.network {
            SparkNetwork::Mainnet => spark::Network::Mainnet,
            SparkNetwork::Regtest => spark::Network::Regtest,
        },
        api_key: Some(api_key.clone()),
        storage_dir: std::path::PathBuf::from(format!(
            "./output/spark-wallet-executor/{wallet_id}"
        )),
    };

    spark::SparkWallet::new(signer, wallet_config)
        .await
        .map_err(|error| WalletCompatHttpError::new(502, "spark_executor_error", error.to_string()))
}

async fn spark_wallet_snapshot(
    config: &WalletExecutorConfig,
    wallet_id: &str,
    mnemonic: &str,
) -> Result<CompatWalletRecord, WalletCompatHttpError> {
    let wallet = build_spark_wallet(config, wallet_id, mnemonic).await?;

    let info = wallet.get_info(true).await.map_err(|error| {
        WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
    })?;

    let lightning_address = wallet
        .get_lightning_address()
        .await
        .ok()
        .flatten()
        .map(|info| info.lightning_address)
        .unwrap_or_else(|| wallet_id.to_string());

    Ok(CompatWalletRecord {
        wallet_id: wallet_id.to_string(),
        mnemonic: normalize_mnemonic(mnemonic),
        spark_address: lightning_address.clone(),
        lightning_address,
        identity_pubkey: mock_identity(wallet_id),
        balance_sats: info.balance_sats,
    })
}

async fn spark_create_invoice(
    config: &WalletExecutorConfig,
    wallet_id: &str,
    mnemonic: &str,
    amount_sats: u64,
    description: &str,
) -> Result<String, WalletCompatHttpError> {
    let wallet = build_spark_wallet(config, wallet_id, mnemonic).await?;

    let invoice = wallet
        .create_invoice(amount_sats, Some(description.to_string()), None)
        .await
        .map_err(|error| {
            WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
        })?;

    Ok(invoice.payment_request)
}

struct SparkCompatPayOutcome {
    payment_id: String,
    preimage: Option<String>,
    status: &'static str,
    amount_msats: u64,
    paid_at_ms: i64,
}

async fn spark_pay_bolt11(
    config: &WalletExecutorConfig,
    wallet_id: &str,
    mnemonic: &str,
    invoice: &str,
    max_amount_msats: u64,
    timeout_ms: u64,
) -> Result<SparkCompatPayOutcome, WalletCompatHttpError> {
    let wallet = build_spark_wallet(config, wallet_id, mnemonic).await?;

    let prepared = wallet
        .prepare_send_payment(invoice, None)
        .await
        .map_err(|error| {
            WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
        })?;

    let quoted_amount_msats = u64::try_from(prepared.amount)
        .unwrap_or(u64::MAX)
        .saturating_mul(1_000);

    if quoted_amount_msats > max_amount_msats {
        let mut error = WalletCompatHttpError::new(
            422,
            "quoted_amount_exceeds_cap",
            "quoted amount exceeds maxAmountMsats",
        );
        error.details = Some(json!({
            "quotedAmountMsats": quoted_amount_msats,
            "maxAmountMsats": max_amount_msats,
        }));
        return Err(error);
    }

    let sent = wallet
        .send_payment_with_options(
            prepared,
            spark::SendPaymentOptions::Bolt11Invoice {
                prefer_spark: true,
                completion_timeout_secs: Some(
                    u32::try_from(timeout_ms / 1_000).unwrap_or(u32::MAX).max(1),
                ),
            },
            None,
        )
        .await
        .map_err(|error| {
            WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
        })?;

    let status = match sent.payment.status {
        spark::PaymentStatus::Failed => "failed",
        spark::PaymentStatus::Pending => "pending",
        spark::PaymentStatus::Completed => "completed",
    };

    Ok(SparkCompatPayOutcome {
        payment_id: sent.payment.id,
        preimage: extract_preimage_hex(sent.payment.details.as_ref()),
        status,
        amount_msats: u64::try_from(sent.payment.amount)
            .unwrap_or(u64::MAX)
            .saturating_mul(1_000),
        paid_at_ms: to_paid_at_ms(sent.payment.timestamp),
    })
}

struct SparkCompatSendOutcome {
    payment_id: String,
    status: &'static str,
    amount_msats: u64,
    paid_at_ms: i64,
}

async fn spark_send_spark(
    config: &WalletExecutorConfig,
    wallet_id: &str,
    mnemonic: &str,
    spark_address: &str,
    amount_sats: u64,
    _timeout_ms: u64,
) -> Result<SparkCompatSendOutcome, WalletCompatHttpError> {
    let wallet = build_spark_wallet(config, wallet_id, mnemonic).await?;

    let prepared = wallet
        .prepare_send_payment(spark_address, Some(amount_sats))
        .await
        .map_err(|error| {
            WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
        })?;

    let sent = wallet
        .send_payment_with_options(
            prepared,
            spark::SendPaymentOptions::SparkAddress { htlc_options: None },
            None,
        )
        .await
        .map_err(|error| {
            WalletCompatHttpError::new(502, "spark_executor_error", error.to_string())
        })?;

    let status = match sent.payment.status {
        spark::PaymentStatus::Failed => "failed",
        spark::PaymentStatus::Pending => "pending",
        spark::PaymentStatus::Completed => "completed",
    };

    Ok(SparkCompatSendOutcome {
        payment_id: sent.payment.id,
        status,
        amount_msats: u64::try_from(sent.payment.amount)
            .unwrap_or(u64::MAX)
            .saturating_mul(1_000),
        paid_at_ms: to_paid_at_ms(sent.payment.timestamp),
    })
}
