//! Settlement Engine - Multi-mode trade settlement for agent exchanges
//!
//! Supports three settlement modes:
//! - **Mock**: Simulated settlement for testing
//! - **ReputationBased**: Higher-reputation party pays first
//! - **AtomicP2PK**: P2PK-locked eCash with HTLC-style timeouts
//!
//! # Example
//!
//! ```ignore
//! use neobank::settlement::{SettlementEngine, SettlementMode};
//!
//! // Mock settlement for testing
//! let engine = SettlementEngine::new_mock();
//! let receipt = engine.settle(&trade).await?;
//!
//! // Reputation-based settlement with wallets
//! let engine = SettlementEngine::new_reputation_based(
//!     btc_wallet,
//!     usd_wallet,
//!     Duration::from_secs(300), // 5 min timeout
//! );
//! let receipt = engine.settle(&trade).await?;
//! ```

use crate::error::{Error, Result};
use crate::exchange::{OrderSide, Trade};
use crate::types::Amount;
use crate::wallet::CashuWallet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Settlement mode configuration
#[derive(Debug, Clone)]
pub enum SettlementMode {
    /// Mock settlement for testing (always succeeds)
    Mock,

    /// Reputation-based settlement
    /// Higher-rep party pays first, then other delivers
    ReputationBased {
        /// Timeout for counterparty to deliver after first payment
        timeout: Duration,
    },

    /// Atomic P2PK settlement using locked eCash
    /// Requires CDK NUT-11 support
    AtomicP2PK {
        /// HTLC-style timeout for refund
        htlc_timeout: Duration,
    },
}

impl Default for SettlementMode {
    fn default() -> Self {
        Self::Mock
    }
}

/// Settlement method used (for receipts)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementMethod {
    /// Mock settlement (testing only)
    Mock,
    /// Reputation-based (trust + timeout)
    ReputationBased,
    /// Atomic P2PK with HTLC
    AtomicP2PK,
}

/// Proof of settlement completion
#[derive(Debug, Clone)]
pub struct SettlementReceipt {
    /// Trade ID
    pub trade_id: String,
    /// Settlement method used
    pub method: SettlementMethod,
    /// Amount settled (sats for BTC side)
    pub btc_amount_sats: u64,
    /// Amount settled (cents for fiat side)
    pub fiat_amount_cents: u64,
    /// Currency of fiat side
    pub fiat_currency: String,
    /// Settlement duration
    pub duration: Duration,
    /// BTC token/proof (if applicable)
    pub btc_proof: Option<String>,
    /// Fiat token/proof (if applicable)
    pub fiat_proof: Option<String>,
    /// Settlement timestamp
    pub settled_at: u64,
}

/// Locked proof for atomic settlement
#[derive(Debug, Clone)]
pub struct LockedProof {
    /// Unique ID
    pub id: String,
    /// Cashu token (serialized proofs)
    pub token: String,
    /// Pubkey the proof is locked to
    pub locked_to: String,
    /// Unlock condition (preimage hash for HTLC-style)
    pub hash_lock: Option<String>,
    /// Expiration timestamp
    pub expires_at: u64,
    /// Amount in smallest unit
    pub amount: u64,
    /// Mint URL
    pub mint_url: String,
}

/// Token transfer result
#[derive(Debug, Clone)]
pub struct TokenTransfer {
    /// Token string (cashuA... format)
    pub token: String,
    /// Amount transferred
    pub amount: u64,
    /// Mint URL
    pub mint_url: String,
}

/// Settlement status for multi-step settlements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementStatus {
    /// Not started
    Pending,
    /// Maker has sent their side
    MakerSent,
    /// Taker has sent their side
    TakerSent,
    /// Both sides complete
    Complete,
    /// Settlement failed
    Failed,
    /// Timed out waiting for counterparty
    TimedOut,
    /// Under dispute
    Disputed,
}

/// Settlement engine for executing trade settlements
pub struct SettlementEngine {
    /// Settlement mode
    mode: SettlementMode,
    /// BTC wallet (optional, needed for real settlement)
    btc_wallet: Option<Arc<CashuWallet>>,
    /// USD wallet (optional, needed for real settlement)
    usd_wallet: Option<Arc<CashuWallet>>,
    /// Reputation scores cache (pubkey -> score)
    reputation_cache: Arc<RwLock<std::collections::HashMap<String, f64>>>,
}

impl SettlementEngine {
    /// Create a mock settlement engine (for testing)
    pub fn new_mock() -> Self {
        Self {
            mode: SettlementMode::Mock,
            btc_wallet: None,
            usd_wallet: None,
            reputation_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Create a reputation-based settlement engine
    pub fn new_reputation_based(
        btc_wallet: Arc<CashuWallet>,
        usd_wallet: Arc<CashuWallet>,
        timeout: Duration,
    ) -> Self {
        Self {
            mode: SettlementMode::ReputationBased { timeout },
            btc_wallet: Some(btc_wallet),
            usd_wallet: Some(usd_wallet),
            reputation_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Create an atomic P2PK settlement engine
    pub fn new_atomic(
        btc_wallet: Arc<CashuWallet>,
        usd_wallet: Arc<CashuWallet>,
        htlc_timeout: Duration,
    ) -> Self {
        Self {
            mode: SettlementMode::AtomicP2PK { htlc_timeout },
            btc_wallet: Some(btc_wallet),
            usd_wallet: Some(usd_wallet),
            reputation_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get the current settlement mode
    pub fn mode(&self) -> &SettlementMode {
        &self.mode
    }

    /// Set reputation score for a pubkey (for testing or caching)
    pub async fn set_reputation(&self, pubkey: &str, score: f64) {
        let mut cache = self.reputation_cache.write().await;
        cache.insert(pubkey.to_string(), score);
    }

    /// Get reputation score for a pubkey
    pub async fn get_reputation(&self, pubkey: &str) -> f64 {
        let cache = self.reputation_cache.read().await;
        cache.get(pubkey).copied().unwrap_or(0.0)
    }

    /// Execute settlement for a trade
    ///
    /// Dispatches to appropriate settlement method based on mode.
    pub async fn settle(&self, trade: &Trade) -> Result<SettlementReceipt> {
        match &self.mode {
            SettlementMode::Mock => self.settle_mock(trade).await,
            SettlementMode::ReputationBased { timeout } => {
                self.settle_reputation_based(trade, *timeout).await
            }
            SettlementMode::AtomicP2PK { htlc_timeout } => {
                self.settle_atomic(trade, *htlc_timeout).await
            }
        }
    }

    /// Mock settlement (for testing)
    async fn settle_mock(&self, trade: &Trade) -> Result<SettlementReceipt> {
        let start = Instant::now();

        // Simulate network delay
        tokio::time::sleep(Duration::from_millis(50)).await;

        Ok(SettlementReceipt {
            trade_id: trade.trade_id.clone(),
            method: SettlementMethod::Mock,
            btc_amount_sats: trade.order.amount_sats,
            fiat_amount_cents: trade.order.fiat_amount,
            fiat_currency: trade.order.currency.clone(),
            duration: start.elapsed(),
            btc_proof: None,
            fiat_proof: None,
            settled_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Reputation-based settlement
    ///
    /// Flow:
    /// 1. Compare reputation scores
    /// 2. Higher-rep party sends first
    /// 3. Wait for counterparty to deliver (with timeout)
    /// 4. If timeout, initiate dispute
    async fn settle_reputation_based(
        &self,
        trade: &Trade,
        _timeout: Duration,
    ) -> Result<SettlementReceipt> {
        let start = Instant::now();

        let btc_wallet = self
            .btc_wallet
            .as_ref()
            .ok_or_else(|| Error::Database("BTC wallet not configured".to_string()))?;
        let usd_wallet = self
            .usd_wallet
            .as_ref()
            .ok_or_else(|| Error::Database("USD wallet not configured".to_string()))?;

        // Get reputation scores
        let maker_rep = self.get_reputation(&trade.order.maker_pubkey).await;
        let taker_rep = self.get_reputation(&trade.taker_pubkey).await;

        // Determine who sends first (higher rep goes first as they're more trusted)
        let maker_sends_first = maker_rep >= taker_rep;

        // Determine what each party sends
        let (maker_sends_btc, taker_sends_btc) = match trade.order.side {
            OrderSide::Sell => (true, false),  // Maker sells BTC, taker buys BTC
            OrderSide::Buy => (false, true),   // Maker buys BTC, taker sells BTC
        };

        let btc_amount = trade.order.amount_sats;
        let fiat_amount = trade.order.fiat_amount;

        // Execute based on who goes first
        let (btc_proof, fiat_proof) = if maker_sends_first {
            // Maker sends first
            let first_proof = if maker_sends_btc {
                let token = self.send_token(btc_wallet, btc_amount).await?;
                (Some(token.token), None)
            } else {
                let token = self.send_token(usd_wallet, fiat_amount).await?;
                (None, Some(token.token))
            };

            // Wait for taker to deliver (simulated in mock/test scenarios)
            // In real implementation, this would listen for incoming tokens
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Taker sends their side
            let second_proof = if taker_sends_btc {
                let token = self.send_token(btc_wallet, btc_amount).await?;
                (Some(token.token), first_proof.1)
            } else {
                let token = self.send_token(usd_wallet, fiat_amount).await?;
                (first_proof.0, Some(token.token))
            };

            second_proof
        } else {
            // Taker sends first
            let first_proof = if taker_sends_btc {
                let token = self.send_token(btc_wallet, btc_amount).await?;
                (Some(token.token), None)
            } else {
                let token = self.send_token(usd_wallet, fiat_amount).await?;
                (None, Some(token.token))
            };

            tokio::time::sleep(Duration::from_millis(50)).await;

            // Maker sends their side
            let second_proof = if maker_sends_btc {
                let token = self.send_token(btc_wallet, btc_amount).await?;
                (Some(token.token), first_proof.1)
            } else {
                let token = self.send_token(usd_wallet, fiat_amount).await?;
                (first_proof.0, Some(token.token))
            };

            second_proof
        };

        Ok(SettlementReceipt {
            trade_id: trade.trade_id.clone(),
            method: SettlementMethod::ReputationBased,
            btc_amount_sats: btc_amount,
            fiat_amount_cents: fiat_amount,
            fiat_currency: trade.order.currency.clone(),
            duration: start.elapsed(),
            btc_proof,
            fiat_proof,
            settled_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Atomic P2PK settlement
    ///
    /// Flow:
    /// 1. Both parties create P2PK-locked proofs
    /// 2. Exchange locked proofs
    /// 3. Reveal preimages to unlock
    /// 4. If timeout, refund
    async fn settle_atomic(
        &self,
        trade: &Trade,
        htlc_timeout: Duration,
    ) -> Result<SettlementReceipt> {
        let start = Instant::now();

        let btc_wallet = self
            .btc_wallet
            .as_ref()
            .ok_or_else(|| Error::Database("BTC wallet not configured".to_string()))?;
        let usd_wallet = self
            .usd_wallet
            .as_ref()
            .ok_or_else(|| Error::Database("USD wallet not configured".to_string()))?;

        let btc_amount = trade.order.amount_sats;
        let fiat_amount = trade.order.fiat_amount;

        // Determine who sends BTC vs fiat
        let (btc_sender, fiat_sender) = match trade.order.side {
            OrderSide::Sell => (&trade.order.maker_pubkey, &trade.taker_pubkey),
            OrderSide::Buy => (&trade.taker_pubkey, &trade.order.maker_pubkey),
        };

        // Generate shared secret for atomic swap
        // In real implementation, this would use proper HTLC preimage
        let _preimage = format!("preimage-{}", trade.trade_id);
        let hash = format!("hash-{}", trade.trade_id);

        // Create locked proofs (simulated)
        // In real implementation, would use NUT-11 P2PK
        let btc_locked = LockedProof {
            id: format!("btc-lock-{}", trade.trade_id),
            token: format!("cashuA-btc-locked-{}", btc_amount),
            locked_to: fiat_sender.clone(),
            hash_lock: Some(hash.clone()),
            expires_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + htlc_timeout.as_secs(),
            amount: btc_amount,
            mint_url: btc_wallet.mint_url().to_string(),
        };

        let fiat_locked = LockedProof {
            id: format!("fiat-lock-{}", trade.trade_id),
            token: format!("cashuA-usd-locked-{}", fiat_amount),
            locked_to: btc_sender.clone(),
            hash_lock: Some(hash.clone()),
            expires_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + htlc_timeout.as_secs(),
            amount: fiat_amount,
            mint_url: usd_wallet.mint_url().to_string(),
        };

        // Simulate atomic exchange
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Unlock proofs with preimage
        // In real implementation, would call unlock_proofs with actual preimage
        let btc_proof = Some(format!("unlocked-{}", btc_locked.token));
        let fiat_proof = Some(format!("unlocked-{}", fiat_locked.token));

        Ok(SettlementReceipt {
            trade_id: trade.trade_id.clone(),
            method: SettlementMethod::AtomicP2PK,
            btc_amount_sats: btc_amount,
            fiat_amount_cents: fiat_amount,
            fiat_currency: trade.order.currency.clone(),
            duration: start.elapsed(),
            btc_proof,
            fiat_proof,
            settled_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    // --- Proof Transfer Helpers ---

    /// Send a token from a wallet
    ///
    /// Creates a Cashu token that can be sent to counterparty.
    async fn send_token(&self, wallet: &CashuWallet, amount: u64) -> Result<TokenTransfer> {
        // Check balance
        let balance = wallet.balance().await?;
        if balance.value < amount {
            return Err(Error::InsufficientBalance {
                have: balance.value,
                need: amount,
            });
        }

        // In real implementation, would use wallet.inner.send() to create token
        // For now, simulate token creation with a mock format
        let token = format!(
            "cashuA_mock_{}_{}_{}",
            wallet.mint_url().host_str().unwrap_or("mint"),
            amount,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        Ok(TokenTransfer {
            token,
            amount,
            mint_url: wallet.mint_url().to_string(),
        })
    }

    /// Receive a token into a wallet
    ///
    /// Redeems a Cashu token and adds proofs to wallet.
    pub async fn receive_token(&self, wallet: &CashuWallet, token: &str) -> Result<Amount> {
        // In real implementation, would use wallet.inner.receive() to redeem token
        // For now, simulate token redemption

        // Parse token to get amount (simplified)
        let amount = if token.starts_with("cashuA") {
            // Extract amount from token (mock implementation)
            1000u64 // Default to 1000 for testing
        } else {
            return Err(Error::Database("Invalid token format".to_string()));
        };

        Ok(Amount::new(amount, wallet.currency()))
    }

    /// Verify a token is valid without redeeming
    pub async fn verify_token(&self, _wallet: &CashuWallet, token: &str) -> Result<bool> {
        // In real implementation, would verify proofs with mint
        Ok(token.starts_with("cashuA"))
    }

    /// Create a P2PK-locked proof
    ///
    /// Locks proofs to a specific pubkey, requiring their signature to spend.
    pub async fn lock_to_pubkey(
        &self,
        wallet: &CashuWallet,
        pubkey: &str,
        amount: u64,
        expires_at: u64,
    ) -> Result<LockedProof> {
        // Check balance
        let balance = wallet.balance().await?;
        if balance.value < amount {
            return Err(Error::InsufficientBalance {
                have: balance.value,
                need: amount,
            });
        }

        // In real implementation, would use NUT-11 P2PK locking
        Ok(LockedProof {
            id: format!("lock-{}-{}", pubkey, amount),
            token: format!("cashuA-p2pk-{}", amount),
            locked_to: pubkey.to_string(),
            hash_lock: None,
            expires_at,
            amount,
            mint_url: wallet.mint_url().to_string(),
        })
    }

    /// Unlock a P2PK-locked proof
    ///
    /// Uses private key to unlock proofs locked to our pubkey.
    pub async fn unlock_proof(
        &self,
        wallet: &CashuWallet,
        proof: &LockedProof,
    ) -> Result<Amount> {
        // In real implementation, would sign with private key to unlock
        Ok(Amount::new(proof.amount, wallet.currency()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::{Order, OrderSide, OrderStatus, Trade, TradeStatus};
    use std::time::Instant;

    fn create_test_trade() -> Trade {
        Trade {
            trade_id: "test-trade-123".to_string(),
            order: Order {
                order_id: "test-order-123".to_string(),
                maker_pubkey: "maker_pubkey_hex".to_string(),
                side: OrderSide::Sell,
                amount_sats: 10_000,
                fiat_amount: 100,
                currency: "USD".to_string(),
                premium_pct: 0.0,
                payment_methods: vec!["cashu".to_string()],
                status: OrderStatus::InProgress,
                created_at: 0,
                expires_at: u64::MAX,
            },
            taker_pubkey: "taker_pubkey_hex".to_string(),
            status: TradeStatus::Matched,
            matched_at: Instant::now(),
        }
    }

    #[tokio::test]
    async fn test_mock_settlement() {
        let engine = SettlementEngine::new_mock();
        let trade = create_test_trade();

        let receipt = engine.settle(&trade).await.unwrap();

        assert_eq!(receipt.trade_id, "test-trade-123");
        assert_eq!(receipt.method, SettlementMethod::Mock);
        assert_eq!(receipt.btc_amount_sats, 10_000);
        assert_eq!(receipt.fiat_amount_cents, 100);
        assert!(receipt.btc_proof.is_none());
        assert!(receipt.fiat_proof.is_none());
    }

    #[tokio::test]
    async fn test_settlement_mode_default() {
        let mode = SettlementMode::default();
        assert!(matches!(mode, SettlementMode::Mock));
    }

    #[tokio::test]
    async fn test_reputation_cache() {
        let engine = SettlementEngine::new_mock();

        // Initially 0
        let rep = engine.get_reputation("alice").await;
        assert_eq!(rep, 0.0);

        // Set and get
        engine.set_reputation("alice", 0.95).await;
        let rep = engine.get_reputation("alice").await;
        assert_eq!(rep, 0.95);
    }

    #[tokio::test]
    async fn test_locked_proof_creation() {
        let proof = LockedProof {
            id: "lock-123".to_string(),
            token: "cashuA-p2pk-1000".to_string(),
            locked_to: "pubkey_hex".to_string(),
            hash_lock: Some("hash_hex".to_string()),
            expires_at: 1234567890,
            amount: 1000,
            mint_url: "https://mint.example.com".to_string(),
        };

        assert_eq!(proof.amount, 1000);
        assert_eq!(proof.locked_to, "pubkey_hex");
        assert!(proof.hash_lock.is_some());
    }

    #[tokio::test]
    async fn test_settlement_receipt_fields() {
        let receipt = SettlementReceipt {
            trade_id: "trade-456".to_string(),
            method: SettlementMethod::ReputationBased,
            btc_amount_sats: 50_000,
            fiat_amount_cents: 500,
            fiat_currency: "USD".to_string(),
            duration: Duration::from_millis(150),
            btc_proof: Some("btc_token".to_string()),
            fiat_proof: Some("usd_token".to_string()),
            settled_at: 1234567890,
        };

        assert_eq!(receipt.method, SettlementMethod::ReputationBased);
        assert!(receipt.btc_proof.is_some());
        assert!(receipt.fiat_proof.is_some());
    }

    #[tokio::test]
    async fn test_settlement_status_enum() {
        assert_ne!(SettlementStatus::Pending, SettlementStatus::Complete);
        assert_eq!(SettlementStatus::Failed, SettlementStatus::Failed);
    }
}
