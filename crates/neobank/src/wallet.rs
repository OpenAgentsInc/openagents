use cdk::nuts::{CurrencyUnit, MintQuoteState, MeltQuoteState};
use cdk::wallet::Wallet;
use cdk::Amount as CdkAmount;
use cdk_redb::WalletRedbDatabase;
use std::path::Path;
use std::sync::Arc;
use url::Url;

use crate::error::{Error, Result};
use crate::types::{Amount, Currency};

/// Cashu wallet wrapper providing a simplified interface to CDK
pub struct CashuWallet {
    inner: Wallet,
    mint_url: Url,
    currency: Currency,
}

impl CashuWallet {
    /// Create a new wallet connected to a mint
    ///
    /// # Arguments
    /// * `mint_url` - URL of the Cashu mint
    /// * `currency` - Currency this wallet handles (BTC or USD)
    /// * `seed` - 32-byte seed for deterministic key derivation
    /// * `db_path` - Path to SQLite database file
    pub async fn new(
        mint_url: Url,
        currency: Currency,
        seed: &[u8],
        db_path: &Path,
    ) -> Result<Self> {
        let unit = match currency {
            Currency::Btc => CurrencyUnit::Sat,
            Currency::Usd => CurrencyUnit::Usd,
        };

        // Create ReDB database
        let db = WalletRedbDatabase::new(db_path)
            .map_err(|e| Error::Database(e.to_string()))?;
        let db = Arc::new(db);

        // Create CDK wallet
        let wallet = Wallet::new(&mint_url.to_string(), unit, db, seed, None)?;

        Ok(Self {
            inner: wallet,
            mint_url,
            currency,
        })
    }

    /// Get current balance from local proofs
    pub async fn balance(&self) -> Result<Amount> {
        let balance = self.inner.total_balance().await?;
        Ok(Amount::new(u64::from(balance), self.currency))
    }

    /// Create a mint quote (get LN invoice to deposit funds)
    ///
    /// Returns an invoice to pay. After paying, call `check_mint_quote` to mint proofs.
    pub async fn create_mint_quote(&self, amount: u64) -> Result<MintQuote> {
        let quote = self.inner.mint_quote(CdkAmount::from(amount), None).await?;

        Ok(MintQuote {
            id: quote.id,
            amount,
            bolt11: quote.request,
            state: QuoteState::Unpaid,
        })
    }

    /// Check mint quote status and mint proofs if paid
    pub async fn check_mint_quote(&self, quote_id: &str) -> Result<QuoteState> {
        let state = self.inner.mint_quote_state(quote_id).await?;

        match state.state {
            MintQuoteState::Paid => {
                // Mint the proofs
                self.inner
                    .mint(quote_id, cdk::amount::SplitTarget::default(), None)
                    .await?;
                Ok(QuoteState::Paid)
            }
            MintQuoteState::Unpaid => Ok(QuoteState::Unpaid),
            MintQuoteState::Pending => Ok(QuoteState::Pending),
            MintQuoteState::Issued => Ok(QuoteState::Paid),
        }
    }

    /// Create a melt quote (prepare to pay LN invoice)
    ///
    /// Returns a quote with the amount and fee. Call `melt` to execute.
    pub async fn create_melt_quote(&self, bolt11: &str) -> Result<MeltQuote> {
        let quote = self.inner.melt_quote(bolt11.to_string(), None).await?;

        Ok(MeltQuote {
            id: quote.id,
            amount: u64::from(quote.amount),
            fee: u64::from(quote.fee_reserve),
            bolt11: bolt11.to_string(),
        })
    }

    /// Execute melt (pay LN invoice with proofs)
    pub async fn melt(&self, quote_id: &str) -> Result<MeltResult> {
        let result = self.inner.melt(quote_id).await?;

        Ok(MeltResult {
            paid: result.state == MeltQuoteState::Paid,
            preimage: result.preimage,
            change: result.change.map(|proofs| {
                proofs.iter().map(|p| u64::from(p.amount)).sum()
            }),
        })
    }

    /// Get the mint URL this wallet is connected to
    pub fn mint_url(&self) -> &Url {
        &self.mint_url
    }

    /// Get the currency this wallet handles
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Get proofs count (for debugging)
    pub async fn proof_count(&self) -> Result<usize> {
        let proofs = self.inner.get_unspent_proofs().await?;
        Ok(proofs.len())
    }
}

/// Mint quote - invoice to pay for receiving eCash
#[derive(Debug, Clone)]
pub struct MintQuote {
    /// Quote ID from mint
    pub id: String,
    /// Amount in smallest unit
    pub amount: u64,
    /// Lightning invoice to pay
    pub bolt11: String,
    /// Current state
    pub state: QuoteState,
}

/// Melt quote - prepared payment to send eCash
#[derive(Debug, Clone)]
pub struct MeltQuote {
    /// Quote ID from mint
    pub id: String,
    /// Amount to pay (excluding fee)
    pub amount: u64,
    /// Fee reserve
    pub fee: u64,
    /// Lightning invoice to pay
    pub bolt11: String,
}

impl MeltQuote {
    /// Total amount needed (amount + fee)
    pub fn total(&self) -> u64 {
        self.amount + self.fee
    }
}

/// Result of a melt operation
#[derive(Debug, Clone)]
pub struct MeltResult {
    /// Whether the payment succeeded
    pub paid: bool,
    /// Payment preimage (proof of payment)
    pub preimage: Option<String>,
    /// Change returned (if any)
    pub change: Option<u64>,
}

/// Quote state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteState {
    /// Invoice not yet paid
    Unpaid,
    /// Payment pending confirmation
    Pending,
    /// Payment received, proofs minted
    Paid,
    /// Quote failed or expired
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_melt_quote_total() {
        let quote = MeltQuote {
            id: "test".to_string(),
            amount: 1000,
            fee: 10,
            bolt11: "lnbc...".to_string(),
        };
        assert_eq!(quote.total(), 1010);
    }

    #[test]
    fn test_quote_state() {
        assert_ne!(QuoteState::Unpaid, QuoteState::Paid);
        assert_eq!(QuoteState::Pending, QuoteState::Pending);
    }
}
