////! Lightning payments integration for marketplace
//!
//! Provides Bitcoin Lightning payment flows for compute jobs, skill purchases,
//! and data transactions. Integrates with Spark SDK (d-001) when available.
//!
//! # Payment Flows
//!
//! ## Compute Job Payment
//! 1. Provider sends payment-required feedback with invoice
//! 2. Consumer pays invoice (hold invoice for escrow)
//! 3. Provider delivers result
//! 4. Consumer verifies preimage and releases payment
//!
//! ## Skill Purchase Payment
//! 1. Browse skills and select for installation
//! 2. Pay license fee via Lightning invoice
//! 3. Verify payment and receive encrypted skill delivery
//!
//! ## Data Access Payment
//! 1. Select dataset to purchase
//! 2. Pay access fee via Lightning invoice
//! 3. Receive decryption key for NIP-44 encrypted data

use anyhow::Result;
use serde::{Deserialize, Serialize};

// TEMP: When Spark SDK is available, uncomment:
// use openagents_spark::{SparkWallet, SendPaymentResponse, ReceivePaymentResponse};

/// Payment status for tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentStatus {
    /// Payment not yet initiated
    Pending,
    /// Payment in flight (hold invoice)
    InFlight,
    /// Payment completed successfully
    Completed,
    /// Payment failed
    Failed,
    /// Payment cancelled
    Cancelled,
}

/// Payment record for marketplace transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    /// Unique payment ID
    pub id: String,
    /// Payment type (compute, skill, data)
    pub payment_type: PaymentType,
    /// Associated item ID (job_id, skill_id, dataset_id)
    pub item_id: String,
    /// Amount in millisatoshis
    pub amount_msats: u64,
    /// BOLT-11 invoice
    pub invoice: String,
    /// Payment hash
    pub payment_hash: Option<String>,
    /// Payment preimage (proof of payment)
    pub preimage: Option<String>,
    /// Current status
    pub status: PaymentStatus,
    /// Created timestamp (unix seconds)
    pub created_at: u64,
    /// Completed timestamp (unix seconds)
    pub completed_at: Option<u64>,
}

/// Type of marketplace payment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentType {
    /// Compute job payment
    Compute,
    /// Skill license payment
    Skill,
    /// Data access payment
    Data,
    /// Trajectory contribution reward
    Trajectory,
}

impl PaymentRecord {
    /// Create a new payment record
    pub fn new(
        payment_type: PaymentType,
        item_id: String,
        amount_msats: u64,
        invoice: String,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            payment_type,
            item_id,
            amount_msats,
            invoice,
            payment_hash: None,
            preimage: None,
            status: PaymentStatus::Pending,
            created_at,
            completed_at: None,
        }
    }

    /// Mark payment as in-flight
    pub fn mark_in_flight(&mut self, payment_hash: String) {
        self.payment_hash = Some(payment_hash);
        self.status = PaymentStatus::InFlight;
    }

    /// Mark payment as completed
    pub fn mark_completed(&mut self, preimage: String) {
        self.preimage = Some(preimage);
        self.status = PaymentStatus::Completed;
        self.completed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    /// Mark payment as failed
    pub fn mark_failed(&mut self) {
        self.status = PaymentStatus::Failed;
        self.completed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }
}

/// Payment manager for marketplace transactions
pub struct PaymentManager {
    // TEMP: When Spark SDK is available, add:
    // wallet: Arc<SparkWallet>,
}

impl PaymentManager {
    /// Create a new payment manager
    ///
    /// # Arguments
    /// * `wallet` - Spark wallet instance for Lightning payments
    ///
    /// # Note
    /// Currently returns a stub until Spark SDK integration is complete.
    /// See d-001 for Spark SDK integration status.
    pub fn new(/* wallet: Arc<SparkWallet> */) -> Self {
        Self {
            // wallet,
        }
    }

    /// Pay a Lightning invoice for a compute job
    ///
    /// # Arguments
    /// * `job_id` - The compute job ID
    /// * `invoice` - BOLT-11 Lightning invoice from provider
    /// * `amount_msats` - Amount in millisatoshis (optional, extracted from invoice if not provided)
    ///
    /// # Returns
    /// Payment record with status and preimage
    ///
    /// # Errors
    /// Returns error if:
    /// - Spark SDK is not available (current state)
    /// - Invoice is invalid
    /// - Insufficient balance
    /// - Payment fails
    pub async fn pay_compute_job(
        &self,
        job_id: &str,
        invoice: &str,
        amount_msats: Option<u64>,
    ) -> Result<PaymentRecord> {
        // Extract amount from invoice if not provided
        let amount = amount_msats.unwrap_or_else(|| {
            // TODO: Parse invoice to extract amount
            // For now, require explicit amount
            0
        });

        let _payment = PaymentRecord::new(
            PaymentType::Compute,
            job_id.to_string(),
            amount,
            invoice.to_string(),
        );

        // TEMP: Until Spark SDK is available, return NotImplemented error
        // When Spark SDK is ready, replace with:
        /*
        let response = self.wallet
            .send_payment_simple(invoice, amount_msats)
            .await
            .context("Failed to send Lightning payment")?;

        payment.mark_in_flight(response.payment.id.clone());

        // Wait for payment completion
        // Note: In real implementation, this would poll payment status
        if response.payment.status == PaymentStatus::Complete {
            payment.mark_completed(response.preimage);
        } else {
            payment.mark_failed();
        }
        */

        Err(anyhow::anyhow!(
            "Spark SDK integration incomplete - see d-001 directive. \
             Breez SDK is commented out in crates/spark/Cargo.toml. \
             Payment functionality requires completing Spark SDK integration first."
        ))
    }

    /// Pay for skill license
    ///
    /// # Arguments
    /// * `skill_id` - The skill identifier
    /// * `invoice` - BOLT-11 Lightning invoice from skill creator
    /// * `amount_msats` - License fee in millisatoshis
    ///
    /// # Returns
    /// Payment record with preimage for skill delivery verification
    pub async fn pay_skill_license(
        &self,
        skill_id: &str,
        invoice: &str,
        amount_msats: u64,
    ) -> Result<PaymentRecord> {
        let _payment = PaymentRecord::new(
            PaymentType::Skill,
            skill_id.to_string(),
            amount_msats,
            invoice.to_string(),
        );

        // TEMP: Until Spark SDK is available
        Err(anyhow::anyhow!(
            "Spark SDK integration incomplete - see d-001 directive"
        ))

        // When Spark SDK is ready:
        // Same flow as pay_compute_job
    }

    /// Pay for data access
    ///
    /// # Arguments
    /// * `dataset_id` - The dataset identifier
    /// * `invoice` - BOLT-11 Lightning invoice from data provider
    /// * `amount_msats` - Access fee in millisatoshis
    ///
    /// # Returns
    /// Payment record with preimage for decryption key delivery
    pub async fn pay_data_access(
        &self,
        dataset_id: &str,
        invoice: &str,
        amount_msats: u64,
    ) -> Result<PaymentRecord> {
        let _payment = PaymentRecord::new(
            PaymentType::Data,
            dataset_id.to_string(),
            amount_msats,
            invoice.to_string(),
        );

        // TEMP: Until Spark SDK is available
        Err(anyhow::anyhow!(
            "Spark SDK integration incomplete - see d-001 directive"
        ))

        // When Spark SDK is ready:
        // Same flow as pay_compute_job
    }

    /// Create an invoice to receive payment
    ///
    /// # Arguments
    /// * `amount_msats` - Amount to receive in millisatoshis
    /// * `description` - Payment description
    ///
    /// # Returns
    /// BOLT-11 invoice string
    ///
    /// # Errors
    /// Returns error if Spark SDK is not available or invoice creation fails
    pub async fn create_invoice(
        &self,
        _amount_msats: u64,
        _description: &str,
    ) -> Result<String> {
        // TEMP: Until Spark SDK is available
        Err(anyhow::anyhow!(
            "Spark SDK integration incomplete - see d-001 directive"
        ))

        // When Spark SDK is ready:
        /*
        let response = self.wallet
            .create_invoice(amount_msats, Some(description.to_string()), None)
            .await
            .context("Failed to create Lightning invoice")?;

        Ok(response.payment_request)
        */
    }

    /// Verify a payment preimage matches the payment hash
    ///
    /// This is critical for hold invoice flows where payment is only released
    /// after verifying the provider delivered the result.
    ///
    /// # Arguments
    /// * `payment_hash` - The payment hash from the invoice
    /// * `preimage` - The preimage provided by the payee
    ///
    /// # Returns
    /// true if preimage is valid for the given hash
    pub fn verify_preimage(&self, _payment_hash: &str, _preimage: &str) -> bool {
        // TODO: Implement actual preimage verification
        // sha256(preimage) == payment_hash
        false
    }

    /// Check payment status
    ///
    /// # Arguments
    /// * `payment_id` - The payment ID to check
    ///
    /// # Returns
    /// Current payment status
    pub async fn get_payment_status(&self, _payment_id: &str) -> Result<PaymentStatus> {
        // TEMP: Until Spark SDK is available
        Err(anyhow::anyhow!(
            "Spark SDK integration incomplete - see d-001 directive"
        ))

        // When Spark SDK is ready:
        /*
        let payment = self.wallet
            .get_payment(payment_id)
            .await
            .context("Failed to get payment status")?;

        Ok(payment.status)
        */
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_record_creation() {
        let payment = PaymentRecord::new(
            PaymentType::Compute,
            "job-123".to_string(),
            10000,
            "lnbc100u1...".to_string(),
        );

        assert_eq!(payment.payment_type, PaymentType::Compute);
        assert_eq!(payment.item_id, "job-123");
        assert_eq!(payment.amount_msats, 10000);
        assert_eq!(payment.status, PaymentStatus::Pending);
        assert!(payment.preimage.is_none());
    }

    #[test]
    fn test_payment_lifecycle() {
        let mut payment = PaymentRecord::new(
            PaymentType::Skill,
            "skill-456".to_string(),
            50000,
            "lnbc500u1...".to_string(),
        );

        // Mark in-flight
        payment.mark_in_flight("abc123".to_string());
        assert_eq!(payment.status, PaymentStatus::InFlight);
        assert_eq!(payment.payment_hash.as_deref(), Some("abc123"));

        // Mark completed
        payment.mark_completed("preimage123".to_string());
        assert_eq!(payment.status, PaymentStatus::Completed);
        assert_eq!(payment.preimage.as_deref(), Some("preimage123"));
        assert!(payment.completed_at.is_some());
    }

    #[test]
    fn test_payment_failure() {
        let mut payment = PaymentRecord::new(
            PaymentType::Data,
            "dataset-789".to_string(),
            25000,
            "lnbc250u1...".to_string(),
        );

        payment.mark_failed();
        assert_eq!(payment.status, PaymentStatus::Failed);
        assert!(payment.completed_at.is_some());
        assert!(payment.preimage.is_none());
    }

    #[tokio::test]
    async fn test_payment_manager_returns_not_implemented() {
        let manager = PaymentManager::new();

        // All payment methods should return NotImplemented until Spark SDK is ready
        let result = manager
            .pay_compute_job("job-1", "lnbc...", Some(1000))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Spark SDK"));

        let result = manager.create_invoice(1000, "test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Spark SDK"));
    }
}
