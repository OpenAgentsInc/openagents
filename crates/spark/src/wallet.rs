use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use breez_sdk_spark::{
    BreezSdk, ClaimDepositRequest, Config as SdkConfig, DepositClaimError, GetInfoRequest,
    GetPaymentRequest, ListPaymentsRequest, ListUnclaimedDepositsRequest, MaxFee,
    Network as SdkNetwork, Payment, PaymentDetails, PaymentStatus, PaymentType,
    PrepareSendPaymentRequest, ReceivePaymentMethod, ReceivePaymentRequest, SdkBuilder,
    SdkError as BreezSparkSdkError, Seed, SendPaymentRequest, SyncWalletRequest, default_config,
};
use spark_wallet::{
    DefaultSigner as DirectSparkSigner, ListTransfersRequest as DirectListTransfersRequest,
    Network as DirectSparkNetwork, SparkAddress as DirectSparkAddress,
    SparkWallet as DirectSparkWallet, SparkWalletConfig as DirectSparkWalletConfig,
    TransferId as DirectTransferId, WalletBuilder as DirectSparkWalletBuilder,
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

    fn to_direct_network(self) -> Result<DirectSparkNetwork, SparkError> {
        match self {
            Network::Mainnet => Ok(DirectSparkNetwork::Mainnet),
            Network::Regtest => Ok(DirectSparkNetwork::Regtest),
            Network::Testnet | Network::Signet => Err(SparkError::UnsupportedNetwork(self)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub network: Network,
    pub api_key: Option<String>,
    pub storage_dir: PathBuf,
    pub deposit_claim_fee_policy: DepositClaimFeePolicy,
    pub background_processing: bool,
    pub real_time_sync_enabled: bool,
    pub prefer_spark_over_lightning: bool,
}

impl Default for WalletConfig {
    fn default() -> Self {
        let storage_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagents")
            .join("spark");

        Self {
            network: Network::Mainnet,
            api_key: None,
            storage_dir,
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
            background_processing: true,
            real_time_sync_enabled: true,
            prefer_spark_over_lightning: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositClaimFeePolicy {
    Auto,
    Fixed { amount_sats: u64 },
    Rate { sat_per_vbyte: u64 },
    NetworkRecommended { leeway_sat_per_vbyte: u64 },
    Disabled,
}

impl Default for DepositClaimFeePolicy {
    fn default() -> Self {
        Self::Auto
    }
}

impl DepositClaimFeePolicy {
    pub fn resolved_for_network(&self, network: Network) -> Self {
        match self {
            Self::Auto => match network {
                Network::Mainnet => Self::NetworkRecommended {
                    leeway_sat_per_vbyte: 1,
                },
                Network::Regtest => Self::Rate { sat_per_vbyte: 1 },
                Network::Testnet | Network::Signet => Self::Disabled,
            },
            other => other.clone(),
        }
    }

    pub fn to_sdk_max_fee(&self, network: Network) -> Option<MaxFee> {
        match self.resolved_for_network(network) {
            Self::Auto => None,
            Self::Fixed { amount_sats } => Some(MaxFee::Fixed {
                amount: amount_sats,
            }),
            Self::Rate { sat_per_vbyte } => Some(MaxFee::Rate { sat_per_vbyte }),
            Self::NetworkRecommended {
                leeway_sat_per_vbyte,
            } => Some(MaxFee::NetworkRecommended {
                leeway_sat_per_vbyte,
            }),
            Self::Disabled => None,
        }
    }

    pub fn label(&self, network: Network) -> String {
        match self.resolved_for_network(network) {
            Self::Auto => "auto".to_string(),
            Self::Fixed { amount_sats } => format!("fixed:{amount_sats}sats"),
            Self::Rate { sat_per_vbyte } => format!("rate:{sat_per_vbyte}sat/vb"),
            Self::NetworkRecommended {
                leeway_sat_per_vbyte,
            } => format!("recommended:+{leeway_sat_per_vbyte}sat/vb"),
            Self::Disabled => "disabled".to_string(),
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

#[derive(Debug, Clone, Default)]
pub struct PaymentSummary {
    pub id: String,
    pub direction: String,
    pub status: String,
    pub amount_sats: u64,
    pub fees_sats: u64,
    pub timestamp: u64,
    pub method: String,
    pub description: Option<String>,
    pub invoice: Option<String>,
    pub destination_pubkey: Option<String>,
    pub payment_hash: Option<String>,
    pub htlc_status: Option<String>,
    pub htlc_expiry_epoch_seconds: Option<u64>,
    pub status_detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnclaimedDeposit {
    pub txid: String,
    pub vout: u32,
    pub amount_sats: u64,
    pub refund_tx_id: Option<String>,
    pub claim_error: Option<String>,
    pub claim_error_code: Option<String>,
    pub required_fee_sats: Option<u64>,
    pub required_fee_rate_sat_per_vbyte: Option<u64>,
}

impl PaymentSummary {
    pub fn is_returned_htlc_failure(&self) -> bool {
        self.status.eq_ignore_ascii_case("failed")
            && self
                .htlc_status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("returned"))
    }
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

        let sdk_config = sdk_config_for_wallet(&config)?;

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

    pub async fn sync_wallet_state(&self) -> Result<(), SparkError> {
        self.sdk
            .sync_wallet(SyncWalletRequest {})
            .await
            .map(|_| ())
            .map_err(|error| SparkError::Wallet(error.to_string()))
    }

    pub async fn network_status(&self) -> NetworkStatusReport {
        match self.sync_wallet_state().await {
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
        self.get_balance_with_sync(true).await
    }

    pub async fn get_balance_cached(&self) -> Result<Balance, SparkError> {
        self.get_balance_with_sync(false).await
    }

    async fn get_balance_with_sync(&self, ensure_synced: bool) -> Result<Balance, SparkError> {
        let info = self
            .sdk
            .get_info(GetInfoRequest {
                ensure_synced: Some(ensure_synced),
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
                payment_method: ReceivePaymentMethod::BitcoinAddress { new_address: None },
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

    pub async fn create_bolt11_invoice(
        &self,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    ) -> Result<String, SparkError> {
        if amount_sats == 0 {
            return Err(SparkError::InvalidPaymentRequest(
                "amount must be greater than zero".to_string(),
            ));
        }

        let response = self
            .sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::Bolt11Invoice {
                    description: description
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "openagents-spark-invoice".to_string()),
                    amount_sats: Some(amount_sats),
                    expiry_secs: expiry_seconds,
                    payment_hash: None,
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

        self.sync_wallet_state().await?;

        let prepare_response = self
            .sdk
            .prepare_send_payment(PrepareSendPaymentRequest {
                payment_request: request.to_string(),
                amount: amount_sats.map(u128::from),
                token_identifier: None,
                conversion_options: None,
                fee_policy: None,
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

    pub async fn send_spark_address_direct(
        &self,
        spark_address: &str,
        amount_sats: u64,
    ) -> Result<String, SparkError> {
        let request = spark_address.trim();
        if request.is_empty() {
            return Err(SparkError::InvalidPaymentRequest(
                "Spark address cannot be empty".to_string(),
            ));
        }
        if amount_sats == 0 {
            return Err(SparkError::InvalidPaymentRequest(
                "Spark transfer amount must be greater than zero".to_string(),
            ));
        }

        let receiver_address = DirectSparkAddress::from_str(request)
            .map_err(|error| SparkError::InvalidPaymentRequest(error.to_string()))?;
        let wallet = self
            .build_direct_wallet("openagents-spark-direct-transfer")
            .await?;
        let transfer = wallet
            .transfer(amount_sats, &receiver_address, None)
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(transfer.id.to_string())
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
            .map(payment_summary_from_sdk_payment)
            .collect();

        Ok(payments)
    }

    pub async fn list_all_payments(&self) -> Result<Vec<PaymentSummary>, SparkError> {
        const PAGE_SIZE: u32 = 100;

        let mut payments = Vec::new();
        let mut offset = 0u32;
        loop {
            let mut page = self.list_payments(Some(PAGE_SIZE), Some(offset)).await?;
            let page_len = page.len();
            payments.append(&mut page);
            if page_len < PAGE_SIZE as usize {
                break;
            }
            offset = offset.saturating_add(PAGE_SIZE);
        }
        Ok(payments)
    }

    pub async fn get_payment(&self, payment_id: &str) -> Result<PaymentSummary, SparkError> {
        let payment_id = payment_id.trim();
        if payment_id.is_empty() {
            return Err(SparkError::InvalidPaymentRequest(
                "payment id cannot be empty".to_string(),
            ));
        }

        let response = self
            .sdk
            .get_payment(GetPaymentRequest {
                payment_id: payment_id.to_string(),
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(payment_summary_from_sdk_payment(response.payment))
    }

    pub async fn refresh_transfer_payment_from_network(
        &self,
        payment_id: &str,
    ) -> Result<Option<PaymentSummary>, SparkError> {
        let payment_id = payment_id.trim();
        if payment_id.is_empty() {
            return Err(SparkError::InvalidPaymentRequest(
                "payment id cannot be empty".to_string(),
            ));
        }

        let Ok(transfer_id) = DirectTransferId::from_str(payment_id) else {
            return Ok(None);
        };

        let wallet = self
            .build_direct_wallet("openagents-spark-transfer-lookup")
            .await?;
        let response = wallet
            .list_transfers(DirectListTransfersRequest {
                transfer_ids: vec![transfer_id],
                ..Default::default()
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;
        let Some(transfer) = response.items.into_iter().next() else {
            return Ok(None);
        };
        let payment: Payment = transfer
            .try_into()
            .map_err(|error: BreezSparkSdkError| SparkError::Wallet(error.to_string()))?;
        Ok(Some(payment_summary_from_sdk_payment(payment)))
    }

    pub async fn list_unclaimed_deposits(&self) -> Result<Vec<UnclaimedDeposit>, SparkError> {
        let response = self
            .sdk
            .list_unclaimed_deposits(ListUnclaimedDepositsRequest {})
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(response
            .deposits
            .into_iter()
            .map(unclaimed_deposit_from_sdk_deposit)
            .collect())
    }

    pub async fn claim_unclaimed_deposit(
        &self,
        txid: &str,
        vout: u32,
        fee_policy_override: Option<DepositClaimFeePolicy>,
    ) -> Result<PaymentSummary, SparkError> {
        let txid = txid.trim();
        if txid.is_empty() {
            return Err(SparkError::InvalidPaymentRequest(
                "deposit txid cannot be empty".to_string(),
            ));
        }

        let max_fee = fee_policy_override
            .unwrap_or_else(|| self.config.deposit_claim_fee_policy.clone())
            .to_sdk_max_fee(self.config.network);

        let response = self
            .sdk
            .claim_deposit(ClaimDepositRequest {
                txid: txid.to_string(),
                vout,
                max_fee,
            })
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))?;

        Ok(payment_summary_from_sdk_payment(response.payment))
    }

    pub async fn disconnect(&self) -> Result<(), SparkError> {
        self.sdk
            .disconnect()
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))
    }

    async fn build_direct_wallet(&self, user_agent: &str) -> Result<DirectSparkWallet, SparkError> {
        let parsed_mnemonic = bip39::Mnemonic::parse(self.signer.mnemonic())
            .map_err(|error| SparkError::InvalidMnemonic(error.to_string()))?;
        let seed = parsed_mnemonic.to_seed(self.signer.passphrase());
        let signer = Arc::new(
            DirectSparkSigner::new(seed.as_ref(), self.config.network.to_direct_network()?)
                .map_err(|error| SparkError::KeyDerivation(error.to_string()))?,
        );
        let direct_config = direct_wallet_config(&self.config, user_agent)?;
        DirectSparkWalletBuilder::new(direct_config, signer)
            .with_background_processing(false)
            .build()
            .await
            .map_err(|error| SparkError::Wallet(error.to_string()))
    }
}

fn sdk_config_for_wallet(config: &WalletConfig) -> Result<SdkConfig, SparkError> {
    let mut sdk_config = default_config(config.network.to_sdk_network()?);
    if let Some(api_key) = &config.api_key {
        sdk_config.api_key = Some(api_key.clone());
    } else {
        sdk_config.real_time_sync_server_url = None;
    }
    if !config.real_time_sync_enabled {
        sdk_config.real_time_sync_server_url = None;
    }
    sdk_config.prefer_spark_over_lightning = config.prefer_spark_over_lightning;
    sdk_config.max_deposit_claim_fee = config
        .deposit_claim_fee_policy
        .to_sdk_max_fee(config.network);
    Ok(sdk_config)
}

fn direct_wallet_config(
    config: &WalletConfig,
    user_agent: &str,
) -> Result<DirectSparkWalletConfig, SparkError> {
    let mut wallet_config =
        DirectSparkWalletConfig::default_config(config.network.to_direct_network()?);
    wallet_config.service_provider_config.user_agent = Some(user_agent.to_string());
    Ok(wallet_config)
}

fn payment_direction_label(payment_type: PaymentType) -> &'static str {
    match payment_type {
        PaymentType::Send => "send",
        PaymentType::Receive => "receive",
    }
}

fn payment_summary_from_sdk_payment(payment: Payment) -> PaymentSummary {
    let mut description = None;
    let mut invoice = None;
    let mut destination_pubkey = None;
    let mut payment_hash = None;
    let mut htlc_status = None;
    let mut htlc_expiry_epoch_seconds = None;

    if let Some(details) = payment.details.as_ref() {
        match details {
            PaymentDetails::Lightning {
                description: lightning_description,
                invoice: lightning_invoice,
                destination_pubkey: lightning_destination_pubkey,
                htlc_details,
                ..
            } => {
                description.clone_from(lightning_description);
                invoice = Some(lightning_invoice.clone());
                destination_pubkey = Some(lightning_destination_pubkey.clone());
                payment_hash = Some(htlc_details.payment_hash.clone());
                htlc_expiry_epoch_seconds = Some(htlc_details.expiry_time);
                if matches!(payment.status, PaymentStatus::Failed)
                    && htlc_details.preimage.is_none()
                {
                    htlc_status = Some("preimage-missing".to_string());
                }
            }
            PaymentDetails::Spark {
                invoice_details,
                htlc_details,
                ..
            } => {
                if let Some(invoice_details) = invoice_details.as_ref() {
                    description.clone_from(&invoice_details.description);
                    invoice = Some(invoice_details.invoice.clone());
                }
                if let Some(htlc_details) = htlc_details.as_ref() {
                    payment_hash = Some(htlc_details.payment_hash.clone());
                    htlc_status = Some(htlc_details.status.to_string().to_ascii_lowercase());
                    htlc_expiry_epoch_seconds = Some(htlc_details.expiry_time);
                }
            }
            PaymentDetails::Token {
                invoice_details, ..
            } => {
                if let Some(invoice_details) = invoice_details.as_ref() {
                    description.clone_from(&invoice_details.description);
                    invoice = Some(invoice_details.invoice.clone());
                }
            }
            PaymentDetails::Withdraw { .. } | PaymentDetails::Deposit { .. } => {}
        }
    }

    let status = payment.status.to_string();
    PaymentSummary {
        id: payment.id,
        direction: payment_direction_label(payment.payment_type).to_string(),
        status_detail: payment_status_detail(payment.status, htlc_status.as_deref()),
        status,
        amount_sats: u64::try_from(payment.amount).unwrap_or(u64::MAX),
        fees_sats: u64::try_from(payment.fees).unwrap_or(u64::MAX),
        timestamp: payment.timestamp,
        method: payment.method.to_string(),
        description,
        invoice,
        destination_pubkey,
        payment_hash,
        htlc_status,
        htlc_expiry_epoch_seconds,
    }
}

fn unclaimed_deposit_from_sdk_deposit(deposit: breez_sdk_spark::DepositInfo) -> UnclaimedDeposit {
    let mut claim_error = None;
    let mut claim_error_code = None;
    let mut required_fee_sats = None;
    let mut required_fee_rate_sat_per_vbyte = None;

    if let Some(error) = deposit.claim_error.as_ref() {
        claim_error = Some(error.to_string());
        match error {
            DepositClaimError::MaxDepositClaimFeeExceeded {
                required_fee_sats: sats,
                required_fee_rate_sat_per_vbyte: fee_rate,
                ..
            } => {
                claim_error_code = Some("max_fee_exceeded".to_string());
                required_fee_sats = Some(*sats);
                required_fee_rate_sat_per_vbyte = Some(*fee_rate);
            }
            DepositClaimError::MissingUtxo { .. } => {
                claim_error_code = Some("missing_utxo".to_string());
            }
            DepositClaimError::Generic { .. } => {
                claim_error_code = Some("generic".to_string());
            }
        }
    }

    UnclaimedDeposit {
        txid: deposit.txid,
        vout: deposit.vout,
        amount_sats: deposit.amount_sats,
        refund_tx_id: deposit.refund_tx_id,
        claim_error,
        claim_error_code,
        required_fee_sats,
        required_fee_rate_sat_per_vbyte,
    }
}

fn payment_status_detail(status: PaymentStatus, htlc_status: Option<&str>) -> Option<String> {
    if matches!(status, PaymentStatus::Failed)
        && htlc_status.is_some_and(|value| value.eq_ignore_ascii_case("returned"))
    {
        return Some(
            "lightning htlc returned after expiry; refund should settle back to the wallet"
                .to_string(),
        );
    }
    if matches!(status, PaymentStatus::Failed)
        && htlc_status.is_some_and(|value| value.eq_ignore_ascii_case("preimage-missing"))
    {
        return Some(
            "lightning send failed before preimage settlement; see Mission Control log for Breez terminal detail"
                .to_string(),
        );
    }
    if matches!(status, PaymentStatus::Pending)
        && htlc_status.is_some_and(|value| value.eq_ignore_ascii_case("waitingforpreimage"))
    {
        return Some("waiting for receiver preimage".to_string());
    }
    if matches!(status, PaymentStatus::Pending)
        && htlc_status.is_some_and(|value| value.eq_ignore_ascii_case("preimageshared"))
    {
        return Some("receiver preimage shared; settlement still pending".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        Balance, DepositClaimFeePolicy, DirectSparkNetwork, Network, PaymentSummary, PaymentType,
        SdkNetwork, UnclaimedDeposit, WalletConfig, direct_wallet_config, payment_direction_label,
        payment_summary_from_sdk_payment, sdk_config_for_wallet,
        unclaimed_deposit_from_sdk_deposit,
    };
    use crate::SparkError;
    use breez_sdk_spark::{
        DepositClaimError, Fee, Payment, PaymentDetails, PaymentMethod, PaymentStatus,
        SparkHtlcDetails, SparkHtlcStatus,
    };
    use std::path::PathBuf;

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
    fn direct_network_mapping_mainnet_is_explicit() {
        assert!(matches!(
            Network::Mainnet.to_direct_network(),
            Ok(DirectSparkNetwork::Mainnet)
        ));
    }

    #[test]
    fn direct_wallet_lookup_config_sets_user_agent() {
        let config = WalletConfig {
            network: Network::Mainnet,
            api_key: None,
            storage_dir: PathBuf::from("/tmp/openagents-spark-test"),
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
            background_processing: false,
            real_time_sync_enabled: false,
            prefer_spark_over_lightning: false,
        };
        let direct_config = direct_wallet_config(&config, "openagents-spark-transfer-lookup")
            .expect("direct config");

        assert_eq!(
            direct_config.service_provider_config.user_agent.as_deref(),
            Some("openagents-spark-transfer-lookup")
        );
    }

    #[test]
    fn wallet_config_defaults_to_mainnet() {
        assert_eq!(WalletConfig::default().network, Network::Mainnet);
    }

    #[test]
    fn wallet_config_defaults_to_network_recommended_claim_fee_on_mainnet() {
        assert_eq!(
            WalletConfig::default()
                .deposit_claim_fee_policy
                .label(Network::Mainnet),
            "recommended:+1sat/vb"
        );
    }

    #[test]
    fn wallet_config_defaults_to_background_processing_enabled() {
        assert!(WalletConfig::default().background_processing);
    }

    #[test]
    fn wallet_config_defaults_to_real_time_sync_enabled() {
        assert!(WalletConfig::default().real_time_sync_enabled);
    }

    #[test]
    fn spark_sdk_dependencies_stay_on_upstream_breez() {
        let manifest =
            std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
                .expect("spark crate manifest");

        assert!(
            manifest.contains("https://github.com/breez/spark-sdk"),
            "Spark SDK dependencies must use upstream Breez"
        );
        assert!(
            !manifest.contains("AtlantisPleb/spark-sdk"),
            "Spark SDK dependencies must not use the AtlantisPleb fork"
        );
    }

    #[test]
    fn sdk_config_disables_real_time_sync_without_api_key() {
        let config = WalletConfig {
            network: Network::Mainnet,
            api_key: None,
            storage_dir: PathBuf::from("/tmp/openagents-spark-test"),
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
            background_processing: false,
            real_time_sync_enabled: true,
            prefer_spark_over_lightning: false,
        };
        let sdk_config = sdk_config_for_wallet(&config).expect("sdk config");

        assert_eq!(sdk_config.api_key, None);
        assert_eq!(sdk_config.real_time_sync_server_url, None);
    }

    #[test]
    fn sdk_config_can_disable_real_time_sync_with_api_key() {
        let config = WalletConfig {
            network: Network::Mainnet,
            api_key: Some("test-api-key".to_string()),
            storage_dir: PathBuf::from("/tmp/openagents-spark-test"),
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
            background_processing: false,
            real_time_sync_enabled: false,
            prefer_spark_over_lightning: false,
        };
        let sdk_config = sdk_config_for_wallet(&config).expect("sdk config");

        assert_eq!(sdk_config.api_key.as_deref(), Some("test-api-key"));
        assert_eq!(sdk_config.real_time_sync_server_url, None);
    }

    #[test]
    fn sdk_config_can_prefer_spark_over_lightning() {
        let config = WalletConfig {
            network: Network::Mainnet,
            api_key: Some("test-api-key".to_string()),
            storage_dir: PathBuf::from("/tmp/openagents-spark-test"),
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
            background_processing: false,
            real_time_sync_enabled: false,
            prefer_spark_over_lightning: true,
        };
        let sdk_config = sdk_config_for_wallet(&config).expect("sdk config");

        assert!(sdk_config.prefer_spark_over_lightning);
    }

    #[test]
    fn auto_claim_policy_stays_regtest_friendly() {
        assert_eq!(
            DepositClaimFeePolicy::Auto.label(Network::Regtest),
            "rate:1sat/vb"
        );
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

    #[test]
    fn payment_summary_preserves_lightning_htlc_return_detail() {
        let payment = Payment {
            id: "wallet-payment-001".to_string(),
            payment_type: PaymentType::Send,
            status: PaymentStatus::Failed,
            amount: 25,
            fees: 3,
            timestamp: 1_773_249_634,
            method: PaymentMethod::Lightning,
            details: Some(PaymentDetails::Lightning {
                description: Some("DVM textgen".to_string()),
                invoice: "lnbc250n1example".to_string(),
                destination_pubkey:
                    "02c8e87a7ab29092eba909533919c508839aea48d8e6a88c39c42a0f198a5f6401".to_string(),
                htlc_details: SparkHtlcDetails {
                    payment_hash:
                        "6b4921d489584b67a8d073e152eda483e69397d7ff06b33b45b74fc37b88d01a"
                            .to_string(),
                    preimage: None,
                    expiry_time: 1_773_249_700,
                    status: SparkHtlcStatus::Returned,
                },
                lnurl_pay_info: None,
                lnurl_withdraw_info: None,
                lnurl_receive_metadata: None,
            }),
            conversion_details: None,
        };

        let summary = payment_summary_from_sdk_payment(payment);
        assert_eq!(summary.status, "failed");
        assert_eq!(summary.method, "lightning");
        assert_eq!(summary.fees_sats, 3);
        assert_eq!(
            summary.destination_pubkey.as_deref(),
            Some("02c8e87a7ab29092eba909533919c508839aea48d8e6a88c39c42a0f198a5f6401")
        );
        assert_eq!(
            summary.payment_hash.as_deref(),
            Some("6b4921d489584b67a8d073e152eda483e69397d7ff06b33b45b74fc37b88d01a")
        );
        assert_eq!(summary.htlc_status.as_deref(), Some("preimage-missing"));
        assert!(!summary.is_returned_htlc_failure());
        assert_eq!(
            summary.status_detail.as_deref(),
            Some(
                "lightning send failed before preimage settlement; see Mission Control log for Breez terminal detail"
            )
        );
    }

    #[test]
    fn payment_summary_retains_pending_preimage_detail() {
        let summary = PaymentSummary {
            id: "wallet-payment-002".to_string(),
            direction: "send".to_string(),
            status: "pending".to_string(),
            amount_sats: 2,
            fees_sats: 0,
            timestamp: 1_762_700_040,
            method: "lightning".to_string(),
            description: None,
            invoice: None,
            destination_pubkey: None,
            payment_hash: Some("hash-002".to_string()),
            htlc_status: Some("waitingforpreimage".to_string()),
            htlc_expiry_epoch_seconds: Some(1_762_700_070),
            status_detail: Some("waiting for receiver preimage".to_string()),
        };

        assert!(!summary.is_returned_htlc_failure());
        assert_eq!(
            summary.status_detail.as_deref(),
            Some("waiting for receiver preimage")
        );
    }

    #[test]
    fn unclaimed_deposit_summary_preserves_claim_fee_requirements() {
        let summary = unclaimed_deposit_from_sdk_deposit(breez_sdk_spark::DepositInfo {
            txid: "deposit-txid-123".to_string(),
            vout: 1,
            amount_sats: 10_000,
            is_mature: true,
            refund_tx: None,
            refund_tx_id: None,
            claim_error: Some(DepositClaimError::MaxDepositClaimFeeExceeded {
                tx: "deposit-txid-123".to_string(),
                vout: 1,
                max_fee: Some(Fee::Rate { sat_per_vbyte: 1 }),
                required_fee_sats: 640,
                required_fee_rate_sat_per_vbyte: 28,
            }),
        });

        assert_eq!(
            summary,
            UnclaimedDeposit {
                txid: "deposit-txid-123".to_string(),
                vout: 1,
                amount_sats: 10_000,
                refund_tx_id: None,
                claim_error: Some(
                    "Max deposit claim fee exceeded for utxo: deposit-txid-123:1 with max fee: Some(Rate { sat_per_vbyte: 1 }) and required fee: 640 sats or 28 sats/vbyte".to_string()
                ),
                claim_error_code: Some("max_fee_exceeded".to_string()),
                required_fee_sats: Some(640),
                required_fee_rate_sat_per_vbyte: Some(28),
            }
        );
    }
}
