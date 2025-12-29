use cdk::nuts::{CurrencyUnit, MintQuoteState, MeltQuoteState};
use cdk::wallet::Wallet;
use cdk::Amount as CdkAmount;
use cdk_redb::WalletRedbDatabase;
use std::path::Path;
use std::str::FromStr;
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

    // ============================================================
    // Proof Transfer Methods (for settlement)
    // ============================================================

    /// Create a cashu token from wallet proofs
    ///
    /// Selects proofs to cover the amount and serializes them as a token string.
    /// The token can be sent to another party to receive the funds.
    ///
    /// # Arguments
    /// * `amount` - Amount to send in smallest unit (sats for BTC, cents for USD)
    ///
    /// # Returns
    /// A cashu token string (cashuA... format) that can be redeemed
    pub async fn send_token(&self, amount: u64) -> Result<String> {
        // Check balance first
        let balance = self.balance().await?;
        if balance.value < amount {
            return Err(Error::InsufficientBalance {
                have: balance.value,
                need: amount,
            });
        }

        // Use CDK to create a token
        // The send method selects proofs and creates a serialized token
        let token = self
            .inner
            .send(
                CdkAmount::from(amount),
                None, // memo
                None, // conditions (for P2PK)
                &cdk::amount::SplitTarget::default(),
                &cdk::wallet::SendKind::default(),
                false, // include fees
            )
            .await?;

        Ok(token.to_string())
    }

    /// Receive a cashu token into the wallet
    ///
    /// Verifies the token with the mint and adds the proofs to the wallet.
    /// Returns the amount received.
    ///
    /// # Arguments
    /// * `token` - Cashu token string to redeem
    ///
    /// # Returns
    /// The amount received
    pub async fn receive_token(&self, token: &str) -> Result<Amount> {
        // Parse and receive the token
        let received = self
            .inner
            .receive(
                token,
                cdk::amount::SplitTarget::default(),
                &[], // signing keys (for P2PK)
                &[], // preimages (for HTLC)
            )
            .await?;

        Ok(Amount::new(u64::from(received), self.currency))
    }

    /// Verify a cashu token is valid without redeeming
    ///
    /// Checks with the mint that the proofs are valid and unspent.
    /// Does NOT consume the proofs - they can still be received later.
    ///
    /// # Arguments
    /// * `token` - Cashu token string to verify
    ///
    /// # Returns
    /// true if token is valid and unspent
    pub async fn verify_token(&self, token: &str) -> Result<bool> {
        // Parse the token
        let token_parsed = cdk::nuts::Token::from_str(token)
            .map_err(|e| Error::Database(format!("Invalid token format: {}", e)))?;

        // Extract proofs (Token::proofs() returns Vec<Proof>)
        let proofs = token_parsed.proofs();

        if proofs.is_empty() {
            return Ok(false);
        }

        // Check if proofs are spent using the mint
        let spent_states = self.inner.check_proofs_spent(proofs).await?;

        // All proofs must be unspent for the token to be valid
        Ok(spent_states.iter().all(|state| {
            matches!(state.state, cdk::nuts::State::Unspent)
        }))
    }

    /// Get the total amount in a token without redeeming
    ///
    /// # Arguments
    /// * `token` - Cashu token string
    ///
    /// # Returns
    /// The total amount in the token
    pub fn token_amount(&self, token: &str) -> Result<Amount> {
        let token_parsed = cdk::nuts::Token::from_str(token)
            .map_err(|e| Error::Database(format!("Invalid token format: {}", e)))?;

        // Token::proofs() returns Vec<Proof>
        let amount: u64 = token_parsed
            .proofs()
            .iter()
            .map(|p| u64::from(p.amount))
            .sum();

        Ok(Amount::new(amount, self.currency))
    }

    /// Get a reference to the inner CDK wallet
    ///
    /// For advanced operations not exposed by this wrapper.
    pub fn inner(&self) -> &Wallet {
        &self.inner
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
