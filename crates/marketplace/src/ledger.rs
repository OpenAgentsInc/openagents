//! Immutable ledger for payment audit trail
//!
//! Provides ledger entry types, balance tracking, and audit trail
//! functionality with cryptographic hashing for immutability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur with ledger operations
#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("Invalid entry: {0}")]
    InvalidEntry(String),

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u64, required: u64 },

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Type of ledger entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEntryType {
    /// Payment for skill usage
    SkillPayment,

    /// Payment for compute job
    ComputePayment,

    /// Payment for purchased data
    DataPayment,

    /// Coalition payment settlement
    CoalitionSettlement,

    /// Refund to payer
    Refund,

    /// Payout to provider/creator
    Payout,

    /// Account top-up
    TopUp,
}

impl LedgerEntryType {
    /// Get a description of this entry type
    pub fn description(&self) -> &'static str {
        match self {
            Self::SkillPayment => "Payment for skill usage",
            Self::ComputePayment => "Payment for compute job",
            Self::DataPayment => "Payment for purchased data",
            Self::CoalitionSettlement => "Coalition payment settlement",
            Self::Refund => "Refund to payer",
            Self::Payout => "Payout to provider/creator",
            Self::TopUp => "Account top-up",
        }
    }
}

/// Direction of money flow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Money coming in
    Inbound,

    /// Money going out
    Outbound,

    /// Internal transfer
    Internal,
}

/// Ledger operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerOperation {
    /// Add to available balance
    Credit,

    /// Subtract from available balance
    Debit,

    /// Reserve funds for pending transaction
    Hold,

    /// Free held funds back to available
    Release,

    /// Return funds to payer
    Refund,
}

impl LedgerOperation {
    /// Check if this operation increases available balance
    pub fn increases_available(&self) -> bool {
        matches!(self, Self::Credit | Self::Release | Self::Refund)
    }

    /// Check if this operation decreases available balance
    pub fn decreases_available(&self) -> bool {
        matches!(self, Self::Debit | Self::Hold)
    }
}

/// Amounts in a ledger entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerAmounts {
    /// Gross amount before fees (satoshis)
    pub gross_sats: u64,

    /// Platform fee deducted (satoshis)
    pub platform_fee_sats: u64,

    /// Net amount after fees (satoshis)
    pub net_sats: u64,
}

impl LedgerAmounts {
    /// Create new ledger amounts
    pub fn new(gross_sats: u64, platform_fee_sats: u64) -> Self {
        let net_sats = gross_sats.saturating_sub(platform_fee_sats);
        Self {
            gross_sats,
            platform_fee_sats,
            net_sats,
        }
    }

    /// Create from gross with fee rate
    pub fn from_gross_with_rate(gross_sats: u64, fee_rate: f32) -> Self {
        let platform_fee_sats = (gross_sats as f32 * fee_rate) as u64;
        Self::new(gross_sats, platform_fee_sats)
    }

    /// Validate amounts
    pub fn validate(&self) -> Result<(), LedgerError> {
        if self.net_sats != self.gross_sats.saturating_sub(self.platform_fee_sats) {
            return Err(LedgerError::InvalidEntry(
                "Net amount doesn't match gross - fees".to_string(),
            ));
        }
        Ok(())
    }
}

/// Parties involved in a ledger entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerParties {
    /// Payer's account ID
    pub payer: String,

    /// Payee's account ID
    pub payee: String,

    /// Intermediaries (e.g., coalition members)
    #[serde(default)]
    pub intermediaries: Vec<String>,
}

impl LedgerParties {
    /// Create new ledger parties
    pub fn new(payer: impl Into<String>, payee: impl Into<String>) -> Self {
        Self {
            payer: payer.into(),
            payee: payee.into(),
            intermediaries: Vec::new(),
        }
    }

    /// Add an intermediary
    pub fn with_intermediary(mut self, intermediary: impl Into<String>) -> Self {
        self.intermediaries.push(intermediary.into());
        self
    }

    /// Add multiple intermediaries
    pub fn with_intermediaries(mut self, intermediaries: Vec<String>) -> Self {
        self.intermediaries = intermediaries;
        self
    }
}

/// References to related entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerReferences {
    /// Related job ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,

    /// Related invoice ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<String>,

    /// Lightning transaction hash
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,

    /// Coalition ID for split payments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coalition_id: Option<String>,
}

impl LedgerReferences {
    /// Create empty references
    pub fn new() -> Self {
        Self::default()
    }

    /// Add job ID
    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self
    }

    /// Add invoice ID
    pub fn with_invoice_id(mut self, invoice_id: impl Into<String>) -> Self {
        self.invoice_id = Some(invoice_id.into());
        self
    }

    /// Add transaction hash
    pub fn with_tx_hash(mut self, tx_hash: impl Into<String>) -> Self {
        self.tx_hash = Some(tx_hash.into());
        self
    }

    /// Add coalition ID
    pub fn with_coalition_id(mut self, coalition_id: impl Into<String>) -> Self {
        self.coalition_id = Some(coalition_id.into());
        self
    }
}

/// Immutable ledger entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Unique entry ID
    pub id: String,

    /// When the entry was created
    pub timestamp: DateTime<Utc>,

    /// Type of entry
    pub entry_type: LedgerEntryType,

    /// Direction of money flow
    pub direction: Direction,

    /// Operation performed
    pub operation: LedgerOperation,

    /// Amounts involved
    pub amounts: LedgerAmounts,

    /// Parties involved
    pub parties: LedgerParties,

    /// References to related entities
    pub references: LedgerReferences,

    /// Hash of previous entry (for chain integrity)
    pub previous_hash: String,

    /// Hash of this entry
    pub entry_hash: String,
}

impl LedgerEntry {
    /// Create a new ledger entry
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        entry_type: LedgerEntryType,
        direction: Direction,
        operation: LedgerOperation,
        amounts: LedgerAmounts,
        parties: LedgerParties,
        references: LedgerReferences,
        previous_hash: impl Into<String>,
    ) -> Result<Self, LedgerError> {
        // Validate amounts
        amounts.validate()?;

        let entry = Self {
            id: id.into(),
            timestamp: Utc::now(),
            entry_type,
            direction,
            operation,
            amounts,
            parties,
            references,
            previous_hash: previous_hash.into(),
            entry_hash: String::new(), // Will be calculated
        };

        Ok(entry)
    }

    /// Calculate the entry hash (simplified - in production use SHA256)
    pub fn calculate_hash(&self) -> String {
        // In production, this should use proper SHA256 hashing
        // For now, create a simple hash from key fields
        format!(
            "{}-{}-{}-{}-{}-{}",
            self.id,
            self.timestamp.timestamp(),
            self.amounts.gross_sats,
            self.parties.payer,
            self.parties.payee,
            self.previous_hash
        )
    }

    /// Finalize the entry by calculating and setting the hash
    pub fn finalize(mut self) -> Self {
        self.entry_hash = self.calculate_hash();
        self
    }

    /// Verify the entry hash
    pub fn verify_hash(&self) -> Result<(), LedgerError> {
        let expected = self.calculate_hash();
        if self.entry_hash != expected {
            return Err(LedgerError::HashMismatch {
                expected,
                actual: self.entry_hash.clone(),
            });
        }
        Ok(())
    }
}

/// Account balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    /// Account ID
    pub account_id: String,

    /// Available balance (satoshis)
    pub available_sats: u64,

    /// Held/reserved balance (satoshis)
    pub held_sats: u64,

    /// Total balance (available + held)
    pub total_sats: u64,

    /// When last updated
    pub last_updated: DateTime<Utc>,
}

impl Balance {
    /// Create a new balance
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            available_sats: 0,
            held_sats: 0,
            total_sats: 0,
            last_updated: Utc::now(),
        }
    }

    /// Apply a credit operation
    pub fn credit(&mut self, amount_sats: u64) {
        self.available_sats += amount_sats;
        self.total_sats += amount_sats;
        self.last_updated = Utc::now();
    }

    /// Apply a debit operation
    pub fn debit(&mut self, amount_sats: u64) -> Result<(), LedgerError> {
        if self.available_sats < amount_sats {
            return Err(LedgerError::InsufficientBalance {
                available: self.available_sats,
                required: amount_sats,
            });
        }
        self.available_sats -= amount_sats;
        self.total_sats -= amount_sats;
        self.last_updated = Utc::now();
        Ok(())
    }

    /// Apply a hold operation
    pub fn hold(&mut self, amount_sats: u64) -> Result<(), LedgerError> {
        if self.available_sats < amount_sats {
            return Err(LedgerError::InsufficientBalance {
                available: self.available_sats,
                required: amount_sats,
            });
        }
        self.available_sats -= amount_sats;
        self.held_sats += amount_sats;
        self.last_updated = Utc::now();
        Ok(())
    }

    /// Apply a release operation
    pub fn release(&mut self, amount_sats: u64) -> Result<(), LedgerError> {
        if self.held_sats < amount_sats {
            return Err(LedgerError::InvalidOperation(format!(
                "Cannot release {} sats, only {} held",
                amount_sats, self.held_sats
            )));
        }
        self.held_sats -= amount_sats;
        self.available_sats += amount_sats;
        self.last_updated = Utc::now();
        Ok(())
    }

    /// Apply a ledger entry to this balance
    pub fn apply_entry(&mut self, entry: &LedgerEntry) -> Result<(), LedgerError> {
        match entry.operation {
            LedgerOperation::Credit => self.credit(entry.amounts.net_sats),
            LedgerOperation::Debit => self.debit(entry.amounts.net_sats)?,
            LedgerOperation::Hold => self.hold(entry.amounts.net_sats)?,
            LedgerOperation::Release => self.release(entry.amounts.net_sats)?,
            LedgerOperation::Refund => self.credit(entry.amounts.net_sats),
        }
        Ok(())
    }
}

/// Ledger filters for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerFilters {
    /// Filter by entry type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_type: Option<LedgerEntryType>,

    /// Filter by direction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,

    /// Filter by date range start
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_date: Option<DateTime<Utc>>,

    /// Filter by date range end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_date: Option<DateTime<Utc>>,

    /// Limit number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl LedgerFilters {
    /// Create empty filters
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by entry type
    pub fn with_entry_type(mut self, entry_type: LedgerEntryType) -> Self {
        self.entry_type = Some(entry_type);
        self
    }

    /// Filter by direction
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Filter by date range
    pub fn with_date_range(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.from_date = Some(from);
        self.to_date = Some(to);
        self
    }

    /// Set result limit
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if an entry matches these filters
    pub fn matches(&self, entry: &LedgerEntry) -> bool {
        if let Some(entry_type) = self.entry_type {
            if entry.entry_type != entry_type {
                return false;
            }
        }

        if let Some(direction) = self.direction {
            if entry.direction != direction {
                return false;
            }
        }

        if let Some(from) = self.from_date {
            if entry.timestamp < from {
                return false;
            }
        }

        if let Some(to) = self.to_date {
            if entry.timestamp > to {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ledger_entry_type_description() {
        assert_eq!(
            LedgerEntryType::SkillPayment.description(),
            "Payment for skill usage"
        );
        assert_eq!(
            LedgerEntryType::ComputePayment.description(),
            "Payment for compute job"
        );
    }

    #[test]
    fn test_ledger_operation_checks() {
        assert!(LedgerOperation::Credit.increases_available());
        assert!(LedgerOperation::Release.increases_available());
        assert!(LedgerOperation::Debit.decreases_available());
        assert!(LedgerOperation::Hold.decreases_available());
    }

    #[test]
    fn test_ledger_amounts() {
        let amounts = LedgerAmounts::new(10000, 500);
        assert_eq!(amounts.gross_sats, 10000);
        assert_eq!(amounts.platform_fee_sats, 500);
        assert_eq!(amounts.net_sats, 9500);
        assert!(amounts.validate().is_ok());

        let from_rate = LedgerAmounts::from_gross_with_rate(10000, 0.05);
        assert_eq!(from_rate.platform_fee_sats, 500);
        assert_eq!(from_rate.net_sats, 9500);
    }

    #[test]
    fn test_ledger_parties() {
        let parties = LedgerParties::new("payer1", "payee1")
            .with_intermediary("platform")
            .with_intermediary("coalition");

        assert_eq!(parties.payer, "payer1");
        assert_eq!(parties.payee, "payee1");
        assert_eq!(parties.intermediaries.len(), 2);
    }

    #[test]
    fn test_ledger_references() {
        let refs = LedgerReferences::new()
            .with_job_id("job123")
            .with_invoice_id("inv456")
            .with_tx_hash("tx789");

        assert_eq!(refs.job_id.as_deref(), Some("job123"));
        assert_eq!(refs.invoice_id.as_deref(), Some("inv456"));
        assert_eq!(refs.tx_hash.as_deref(), Some("tx789"));
    }

    #[test]
    fn test_ledger_entry_creation() {
        let amounts = LedgerAmounts::new(10000, 500);
        let parties = LedgerParties::new("payer1", "payee1");
        let refs = LedgerReferences::new().with_job_id("job123");

        let entry = LedgerEntry::new(
            "entry1",
            LedgerEntryType::ComputePayment,
            Direction::Outbound,
            LedgerOperation::Debit,
            amounts,
            parties,
            refs,
            "genesis",
        )
        .unwrap()
        .finalize();

        assert_eq!(entry.id, "entry1");
        assert!(!entry.entry_hash.is_empty());
    }

    #[test]
    fn test_ledger_entry_hash_verification() {
        let amounts = LedgerAmounts::new(10000, 500);
        let parties = LedgerParties::new("payer1", "payee1");
        let refs = LedgerReferences::new();

        let entry = LedgerEntry::new(
            "entry1",
            LedgerEntryType::SkillPayment,
            Direction::Inbound,
            LedgerOperation::Credit,
            amounts,
            parties,
            refs,
            "genesis",
        )
        .unwrap()
        .finalize();

        assert!(entry.verify_hash().is_ok());
    }

    #[test]
    fn test_balance_operations() {
        let mut balance = Balance::new("account1");

        // Credit
        balance.credit(10000);
        assert_eq!(balance.available_sats, 10000);
        assert_eq!(balance.total_sats, 10000);

        // Hold
        balance.hold(3000).unwrap();
        assert_eq!(balance.available_sats, 7000);
        assert_eq!(balance.held_sats, 3000);
        assert_eq!(balance.total_sats, 10000);

        // Release
        balance.release(1000).unwrap();
        assert_eq!(balance.available_sats, 8000);
        assert_eq!(balance.held_sats, 2000);

        // Debit
        balance.debit(5000).unwrap();
        assert_eq!(balance.available_sats, 3000);
        assert_eq!(balance.total_sats, 5000);
    }

    #[test]
    fn test_balance_insufficient_funds() {
        let mut balance = Balance::new("account1");
        balance.credit(1000);

        assert!(balance.debit(2000).is_err());
        assert!(balance.hold(2000).is_err());
    }

    #[test]
    fn test_balance_apply_entry() {
        let mut balance = Balance::new("account1");

        let amounts = LedgerAmounts::new(10000, 0);
        let parties = LedgerParties::new("payer", "account1");
        let refs = LedgerReferences::new();

        let credit_entry = LedgerEntry::new(
            "entry1",
            LedgerEntryType::TopUp,
            Direction::Inbound,
            LedgerOperation::Credit,
            amounts,
            parties,
            refs,
            "genesis",
        )
        .unwrap()
        .finalize();

        balance.apply_entry(&credit_entry).unwrap();
        assert_eq!(balance.available_sats, 10000);
    }

    #[test]
    fn test_ledger_filters() {
        let filters = LedgerFilters::new()
            .with_entry_type(LedgerEntryType::ComputePayment)
            .with_direction(Direction::Outbound)
            .with_limit(10);

        assert_eq!(filters.entry_type, Some(LedgerEntryType::ComputePayment));
        assert_eq!(filters.direction, Some(Direction::Outbound));
        assert_eq!(filters.limit, Some(10));
    }

    #[test]
    fn test_ledger_filters_matching() {
        let filters = LedgerFilters::new()
            .with_entry_type(LedgerEntryType::ComputePayment)
            .with_direction(Direction::Outbound);

        let amounts = LedgerAmounts::new(10000, 0);
        let parties = LedgerParties::new("payer", "payee");
        let refs = LedgerReferences::new();

        let matching_entry = LedgerEntry::new(
            "entry1",
            LedgerEntryType::ComputePayment,
            Direction::Outbound,
            LedgerOperation::Debit,
            amounts.clone(),
            parties.clone(),
            refs.clone(),
            "genesis",
        )
        .unwrap()
        .finalize();

        assert!(filters.matches(&matching_entry));

        let non_matching = LedgerEntry::new(
            "entry2",
            LedgerEntryType::SkillPayment,
            Direction::Inbound,
            LedgerOperation::Credit,
            amounts,
            parties,
            refs,
            "genesis",
        )
        .unwrap()
        .finalize();

        assert!(!filters.matches(&non_matching));
    }

    #[test]
    fn test_ledger_entry_serde() {
        let amounts = LedgerAmounts::new(10000, 500);
        let parties = LedgerParties::new("payer1", "payee1");
        let refs = LedgerReferences::new();

        let entry = LedgerEntry::new(
            "entry1",
            LedgerEntryType::ComputePayment,
            Direction::Outbound,
            LedgerOperation::Debit,
            amounts,
            parties,
            refs,
            "genesis",
        )
        .unwrap()
        .finalize();

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LedgerEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.id, deserialized.id);
        assert_eq!(entry.entry_hash, deserialized.entry_hash);
    }
}
