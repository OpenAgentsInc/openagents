use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use breez_sdk_spark::{
    BreezSdk, GetInfoRequest, ListPaymentsRequest, Network as SdkNetwork, PaymentType,
    PrepareSendPaymentRequest, ReceivePaymentMethod, ReceivePaymentRequest, SdkBuilder, Seed,
    SendPaymentRequest, SyncWalletRequest, default_config,
};

use crate::{SparkError, SparkSigner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl Network {
    fn to_sdk_network(self) -> Result<SdkNetwork, SparkError> {
        match self {
            Network::Mainnet => Ok(SdkNetwork::Mainnet),
            Network::Regtest => Ok(SdkNetwork::Regtest),
            Network::Testnet | Network::Signet => Err(SparkError::UnsupportedNetwork(self)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub network: Network,
    pub api_key: Option<String>,
    pub storage_dir: PathBuf,
}

impl Default for WalletConfig {
    fn default() -> Self {
        let storage_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagents")
            .join("spark");

        Self {
            network: Network::Regtest,
            api_key: None,
            storage_dir,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Balance {
    pub spark_sats: u64,
    pub lightning_sats: u64,
    pub onchain_sats: u64,
}

impl Balance {
    pub fn total_sats(&self) -> u64 {
        self.spark_sats
            .saturating_add(self.lightning_sats)
            .saturating_add(self.onchain_sats)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkStatus {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct NetworkStatusReport {
    pub status: NetworkStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PaymentSummary {
    pub id: String,
    pub direction: String,
    pub status: String,
    pub amount_sats: u64,
    pub timestamp: u64,
}

pub struct SparkWallet {
    signer: SparkSigner,
    config: WalletConfig,
    sdk: Arc<BreezSdk>,
}

impl SparkWallet {
    pub async fn new(signer: SparkSigner, config: WalletConfig) -> Result<Self, SparkError> {
        let seed = Seed::Mnemonic {
            mnemonic: signer.mnemonic().to_string(),
            passphrase: if signer.passphrase().is_empty() {
                None
            } else {
                Some(signer.passphrase().to_string())
            },
        };

        let mut sdk_config = default_config(config.network.to_sdk_network()?);
        if let Some(api_key) = &config.api_key {
            sdk_config.api_key = Some(api_key.clone());
        } else {
            sdk_config.real_time_sync_server_url = None;
        }

        let builder = SdkBuilder::new(sdk_config, seed)
            .with_default_storage(config.storage_dir.to_string_lossy().to_string());
        let sdk = builder
            .build()
            .await
            .map_err(|error| SparkError::InitializationFailed(error.to_string()))?;

        Ok(Self {
            signer,
            config,
            sdk: Arc::new(sdk),
        })
    }

    pub fn signer(&self) -> &SparkSigner {
        &self.signer
    }

    pub fn config(&self) -> &WalletConfig {
        &self.config
    }

    pub async fn network_status(&self) -> NetworkStatusReport {
        match self.sdk.sync_wallet(SyncWalletRequest {}).await {
            Ok(_) => NetworkStatusReport {
                status: NetworkStatus::Connected,
                detail: None,
            },
            Err(error) => NetworkStatusReport {
                status: NetworkStatus::Disconnected,
                detail: Some(error.to_string()),
            },
        }
    }

    pub async fn get_balance(&self) -> Result<Balance, SparkError> {
        let info = self
            .sdk
            .get_info(GetInfoRequest {
                ensure_synced: Some(true),
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(Balance {
            spark_sats: info.balance_sats,
            lightning_sats: 0,
            onchain_sats: 0,
        })
    }

    pub async fn get_spark_address(&self) -> Result<String, SparkError> {
        let response = self
            .sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::SparkAddress,
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;
        Ok(response.payment_request)
    }

    pub async fn get_bitcoin_address(&self) -> Result<String, SparkError> {
        let response = self
            .sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::BitcoinAddress,
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;
        Ok(response.payment_request)
    }

    pub async fn create_invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u64>,
    ) -> Result<String, SparkError> {
        if amount_sats == 0 {
            return Err(SparkError::InvalidPaymentRequest(
                "amount must be greater than zero".to_string(),
            ));
        }

        let expiry_time = expiry_seconds.map(|seconds| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs().saturating_add(seconds))
                .unwrap_or(seconds)
        });

        let response = self
            .sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::SparkInvoice {
                    amount: Some(u128::from(amount_sats)),
                    token_identifier: None,
                    expiry_time,
                    description,
                    sender_public_key: None,
                },
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(response.payment_request)
    }

    pub async fn send_payment_simple(
        &self,
        payment_request: &str,
        amount_sats: Option<u64>,
    ) -> Result<String, SparkError> {
        let request = payment_request.trim();
        if request.is_empty() {
            return Err(SparkError::InvalidPaymentRequest(
                "payment request cannot be empty".to_string(),
            ));
        }

        let prepare_response = self
            .sdk
            .prepare_send_payment(PrepareSendPaymentRequest {
                payment_request: request.to_string(),
                amount: amount_sats.map(u128::from),
                token_identifier: None,
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        let response = self
            .sdk
            .send_payment(SendPaymentRequest {
                prepare_response,
                options: None,
                idempotency_key: None,
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(response.payment.id)
    }

    pub async fn list_payments(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<PaymentSummary>, SparkError> {
        let response = self
            .sdk
            .list_payments(ListPaymentsRequest {
                limit,
                offset,
                sort_ascending: Some(false),
                ..Default::default()
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        let payments = response
            .payments
            .into_iter()
            .map(|payment| PaymentSummary {
                id: payment.id,
                direction: payment_direction_label(payment.payment_type).to_string(),
                status: format!("{:?}", payment.status).to_ascii_lowercase(),
                amount_sats: u64::try_from(payment.amount).unwrap_or(u64::MAX),
                timestamp: payment.timestamp,
            })
            .collect();

        Ok(payments)
    }
}

fn payment_direction_label(payment_type: PaymentType) -> &'static str {
    match payment_type {
        PaymentType::Send => "send",
        PaymentType::Receive => "receive",
    }
}

#[cfg(test)]
mod tests {
    use super::{Balance, Network, PaymentType, SdkNetwork, payment_direction_label};
    use crate::SparkError;

    #[test]
    fn network_mapping_mainnet_is_explicit() {
        assert!(matches!(
            Network::Mainnet.to_sdk_network(),
            Ok(SdkNetwork::Mainnet)
        ));
    }

    #[test]
    fn network_mapping_regtest_is_explicit() {
        assert!(matches!(
            Network::Regtest.to_sdk_network(),
            Ok(SdkNetwork::Regtest)
        ));
    }

    #[test]
    fn network_mapping_testnet_is_rejected() {
        let result = Network::Testnet.to_sdk_network();
        assert!(matches!(
            result,
            Err(SparkError::UnsupportedNetwork(Network::Testnet))
        ));
    }

    #[test]
    fn network_mapping_signet_is_rejected() {
        let result = Network::Signet.to_sdk_network();
        assert!(matches!(
            result,
            Err(SparkError::UnsupportedNetwork(Network::Signet))
        ));
    }

    #[test]
    fn balance_total_sats_is_saturating() {
        let balance = Balance {
            spark_sats: u64::MAX,
            lightning_sats: 1,
            onchain_sats: 10,
        };
        assert_eq!(balance.total_sats(), u64::MAX);
    }

    #[test]
    fn payment_direction_labels_match_payment_type() {
        assert_eq!(payment_direction_label(PaymentType::Send), "send");
        assert_eq!(payment_direction_label(PaymentType::Receive), "receive");
    }
}
