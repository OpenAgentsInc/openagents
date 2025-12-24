//! Spark Bridge - Payment provider integration for agent cost tracking
//!
//! This module bridges the agent-orchestrator cost tracking system to real
//! Bitcoin payments via the Spark wallet.
//!
//! # Architecture
//!
//! The PaymentProvider trait abstracts wallet operations, allowing:
//! - Cost tracking without requiring a live wallet
//! - Graceful degradation when wallet is unavailable
//! - Future integration with other payment backends
//!
//! # Status
//!
//! - SparkSigner: WORKING (BIP44 key derivation)
//! - SparkWallet: STUBBED (pending Breez SDK integration)
//! - PaymentProvider: Ready for integration when wallet is available

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    #[error("Wallet not available: {0}")]
    WalletNotAvailable(String),

    #[error("Insufficient balance: need {need} sats, have {have} sats")]
    InsufficientBalance { need: u64, have: u64 },

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Invoice creation failed: {0}")]
    InvoiceCreationFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    pub payment_id: String,
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub status: PaymentStatus,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletBalance {
    pub total_sats: u64,
    pub available_sats: u64,
    pub pending_sats: u64,
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    async fn get_balance(&self) -> Result<WalletBalance, PaymentError>;
    async fn send_payment(
        &self,
        destination: &str,
        amount_sats: u64,
        memo: Option<&str>,
    ) -> Result<PaymentResult, PaymentError>;
    async fn create_invoice(
        &self,
        amount_sats: u64,
        memo: Option<&str>,
    ) -> Result<String, PaymentError>;
}

pub struct SparkPaymentProvider {
    #[cfg(feature = "spark")]
    signer: Option<openagents_spark::SparkSigner>,
    cached_balance: RwLock<Option<WalletBalance>>,
    available: bool,
}

impl SparkPaymentProvider {
    #[cfg(feature = "spark")]
    pub fn new(mnemonic: &str, passphrase: &str) -> Result<Self, PaymentError> {
        let signer = openagents_spark::SparkSigner::from_mnemonic(mnemonic, passphrase)
            .map_err(|e| PaymentError::WalletNotAvailable(e.to_string()))?;

        Ok(Self {
            signer: Some(signer),
            cached_balance: RwLock::new(None),
            available: false,
        })
    }

    #[cfg(not(feature = "spark"))]
    pub fn new(_mnemonic: &str, _passphrase: &str) -> Result<Self, PaymentError> {
        Ok(Self {
            cached_balance: RwLock::new(None),
            available: false,
        })
    }

    pub fn unavailable() -> Self {
        Self {
            #[cfg(feature = "spark")]
            signer: None,
            cached_balance: RwLock::new(None),
            available: false,
        }
    }

    #[cfg(feature = "spark")]
    pub fn public_key(&self) -> Option<String> {
        self.signer.as_ref().map(|s| s.public_key_hex())
    }

    #[cfg(not(feature = "spark"))]
    pub fn public_key(&self) -> Option<String> {
        None
    }
}

#[async_trait]
impl PaymentProvider for SparkPaymentProvider {
    fn name(&self) -> &str {
        "spark"
    }

    fn is_available(&self) -> bool {
        self.available
    }

    async fn get_balance(&self) -> Result<WalletBalance, PaymentError> {
        if let Some(cached) = self.cached_balance.read().await.as_ref() {
            return Ok(cached.clone());
        }

        Err(PaymentError::WalletNotAvailable(
            "SparkWallet not yet integrated - pending Breez SDK".to_string(),
        ))
    }

    async fn send_payment(
        &self,
        _destination: &str,
        _amount_sats: u64,
        _memo: Option<&str>,
    ) -> Result<PaymentResult, PaymentError> {
        Err(PaymentError::WalletNotAvailable(
            "SparkWallet not yet integrated - pending Breez SDK".to_string(),
        ))
    }

    async fn create_invoice(
        &self,
        _amount_sats: u64,
        _memo: Option<&str>,
    ) -> Result<String, PaymentError> {
        Err(PaymentError::WalletNotAvailable(
            "SparkWallet not yet integrated - pending Breez SDK".to_string(),
        ))
    }
}

pub struct MockPaymentProvider {
    balance: RwLock<WalletBalance>,
    next_payment_id: RwLock<u64>,
}

impl MockPaymentProvider {
    pub fn new(initial_balance: u64) -> Self {
        Self {
            balance: RwLock::new(WalletBalance {
                total_sats: initial_balance,
                available_sats: initial_balance,
                pending_sats: 0,
            }),
            next_payment_id: RwLock::new(1),
        }
    }
}

#[async_trait]
impl PaymentProvider for MockPaymentProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn get_balance(&self) -> Result<WalletBalance, PaymentError> {
        Ok(self.balance.read().await.clone())
    }

    async fn send_payment(
        &self,
        _destination: &str,
        amount_sats: u64,
        _memo: Option<&str>,
    ) -> Result<PaymentResult, PaymentError> {
        let mut balance = self.balance.write().await;

        if balance.available_sats < amount_sats {
            return Err(PaymentError::InsufficientBalance {
                need: amount_sats,
                have: balance.available_sats,
            });
        }

        balance.available_sats -= amount_sats;
        balance.total_sats -= amount_sats;

        let mut payment_id = self.next_payment_id.write().await;
        let id = format!("mock-payment-{}", *payment_id);
        *payment_id += 1;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(PaymentResult {
            payment_id: id,
            amount_sats,
            fee_sats: 0,
            status: PaymentStatus::Complete,
            timestamp,
        })
    }

    async fn create_invoice(
        &self,
        amount_sats: u64,
        memo: Option<&str>,
    ) -> Result<String, PaymentError> {
        let desc = memo.unwrap_or("mock invoice");
        Ok(format!("lnbcmock{}sat1{}", amount_sats, desc))
    }
}

pub struct CostPaymentBridge {
    provider: Arc<dyn PaymentProvider>,
    payment_history: RwLock<Vec<PaymentResult>>,
}

impl CostPaymentBridge {
    pub fn new(provider: Arc<dyn PaymentProvider>) -> Self {
        Self {
            provider,
            payment_history: RwLock::new(Vec::new()),
        }
    }

    pub fn with_mock(initial_balance: u64) -> Self {
        Self::new(Arc::new(MockPaymentProvider::new(initial_balance)))
    }

    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    pub fn is_available(&self) -> bool {
        self.provider.is_available()
    }

    pub async fn get_balance(&self) -> Result<WalletBalance, PaymentError> {
        self.provider.get_balance().await
    }

    pub async fn pay_for_service(
        &self,
        service_id: &str,
        amount_sats: u64,
        description: &str,
    ) -> Result<PaymentResult, PaymentError> {
        let memo = format!("Agent service: {} - {}", service_id, description);
        let result = self
            .provider
            .send_payment(service_id, amount_sats, Some(&memo))
            .await?;

        self.payment_history.write().await.push(result.clone());
        Ok(result)
    }

    pub async fn create_payment_request(
        &self,
        amount_sats: u64,
        description: &str,
    ) -> Result<String, PaymentError> {
        self.provider.create_invoice(amount_sats, Some(description)).await
    }

    pub async fn total_spent(&self) -> u64 {
        self.payment_history
            .read()
            .await
            .iter()
            .filter(|p| p.status == PaymentStatus::Complete)
            .map(|p| p.amount_sats + p.fee_sats)
            .sum()
    }

    pub async fn payment_count(&self) -> usize {
        self.payment_history.read().await.len()
    }
}

pub fn is_spark_available() -> bool {
    cfg!(feature = "spark")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_payment_provider() {
        let provider = MockPaymentProvider::new(10_000);

        let balance = provider.get_balance().await.unwrap();
        assert_eq!(balance.total_sats, 10_000);
        assert_eq!(balance.available_sats, 10_000);

        let result = provider
            .send_payment("dest", 1000, Some("test"))
            .await
            .unwrap();
        assert_eq!(result.amount_sats, 1000);
        assert_eq!(result.status, PaymentStatus::Complete);

        let balance = provider.get_balance().await.unwrap();
        assert_eq!(balance.total_sats, 9_000);
    }

    #[tokio::test]
    async fn test_mock_insufficient_balance() {
        let provider = MockPaymentProvider::new(100);

        let result = provider.send_payment("dest", 1000, None).await;
        assert!(matches!(result, Err(PaymentError::InsufficientBalance { .. })));
    }

    #[tokio::test]
    async fn test_cost_payment_bridge() {
        let bridge = CostPaymentBridge::with_mock(50_000);

        assert!(bridge.is_available());
        assert_eq!(bridge.provider_name(), "mock");

        let result = bridge
            .pay_for_service("skill-123", 5000, "web-scraper skill")
            .await
            .unwrap();

        assert_eq!(result.amount_sats, 5000);
        assert_eq!(bridge.total_spent().await, 5000);
        assert_eq!(bridge.payment_count().await, 1);
    }

    #[tokio::test]
    async fn test_spark_provider_unavailable() {
        let provider = SparkPaymentProvider::unavailable();

        assert!(!provider.is_available());

        let result = provider.get_balance().await;
        assert!(matches!(result, Err(PaymentError::WalletNotAvailable(_))));
    }

    #[cfg(feature = "spark")]
    #[test]
    fn test_spark_signer_integration() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let provider = SparkPaymentProvider::new(mnemonic, "").unwrap();

        let pubkey = provider.public_key();
        assert!(pubkey.is_some());
        assert!(!pubkey.unwrap().is_empty());
    }
}
