//! Wallet service traits for Lightning payments and FX rates.

use crate::fx::{FxError, FxRateProvider, FxRateSnapshot};
use std::sync::Arc;

/// Wallet errors.
#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    /// Wallet is unavailable.
    #[error("wallet unavailable: {0}")]
    Unavailable(String),
    /// Payment failed.
    #[error("payment failed: {0}")]
    PaymentFailed(String),
    /// FX lookup failed.
    #[error("fx unavailable: {0}")]
    FxUnavailable(String),
    /// Invalid request.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

/// Wallet payment record.
#[derive(Debug, Clone)]
pub struct WalletPayment {
    /// Provider-specific payment id.
    pub payment_id: String,
    /// Amount paid (sats).
    pub amount_sats: u64,
}

/// Wallet invoice record.
#[derive(Debug, Clone)]
pub struct WalletInvoice {
    /// Lightning invoice / payment request string.
    pub payment_request: String,
    /// Amount requested (sats).
    pub amount_sats: u64,
}

/// Wallet service for Lightning payments.
pub trait WalletService: Send + Sync {
    /// Return total balance in sats.
    fn balance_sats(&self) -> Result<u64, WalletError>;
    /// Pay a Lightning invoice.
    fn pay_invoice(
        &self,
        invoice: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayment, WalletError>;
    /// Return FX rate snapshot.
    fn fx_rate(&self) -> Result<FxRateSnapshot, WalletError>;
    /// Create an invoice for receiving funds.
    fn create_invoice(
        &self,
        _amount_sats: u64,
        _memo: Option<String>,
        _expiry_seconds: Option<u64>,
    ) -> Result<WalletInvoice, WalletError> {
        Err(WalletError::InvalidRequest(
            "invoice creation not supported".to_string(),
        ))
    }
}

/// Adapter that exposes wallet FX through FxRateProvider.
pub struct WalletFxProvider {
    wallet: Arc<dyn WalletService>,
}

impl WalletFxProvider {
    /// Create a new FX adapter for a wallet.
    pub fn new(wallet: Arc<dyn WalletService>) -> Self {
        Self { wallet }
    }
}

impl FxRateProvider for WalletFxProvider {
    fn fx_rate(&self) -> Result<FxRateSnapshot, FxError> {
        self.wallet
            .fx_rate()
            .map_err(|err| FxError::Wallet(err.to_string()))
    }
}
