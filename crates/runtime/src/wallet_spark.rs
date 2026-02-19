//! Spark wallet adapter for runtime wallet traits.

use crate::fx::FxRateSnapshot;
use crate::types::Timestamp;
use crate::wallet::{WalletError, WalletInvoice, WalletPayment, WalletService};
use async_trait::async_trait;
use spark::SparkWallet;
use std::sync::Arc;

/// Wallet adapter that bridges SparkWallet into the runtime WalletService trait.
#[derive(Clone)]
pub struct SparkWalletService {
    wallet: Arc<SparkWallet>,
}

impl SparkWalletService {
    /// Create a new Spark wallet service.
    pub fn new(wallet: Arc<SparkWallet>) -> Result<Self, WalletError> {
        Ok(Self { wallet })
    }

    /// Access the underlying Spark wallet.
    pub fn wallet(&self) -> Arc<SparkWallet> {
        Arc::clone(&self.wallet)
    }

    fn usd_rate_to_sats_per_usd(rate: f64) -> Result<u64, WalletError> {
        if !rate.is_finite() || rate <= 0.0 {
            return Err(WalletError::FxUnavailable(
                "invalid USD/BTC rate".to_string(),
            ));
        }
        let sats_per_usd = (100_000_000f64 / rate).round();
        if !sats_per_usd.is_finite() || sats_per_usd <= 0.0 {
            return Err(WalletError::FxUnavailable(
                "invalid sats per USD value".to_string(),
            ));
        }
        Ok(sats_per_usd as u64)
    }
}

#[async_trait]
impl WalletService for SparkWalletService {
    async fn balance_sats(&self) -> Result<u64, WalletError> {
        let balance = self
            .wallet
            .get_balance()
            .await
            .map_err(|err| WalletError::Unavailable(err.to_string()))?;
        Ok(balance.total_sats())
    }

    async fn pay_invoice(
        &self,
        invoice: &str,
        amount_sats: Option<u64>,
    ) -> Result<WalletPayment, WalletError> {
        let response = self
            .wallet
            .send_payment_simple(invoice, amount_sats)
            .await
            .map_err(|err| WalletError::PaymentFailed(err.to_string()))?;
        let amount_sats = u64::try_from(response.payment.amount)
            .map_err(|_| WalletError::PaymentFailed("payment amount overflow".to_string()))?;
        Ok(WalletPayment {
            payment_id: response.payment.id,
            amount_sats,
        })
    }

    async fn fx_rate(&self) -> Result<FxRateSnapshot, WalletError> {
        let response = self
            .wallet
            .list_fiat_rates()
            .await
            .map_err(|err| WalletError::FxUnavailable(err.to_string()))?;
        let usd_rate = response
            .rates
            .iter()
            .find(|rate| rate.coin.eq_ignore_ascii_case("usd"))
            .ok_or_else(|| WalletError::FxUnavailable("USD rate unavailable".to_string()))?;
        let sats_per_usd = Self::usd_rate_to_sats_per_usd(usd_rate.value)?;
        Ok(FxRateSnapshot {
            sats_per_usd,
            updated_at: Timestamp::now(),
        })
    }

    async fn create_invoice(
        &self,
        amount_sats: u64,
        memo: Option<String>,
        expiry_seconds: Option<u64>,
    ) -> Result<WalletInvoice, WalletError> {
        let response = self
            .wallet
            .create_invoice(amount_sats, memo, expiry_seconds)
            .await
            .map_err(|err| WalletError::PaymentFailed(err.to_string()))?;
        Ok(WalletInvoice {
            payment_request: response.payment_request,
            amount_sats,
        })
    }
}
