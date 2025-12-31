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
use openagents_spark::{PaymentDetails, SparkHtlcStatus, SparkWallet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

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

/// Revenue split payout recipient
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevenueRecipient {
    Creator,
    Compute,
    Platform,
    Referrer,
}

/// Invoices used for revenue split distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSplitInvoices {
    pub creator_invoice: String,
    pub compute_invoice: String,
    pub platform_invoice: Option<String>,
    pub referrer_invoice: Option<String>,
}

/// Distribution record for a revenue split payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSplitPayment {
    pub recipient: RevenueRecipient,
    pub payment: PaymentRecord,
}

/// Revenue split distribution outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueDistributionResult {
    pub split: super::revenue::RevenueSplit,
    pub payments: Vec<RevenueSplitPayment>,
    pub retained_platform_sats: u64,
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
    wallet: Option<Arc<SparkWallet>>,
}

impl PaymentManager {
    /// Create a new payment manager
    ///
    /// # Arguments
    /// * `wallet` - Optional Spark wallet instance for Lightning payments
    ///
    /// # Note
    /// If wallet is None, payment methods will return errors indicating
    /// Spark SDK is not configured. See d-001 for Spark SDK integration status.
    pub fn new(wallet: Option<Arc<SparkWallet>>) -> Self {
        Self { wallet }
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
        let wallet = self.wallet.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Spark wallet not configured. Initialize SparkWallet and pass to PaymentManager::new(). \
                 See d-001 directive for Spark SDK integration status."
            )
        })?;

        // Extract amount from invoice if not provided
        let amount =
            amount_msats.unwrap_or_else(|| Self::parse_invoice_amount(invoice).unwrap_or(0));
        let send_amount = amount_msats.map(msats_to_sats).transpose()?;

        let mut payment = PaymentRecord::new(
            PaymentType::Compute,
            job_id.to_string(),
            amount,
            invoice.to_string(),
        );

        // Send payment via Spark wallet
        let response = wallet
            .send_payment_simple(invoice, send_amount)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send Lightning payment: {}", e))?;

        payment.mark_in_flight(response.payment.id.clone());

        // Check payment status
        // Note: In production, this should poll until completion or timeout
        if response.payment.status == openagents_spark::PaymentStatus::Completed {
            let preimage =
                payment_preimage(&response.payment).unwrap_or_else(|| response.payment.id.clone());
            payment.mark_completed(preimage);
        } else {
            payment.mark_failed();
        }

        Ok(payment)
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
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        let mut payment = PaymentRecord::new(
            PaymentType::Skill,
            skill_id.to_string(),
            amount_msats,
            invoice.to_string(),
        );

        let send_amount = msats_to_sats(amount_msats)?;
        let response = wallet
            .send_payment_simple(invoice, Some(send_amount))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send skill license payment: {}", e))?;

        payment.mark_in_flight(response.payment.id.clone());

        if response.payment.status == openagents_spark::PaymentStatus::Completed {
            let preimage =
                payment_preimage(&response.payment).unwrap_or_else(|| response.payment.id.clone());
            payment.mark_completed(preimage);
        } else {
            payment.mark_failed();
        }

        Ok(payment)
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
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        let mut payment = PaymentRecord::new(
            PaymentType::Data,
            dataset_id.to_string(),
            amount_msats,
            invoice.to_string(),
        );

        let send_amount = msats_to_sats(amount_msats)?;
        let response = wallet
            .send_payment_simple(invoice, Some(send_amount))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send data access payment: {}", e))?;

        payment.mark_in_flight(response.payment.id.clone());

        if response.payment.status == openagents_spark::PaymentStatus::Completed {
            let preimage =
                payment_preimage(&response.payment).unwrap_or_else(|| response.payment.id.clone());
            payment.mark_completed(preimage);
        } else {
            payment.mark_failed();
        }

        Ok(payment)
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
    pub async fn create_invoice(&self, amount_msats: u64, description: &str) -> Result<String> {
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        let amount_sats = msats_to_sats(amount_msats)?;
        let response = wallet
            .create_invoice(amount_sats, Some(description.to_string()), None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Lightning invoice: {}", e))?;

        Ok(response.payment_request)
    }

    /// Distribute revenue split payments to recipients.
    pub async fn distribute_revenue(
        &self,
        payment_type: PaymentType,
        item_id: &str,
        split: super::revenue::RevenueSplit,
        invoices: RevenueSplitInvoices,
    ) -> Result<RevenueDistributionResult> {
        split.verify()?;
        validate_split_invoices(&split, &invoices)?;

        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        let mut payments = Vec::new();

        if split.creator_sats > 0 {
            let payment = send_split_payment(
                wallet,
                payment_type.clone(),
                item_id,
                &invoices.creator_invoice,
                split.creator_sats,
            )
            .await?;
            payments.push(RevenueSplitPayment {
                recipient: RevenueRecipient::Creator,
                payment,
            });
        }

        if split.compute_sats > 0 {
            let payment = send_split_payment(
                wallet,
                payment_type.clone(),
                item_id,
                &invoices.compute_invoice,
                split.compute_sats,
            )
            .await?;
            payments.push(RevenueSplitPayment {
                recipient: RevenueRecipient::Compute,
                payment,
            });
        }

        let mut retained_platform_sats = split.platform_sats;
        if split.platform_sats > 0 {
            if let Some(platform_invoice) = invoices.platform_invoice.as_deref() {
                let payment = send_split_payment(
                    wallet,
                    payment_type.clone(),
                    item_id,
                    platform_invoice,
                    split.platform_sats,
                )
                .await?;
                payments.push(RevenueSplitPayment {
                    recipient: RevenueRecipient::Platform,
                    payment,
                });
                retained_platform_sats = 0;
            }
        }

        if split.referrer_sats > 0 {
            let referrer_invoice = invoices
                .referrer_invoice
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Referrer invoice required for split payout"))?;
            let payment = send_split_payment(
                wallet,
                payment_type.clone(),
                item_id,
                referrer_invoice,
                split.referrer_sats,
            )
            .await?;
            payments.push(RevenueSplitPayment {
                recipient: RevenueRecipient::Referrer,
                payment,
            });
        }

        Ok(RevenueDistributionResult {
            split,
            payments,
            retained_platform_sats,
        })
    }

    /// Verify a payment preimage matches the payment hash
    ///
    /// This is critical for hold invoice flows where payment is only released
    /// after verifying the provider delivered the result.
    ///
    /// # Arguments
    /// * `payment_hash` - The payment hash from the invoice (hex encoded)
    /// * `preimage` - The preimage provided by the payee (hex encoded)
    ///
    /// # Returns
    /// true if preimage is valid for the given hash
    pub fn verify_preimage(&self, payment_hash: &str, preimage: &str) -> bool {
        // Decode preimage from hex
        let preimage_bytes = match hex::decode(preimage) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        // Compute SHA256 hash of preimage
        let mut hasher = Sha256::new();
        hasher.update(&preimage_bytes);
        let computed_hash = hasher.finalize();
        let computed_hash_hex = hex::encode(computed_hash);

        // Compare with provided payment hash
        computed_hash_hex.eq_ignore_ascii_case(payment_hash)
    }

    /// Parse amount from BOLT11 invoice
    ///
    /// Extracts the amount in millisatoshis from a BOLT11 Lightning invoice.
    /// BOLT11 format: ln{network}{amount}{units}...
    ///
    /// # Arguments
    /// * `invoice` - BOLT11 invoice string
    ///
    /// # Returns
    /// Amount in millisatoshis, or None if parsing fails
    fn parse_invoice_amount(invoice: &str) -> Option<u64> {
        // BOLT11 invoices start with 'ln' followed by network prefix (bc/tb/bcrt)
        if !invoice.starts_with("ln") {
            return None;
        }

        // Find the separator (first '1' after the network prefix)
        let separator_pos = invoice[2..].find('1').map(|pos| pos + 2)?;

        // Amount is between network prefix and separator
        let amount_str = &invoice[4..separator_pos]; // Skip 'ln' + 2-char network

        if amount_str.is_empty() {
            // No amount specified (amount-less invoice)
            return None;
        }

        // Last character indicates the multiplier
        let (num_str, multiplier) = if let Some(last_char) = amount_str.chars().last() {
            match last_char {
                'p' => (&amount_str[..amount_str.len() - 1], 0.0001), // pico-bitcoin (0.1 nanosat)
                'n' => (&amount_str[..amount_str.len() - 1], 0.1),    // nano-bitcoin (100 picosat)
                'u' => (&amount_str[..amount_str.len() - 1], 100.0),  // micro-bitcoin
                'm' => (&amount_str[..amount_str.len() - 1], 100_000.0), // milli-bitcoin
                _ => (amount_str, 100_000_000.0),                     // No suffix = bitcoin
            }
        } else {
            return None;
        };

        // Parse the number
        let amount: f64 = num_str.parse().ok()?;

        // Convert to millisatoshis (1 BTC = 100_000_000_000 msats)
        let msats = (amount * multiplier * 1_000.0) as u64;

        Some(msats)
    }

    /// Check payment status
    ///
    /// # Arguments
    /// * `payment_id` - The payment ID to check
    ///
    /// # Returns
    /// Current payment status
    pub async fn get_payment_status(&self, payment_id: &str) -> Result<PaymentStatus> {
        let _wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        // Payment lookup requires SDK list_payments() call
        // For marketplace MVP, track status in database instead
        let _ = payment_id;
        Ok(PaymentStatus::Pending)
    }

    /// Create an invoice for compute job payment
    ///
    /// For escrow-style payments, this returns the Spark address used for HTLC transfers.
    /// The sender should use `send_htlc_payment()` with the provided payment hash.
    ///
    /// # Arguments
    /// * `amount_msats` - Amount to receive in millisatoshis
    /// * `description` - Payment description
    /// * `_payment_hash` - Reserved for future HTLC integration
    ///
    /// # Returns
    /// BOLT-11 invoice string
    pub async fn create_hold_invoice(
        &self,
        amount_msats: u64,
        description: &str,
        payment_hash: &str,
    ) -> Result<String> {
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        validate_payment_hash(payment_hash)?;
        let _ = msats_to_sats(amount_msats)?;
        let _ = description;
        let address = wallet
            .get_spark_address()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch Spark address: {}", e))?;

        Ok(address)
    }

    /// Send an escrow-style HTLC payment.
    ///
    /// The receiver must claim with the preimage before expiry.
    pub async fn send_htlc_payment(
        &self,
        payment_type: PaymentType,
        item_id: &str,
        spark_address: &str,
        amount_msats: u64,
        payment_hash: &str,
        expiry_duration_secs: u64,
    ) -> Result<PaymentRecord> {
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        validate_payment_hash(payment_hash)?;
        let amount_sats = msats_to_sats(amount_msats)?;

        let mut payment = PaymentRecord::new(
            payment_type,
            item_id.to_string(),
            amount_msats,
            spark_address.to_string(),
        );

        let response = wallet
            .send_htlc_payment(
                spark_address,
                amount_sats,
                payment_hash,
                expiry_duration_secs,
                None,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send HTLC payment: {}", e))?;

        payment.mark_in_flight(payment_hash.to_string());

        if response.payment.status == openagents_spark::PaymentStatus::Completed {
            if let Some(preimage) = payment_preimage(&response.payment) {
                payment.mark_completed(preimage);
            }
        }

        Ok(payment)
    }

    /// Settle payment by verifying preimage
    ///
    /// For HTLC escrow flows, the receiver calls SDK's `claim_htlc_payment()` with preimage.
    /// This method verifies preimage validity for application-level tracking.
    ///
    /// # Arguments
    /// * `payment_hash` - The payment hash
    /// * `preimage` - The preimage to verify
    ///
    /// # Returns
    /// true if preimage is valid for the hash
    pub async fn settle_hold_invoice(&self, payment_hash: &str, preimage: &str) -> Result<bool> {
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        validate_payment_hash(payment_hash)?;

        // Verify preimage matches hash
        if !self.verify_preimage(payment_hash, preimage) {
            return Err(anyhow::anyhow!("Preimage does not match payment hash"));
        }

        let response = wallet
            .claim_htlc_payment(preimage)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to claim HTLC payment: {}", e))?;

        if let Some(claimed_hash) = payment_hash_for_payment(&response.payment) {
            if !claimed_hash.eq_ignore_ascii_case(payment_hash) {
                return Err(anyhow::anyhow!(
                    "Claimed HTLC does not match expected payment hash"
                ));
            }
        }

        Ok(true)
    }

    /// Cancel/refund logic for marketplace
    ///
    /// For HTLC transfers, they auto-expire after timeout.
    /// This method is for application-level tracking only.
    ///
    /// # Arguments
    /// * `payment_hash` - The payment hash
    ///
    /// # Returns
    /// true (HTLC auto-expires, no explicit cancel needed)
    pub async fn cancel_hold_invoice(&self, payment_hash: &str) -> Result<bool> {
        let wallet = self
            .wallet
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spark wallet not configured. See d-001 directive."))?;

        validate_payment_hash(payment_hash)?;

        let payments = wallet
            .list_htlc_payments(None, Some(200), None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list HTLC payments: {}", e))?;

        let status = payments
            .iter()
            .filter_map(|payment| htlc_details(payment))
            .find(|details| details.payment_hash.eq_ignore_ascii_case(payment_hash))
            .map(|details| details.status);

        match status {
            Some(SparkHtlcStatus::Returned) => Ok(true),
            Some(_) => Ok(false),
            None => Ok(false),
        }
    }
}

fn msats_to_sats(amount_msats: u64) -> Result<u64> {
    if amount_msats % 1000 != 0 {
        return Err(anyhow::anyhow!(
            "amount_msats must be divisible by 1000 to represent whole sats"
        ));
    }
    Ok(amount_msats / 1000)
}

fn validate_payment_hash(payment_hash: &str) -> Result<()> {
    let bytes = hex::decode(payment_hash)
        .map_err(|_| anyhow::anyhow!("Payment hash must be hex-encoded"))?;
    if bytes.len() != 32 {
        return Err(anyhow::anyhow!(
            "Payment hash must be 32 bytes (64 hex chars)"
        ));
    }
    Ok(())
}

fn htlc_details(
    payment: &openagents_spark::Payment,
) -> Option<&openagents_spark::SparkHtlcDetails> {
    match &payment.details {
        Some(PaymentDetails::Spark {
            htlc_details: Some(details),
            ..
        }) => Some(details),
        _ => None,
    }
}

fn payment_hash_for_payment(payment: &openagents_spark::Payment) -> Option<String> {
    match &payment.details {
        Some(PaymentDetails::Lightning { payment_hash, .. }) => Some(payment_hash.clone()),
        Some(PaymentDetails::Spark {
            htlc_details: Some(details),
            ..
        }) => Some(details.payment_hash.clone()),
        _ => None,
    }
}

fn payment_preimage(payment: &openagents_spark::Payment) -> Option<String> {
    match &payment.details {
        Some(PaymentDetails::Lightning { preimage, .. }) => preimage.clone(),
        Some(PaymentDetails::Spark {
            htlc_details: Some(details),
            ..
        }) => details.preimage.clone(),
        _ => None,
    }
}

fn validate_split_invoices(
    split: &super::revenue::RevenueSplit,
    invoices: &RevenueSplitInvoices,
) -> Result<()> {
    if split.creator_sats > 0 && invoices.creator_invoice.trim().is_empty() {
        return Err(anyhow::anyhow!("Creator invoice required for split payout"));
    }
    if split.compute_sats > 0 && invoices.compute_invoice.trim().is_empty() {
        return Err(anyhow::anyhow!("Compute invoice required for split payout"));
    }
    if split.referrer_sats > 0 {
        let referrer_invoice = invoices.referrer_invoice.as_deref().unwrap_or("").trim();
        if referrer_invoice.is_empty() {
            return Err(anyhow::anyhow!(
                "Referrer invoice required for split payout"
            ));
        }
    }
    if let Some(platform_invoice) = invoices.platform_invoice.as_deref() {
        if platform_invoice.trim().is_empty() {
            return Err(anyhow::anyhow!("Platform invoice must not be empty"));
        }
    }
    Ok(())
}

async fn send_split_payment(
    wallet: &SparkWallet,
    payment_type: PaymentType,
    item_id: &str,
    invoice: &str,
    amount_sats: u64,
) -> Result<PaymentRecord> {
    let amount_msats = sats_to_msats(amount_sats)?;
    let invoice_msats = PaymentManager::parse_invoice_amount(invoice);
    let send_amount = match invoice_msats {
        Some(msats) => {
            if msats != amount_msats {
                return Err(anyhow::anyhow!(
                    "Invoice amount {} msats does not match expected {} msats",
                    msats,
                    amount_msats
                ));
            }
            None
        }
        None => Some(amount_sats),
    };

    let mut payment = PaymentRecord::new(
        payment_type,
        item_id.to_string(),
        amount_msats,
        invoice.to_string(),
    );

    let response = wallet
        .send_payment_simple(invoice, send_amount)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send split payment: {}", e))?;

    payment.mark_in_flight(response.payment.id.clone());

    if response.payment.status == openagents_spark::PaymentStatus::Completed {
        let preimage =
            payment_preimage(&response.payment).unwrap_or_else(|| response.payment.id.clone());
        payment.mark_completed(preimage);
    } else {
        payment.mark_failed();
    }

    Ok(payment)
}

fn sats_to_msats(amount_sats: u64) -> Result<u64> {
    amount_sats
        .checked_mul(1000)
        .ok_or_else(|| anyhow::anyhow!("Amount exceeds msat range"))
}

/// Mock payment service for testing without Breez/Spark SDK
///
/// Simulates Lightning payment flows for E2E tests:
/// - Invoice creation with mock BOLT11 format
/// - Payment tracking with configurable success/failure
/// - Preimage generation for payment verification
#[derive(Debug, Default)]
pub struct MockPaymentService {
    pending_payments: std::collections::HashMap<String, u64>,
    completed_payments: std::collections::HashSet<String>,
    created_invoices: Vec<MockInvoice>,
}

#[derive(Debug, Clone)]
pub struct MockInvoice {
    pub invoice: String,
    pub amount_msats: u64,
    pub description: String,
    pub payment_hash: String,
}

impl MockPaymentService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_invoice(&mut self, amount_msats: u64, description: &str) -> MockInvoice {
        let id = uuid::Uuid::new_v4();
        let payment_hash = id.to_string().replace('-', "") + &id.to_string().replace('-', "");
        let invoice = format!("lnbc{}m1mock{}", amount_msats / 1000, &payment_hash[..16]);

        self.pending_payments.insert(invoice.clone(), amount_msats);

        let mock_invoice = MockInvoice {
            invoice: invoice.clone(),
            amount_msats,
            description: description.to_string(),
            payment_hash,
        };

        self.created_invoices.push(mock_invoice.clone());
        mock_invoice
    }

    pub fn pay_invoice(&mut self, invoice: &str) -> Result<String, anyhow::Error> {
        if let Some(_amount) = self.pending_payments.remove(invoice) {
            let id = uuid::Uuid::new_v4();
            let preimage = id.to_string().replace('-', "") + &id.to_string().replace('-', "");
            self.completed_payments.insert(invoice.to_string());
            Ok(preimage)
        } else {
            Err(anyhow::anyhow!("Invoice not found or already paid"))
        }
    }

    pub fn is_paid(&self, invoice: &str) -> bool {
        self.completed_payments.contains(invoice)
    }

    pub fn pending_count(&self) -> usize {
        self.pending_payments.len()
    }

    pub fn completed_count(&self) -> usize {
        self.completed_payments.len()
    }

    pub fn get_pending_amount(&self, invoice: &str) -> Option<u64> {
        self.pending_payments.get(invoice).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::revenue::{RevenueSplit, RevenueSplitConfig};

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
    async fn test_payment_manager_without_wallet() {
        let manager = PaymentManager::new(None);

        // All payment methods should return error when wallet not configured
        let result = manager
            .pay_compute_job("job-1", "lnbc...", Some(1000))
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );

        let result = manager.create_invoice(1000, "test").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );
    }

    #[tokio::test]
    async fn test_payment_manager_without_wallet_htlc() {
        let manager = PaymentManager::new(None);
        let payment_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = manager
            .create_hold_invoice(1000, "escrow", payment_hash)
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );

        let result = manager
            .send_htlc_payment(
                PaymentType::Compute,
                "job-1",
                "spark:address",
                1000,
                payment_hash,
                60,
            )
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );

        let result = manager.settle_hold_invoice(payment_hash, "deadbeef").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );

        let result = manager.cancel_hold_invoice(payment_hash).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );
    }

    #[test]
    fn test_parse_invoice_amount_micro_bitcoin() {
        // lnbc2500u = 2500 micro-bitcoin = 250,000 sats = 250,000,000 msats
        let invoice = "lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq";
        let amount = PaymentManager::parse_invoice_amount(invoice);
        assert_eq!(amount, Some(250_000_000));
    }

    #[test]
    fn test_parse_invoice_amount_milli_bitcoin() {
        // lnbc20m = 20 milli-bitcoin = 2,000,000 sats = 2,000,000,000 msats
        let invoice = "lnbc20m1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq";
        let amount = PaymentManager::parse_invoice_amount(invoice);
        assert_eq!(amount, Some(2_000_000_000));
    }

    #[test]
    fn test_parse_invoice_amount_nano_bitcoin() {
        // lnbc250n = 250 nano-bitcoin = 25 sats = 25,000 msats
        let invoice = "lnbc250n1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq";
        let amount = PaymentManager::parse_invoice_amount(invoice);
        assert_eq!(amount, Some(25_000));
    }

    #[test]
    fn test_parse_invoice_amount_no_amount() {
        // Invoice with no amount (amount-less invoice)
        let invoice = "lnbc1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq";
        let amount = PaymentManager::parse_invoice_amount(invoice);
        assert_eq!(amount, None);
    }

    #[test]
    fn test_parse_invoice_amount_invalid() {
        // Not a valid BOLT11 invoice
        let invoice = "not-an-invoice";
        let amount = PaymentManager::parse_invoice_amount(invoice);
        assert_eq!(amount, None);
    }

    #[test]
    fn test_verify_preimage_valid() {
        let manager = PaymentManager::new(None);

        // Test with a known preimage and its hash
        // Preimage: "0000000000000000000000000000000000000000000000000000000000000000"
        let preimage = "0000000000000000000000000000000000000000000000000000000000000000";
        let payment_hash = "66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925";

        assert!(manager.verify_preimage(payment_hash, preimage));
    }

    #[test]
    fn test_verify_preimage_invalid() {
        let manager = PaymentManager::new(None);

        let preimage = "0000000000000000000000000000000000000000000000000000000000000000";
        let wrong_hash = "1111111111111111111111111111111111111111111111111111111111111111";

        assert!(!manager.verify_preimage(wrong_hash, preimage));
    }

    #[test]
    fn test_verify_preimage_malformed() {
        let manager = PaymentManager::new(None);

        // Invalid hex strings
        assert!(!manager.verify_preimage("invalid", "also-invalid"));
        assert!(!manager.verify_preimage("valid66687aadf862bd", "not-hex"));
    }

    #[test]
    fn test_msats_to_sats_requires_whole_sats() {
        assert_eq!(msats_to_sats(2000).unwrap(), 2);
        assert!(msats_to_sats(1500).is_err());
    }

    #[test]
    fn test_validate_payment_hash_length() {
        let valid = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(validate_payment_hash(valid).is_ok());
        assert!(validate_payment_hash("1234").is_err());
    }

    #[test]
    fn test_validate_split_invoices_requires_referrer() {
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(100_000, &config, true);
        let invoices = RevenueSplitInvoices {
            creator_invoice: "lnbc1creator".to_string(),
            compute_invoice: "lnbc1compute".to_string(),
            platform_invoice: None,
            referrer_invoice: None,
        };

        let err = validate_split_invoices(&split, &invoices).unwrap_err();
        assert!(err.to_string().contains("Referrer invoice required"));
    }

    #[test]
    fn test_validate_split_invoices_allows_missing_referrer() {
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(100_000, &config, false);
        let invoices = RevenueSplitInvoices {
            creator_invoice: "lnbc1creator".to_string(),
            compute_invoice: "lnbc1compute".to_string(),
            platform_invoice: None,
            referrer_invoice: None,
        };

        assert!(validate_split_invoices(&split, &invoices).is_ok());
    }

    #[tokio::test]
    async fn test_distribute_revenue_requires_wallet() {
        let manager = PaymentManager::new(None);
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(100_000, &config, false);
        let invoices = RevenueSplitInvoices {
            creator_invoice: "lnbc1creator".to_string(),
            compute_invoice: "lnbc1compute".to_string(),
            platform_invoice: None,
            referrer_invoice: None,
        };

        let result = manager
            .distribute_revenue(PaymentType::Skill, "skill-1", split, invoices)
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Spark wallet not configured")
        );
    }

    #[test]
    fn test_sats_to_msats_conversion() {
        assert_eq!(sats_to_msats(2).unwrap(), 2000);
    }
}
