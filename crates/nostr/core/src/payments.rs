//! Lightning Network payment primitives for the Nostr marketplace.
//!
//! This module provides types for managing Lightning payments, invoices,
//! and multi-party payment splits in the marketplace ecosystem.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with payments
#[derive(Error, Debug)]
pub enum PaymentError {
    /// Invalid BOLT11 invoice format
    #[error("invalid BOLT11 invoice: {0}")]
    InvalidBolt11(String),

    /// Invalid Lightning address format
    #[error("invalid lightning address: {0}")]
    InvalidLightningAddress(String),

    /// Invalid payment amount
    #[error("invalid payment amount: {0}")]
    InvalidAmount(String),

    /// Payment splits don't sum to total
    #[error("payment splits ({0} sats) don't match total ({1} sats)")]
    SplitMismatch(u64, u64),

    /// Invalid payment destination
    #[error("invalid payment destination: {0}")]
    InvalidDestination(String),
}

/// Status of a Lightning invoice
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    /// Invoice created but not yet paid
    Pending,
    /// Payment in transit
    Paid,
    /// Payment received, being settled
    Settling,
    /// Payment successfully settled
    Settled,
    /// Invoice expired without payment
    Expired,
    /// Payment failed
    Failed,
    /// Payment was refunded
    Refunded,
}

impl InvoiceStatus {
    /// Check if this is a final status (no further changes)
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            InvoiceStatus::Settled
                | InvoiceStatus::Expired
                | InvoiceStatus::Failed
                | InvoiceStatus::Refunded
        )
    }

    /// Check if payment was successful
    pub fn is_successful(&self) -> bool {
        matches!(self, InvoiceStatus::Settled)
    }
}

/// A Lightning Network invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningInvoice {
    /// Unique identifier for this invoice
    pub id: String,

    /// BOLT11 payment request string
    pub bolt11: String,

    /// Amount in satoshis
    pub amount_sats: u64,

    /// Optional memo/description
    pub memo: String,

    /// Current status of the invoice
    pub status: InvoiceStatus,

    /// When this invoice expires
    pub expires_at: DateTime<Utc>,

    /// When the invoice was paid (if paid)
    pub paid_at: Option<DateTime<Utc>>,

    /// Payment preimage (revealed when paid)
    pub preimage: Option<String>,
}

impl LightningInvoice {
    /// Create a new pending invoice
    pub fn new(
        id: impl Into<String>,
        bolt11: impl Into<String>,
        amount_sats: u64,
        memo: impl Into<String>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.into(),
            bolt11: bolt11.into(),
            amount_sats,
            memo: memo.into(),
            status: InvoiceStatus::Pending,
            expires_at,
            paid_at: None,
            preimage: None,
        }
    }

    /// Mark the invoice as paid with preimage
    pub fn mark_paid(&mut self, preimage: impl Into<String>) {
        self.status = InvoiceStatus::Paid;
        self.paid_at = Some(Utc::now());
        self.preimage = Some(preimage.into());
    }

    /// Mark the invoice as settled
    pub fn mark_settled(&mut self) {
        self.status = InvoiceStatus::Settled;
    }

    /// Mark the invoice as expired
    pub fn mark_expired(&mut self) {
        self.status = InvoiceStatus::Expired;
    }

    /// Mark the invoice as failed
    pub fn mark_failed(&mut self) {
        self.status = InvoiceStatus::Failed;
    }

    /// Check if this invoice has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Destination for a Lightning payment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentDestination {
    /// Lightning address (user@domain.com)
    LightningAddress { address: String },

    /// BOLT11 invoice string
    Bolt11 { invoice: String },

    /// Keysend direct to node pubkey
    Keysend { pubkey: String },
}

impl PaymentDestination {
    /// Create a Lightning address destination
    pub fn lightning_address(address: impl Into<String>) -> Result<Self, PaymentError> {
        let address = address.into();

        // Basic validation - should be email-like
        if !address.contains('@') || address.split('@').count() != 2 {
            return Err(PaymentError::InvalidLightningAddress(address));
        }

        Ok(Self::LightningAddress { address })
    }

    /// Create a BOLT11 invoice destination
    pub fn bolt11(invoice: impl Into<String>) -> Result<Self, PaymentError> {
        let invoice = invoice.into();

        // Basic validation - should start with ln
        if !invoice.to_lowercase().starts_with("ln") {
            return Err(PaymentError::InvalidBolt11(invoice));
        }

        Ok(Self::Bolt11 { invoice })
    }

    /// Create a keysend destination
    pub fn keysend(pubkey: impl Into<String>) -> Result<Self, PaymentError> {
        let pubkey = pubkey.into();

        // Basic validation - should be 66 hex chars (33 bytes)
        if pubkey.len() != 66 || !pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(PaymentError::InvalidDestination(format!(
                "invalid pubkey: {}",
                pubkey
            )));
        }

        Ok(Self::Keysend { pubkey })
    }
}

/// Request to send a Lightning payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    /// Where to send the payment
    pub destination: PaymentDestination,

    /// Amount to send in satoshis
    pub amount_sats: u64,

    /// Maximum fee willing to pay in satoshis
    pub max_fee_sats: u64,
}

impl PaymentRequest {
    /// Create a new payment request
    pub fn new(
        destination: PaymentDestination,
        amount_sats: u64,
        max_fee_sats: u64,
    ) -> Result<Self, PaymentError> {
        if amount_sats == 0 {
            return Err(PaymentError::InvalidAmount(
                "amount must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            destination,
            amount_sats,
            max_fee_sats,
        })
    }

    /// Calculate the maximum total cost (amount + fee)
    pub fn max_total_sats(&self) -> u64 {
        self.amount_sats.saturating_add(self.max_fee_sats)
    }
}

/// Result of a Lightning payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    /// Whether the payment succeeded
    pub success: bool,

    /// Payment preimage (proof of payment)
    pub preimage: Option<String>,

    /// Actual fee paid in satoshis
    pub fee_sats: u64,

    /// Error message if payment failed
    pub error: Option<String>,
}

impl PaymentResult {
    /// Create a successful payment result
    pub fn success(preimage: impl Into<String>, fee_sats: u64) -> Self {
        Self {
            success: true,
            preimage: Some(preimage.into()),
            fee_sats,
            error: None,
        }
    }

    /// Create a failed payment result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            preimage: None,
            fee_sats: 0,
            error: Some(error.into()),
        }
    }
}

/// A single payment split in a multi-party payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentSplit {
    /// Who receives this payment
    pub recipient: PaymentDestination,

    /// Amount to send to this recipient in satoshis
    pub amount_sats: u64,

    /// Purpose of this split (e.g., "creator", "compute", "platform")
    pub purpose: String,
}

impl PaymentSplit {
    /// Create a new payment split
    pub fn new(
        recipient: PaymentDestination,
        amount_sats: u64,
        purpose: impl Into<String>,
    ) -> Self {
        Self {
            recipient,
            amount_sats,
            purpose: purpose.into(),
        }
    }
}

/// Multi-party payment with automatic splits (Coalition/MPP)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoalitionPayment {
    /// Total payment amount in satoshis
    pub total_sats: u64,

    /// How to split the payment among recipients
    pub splits: Vec<PaymentSplit>,
}

impl CoalitionPayment {
    /// Create a new coalition payment
    pub fn new(total_sats: u64, splits: Vec<PaymentSplit>) -> Result<Self, PaymentError> {
        // Validate that splits sum to total
        let splits_sum: u64 = splits.iter().map(|s| s.amount_sats).sum();

        if splits_sum != total_sats {
            return Err(PaymentError::SplitMismatch(splits_sum, total_sats));
        }

        Ok(Self { total_sats, splits })
    }

    /// Add a new split to the payment
    pub fn add_split(&mut self, split: PaymentSplit) -> Result<(), PaymentError> {
        self.splits.push(split);

        // Recalculate total
        let new_total: u64 = self.splits.iter().map(|s| s.amount_sats).sum();
        self.total_sats = new_total;

        Ok(())
    }

    /// Get splits for a specific purpose
    pub fn splits_for_purpose(&self, purpose: &str) -> Vec<&PaymentSplit> {
        self.splits
            .iter()
            .filter(|s| s.purpose == purpose)
            .collect()
    }

    /// Calculate percentage for a specific purpose
    pub fn purpose_percentage(&self, purpose: &str) -> f64 {
        if self.total_sats == 0 {
            return 0.0;
        }

        let purpose_total: u64 = self
            .splits_for_purpose(purpose)
            .iter()
            .map(|s| s.amount_sats)
            .sum();

        (purpose_total as f64 / self.total_sats as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_invoice_status() {
        assert!(InvoiceStatus::Settled.is_final());
        assert!(InvoiceStatus::Expired.is_final());
        assert!(!InvoiceStatus::Pending.is_final());

        assert!(InvoiceStatus::Settled.is_successful());
        assert!(!InvoiceStatus::Failed.is_successful());
    }

    #[test]
    fn test_lightning_invoice() {
        let expires = Utc::now() + Duration::hours(1);
        let mut invoice =
            LightningInvoice::new("inv_123", "lnbc1...", 1000, "Test payment", expires);

        assert_eq!(invoice.status, InvoiceStatus::Pending);
        assert!(!invoice.is_expired());

        invoice.mark_paid("preimage_abc");
        assert_eq!(invoice.status, InvoiceStatus::Paid);
        assert!(invoice.paid_at.is_some());
        assert_eq!(invoice.preimage.as_ref().unwrap(), "preimage_abc");

        invoice.mark_settled();
        assert_eq!(invoice.status, InvoiceStatus::Settled);
    }

    #[test]
    fn test_payment_destination() {
        let dest = PaymentDestination::lightning_address("user@domain.com").unwrap();
        assert!(matches!(dest, PaymentDestination::LightningAddress { .. }));

        let dest = PaymentDestination::bolt11("lnbc1234").unwrap();
        assert!(matches!(dest, PaymentDestination::Bolt11 { .. }));

        let pubkey = "02".to_string() + &"a".repeat(64);
        let dest = PaymentDestination::keysend(&pubkey).unwrap();
        assert!(matches!(dest, PaymentDestination::Keysend { .. }));
    }

    #[test]
    fn test_invalid_destinations() {
        assert!(PaymentDestination::lightning_address("invalid").is_err());
        assert!(PaymentDestination::bolt11("invalid").is_err());
        assert!(PaymentDestination::keysend("invalid").is_err());
    }

    #[test]
    fn test_payment_request() {
        let dest = PaymentDestination::lightning_address("user@domain.com").unwrap();
        let request = PaymentRequest::new(dest, 1000, 100).unwrap();

        assert_eq!(request.amount_sats, 1000);
        assert_eq!(request.max_fee_sats, 100);
        assert_eq!(request.max_total_sats(), 1100);
    }

    #[test]
    fn test_payment_request_zero_amount() {
        let dest = PaymentDestination::lightning_address("user@domain.com").unwrap();
        assert!(PaymentRequest::new(dest, 0, 100).is_err());
    }

    #[test]
    fn test_payment_result() {
        let success = PaymentResult::success("preimage", 10);
        assert!(success.success);
        assert_eq!(success.fee_sats, 10);
        assert_eq!(success.preimage.unwrap(), "preimage");

        let failure = PaymentResult::failure("insufficient funds");
        assert!(!failure.success);
        assert_eq!(failure.error.unwrap(), "insufficient funds");
    }

    #[test]
    fn test_coalition_payment() {
        let dest1 = PaymentDestination::lightning_address("creator@domain.com").unwrap();
        let dest2 = PaymentDestination::lightning_address("platform@domain.com").unwrap();

        let splits = vec![
            PaymentSplit::new(dest1, 800, "creator"),
            PaymentSplit::new(dest2, 200, "platform"),
        ];

        let payment = CoalitionPayment::new(1000, splits).unwrap();

        assert_eq!(payment.total_sats, 1000);
        assert_eq!(payment.splits.len(), 2);
        assert_eq!(payment.purpose_percentage("creator"), 80.0);
        assert_eq!(payment.purpose_percentage("platform"), 20.0);
    }

    #[test]
    fn test_coalition_payment_mismatch() {
        let dest = PaymentDestination::lightning_address("user@domain.com").unwrap();
        let splits = vec![PaymentSplit::new(dest, 900, "creator")];

        // Splits sum to 900 but total is 1000
        assert!(CoalitionPayment::new(1000, splits).is_err());
    }

    #[test]
    fn test_coalition_add_split() {
        let dest1 = PaymentDestination::lightning_address("creator@domain.com").unwrap();
        let splits = vec![PaymentSplit::new(dest1, 800, "creator")];

        let mut payment = CoalitionPayment::new(800, splits).unwrap();

        let dest2 = PaymentDestination::lightning_address("platform@domain.com").unwrap();
        payment
            .add_split(PaymentSplit::new(dest2, 200, "platform"))
            .unwrap();

        assert_eq!(payment.total_sats, 1000);
        assert_eq!(payment.splits.len(), 2);
    }

    #[test]
    fn test_splits_for_purpose() {
        let dest1 = PaymentDestination::lightning_address("creator1@domain.com").unwrap();
        let dest2 = PaymentDestination::lightning_address("creator2@domain.com").unwrap();
        let dest3 = PaymentDestination::lightning_address("platform@domain.com").unwrap();

        let splits = vec![
            PaymentSplit::new(dest1, 400, "creator"),
            PaymentSplit::new(dest2, 400, "creator"),
            PaymentSplit::new(dest3, 200, "platform"),
        ];

        let payment = CoalitionPayment::new(1000, splits).unwrap();

        let creator_splits = payment.splits_for_purpose("creator");
        assert_eq!(creator_splits.len(), 2);

        let platform_splits = payment.splits_for_purpose("platform");
        assert_eq!(platform_splits.len(), 1);
    }
}
