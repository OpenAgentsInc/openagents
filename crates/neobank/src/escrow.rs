//! Escrow - Bond and collateral system for secure trading
//!
//! This module provides a bond/escrow system for protecting trades,
//! including bond creation, locking, release, and dispute handling.
//!
//! # Example
//!
//! ```ignore
//! use neobank::escrow::{EscrowService, Bond, Escrow};
//!
//! // Create escrow service
//! let service = EscrowService::new();
//!
//! // Create bond for a trade
//! let bond = service.create_bond(10_000, "my_pubkey", 3600).await?;
//!
//! // Lock bond to a trade
//! service.lock_bond(&bond, "trade-123").await?;
//!
//! // On successful trade, release bond
//! let amount = service.release_bond(&bond).await?;
//! ```

use crate::error::{Error, Result};
use crate::types::Amount;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Global counter for unique IDs
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Trade side for escrow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Maker,
    Taker,
}

impl TradeSide {
    pub fn as_str(&self) -> &str {
        match self {
            TradeSide::Maker => "maker",
            TradeSide::Taker => "taker",
        }
    }
}

/// Escrow status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowStatus {
    /// Escrow created, awaiting funding
    Pending,
    /// Partially funded (one side)
    PartiallyFunded,
    /// Fully funded (both sides)
    Funded,
    /// Trade completed, funds released
    Released,
    /// Dispute in progress
    Disputed,
    /// Bond slashed due to default
    Slashed,
    /// Expired
    Expired,
}

impl EscrowStatus {
    pub fn as_str(&self) -> &str {
        match self {
            EscrowStatus::Pending => "pending",
            EscrowStatus::PartiallyFunded => "partially_funded",
            EscrowStatus::Funded => "funded",
            EscrowStatus::Released => "released",
            EscrowStatus::Disputed => "disputed",
            EscrowStatus::Slashed => "slashed",
            EscrowStatus::Expired => "expired",
        }
    }
}

/// A collateral bond
#[derive(Debug, Clone)]
pub struct Bond {
    /// Unique bond ID
    pub id: String,
    /// Trader's public key
    pub trader: String,
    /// Bond amount in satoshis
    pub amount_sats: u64,
    /// Token representing locked funds (cashu token string)
    pub locked_token: Option<String>,
    /// Bond creation timestamp
    pub created_at: u64,
    /// Bond expiration timestamp
    pub expires_at: u64,
    /// Trade ID this bond is locked to (if any)
    pub trade_id: Option<String>,
    /// Whether bond has been released
    pub released: bool,
    /// Whether bond has been slashed
    pub slashed: bool,
}

impl Bond {
    /// Create a new bond
    pub fn new(trader: impl Into<String>, amount_sats: u64, duration: Duration) -> Self {
        let now = now_secs();
        let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        Self {
            id: format!("bond-{}-{}", now, counter),
            trader: trader.into(),
            amount_sats,
            locked_token: None,
            created_at: now,
            expires_at: now + duration.as_secs(),
            trade_id: None,
            released: false,
            slashed: false,
        }
    }

    /// Check if bond has expired
    pub fn is_expired(&self) -> bool {
        now_secs() > self.expires_at
    }

    /// Check if bond is active (not released, slashed, or expired)
    pub fn is_active(&self) -> bool {
        !self.released && !self.slashed && !self.is_expired()
    }

    /// Check if bond is locked to a trade
    pub fn is_locked(&self) -> bool {
        self.trade_id.is_some()
    }

    /// Lock bond to a trade
    pub fn lock_to_trade(&mut self, trade_id: impl Into<String>) {
        self.trade_id = Some(trade_id.into());
    }

    /// Mark bond as released
    pub fn mark_released(&mut self) {
        self.released = true;
    }

    /// Mark bond as slashed
    pub fn mark_slashed(&mut self) {
        self.slashed = true;
    }
}

/// Escrow for a trade
#[derive(Debug, Clone)]
pub struct Escrow {
    /// Unique escrow ID
    pub id: String,
    /// Trade ID this escrow is for
    pub trade_id: String,
    /// Maker's bond
    pub maker_bond: Option<Bond>,
    /// Taker's bond
    pub taker_bond: Option<Bond>,
    /// Current status
    pub status: EscrowStatus,
    /// Bond percentage of trade amount
    pub bond_pct: f64,
    /// Trade amount in sats
    pub trade_amount_sats: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Expiration timestamp
    pub expires_at: u64,
}

impl Escrow {
    /// Create a new escrow for a trade
    pub fn new(
        trade_id: impl Into<String>,
        trade_amount_sats: u64,
        bond_pct: f64,
        duration: Duration,
    ) -> Self {
        let now = now_secs();
        let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        Self {
            id: format!("escrow-{}-{}", now, counter),
            trade_id: trade_id.into(),
            maker_bond: None,
            taker_bond: None,
            status: EscrowStatus::Pending,
            bond_pct,
            trade_amount_sats,
            created_at: now,
            expires_at: now + duration.as_secs(),
        }
    }

    /// Calculate required bond amount
    pub fn bond_amount(&self) -> u64 {
        ((self.trade_amount_sats as f64) * (self.bond_pct / 100.0)) as u64
    }

    /// Check if both sides have funded
    pub fn is_fully_funded(&self) -> bool {
        self.maker_bond.is_some() && self.taker_bond.is_some()
    }

    /// Check if escrow has expired
    pub fn is_expired(&self) -> bool {
        now_secs() > self.expires_at
    }

    /// Set bond for a side
    pub fn set_bond(&mut self, side: TradeSide, bond: Bond) {
        match side {
            TradeSide::Maker => self.maker_bond = Some(bond),
            TradeSide::Taker => self.taker_bond = Some(bond),
        }

        // Update status
        if self.is_fully_funded() {
            self.status = EscrowStatus::Funded;
        } else {
            self.status = EscrowStatus::PartiallyFunded;
        }
    }

    /// Get bond for a side
    pub fn get_bond(&self, side: TradeSide) -> Option<&Bond> {
        match side {
            TradeSide::Maker => self.maker_bond.as_ref(),
            TradeSide::Taker => self.taker_bond.as_ref(),
        }
    }

    /// Get mutable bond for a side
    pub fn get_bond_mut(&mut self, side: TradeSide) -> Option<&mut Bond> {
        match side {
            TradeSide::Maker => self.maker_bond.as_mut(),
            TradeSide::Taker => self.taker_bond.as_mut(),
        }
    }
}

/// Dispute information
#[derive(Debug, Clone)]
pub struct Dispute {
    /// Dispute ID
    pub id: String,
    /// Escrow ID this dispute is for
    pub escrow_id: String,
    /// Trade ID
    pub trade_id: String,
    /// Initiator's pubkey
    pub initiator: String,
    /// Reason for dispute
    pub reason: String,
    /// Dispute status
    pub status: DisputeStatus,
    /// Winner (if resolved)
    pub winner: Option<String>,
    /// Resolution notes
    pub resolution_notes: Option<String>,
    /// Creation timestamp
    pub created_at: u64,
    /// Resolution timestamp
    pub resolved_at: Option<u64>,
}

/// Dispute status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeStatus {
    /// Dispute opened
    Open,
    /// Under review
    UnderReview,
    /// Resolved
    Resolved,
    /// Dismissed
    Dismissed,
}

impl DisputeStatus {
    pub fn as_str(&self) -> &str {
        match self {
            DisputeStatus::Open => "open",
            DisputeStatus::UnderReview => "under_review",
            DisputeStatus::Resolved => "resolved",
            DisputeStatus::Dismissed => "dismissed",
        }
    }
}

/// Escrow service for managing bonds and escrows
pub struct EscrowService {
    /// Active bonds by ID
    bonds: Arc<RwLock<HashMap<String, Bond>>>,
    /// Active escrows by ID
    escrows: Arc<RwLock<HashMap<String, Escrow>>>,
    /// Active disputes by ID
    disputes: Arc<RwLock<HashMap<String, Dispute>>>,
    /// Configuration
    config: EscrowConfig,
}

/// Configuration for escrow service
#[derive(Debug, Clone)]
pub struct EscrowConfig {
    /// Default bond percentage
    pub default_bond_pct: f64,
    /// Minimum bond amount in sats
    pub min_bond_sats: u64,
    /// Maximum bond amount in sats
    pub max_bond_sats: u64,
    /// Default escrow duration
    pub default_duration: Duration,
    /// Dispute resolution timeout
    pub dispute_timeout: Duration,
}

impl Default for EscrowConfig {
    fn default() -> Self {
        Self {
            default_bond_pct: 5.0, // 5%
            min_bond_sats: 1_000,
            max_bond_sats: 10_000_000,                    // 0.1 BTC
            default_duration: Duration::from_secs(86400), // 24 hours
            dispute_timeout: Duration::from_secs(604800), // 7 days
        }
    }
}

impl EscrowService {
    /// Create a new escrow service
    pub fn new() -> Self {
        Self {
            bonds: Arc::new(RwLock::new(HashMap::new())),
            escrows: Arc::new(RwLock::new(HashMap::new())),
            disputes: Arc::new(RwLock::new(HashMap::new())),
            config: EscrowConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(mut self, config: EscrowConfig) -> Self {
        self.config = config;
        self
    }

    /// Get configuration
    pub fn config(&self) -> &EscrowConfig {
        &self.config
    }

    // ============================================================
    // Bond Management
    // ============================================================

    /// Create a new bond
    pub async fn create_bond(
        &self,
        trader: impl Into<String>,
        amount_sats: u64,
        duration: Duration,
    ) -> Result<Bond> {
        // Validate amount
        if amount_sats < self.config.min_bond_sats {
            return Err(Error::Database(format!(
                "Bond amount {} below minimum {}",
                amount_sats, self.config.min_bond_sats
            )));
        }
        if amount_sats > self.config.max_bond_sats {
            return Err(Error::Database(format!(
                "Bond amount {} exceeds maximum {}",
                amount_sats, self.config.max_bond_sats
            )));
        }

        let bond = Bond::new(trader, amount_sats, duration);
        let bond_id = bond.id.clone();

        self.bonds.write().await.insert(bond_id, bond.clone());

        Ok(bond)
    }

    /// Get a bond by ID
    pub async fn get_bond(&self, bond_id: &str) -> Option<Bond> {
        self.bonds.read().await.get(bond_id).cloned()
    }

    /// Lock a bond to a trade
    pub async fn lock_bond(&self, bond_id: &str, trade_id: &str) -> Result<()> {
        let mut bonds = self.bonds.write().await;
        let bond = bonds
            .get_mut(bond_id)
            .ok_or_else(|| Error::Database("Bond not found".to_string()))?;

        if !bond.is_active() {
            return Err(Error::Database("Bond is not active".to_string()));
        }

        if bond.is_locked() {
            return Err(Error::Database(
                "Bond already locked to a trade".to_string(),
            ));
        }

        bond.lock_to_trade(trade_id);
        Ok(())
    }

    /// Release a bond (return funds to owner)
    pub async fn release_bond(&self, bond_id: &str) -> Result<Amount> {
        let mut bonds = self.bonds.write().await;
        let bond = bonds
            .get_mut(bond_id)
            .ok_or_else(|| Error::Database("Bond not found".to_string()))?;

        if bond.released {
            return Err(Error::Database("Bond already released".to_string()));
        }

        if bond.slashed {
            return Err(Error::Database("Bond was slashed".to_string()));
        }

        bond.mark_released();

        // In real implementation, would send token back to owner
        Ok(Amount::new(bond.amount_sats, crate::types::Currency::Btc))
    }

    /// Slash a bond (transfer to counterparty)
    pub async fn slash_bond(&self, bond_id: &str, to: &str) -> Result<Amount> {
        let mut bonds = self.bonds.write().await;
        let bond = bonds
            .get_mut(bond_id)
            .ok_or_else(|| Error::Database("Bond not found".to_string()))?;

        if bond.released {
            return Err(Error::Database("Bond already released".to_string()));
        }

        if bond.slashed {
            return Err(Error::Database("Bond already slashed".to_string()));
        }

        bond.mark_slashed();

        // In real implementation, would send token to `to` address
        tracing::info!("Bond {} slashed to {}", bond_id, to);

        Ok(Amount::new(bond.amount_sats, crate::types::Currency::Btc))
    }

    // ============================================================
    // Escrow Management
    // ============================================================

    /// Create an escrow for a trade
    pub async fn create_escrow(
        &self,
        trade_id: impl Into<String>,
        trade_amount_sats: u64,
        bond_pct: Option<f64>,
    ) -> Result<Escrow> {
        let bond_pct = bond_pct.unwrap_or(self.config.default_bond_pct);

        let escrow = Escrow::new(
            trade_id,
            trade_amount_sats,
            bond_pct,
            self.config.default_duration,
        );

        let escrow_id = escrow.id.clone();
        self.escrows.write().await.insert(escrow_id, escrow.clone());

        Ok(escrow)
    }

    /// Get an escrow by ID
    pub async fn get_escrow(&self, escrow_id: &str) -> Option<Escrow> {
        self.escrows.read().await.get(escrow_id).cloned()
    }

    /// Fund escrow for a side
    pub async fn fund_escrow(
        &self,
        escrow_id: &str,
        side: TradeSide,
        trader: &str,
    ) -> Result<String> {
        let bond_amount = {
            let escrows = self.escrows.read().await;
            let escrow = escrows
                .get(escrow_id)
                .ok_or_else(|| Error::Database("Escrow not found".to_string()))?;

            if escrow.is_expired() {
                return Err(Error::Database("Escrow has expired".to_string()));
            }

            if escrow.get_bond(side).is_some() {
                return Err(Error::Database("Side already funded".to_string()));
            }

            escrow.bond_amount()
        };

        // Create bond
        let bond = self
            .create_bond(trader, bond_amount, self.config.default_duration)
            .await?;

        let bond_id = bond.id.clone();

        // Lock bond to trade and add to escrow
        {
            let mut escrows = self.escrows.write().await;
            let escrow = escrows.get_mut(escrow_id).unwrap();

            let mut new_bond = bond;
            new_bond.lock_to_trade(&escrow.trade_id);
            escrow.set_bond(side, new_bond);
        }

        Ok(bond_id)
    }

    /// Release escrow (both bonds returned)
    pub async fn release_escrow(&self, escrow_id: &str) -> Result<()> {
        let bond_ids = {
            let mut escrows = self.escrows.write().await;
            let escrow = escrows
                .get_mut(escrow_id)
                .ok_or_else(|| Error::Database("Escrow not found".to_string()))?;

            if escrow.status == EscrowStatus::Released {
                return Err(Error::Database("Escrow already released".to_string()));
            }

            if escrow.status == EscrowStatus::Slashed {
                return Err(Error::Database("Escrow was slashed".to_string()));
            }

            // Get bond IDs
            let maker_id = escrow.maker_bond.as_ref().map(|b| b.id.clone());
            let taker_id = escrow.taker_bond.as_ref().map(|b| b.id.clone());

            escrow.status = EscrowStatus::Released;

            (maker_id, taker_id)
        };

        // Release bonds
        if let Some(id) = bond_ids.0 {
            self.release_bond(&id).await?;
        }
        if let Some(id) = bond_ids.1 {
            self.release_bond(&id).await?;
        }

        Ok(())
    }

    // ============================================================
    // Dispute Handling
    // ============================================================

    /// Initiate a dispute
    pub async fn initiate_dispute(
        &self,
        escrow_id: &str,
        initiator: &str,
        reason: &str,
    ) -> Result<String> {
        let trade_id = {
            let mut escrows = self.escrows.write().await;
            let escrow = escrows
                .get_mut(escrow_id)
                .ok_or_else(|| Error::Database("Escrow not found".to_string()))?;

            if escrow.status != EscrowStatus::Funded {
                return Err(Error::Database("Escrow not fully funded".to_string()));
            }

            escrow.status = EscrowStatus::Disputed;
            escrow.trade_id.clone()
        };

        let now = now_secs();
        let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let dispute = Dispute {
            id: format!("dispute-{}-{}", now, counter),
            escrow_id: escrow_id.to_string(),
            trade_id,
            initiator: initiator.to_string(),
            reason: reason.to_string(),
            status: DisputeStatus::Open,
            winner: None,
            resolution_notes: None,
            created_at: now,
            resolved_at: None,
        };

        let dispute_id = dispute.id.clone();
        self.disputes
            .write()
            .await
            .insert(dispute_id.clone(), dispute);

        Ok(dispute_id)
    }

    /// Get a dispute by ID
    pub async fn get_dispute(&self, dispute_id: &str) -> Option<Dispute> {
        self.disputes.read().await.get(dispute_id).cloned()
    }

    /// Resolve a dispute
    pub async fn resolve_dispute(
        &self,
        dispute_id: &str,
        winner: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let escrow_id = {
            let mut disputes = self.disputes.write().await;
            let dispute = disputes
                .get_mut(dispute_id)
                .ok_or_else(|| Error::Database("Dispute not found".to_string()))?;

            if dispute.status != DisputeStatus::Open && dispute.status != DisputeStatus::UnderReview
            {
                return Err(Error::Database("Dispute not open".to_string()));
            }

            dispute.status = DisputeStatus::Resolved;
            dispute.winner = Some(winner.to_string());
            dispute.resolution_notes = notes.map(|s| s.to_string());
            dispute.resolved_at = Some(now_secs());

            dispute.escrow_id.clone()
        };

        // Determine which side won and slash loser's bond
        let (winner_bond_id, loser_bond_id, _loser_trader) = {
            let escrows = self.escrows.read().await;
            let escrow = escrows.get(&escrow_id).unwrap();

            let maker_bond = escrow.maker_bond.as_ref();
            let taker_bond = escrow.taker_bond.as_ref();

            if let (Some(mb), Some(tb)) = (maker_bond, taker_bond) {
                if &mb.trader == winner {
                    (Some(mb.id.clone()), Some(tb.id.clone()), tb.trader.clone())
                } else {
                    (Some(tb.id.clone()), Some(mb.id.clone()), mb.trader.clone())
                }
            } else {
                (None, None, String::new())
            }
        };

        // Release winner's bond
        if let Some(id) = winner_bond_id {
            self.release_bond(&id).await?;
        }

        // Slash loser's bond to winner
        if let Some(id) = loser_bond_id {
            self.slash_bond(&id, winner).await?;
        }

        // Update escrow status
        {
            let mut escrows = self.escrows.write().await;
            if let Some(escrow) = escrows.get_mut(&escrow_id) {
                escrow.status = EscrowStatus::Slashed;
            }
        }

        Ok(())
    }

    /// List active escrows
    pub async fn list_escrows(&self) -> Vec<Escrow> {
        self.escrows.read().await.values().cloned().collect()
    }

    /// List active disputes
    pub async fn list_disputes(&self) -> Vec<Dispute> {
        self.disputes.read().await.values().cloned().collect()
    }

    /// Clear expired items
    pub async fn cleanup_expired(&self) {
        let now = now_secs();

        // Mark expired escrows
        {
            let mut escrows = self.escrows.write().await;
            for escrow in escrows.values_mut() {
                if escrow.expires_at < now && escrow.status == EscrowStatus::Pending {
                    escrow.status = EscrowStatus::Expired;
                }
            }
        }
    }
}

impl Default for EscrowService {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bond_creation() {
        let bond = Bond::new("trader1", 10_000, Duration::from_secs(3600));

        assert!(bond.id.starts_with("bond-"));
        assert_eq!(bond.trader, "trader1");
        assert_eq!(bond.amount_sats, 10_000);
        assert!(!bond.is_expired());
        assert!(bond.is_active());
        assert!(!bond.is_locked());
    }

    #[test]
    fn test_bond_lock() {
        let mut bond = Bond::new("trader1", 10_000, Duration::from_secs(3600));

        assert!(!bond.is_locked());
        bond.lock_to_trade("trade-123");
        assert!(bond.is_locked());
        assert_eq!(bond.trade_id, Some("trade-123".to_string()));
    }

    #[test]
    fn test_bond_release_slash() {
        let mut bond = Bond::new("trader1", 10_000, Duration::from_secs(3600));

        assert!(bond.is_active());
        bond.mark_released();
        assert!(!bond.is_active());
        assert!(bond.released);

        let mut bond2 = Bond::new("trader2", 10_000, Duration::from_secs(3600));
        bond2.mark_slashed();
        assert!(!bond2.is_active());
        assert!(bond2.slashed);
    }

    #[test]
    fn test_escrow_creation() {
        let escrow = Escrow::new("trade-123", 100_000, 5.0, Duration::from_secs(86400));

        assert!(escrow.id.starts_with("escrow-"));
        assert_eq!(escrow.trade_id, "trade-123");
        assert_eq!(escrow.trade_amount_sats, 100_000);
        assert_eq!(escrow.bond_pct, 5.0);
        assert_eq!(escrow.bond_amount(), 5_000); // 5% of 100k
        assert_eq!(escrow.status, EscrowStatus::Pending);
        assert!(!escrow.is_fully_funded());
    }

    #[test]
    fn test_escrow_funding() {
        let mut escrow = Escrow::new("trade-123", 100_000, 5.0, Duration::from_secs(86400));

        // Add maker bond
        let maker_bond = Bond::new("maker", 5_000, Duration::from_secs(86400));
        escrow.set_bond(TradeSide::Maker, maker_bond);
        assert_eq!(escrow.status, EscrowStatus::PartiallyFunded);
        assert!(!escrow.is_fully_funded());

        // Add taker bond
        let taker_bond = Bond::new("taker", 5_000, Duration::from_secs(86400));
        escrow.set_bond(TradeSide::Taker, taker_bond);
        assert_eq!(escrow.status, EscrowStatus::Funded);
        assert!(escrow.is_fully_funded());
    }

    #[tokio::test]
    async fn test_create_bond() {
        let service = EscrowService::new();

        let bond = service
            .create_bond("trader1", 10_000, Duration::from_secs(3600))
            .await
            .unwrap();

        assert_eq!(bond.trader, "trader1");
        assert_eq!(bond.amount_sats, 10_000);

        // Retrieve it
        let retrieved = service.get_bond(&bond.id).await.unwrap();
        assert_eq!(retrieved.id, bond.id);
    }

    #[tokio::test]
    async fn test_create_bond_too_small() {
        let service = EscrowService::new();

        let result = service
            .create_bond("trader1", 100, Duration::from_secs(3600)) // Below minimum
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_lock_and_release_bond() {
        let service = EscrowService::new();

        let bond = service
            .create_bond("trader1", 10_000, Duration::from_secs(3600))
            .await
            .unwrap();

        // Lock to trade
        service.lock_bond(&bond.id, "trade-123").await.unwrap();

        let locked_bond = service.get_bond(&bond.id).await.unwrap();
        assert!(locked_bond.is_locked());

        // Release
        let amount = service.release_bond(&bond.id).await.unwrap();
        assert_eq!(amount.value, 10_000);

        // Can't release twice
        let result = service.release_bond(&bond.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_slash_bond() {
        let service = EscrowService::new();

        let bond = service
            .create_bond("trader1", 10_000, Duration::from_secs(3600))
            .await
            .unwrap();

        let amount = service.slash_bond(&bond.id, "winner").await.unwrap();
        assert_eq!(amount.value, 10_000);

        // Can't slash twice
        let result = service.slash_bond(&bond.id, "winner").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_and_fund_escrow() {
        let service = EscrowService::new();

        // Create escrow
        let escrow = service
            .create_escrow("trade-123", 100_000, Some(5.0))
            .await
            .unwrap();

        assert_eq!(escrow.bond_amount(), 5_000);

        // Fund maker side
        let maker_bond_id = service
            .fund_escrow(&escrow.id, TradeSide::Maker, "maker_pubkey")
            .await
            .unwrap();

        let updated_escrow = service.get_escrow(&escrow.id).await.unwrap();
        assert_eq!(updated_escrow.status, EscrowStatus::PartiallyFunded);

        // Fund taker side
        let taker_bond_id = service
            .fund_escrow(&escrow.id, TradeSide::Taker, "taker_pubkey")
            .await
            .unwrap();

        let updated_escrow = service.get_escrow(&escrow.id).await.unwrap();
        assert_eq!(updated_escrow.status, EscrowStatus::Funded);
        assert!(updated_escrow.is_fully_funded());
    }

    #[tokio::test]
    async fn test_release_escrow() {
        let service = EscrowService::new();

        let escrow = service
            .create_escrow("trade-123", 100_000, Some(5.0))
            .await
            .unwrap();

        service
            .fund_escrow(&escrow.id, TradeSide::Maker, "maker")
            .await
            .unwrap();
        service
            .fund_escrow(&escrow.id, TradeSide::Taker, "taker")
            .await
            .unwrap();

        // Release
        service.release_escrow(&escrow.id).await.unwrap();

        let updated = service.get_escrow(&escrow.id).await.unwrap();
        assert_eq!(updated.status, EscrowStatus::Released);
    }

    #[tokio::test]
    async fn test_dispute_flow() {
        let service = EscrowService::new();

        // Create and fund escrow
        let escrow = service
            .create_escrow("trade-123", 100_000, Some(5.0))
            .await
            .unwrap();

        service
            .fund_escrow(&escrow.id, TradeSide::Maker, "maker")
            .await
            .unwrap();
        service
            .fund_escrow(&escrow.id, TradeSide::Taker, "taker")
            .await
            .unwrap();

        // Initiate dispute
        let dispute_id = service
            .initiate_dispute(&escrow.id, "maker", "Counterparty not responding")
            .await
            .unwrap();

        let dispute = service.get_dispute(&dispute_id).await.unwrap();
        assert_eq!(dispute.status, DisputeStatus::Open);
        assert_eq!(dispute.initiator, "maker");

        // Escrow should be in disputed state
        let updated_escrow = service.get_escrow(&escrow.id).await.unwrap();
        assert_eq!(updated_escrow.status, EscrowStatus::Disputed);

        // Resolve dispute in favor of maker
        service
            .resolve_dispute(&dispute_id, "maker", Some("Taker was unresponsive"))
            .await
            .unwrap();

        let resolved = service.get_dispute(&dispute_id).await.unwrap();
        assert_eq!(resolved.status, DisputeStatus::Resolved);
        assert_eq!(resolved.winner, Some("maker".to_string()));

        // Escrow should be slashed
        let final_escrow = service.get_escrow(&escrow.id).await.unwrap();
        assert_eq!(final_escrow.status, EscrowStatus::Slashed);
    }

    #[test]
    fn test_escrow_config() {
        let config = EscrowConfig::default();

        assert_eq!(config.default_bond_pct, 5.0);
        assert_eq!(config.min_bond_sats, 1_000);
        assert_eq!(config.max_bond_sats, 10_000_000);
    }

    #[test]
    fn test_trade_side() {
        assert_eq!(TradeSide::Maker.as_str(), "maker");
        assert_eq!(TradeSide::Taker.as_str(), "taker");
    }

    #[test]
    fn test_escrow_status() {
        assert_eq!(EscrowStatus::Pending.as_str(), "pending");
        assert_eq!(EscrowStatus::Funded.as_str(), "funded");
        assert_eq!(EscrowStatus::Disputed.as_str(), "disputed");
    }
}
