use std::sync::Arc;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::WalletExecutorConfig;
use crate::error::{PolicyDenialCode, PolicyDeniedError, SparkGatewayError, SparkGatewayErrorCode};
use crate::gateway::{PaymentGateway, SparkPaymentStatus};
use crate::receipt::{
    WalletExecutionReceipt, WalletExecutionReceiptInput, build_wallet_execution_receipt,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicePaymentRequest {
    pub invoice: String,
    pub max_amount_msats: u64,
    pub host: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicePaymentResult {
    pub payment_id: String,
    pub amount_msats: u64,
    pub preimage_hex: String,
    pub paid_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayBolt11Result {
    pub request_id: String,
    pub wallet_id: String,
    pub payment: InvoicePaymentResult,
    pub quoted_amount_msats: u64,
    pub window_spend_msats_after_payment: u64,
    pub receipt: WalletExecutionReceipt,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletStatus {
    pub wallet_id: String,
    pub mode: String,
    pub auth_mode: String,
    pub auth_enforced: bool,
    pub auth_token_version: u32,
    pub lifecycle: String,
    pub network: String,
    pub identity_pubkey: Option<String>,
    pub balance_sats: Option<u64>,
    pub api_key_configured: bool,
    pub ready: bool,
    pub allowed_host_count: usize,
    pub request_cap_msats: u64,
    pub window_cap_msats: u64,
    pub window_ms: u64,
    pub recent_payments_count: usize,
    pub last_payment_id: Option<String>,
    pub last_payment_at_ms: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Lifecycle {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl Lifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
struct PaymentHistoryItem {
    amount_msats: u64,
    paid_at_ms: i64,
}

#[derive(Debug, Clone)]
struct StatusState {
    lifecycle: Lifecycle,
    identity_pubkey: Option<String>,
    balance_sats: Option<u64>,
    recent_payments_count: usize,
    last_payment_id: Option<String>,
    last_payment_at_ms: Option<i64>,
    last_error_code: Option<String>,
    last_error_message: Option<String>,
    updated_at_ms: i64,
}

impl Default for StatusState {
    fn default() -> Self {
        Self {
            lifecycle: Lifecycle::Disconnected,
            identity_pubkey: None,
            balance_sats: None,
            recent_payments_count: 0,
            last_payment_id: None,
            last_payment_at_ms: None,
            last_error_code: None,
            last_error_message: None,
            updated_at_ms: 0,
        }
    }
}

pub enum WalletExecutorError {
    Policy(PolicyDeniedError),
    Spark(SparkGatewayError),
}

pub struct WalletExecutorService {
    config: WalletExecutorConfig,
    gateway: Arc<dyn PaymentGateway>,
    status: Mutex<StatusState>,
    history: Mutex<Vec<PaymentHistoryItem>>,
}

impl WalletExecutorService {
    pub fn new(config: WalletExecutorConfig, gateway: Arc<dyn PaymentGateway>) -> Self {
        Self {
            config,
            gateway,
            status: Mutex::new(StatusState::default()),
            history: Mutex::new(Vec::new()),
        }
    }

    pub fn config(&self) -> &WalletExecutorConfig {
        &self.config
    }

    pub async fn status(&self) -> WalletStatus {
        self.refresh_status_from_gateway_best_effort().await;
        let current = self.status.lock().await.clone();

        WalletStatus {
            wallet_id: self.config.wallet_id.clone(),
            mode: match self.config.mode {
                crate::config::ExecutorMode::Mock => "mock".to_string(),
                crate::config::ExecutorMode::Spark => "spark".to_string(),
            },
            auth_mode: self.config.auth_mode().to_string(),
            auth_enforced: self.config.auth_token.is_some(),
            auth_token_version: self.config.auth_token_version,
            lifecycle: current.lifecycle.as_str().to_string(),
            network: match self.config.network {
                crate::config::SparkNetwork::Mainnet => "mainnet".to_string(),
                crate::config::SparkNetwork::Regtest => "regtest".to_string(),
            },
            identity_pubkey: current.identity_pubkey,
            balance_sats: current.balance_sats,
            api_key_configured: self.config.spark_api_key.is_some(),
            ready: current.lifecycle == Lifecycle::Connected,
            allowed_host_count: self.config.allowed_hosts.len(),
            request_cap_msats: self.config.request_cap_msats,
            window_cap_msats: self.config.window_cap_msats,
            window_ms: self.config.window_ms,
            recent_payments_count: current.recent_payments_count,
            last_payment_id: current.last_payment_id,
            last_payment_at_ms: current.last_payment_at_ms,
            last_error_code: current.last_error_code,
            last_error_message: current.last_error_message,
            updated_at_ms: current.updated_at_ms,
        }
    }

    pub async fn pay_bolt11(
        &self,
        request: InvoicePaymentRequest,
        request_id: Option<String>,
    ) -> Result<PayBolt11Result, WalletExecutorError> {
        let request_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let host = sanitize_host(&request.host);
        let invoice_hash = hash_invoice(&request.invoice);

        self.bootstrap().await.map_err(WalletExecutorError::Spark)?;
        self.ensure_host_allowed(&host)
            .await
            .map_err(WalletExecutorError::Policy)?;
        self.ensure_request_cap_allowed(&request)
            .await
            .map_err(WalletExecutorError::Policy)?;

        let prepared = self
            .gateway
            .prepare_payment(&request.invoice)
            .await
            .map_err(WalletExecutorError::Spark)?;

        if prepared.payment_method_type != "bolt11Invoice" {
            return Err(WalletExecutorError::Spark(SparkGatewayError::new(
                SparkGatewayErrorCode::UnsupportedPaymentMethod,
                format!(
                    "unsupported payment method: {}",
                    prepared.payment_method_type
                ),
            )));
        }

        if prepared.amount_msats > request.max_amount_msats
            || prepared.amount_msats > self.config.request_cap_msats
        {
            let mut error = PolicyDeniedError::new(
                PolicyDenialCode::QuotedAmountExceedsCap,
                format!("quoted amount {} msats exceeds cap", prepared.amount_msats),
            );
            error.host = Some(host.clone());
            error.max_allowed_msats =
                Some(request.max_amount_msats.min(self.config.request_cap_msats));
            error.quoted_amount_msats = Some(prepared.amount_msats);
            return Err(WalletExecutorError::Policy(error));
        }

        let _window_spend_before = self
            .enforce_window_cap(&host, prepared.amount_msats)
            .await
            .map_err(WalletExecutorError::Policy)?;

        let sent = self
            .gateway
            .send_payment(prepared.clone(), self.config.payment_timeout_secs)
            .await
            .map_err(WalletExecutorError::Spark)?;

        if sent.status == SparkPaymentStatus::Pending {
            return Err(WalletExecutorError::Spark(SparkGatewayError::new(
                SparkGatewayErrorCode::PaymentPending,
                "payment did not complete before timeout",
            )));
        }

        if sent.status == SparkPaymentStatus::Failed {
            return Err(WalletExecutorError::Spark(SparkGatewayError::new(
                SparkGatewayErrorCode::PaymentFailed,
                "payment failed",
            )));
        }

        let preimage_hex = sent.preimage_hex.clone().ok_or_else(|| {
            WalletExecutorError::Spark(SparkGatewayError::new(
                SparkGatewayErrorCode::PaymentMissingPreimage,
                "payment completed without a preimage",
            ))
        })?;

        let paid_amount_msats = if sent.amount_msats > 0 {
            sent.amount_msats
        } else {
            prepared.amount_msats
        };

        {
            let mut history = self.history.lock().await;
            prune_window(&mut history, sent.paid_at_ms, self.config.window_ms);
            history.push(PaymentHistoryItem {
                amount_msats: paid_amount_msats,
                paid_at_ms: sent.paid_at_ms,
            });
        }

        let window_spend_after = {
            let history = self.history.lock().await;
            sum_msats(&history)
        };

        let receipt = build_wallet_execution_receipt(&WalletExecutionReceiptInput {
            request_id: request_id.clone(),
            wallet_id: self.config.wallet_id.clone(),
            host: host.clone(),
            payment_id: sent.payment_id.clone(),
            invoice_hash,
            quoted_amount_msats: prepared.amount_msats,
            settled_amount_msats: paid_amount_msats,
            preimage_hex: preimage_hex.clone(),
            paid_at_ms: sent.paid_at_ms,
        });

        self.refresh_status_from_gateway_best_effort().await;

        {
            let history_len = self.history.lock().await.len();
            let mut status = self.status.lock().await;
            status.lifecycle = Lifecycle::Connected;
            status.recent_payments_count = history_len;
            status.last_payment_id = Some(sent.payment_id.clone());
            status.last_payment_at_ms = Some(sent.paid_at_ms);
            status.last_error_code = None;
            status.last_error_message = None;
            status.updated_at_ms = chrono::Utc::now().timestamp_millis();
        }

        Ok(PayBolt11Result {
            request_id: request_id.clone(),
            wallet_id: self.config.wallet_id.clone(),
            payment: InvoicePaymentResult {
                payment_id: sent.payment_id,
                amount_msats: paid_amount_msats,
                preimage_hex,
                paid_at_ms: sent.paid_at_ms,
            },
            quoted_amount_msats: prepared.amount_msats,
            window_spend_msats_after_payment: window_spend_after,
            receipt,
        })
    }

    async fn bootstrap(&self) -> Result<(), SparkGatewayError> {
        {
            let mut status = self.status.lock().await;
            status.lifecycle = Lifecycle::Connecting;
            status.updated_at_ms = chrono::Utc::now().timestamp_millis();
        }

        if let Err(error) = self.gateway.connect().await {
            let mut status = self.status.lock().await;
            status.lifecycle = Lifecycle::Error;
            status.last_error_code = Some(error.code.as_str().to_string());
            status.last_error_message = Some(error.message.clone());
            status.updated_at_ms = chrono::Utc::now().timestamp_millis();
            return Err(error);
        }

        self.refresh_from_gateway().await
    }

    async fn refresh_status_from_gateway_best_effort(&self) {
        if let Err(error) = self.refresh_from_gateway().await {
            let mut status = self.status.lock().await;
            if status.lifecycle != Lifecycle::Connected {
                status.lifecycle = Lifecycle::Error;
            }
            status.last_error_code = Some(error.code.as_str().to_string());
            status.last_error_message = Some(error.message);
            status.updated_at_ms = chrono::Utc::now().timestamp_millis();
        }
    }

    async fn refresh_from_gateway(&self) -> Result<(), SparkGatewayError> {
        let info = self.gateway.get_info().await?;
        let mut status = self.status.lock().await;
        status.lifecycle = Lifecycle::Connected;
        status.identity_pubkey = info.identity_pubkey;
        status.balance_sats = info.balance_sats;
        status.last_error_code = None;
        status.last_error_message = None;
        status.updated_at_ms = chrono::Utc::now().timestamp_millis();
        Ok(())
    }

    async fn ensure_host_allowed(&self, host: &str) -> Result<(), PolicyDeniedError> {
        if self.config.allowed_hosts.is_empty() || self.config.allowed_hosts.contains(host) {
            return Ok(());
        }

        let mut error = PolicyDeniedError::new(
            PolicyDenialCode::HostNotAllowed,
            format!("host {host} is not in the wallet executor allowlist"),
        );
        error.host = Some(host.to_string());
        Err(error)
    }

    async fn ensure_request_cap_allowed(
        &self,
        request: &InvoicePaymentRequest,
    ) -> Result<(), PolicyDeniedError> {
        if request.max_amount_msats <= self.config.request_cap_msats {
            return Ok(());
        }

        let mut error = PolicyDeniedError::new(
            PolicyDenialCode::RequestCapExceeded,
            format!(
                "request max amount {} msats exceeds service cap {} msats",
                request.max_amount_msats, self.config.request_cap_msats
            ),
        );
        error.host = Some(sanitize_host(&request.host));
        error.max_allowed_msats = Some(self.config.request_cap_msats);
        error.quoted_amount_msats = Some(request.max_amount_msats);
        Err(error)
    }

    async fn enforce_window_cap(
        &self,
        host: &str,
        quoted_amount_msats: u64,
    ) -> Result<u64, PolicyDeniedError> {
        let now = chrono::Utc::now().timestamp_millis();
        let mut history = self.history.lock().await;
        prune_window(&mut history, now, self.config.window_ms);
        let current_spend = sum_msats(&history);

        if current_spend.saturating_add(quoted_amount_msats) > self.config.window_cap_msats {
            let mut error = PolicyDeniedError::new(
                PolicyDenialCode::WindowCapExceeded,
                format!(
                    "window cap exceeded: {} > {} msats",
                    current_spend.saturating_add(quoted_amount_msats),
                    self.config.window_cap_msats
                ),
            );
            error.host = Some(host.to_string());
            error.quoted_amount_msats = Some(quoted_amount_msats);
            error.window_spend_msats = Some(current_spend);
            error.window_cap_msats = Some(self.config.window_cap_msats);
            return Err(error);
        }

        Ok(current_spend)
    }
}

fn sanitize_host(host: &str) -> String {
    host.trim().to_ascii_lowercase()
}

fn hash_invoice(invoice: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(invoice.as_bytes());
    hex::encode(hasher.finalize())
}

fn prune_window(rows: &mut Vec<PaymentHistoryItem>, now_ms: i64, window_ms: u64) {
    let window_ms = i64::try_from(window_ms).unwrap_or(i64::MAX);
    rows.retain(|row| now_ms.saturating_sub(row.paid_at_ms) <= window_ms);
}

fn sum_msats(rows: &[PaymentHistoryItem]) -> u64 {
    rows.iter()
        .fold(0_u64, |total, row| total.saturating_add(row.amount_msats))
}
