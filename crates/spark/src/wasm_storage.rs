//! In-memory storage for wasm builds (Cloudflare workers / browser).
//!
//! This keeps wallet state in memory for the current runtime and is intended
//! for environments without filesystem-backed storage.

use async_trait::async_trait;
use breez_sdk_spark::{
    AssetFilter, DepositInfo, ListPaymentsRequest, Payment, PaymentDetails, PaymentMetadata,
    PaymentMethod, SetLnurlMetadataItem, SparkHtlcStatus, Storage, StorageError,
    UpdateDepositPayload,
};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub(crate) struct MemoryStorage {
    cache: Mutex<HashMap<String, String>>,
    payments: Mutex<Vec<Payment>>,
    payment_metadata: Mutex<HashMap<String, PaymentMetadata>>,
    deposits: Mutex<HashMap<String, DepositInfo>>,
    lnurl_metadata: Mutex<HashMap<String, SetLnurlMetadataItem>>,
}

impl MemoryStorage {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn deposit_key(txid: &str, vout: u32) -> String {
        format!("{txid}:{vout}")
    }

    fn matches_invoice(payment: &Payment, invoice: &str) -> bool {
        match &payment.details {
            Some(PaymentDetails::Lightning { invoice: inv, .. }) => inv == invoice,
            Some(PaymentDetails::Spark {
                invoice_details, ..
            }) => invoice_details
                .as_ref()
                .map(|details| details.invoice == invoice)
                .unwrap_or(false),
            Some(PaymentDetails::Token {
                invoice_details, ..
            }) => invoice_details
                .as_ref()
                .map(|details| details.invoice == invoice)
                .unwrap_or(false),
            _ => false,
        }
    }

    fn matches_asset_filter(payment: &Payment, filter: &AssetFilter) -> bool {
        match filter {
            AssetFilter::Bitcoin => payment.method != PaymentMethod::Token,
            AssetFilter::Token { token_identifier } => match &payment.details {
                Some(PaymentDetails::Token { metadata, .. }) => token_identifier
                    .as_ref()
                    .map(|id| &metadata.identifier == id)
                    .unwrap_or(true),
                _ => false,
            },
        }
    }

    fn matches_htlc_filter(payment: &Payment, statuses: &[SparkHtlcStatus]) -> bool {
        match &payment.details {
            Some(PaymentDetails::Spark { htlc_details, .. }) => htlc_details
                .as_ref()
                .map(|details| statuses.contains(&details.status))
                .unwrap_or(false),
            _ => false,
        }
    }
}

#[async_trait(?Send)]
impl Storage for MemoryStorage {
    async fn delete_cached_item(&self, key: String) -> Result<(), StorageError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Cache lock poisoned: {e}")))?;
        cache.remove(&key);
        Ok(())
    }

    async fn get_cached_item(&self, key: String) -> Result<Option<String>, StorageError> {
        let cache = self
            .cache
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Cache lock poisoned: {e}")))?;
        Ok(cache.get(&key).cloned())
    }

    async fn set_cached_item(&self, key: String, value: String) -> Result<(), StorageError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Cache lock poisoned: {e}")))?;
        cache.insert(key, value);
        Ok(())
    }

    async fn list_payments(
        &self,
        request: ListPaymentsRequest,
    ) -> Result<Vec<Payment>, StorageError> {
        let payments = self
            .payments
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Payments lock poisoned: {e}")))?;
        let mut filtered = payments.clone();

        if let Some(types) = &request.type_filter {
            filtered.retain(|payment| types.contains(&payment.payment_type));
        }

        if let Some(statuses) = &request.status_filter {
            filtered.retain(|payment| statuses.contains(&payment.status));
        }

        if let Some(filter) = &request.asset_filter {
            filtered.retain(|payment| Self::matches_asset_filter(payment, filter));
        }

        if let Some(statuses) = &request.spark_htlc_status_filter {
            filtered.retain(|payment| Self::matches_htlc_filter(payment, statuses));
        }

        if let Some(from_ts) = request.from_timestamp {
            filtered.retain(|payment| payment.timestamp >= from_ts);
        }

        if let Some(to_ts) = request.to_timestamp {
            filtered.retain(|payment| payment.timestamp < to_ts);
        }

        let ascending = request.sort_ascending.unwrap_or(false);
        if ascending {
            filtered.sort_by_key(|payment| payment.timestamp);
        } else {
            filtered.sort_by_key(|payment| std::cmp::Reverse(payment.timestamp));
        }

        let offset = request.offset.unwrap_or(0) as usize;
        let limit = request
            .limit
            .unwrap_or(filtered.len() as u32)
            .min(filtered.len() as u32) as usize;

        if offset >= filtered.len() {
            return Ok(Vec::new());
        }

        Ok(filtered[offset..(offset + limit).min(filtered.len())].to_vec())
    }

    async fn insert_payment(&self, payment: Payment) -> Result<(), StorageError> {
        let mut payments = self
            .payments
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Payments lock poisoned: {e}")))?;
        if let Some(existing) = payments.iter_mut().find(|item| item.id == payment.id) {
            *existing = payment;
        } else {
            payments.push(payment);
        }
        Ok(())
    }

    async fn set_payment_metadata(
        &self,
        payment_id: String,
        metadata: PaymentMetadata,
    ) -> Result<(), StorageError> {
        let mut meta = self
            .payment_metadata
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Metadata lock poisoned: {e}")))?;
        meta.insert(payment_id, metadata);
        Ok(())
    }

    async fn get_payment_by_id(&self, id: String) -> Result<Payment, StorageError> {
        let payments = self
            .payments
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Payments lock poisoned: {e}")))?;
        payments
            .iter()
            .find(|payment| payment.id == id)
            .cloned()
            .ok_or_else(|| StorageError::Implementation("Payment not found".to_string()))
    }

    async fn get_payment_by_invoice(
        &self,
        invoice: String,
    ) -> Result<Option<Payment>, StorageError> {
        let payments = self
            .payments
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Payments lock poisoned: {e}")))?;
        Ok(payments
            .iter()
            .find(|payment| Self::matches_invoice(payment, &invoice))
            .cloned())
    }

    async fn add_deposit(
        &self,
        txid: String,
        vout: u32,
        amount_sats: u64,
    ) -> Result<(), StorageError> {
        let mut deposits = self
            .deposits
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Deposits lock poisoned: {e}")))?;
        deposits.insert(
            Self::deposit_key(&txid, vout),
            DepositInfo {
                txid,
                vout,
                amount_sats,
                refund_tx: None,
                refund_tx_id: None,
                claim_error: None,
            },
        );
        Ok(())
    }

    async fn delete_deposit(&self, txid: String, vout: u32) -> Result<(), StorageError> {
        let mut deposits = self
            .deposits
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Deposits lock poisoned: {e}")))?;
        deposits.remove(&Self::deposit_key(&txid, vout));
        Ok(())
    }

    async fn list_deposits(&self) -> Result<Vec<DepositInfo>, StorageError> {
        let deposits = self
            .deposits
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Deposits lock poisoned: {e}")))?;
        Ok(deposits.values().cloned().collect())
    }

    async fn update_deposit(
        &self,
        txid: String,
        vout: u32,
        payload: UpdateDepositPayload,
    ) -> Result<(), StorageError> {
        let mut deposits = self
            .deposits
            .lock()
            .map_err(|e| StorageError::Implementation(format!("Deposits lock poisoned: {e}")))?;
        let entry = deposits
            .entry(Self::deposit_key(&txid, vout))
            .or_insert(DepositInfo {
                txid,
                vout,
                amount_sats: 0,
                refund_tx: None,
                refund_tx_id: None,
                claim_error: None,
            });

        match payload {
            UpdateDepositPayload::ClaimError { error } => {
                entry.claim_error = Some(error);
            }
            UpdateDepositPayload::Refund {
                refund_txid,
                refund_tx,
            } => {
                entry.refund_tx_id = Some(refund_txid);
                entry.refund_tx = Some(refund_tx);
            }
        }

        Ok(())
    }

    async fn set_lnurl_metadata(
        &self,
        metadata: Vec<SetLnurlMetadataItem>,
    ) -> Result<(), StorageError> {
        let mut lnurl_metadata = self
            .lnurl_metadata
            .lock()
            .map_err(|e| StorageError::Implementation(format!("LNURL lock poisoned: {e}")))?;
        for item in metadata {
            lnurl_metadata.insert(item.payment_hash.clone(), item);
        }
        Ok(())
    }
}
