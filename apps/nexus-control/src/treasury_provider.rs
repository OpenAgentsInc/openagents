use std::fmt;
use std::path::PathBuf;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type TreasuryProviderResult<T> = std::result::Result<T, TreasuryProviderError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryLightningProviderKind {
    Ldk,
    SparkFinalDrain,
}

impl TreasuryLightningProviderKind {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Ldk);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "ldk" => Ok(Self::Ldk),
            "spark_final_drain" | "spark-final-drain" | "spark_final-drain"
            | "spark-final_drain" => Ok(Self::SparkFinalDrain),
            "spark" => Err(
                "NEXUS_TREASURY_PROVIDER=spark is not supported; use spark_final_drain only for explicit recovery work"
                    .to_string(),
            ),
            other => Err(format!(
                "invalid NEXUS_TREASURY_PROVIDER '{other}' (supported: ldk, spark_final_drain)"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ldk => "ldk",
            Self::SparkFinalDrain => "spark_final_drain",
        }
    }
}

impl Default for TreasuryLightningProviderKind {
    fn default() -> Self {
        Self::Ldk
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkNetwork {
    Regtest,
    Signet,
    Bitcoin,
}

impl LdkNetwork {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Regtest);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "regtest" => Ok(Self::Regtest),
            "signet" => Ok(Self::Signet),
            "bitcoin" | "mainnet" => Ok(Self::Bitcoin),
            other => Err(format!(
                "invalid NEXUS_LDK_NETWORK '{other}' (supported: regtest, signet, bitcoin)"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regtest => "regtest",
            Self::Signet => "signet",
            Self::Bitcoin => "bitcoin",
        }
    }

    const fn invoice_prefix(self) -> &'static str {
        match self {
            Self::Regtest => "lnbcrt",
            Self::Signet => "lntbs",
            Self::Bitcoin => "lnbc",
        }
    }
}

impl Default for LdkNetwork {
    fn default() -> Self {
        Self::Regtest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LdkChainBackend {
    Bitcoind,
    Electrum,
    Esplora,
}

impl LdkChainBackend {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Bitcoind);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "bitcoind" => Ok(Self::Bitcoind),
            "electrum" => Ok(Self::Electrum),
            "esplora" => Ok(Self::Esplora),
            other => Err(format!(
                "invalid NEXUS_LDK_CHAIN_BACKEND '{other}' (supported: bitcoind, electrum, esplora)"
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bitcoind => "bitcoind",
            Self::Electrum => "electrum",
            Self::Esplora => "esplora",
        }
    }
}

impl Default for LdkChainBackend {
    fn default() -> Self {
        Self::Bitcoind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdkTreasuryProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<PathBuf>,
    pub storage_dir: PathBuf,
    pub network: LdkNetwork,
    pub chain_backend: LdkChainBackend,
}

impl LdkTreasuryProviderConfig {
    pub fn local_scaffold(storage_dir: PathBuf) -> Self {
        Self {
            server_url: None,
            api_key_path: None,
            tls_cert_path: None,
            storage_dir,
            network: LdkNetwork::Regtest,
            chain_backend: LdkChainBackend::Bitcoind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreasuryLightningProviderConfig {
    pub provider: TreasuryLightningProviderKind,
    pub spark_final_drain_enabled: bool,
    pub ldk: LdkTreasuryProviderConfig,
}

impl TreasuryLightningProviderConfig {
    pub fn new(
        provider: TreasuryLightningProviderKind,
        spark_final_drain_enabled: bool,
        ldk: LdkTreasuryProviderConfig,
    ) -> Result<Self, String> {
        if provider == TreasuryLightningProviderKind::SparkFinalDrain && !spark_final_drain_enabled
        {
            return Err(
                "NEXUS_TREASURY_PROVIDER=spark_final_drain requires NEXUS_SPARK_FINAL_DRAIN_ENABLED=true"
                    .to_string(),
            );
        }
        Ok(Self {
            provider,
            spark_final_drain_enabled,
            ldk,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderFundingRequest {
    pub amount_sats: Option<u64>,
    pub description: Option<String>,
    pub expiry_seconds: Option<u32>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderFundingTarget {
    pub provider_target: String,
    pub bitcoin_address: String,
    pub bolt11_invoice: Option<String>,
    pub provider_invoice: Option<String>,
    pub balance_sats: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderPayoutRequest {
    pub payout_key: String,
    pub payment_request: String,
    pub amount_sats: u64,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderPayoutReceipt {
    pub payment_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryProviderErrorKind {
    Disabled,
    InvalidConfig,
    InvalidRequest,
    Unavailable,
    Failed,
}

impl TreasuryProviderErrorKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InvalidConfig => "invalid_config",
            Self::InvalidRequest => "invalid_request",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryProviderError {
    pub provider: TreasuryLightningProviderKind,
    pub kind: TreasuryProviderErrorKind,
    pub reason: String,
}

impl TreasuryProviderError {
    pub fn new(
        provider: TreasuryLightningProviderKind,
        kind: TreasuryProviderErrorKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            kind,
            reason: reason.into(),
        }
    }

    pub fn normalized_reason(&self) -> String {
        format!(
            "treasury_provider_error:{}:{}:{}",
            self.provider.as_str(),
            self.kind.as_str(),
            self.reason
        )
    }
}

impl fmt::Display for TreasuryProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.normalized_reason().as_str())
    }
}

impl std::error::Error for TreasuryProviderError {}

pub trait TreasuryLightningProvider: Send + Sync {
    fn provider_kind(&self) -> TreasuryLightningProviderKind;

    fn create_funding_target<'a>(
        &'a self,
        request: TreasuryProviderFundingRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderFundingTarget>>;

    fn dispatch_payout<'a>(
        &'a self,
        request: TreasuryProviderPayoutRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderPayoutReceipt>>;
}

#[derive(Debug, Clone)]
pub struct LdkTreasuryProvider {
    config: LdkTreasuryProviderConfig,
}

impl LdkTreasuryProvider {
    pub fn new(config: LdkTreasuryProviderConfig) -> Self {
        Self { config }
    }

    fn local_invoice(&self, request: &TreasuryProviderFundingRequest) -> Option<String> {
        let amount = request.amount_sats?;
        if amount == 0 {
            return None;
        }
        let digest = short_hash(request.idempotency_key.as_str());
        Some(format!(
            "{}{}nexus{}",
            self.config.network.invoice_prefix(),
            amount,
            digest
        ))
    }
}

impl TreasuryLightningProvider for LdkTreasuryProvider {
    fn provider_kind(&self) -> TreasuryLightningProviderKind {
        TreasuryLightningProviderKind::Ldk
    }

    fn create_funding_target<'a>(
        &'a self,
        request: TreasuryProviderFundingRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderFundingTarget>> {
        Box::pin(async move {
            if matches!(request.amount_sats, Some(0)) {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    "funding_amount_must_be_greater_than_zero",
                ));
            }
            let digest = short_hash(request.idempotency_key.as_str());
            Ok(TreasuryProviderFundingTarget {
                provider_target: format!(
                    "ldk://nexus/{}/{}/{}",
                    self.config.network.as_str(),
                    self.config.chain_backend.as_str(),
                    digest
                ),
                bitcoin_address: format!(
                    "ldk-local-{}-{}-{}",
                    self.config.network.as_str(),
                    self.config.chain_backend.as_str(),
                    digest
                ),
                bolt11_invoice: self.local_invoice(&request),
                provider_invoice: None,
                balance_sats: 0,
            })
        })
    }

    fn dispatch_payout<'a>(
        &'a self,
        request: TreasuryProviderPayoutRequest,
    ) -> BoxFuture<'a, TreasuryProviderResult<TreasuryProviderPayoutReceipt>> {
        Box::pin(async move {
            if request.amount_sats == 0 {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    "payout_amount_must_be_greater_than_zero",
                ));
            }
            if request.payment_request.trim().is_empty() {
                return Err(TreasuryProviderError::new(
                    TreasuryLightningProviderKind::Ldk,
                    TreasuryProviderErrorKind::InvalidRequest,
                    "payment_request_missing",
                ));
            }
            Ok(TreasuryProviderPayoutReceipt {
                payment_id: format!("ldk-local-payment-{}", short_hash(&request.idempotency_key)),
            })
        })
    }
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(&digest[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_parser_defaults_to_ldk() {
        assert_eq!(
            TreasuryLightningProviderKind::parse(None).expect("default"),
            TreasuryLightningProviderKind::Ldk
        );
        assert_eq!(
            TreasuryLightningProviderKind::parse(Some("ldk")).expect("ldk"),
            TreasuryLightningProviderKind::Ldk
        );
        assert!(
            TreasuryLightningProviderKind::parse(Some("spark"))
                .expect_err("plain spark forbidden")
                .contains("spark_final_drain")
        );
    }

    #[test]
    fn ldk_network_and_backend_parse_expected_values() {
        assert_eq!(
            LdkNetwork::parse(Some("signet")).expect("signet"),
            LdkNetwork::Signet
        );
        assert_eq!(
            LdkNetwork::parse(Some("mainnet")).expect("mainnet"),
            LdkNetwork::Bitcoin
        );
        assert_eq!(
            LdkChainBackend::parse(Some("esplora")).expect("esplora"),
            LdkChainBackend::Esplora
        );
    }

    #[test]
    fn spark_final_drain_provider_requires_explicit_flag() {
        let config = LdkTreasuryProviderConfig::local_scaffold(PathBuf::from("/tmp/ldk"));
        assert!(
            TreasuryLightningProviderConfig::new(
                TreasuryLightningProviderKind::SparkFinalDrain,
                false,
                config.clone()
            )
            .expect_err("disabled")
            .contains("NEXUS_SPARK_FINAL_DRAIN_ENABLED")
        );
        assert!(
            TreasuryLightningProviderConfig::new(
                TreasuryLightningProviderKind::SparkFinalDrain,
                true,
                config
            )
            .is_ok()
        );
    }

    #[tokio::test]
    async fn ldk_scaffold_is_deterministic_by_idempotency_key() {
        let provider = LdkTreasuryProvider::new(LdkTreasuryProviderConfig::local_scaffold(
            PathBuf::from("/tmp/ldk"),
        ));
        let request = TreasuryProviderFundingRequest {
            amount_sats: Some(21),
            description: Some("fund".to_string()),
            expiry_seconds: Some(60),
            idempotency_key: "same-key".to_string(),
        };
        let first = provider
            .create_funding_target(request.clone())
            .await
            .expect("first");
        let second = provider
            .create_funding_target(request)
            .await
            .expect("second");
        assert_eq!(first, second);

        let payout = TreasuryProviderPayoutRequest {
            payout_key: "payout-a".to_string(),
            payment_request: first.bolt11_invoice.expect("bolt11"),
            amount_sats: 21,
            idempotency_key: "payout-idempotency".to_string(),
        };
        let receipt = provider.dispatch_payout(payout).await.expect("dispatch");
        assert!(receipt.payment_id.starts_with("ldk-local-payment-"));
    }

    #[tokio::test]
    async fn ldk_scaffold_normalizes_provider_errors() {
        let provider = LdkTreasuryProvider::new(LdkTreasuryProviderConfig::local_scaffold(
            PathBuf::from("/tmp/ldk"),
        ));
        let error = provider
            .dispatch_payout(TreasuryProviderPayoutRequest {
                payout_key: "payout-a".to_string(),
                payment_request: String::new(),
                amount_sats: 1,
                idempotency_key: "payout-idempotency".to_string(),
            })
            .await
            .expect_err("missing request");
        assert_eq!(error.provider, TreasuryLightningProviderKind::Ldk);
        assert_eq!(error.kind, TreasuryProviderErrorKind::InvalidRequest);
        assert_eq!(
            error.normalized_reason(),
            "treasury_provider_error:ldk:invalid_request:payment_request_missing"
        );
    }
}
