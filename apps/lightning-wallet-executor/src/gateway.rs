use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::{SparkNetwork, WalletExecutorConfig};
use crate::error::{SparkGatewayError, SparkGatewayErrorCode};
use crate::secrets::MnemonicSecretProvider;

#[derive(Debug, Clone)]
pub struct SparkWalletInfo {
    pub identity_pubkey: Option<String>,
    pub balance_sats: Option<u64>,
    pub token_balance_count: usize,
}

#[derive(Debug, Clone)]
pub struct SparkPreparedPayment {
    pub prepare_id: String,
    pub amount_msats: u64,
    pub payment_method_type: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SparkPaymentStatus {
    Completed,
    Pending,
    Failed,
}

#[derive(Debug, Clone)]
pub struct SparkSentPayment {
    pub payment_id: String,
    pub status: SparkPaymentStatus,
    pub amount_msats: u64,
    pub preimage_hex: Option<String>,
    pub paid_at_ms: i64,
}

#[async_trait::async_trait]
pub trait PaymentGateway: Send + Sync {
    async fn connect(&self) -> Result<(), SparkGatewayError>;
    async fn get_info(&self) -> Result<SparkWalletInfo, SparkGatewayError>;
    async fn prepare_payment(
        &self,
        invoice: &str,
    ) -> Result<SparkPreparedPayment, SparkGatewayError>;
    async fn send_payment(
        &self,
        prepared: SparkPreparedPayment,
        payment_timeout_secs: u64,
    ) -> Result<SparkSentPayment, SparkGatewayError>;
}

#[derive(Debug, Clone, Default)]
pub struct MockGatewayConfig {
    pub initial_balance_sats: Option<u64>,
    pub quoted_amount_msats: Option<u64>,
    pub fail_prepare: bool,
    pub fail_send: bool,
    pub pending_on_send: bool,
    pub missing_preimage: bool,
    pub status_balance_sequence_sats: Option<Vec<u64>>,
}

#[derive(Debug)]
struct MockGatewayState {
    identity_pubkey: String,
    balance_sats: u64,
    next_payment_id: u64,
    get_info_calls: usize,
}

#[derive(Debug)]
pub struct MockPaymentGateway {
    state: Mutex<MockGatewayState>,
    quoted_amount_msats: u64,
    fail_prepare: bool,
    fail_send: bool,
    pending_on_send: bool,
    missing_preimage: bool,
    status_balance_sequence_sats: Vec<u64>,
}

impl MockPaymentGateway {
    pub fn new(input: Option<MockGatewayConfig>) -> Self {
        let config = input.unwrap_or_default();
        Self {
            state: Mutex::new(MockGatewayState {
                identity_pubkey: "spark-mock-identity".to_string(),
                balance_sats: config.initial_balance_sats.unwrap_or(50_000),
                next_payment_id: 1,
                get_info_calls: 0,
            }),
            quoted_amount_msats: config.quoted_amount_msats.unwrap_or(50_000).max(1_000),
            fail_prepare: config.fail_prepare,
            fail_send: config.fail_send,
            pending_on_send: config.pending_on_send,
            missing_preimage: config.missing_preimage,
            status_balance_sequence_sats: config.status_balance_sequence_sats.unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl PaymentGateway for MockPaymentGateway {
    async fn connect(&self) -> Result<(), SparkGatewayError> {
        Ok(())
    }

    async fn get_info(&self) -> Result<SparkWalletInfo, SparkGatewayError> {
        let mut state = self.state.lock().await;
        let call_index = state.get_info_calls;
        state.get_info_calls = state.get_info_calls.saturating_add(1);

        let balance = if self.status_balance_sequence_sats.is_empty() {
            state.balance_sats
        } else {
            self.status_balance_sequence_sats
                .get(call_index)
                .copied()
                .or_else(|| self.status_balance_sequence_sats.last().copied())
                .unwrap_or(state.balance_sats)
        };

        Ok(SparkWalletInfo {
            identity_pubkey: Some(state.identity_pubkey.clone()),
            balance_sats: Some(balance),
            token_balance_count: 0,
        })
    }

    async fn prepare_payment(
        &self,
        _invoice: &str,
    ) -> Result<SparkPreparedPayment, SparkGatewayError> {
        if self.fail_prepare {
            return Err(SparkGatewayError::new(
                SparkGatewayErrorCode::PrepareFailed,
                "mock prepare failure",
            ));
        }

        Ok(SparkPreparedPayment {
            prepare_id: Uuid::new_v4().to_string(),
            amount_msats: self.quoted_amount_msats,
            payment_method_type: "bolt11Invoice".to_string(),
        })
    }

    async fn send_payment(
        &self,
        _prepared: SparkPreparedPayment,
        _payment_timeout_secs: u64,
    ) -> Result<SparkSentPayment, SparkGatewayError> {
        if self.fail_send {
            return Err(SparkGatewayError::new(
                SparkGatewayErrorCode::SendFailed,
                "mock send failure",
            ));
        }

        let mut state = self.state.lock().await;
        let payment_id = format!("mock-pay-{}", state.next_payment_id);
        state.next_payment_id = state.next_payment_id.saturating_add(1);

        let amount_sats = self.quoted_amount_msats / 1_000;
        state.balance_sats = state.balance_sats.saturating_sub(amount_sats);

        if self.pending_on_send {
            return Ok(SparkSentPayment {
                payment_id,
                status: SparkPaymentStatus::Pending,
                amount_msats: self.quoted_amount_msats,
                preimage_hex: None,
                paid_at_ms: chrono::Utc::now().timestamp_millis(),
            });
        }

        Ok(SparkSentPayment {
            payment_id,
            status: SparkPaymentStatus::Completed,
            amount_msats: self.quoted_amount_msats,
            preimage_hex: if self.missing_preimage {
                None
            } else {
                Some("ab".repeat(32))
            },
            paid_at_ms: chrono::Utc::now().timestamp_millis(),
        })
    }
}

pub struct LivePaymentGateway {
    config: WalletExecutorConfig,
    mnemonic_provider: Arc<dyn MnemonicSecretProvider>,
    wallet: Mutex<Option<Arc<spark::SparkWallet>>>,
    prepared: Mutex<HashMap<String, spark::PrepareSendPaymentResponse>>,
}

impl LivePaymentGateway {
    pub fn new(
        config: WalletExecutorConfig,
        mnemonic_provider: Arc<dyn MnemonicSecretProvider>,
    ) -> Self {
        Self {
            config,
            mnemonic_provider,
            wallet: Mutex::new(None),
            prepared: Mutex::new(HashMap::new()),
        }
    }

    async fn ensure_wallet(&self) -> Result<Arc<spark::SparkWallet>, SparkGatewayError> {
        if let Some(existing) = self.wallet.lock().await.clone() {
            return Ok(existing);
        }

        let api_key = self
            .config
            .spark_api_key
            .clone()
            .ok_or_else(|| {
                SparkGatewayError::new(
                    SparkGatewayErrorCode::ApiKeyMissing,
                    "spark api key is missing",
                )
            })?
            .trim()
            .to_string();

        if api_key.is_empty() {
            return Err(SparkGatewayError::new(
                SparkGatewayErrorCode::ApiKeyMissing,
                "spark api key is missing",
            ));
        }

        let mnemonic = self
            .mnemonic_provider
            .load_mnemonic()
            .await
            .map_err(|error| {
                SparkGatewayError::new(SparkGatewayErrorCode::MnemonicMissing, error.message)
            })?;

        let signer = spark::SparkSigner::from_mnemonic(&mnemonic, "").map_err(|error| {
            SparkGatewayError::new(
                SparkGatewayErrorCode::MnemonicInvalid,
                format!("failed to parse mnemonic: {error}"),
            )
        })?;

        let wallet_config = spark::WalletConfig {
            network: match self.config.network {
                SparkNetwork::Mainnet => spark::Network::Mainnet,
                SparkNetwork::Regtest => spark::Network::Regtest,
            },
            api_key: Some(api_key),
            storage_dir: PathBuf::from(format!(
                "./output/spark-wallet-executor/{}",
                self.config.wallet_id
            )),
        };

        let wallet = spark::SparkWallet::new(signer, wallet_config)
            .await
            .map_err(|error| {
                SparkGatewayError::new(
                    SparkGatewayErrorCode::ConnectFailed,
                    format!("failed to build spark sdk: {error}"),
                )
            })?;

        let wallet = Arc::new(wallet);
        let mut lock = self.wallet.lock().await;
        *lock = Some(wallet.clone());
        Ok(wallet)
    }
}

#[async_trait::async_trait]
impl PaymentGateway for LivePaymentGateway {
    async fn connect(&self) -> Result<(), SparkGatewayError> {
        let _ = self.ensure_wallet().await?;
        Ok(())
    }

    async fn get_info(&self) -> Result<SparkWalletInfo, SparkGatewayError> {
        let wallet = self.ensure_wallet().await?;
        let info = wallet.get_info(true).await.map_err(|error| {
            SparkGatewayError::new(
                SparkGatewayErrorCode::ConnectFailed,
                format!("spark get_info failed: {error}"),
            )
        })?;

        Ok(SparkWalletInfo {
            identity_pubkey: None,
            balance_sats: Some(info.balance_sats),
            token_balance_count: info.token_balances.len(),
        })
    }

    async fn prepare_payment(
        &self,
        invoice: &str,
    ) -> Result<SparkPreparedPayment, SparkGatewayError> {
        let wallet = self.ensure_wallet().await?;
        let prepared = wallet
            .prepare_send_payment(invoice, None)
            .await
            .map_err(|error| {
                SparkGatewayError::new(
                    SparkGatewayErrorCode::PrepareFailed,
                    format!("spark prepare payment failed: {error}"),
                )
            })?;

        let amount_sats = u64::try_from(prepared.amount).unwrap_or(u64::MAX);
        let payment_method_type = payment_method_type_label(&prepared.payment_method).to_string();
        let prepare_id = Uuid::new_v4().to_string();

        self.prepared
            .lock()
            .await
            .insert(prepare_id.clone(), prepared);

        Ok(SparkPreparedPayment {
            prepare_id,
            amount_msats: amount_sats.saturating_mul(1_000),
            payment_method_type,
        })
    }

    async fn send_payment(
        &self,
        prepared: SparkPreparedPayment,
        payment_timeout_secs: u64,
    ) -> Result<SparkSentPayment, SparkGatewayError> {
        let wallet = self.ensure_wallet().await?;
        let mut map = self.prepared.lock().await;
        let Some(raw_prepare) = map.remove(&prepared.prepare_id) else {
            return Err(SparkGatewayError::new(
                SparkGatewayErrorCode::SendFailed,
                "prepared payment token not found",
            ));
        };
        drop(map);

        let sent = if prepared.payment_method_type == "bolt11Invoice" {
            wallet
                .send_payment_with_options(
                    raw_prepare,
                    spark::SendPaymentOptions::Bolt11Invoice {
                        prefer_spark: true,
                        completion_timeout_secs: Some(
                            u32::try_from(payment_timeout_secs).unwrap_or(u32::MAX),
                        ),
                    },
                    None,
                )
                .await
        } else {
            wallet.send_payment(raw_prepare, None).await
        }
        .map_err(|error| {
            SparkGatewayError::new(
                SparkGatewayErrorCode::SendFailed,
                format!("spark send payment failed: {error}"),
            )
        })?;

        let status = match sent.payment.status {
            spark::PaymentStatus::Completed => SparkPaymentStatus::Completed,
            spark::PaymentStatus::Pending => SparkPaymentStatus::Pending,
            spark::PaymentStatus::Failed => SparkPaymentStatus::Failed,
        };

        let amount_sats = u64::try_from(sent.payment.amount).unwrap_or(u64::MAX);
        let preimage_hex = extract_preimage_hex(sent.payment.details.as_ref());
        let paid_at_ms = to_paid_at_ms(sent.payment.timestamp);

        Ok(SparkSentPayment {
            payment_id: sent.payment.id,
            status,
            amount_msats: amount_sats.saturating_mul(1_000),
            preimage_hex,
            paid_at_ms,
        })
    }
}

fn payment_method_type_label(method: &spark::SendPaymentMethod) -> &'static str {
    match method {
        spark::SendPaymentMethod::Bolt11Invoice { .. } => "bolt11Invoice",
        spark::SendPaymentMethod::SparkAddress { .. } => "sparkAddress",
        spark::SendPaymentMethod::SparkInvoice { .. } => "sparkInvoice",
        spark::SendPaymentMethod::BitcoinAddress { .. } => "bitcoinAddress",
    }
}

pub fn extract_preimage_hex(details: Option<&spark::PaymentDetails>) -> Option<String> {
    let value = match details {
        Some(spark::PaymentDetails::Lightning { preimage, .. }) => preimage.clone(),
        Some(spark::PaymentDetails::Spark { htlc_details, .. }) => htlc_details
            .as_ref()
            .and_then(|details| details.preimage.clone()),
        _ => None,
    }?;

    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() % 2 != 0 {
        return None;
    }
    if !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    Some(normalized)
}

pub fn to_paid_at_ms(timestamp: u64) -> i64 {
    if timestamp > 1_000_000_000_000 {
        i64::try_from(timestamp).unwrap_or(i64::MAX)
    } else {
        i64::try_from(timestamp.saturating_mul(1_000)).unwrap_or(i64::MAX)
    }
}
