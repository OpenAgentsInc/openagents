//! A file-backed store for Breez's SDK on computers.
//!
//! Breez keeps a wallet's payments, deposits, contacts, and settings through
//! its [`Storage`] trait. Its SQLite implementation cannot be linked into the
//! computers' workspace (see this crate's `Cargo.toml`), so [`FileStorage`]
//! keeps the same records in memory and writes them, whole, to one JSON file
//! after every change: a temporary file, then a rename, readable by the owner
//! only. It follows the SQLite implementation's semantics row for row: the
//! tables become maps, the joins become lookups, and `list_payments` applies
//! the same filters in the same order.
//!
//! The file holds no key material. The SDK's tree and token stores and its
//! session tokens stay in memory (Breez's defaults for a custom store) and
//! are read again from Spark's operators when the wallet starts.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use breez_sdk_spark::sync_storage::{
    IncomingChange, OutgoingChange, Record, RecordChange, RecordId, UnversionedRecordChange,
};
use breez_sdk_spark::{
    AssetFilter, Contact, ConversionFilter, ConversionInfo, ConversionStatus, DepositInfo,
    ListContactsRequest, LnurlPayInfo, LnurlReceiveMetadata, LnurlWithdrawInfo, Payment,
    PaymentDetails, PaymentMetadata, PaymentMethod, PaymentStatus, PaymentType,
    SetLnurlMetadataItem, SparkHtlcDetails, SparkHtlcStatus, SparkInvoicePaymentDetails, Storage,
    StorageBackend, StorageError, StorageListPaymentsRequest, StoragePaymentDetailsFilter,
    StoredCrossChainSwap, TokenMetadata, TokenTransactionType, UpdateDepositPayload,
    UpdateWatchedAddressPayload, WatchedDepositAddress,
};
use serde::{Deserialize, Serialize};

/// The file format's version, written in every file.
const VERSION: &str = "openagents-spark.store.v1";

/// Breez's records for one wallet, kept in one JSON file.
pub struct FileStorage {
    path: PathBuf,
    state: Mutex<State>,
}

/// The store as the SDK's [`StorageBackend`], for
/// `SdkBuilder::with_storage_backend`. The tree, token, and session stores
/// are Breez's in-memory defaults.
///
/// # Errors
///
/// The file exists and cannot be read, or its directory cannot be made.
pub fn backend(path: &Path) -> Result<Arc<dyn StorageBackend>, String> {
    let storage = FileStorage::open(path)?;
    Ok(breez_sdk_spark::custom_storage(Arc::new(storage)))
}

impl FileStorage {
    /// Open the store at `path`, reading it when it exists. Its directory is
    /// created, readable by the owner only.
    ///
    /// # Errors
    ///
    /// The file exists and cannot be read or parsed, or the directory cannot
    /// be made.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            make_private_dir(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        let state = match std::fs::read(path) {
            Ok(bytes) => {
                let state: State = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                if state.v != VERSION {
                    return Err(format!(
                        "{} is a store of another version ({})",
                        path.display(),
                        state.v
                    ));
                }
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        };
        Ok(Self {
            path: path.to_path_buf(),
            state: Mutex::new(state),
        })
    }

    /// Where the store lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Run `change` on the state and write the file. When the write fails,
    /// the state is restored, so memory never runs ahead of the file.
    fn change<T>(
        &self,
        change: impl FnOnce(&mut State) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let mut state = self.lock();
        let before = state.clone();
        let result = change(&mut state)?;
        if let Err(error) = write_private(&self.path, &state) {
            *state = before;
            return Err(StorageError::Implementation(format!(
                "cannot write {}: {error}",
                self.path.display()
            )));
        }
        Ok(result)
    }
}

#[cfg(unix)]
fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if !dir.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

fn write_private(path: &Path, state: &State) -> std::io::Result<()> {
    use std::io::Write;
    let bytes = serde_json::to_vec(state).map_err(std::io::Error::other)?;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temp, path)
}

/// Every table, as maps. Field names follow the SQLite columns.
#[derive(Clone, Serialize, Deserialize)]
struct State {
    v: String,
    /// Insertion order counter, standing in for SQLite's `rowid`.
    next_row: u64,
    payments: BTreeMap<String, PaymentRow>,
    lightning: BTreeMap<String, LightningRow>,
    token: BTreeMap<String, TokenRow>,
    spark: BTreeMap<String, SparkRow>,
    deposit: BTreeMap<String, DepositRow>,
    metadata: BTreeMap<String, MetadataRow>,
    lnurl_receive: BTreeMap<String, LnurlReceiveRow>,
    settings: BTreeMap<String, String>,
    unclaimed_deposits: Vec<DepositInfo>,
    watched: BTreeMap<String, WatchedRow>,
    contacts: Vec<Contact>,
    swaps: Vec<StoredCrossChainSwap>,
    sync_revision: u64,
    sync_outgoing: Vec<OutgoingRow>,
    sync_state: Vec<RecordRow>,
    sync_incoming: Vec<RecordRow>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            v: VERSION.into(),
            next_row: 0,
            payments: BTreeMap::new(),
            lightning: BTreeMap::new(),
            token: BTreeMap::new(),
            spark: BTreeMap::new(),
            deposit: BTreeMap::new(),
            metadata: BTreeMap::new(),
            lnurl_receive: BTreeMap::new(),
            settings: BTreeMap::new(),
            unclaimed_deposits: Vec::new(),
            watched: BTreeMap::new(),
            contacts: Vec::new(),
            swaps: Vec::new(),
            sync_revision: 0,
            sync_outgoing: Vec::new(),
            sync_state: Vec::new(),
            sync_incoming: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct PaymentRow {
    row: u64,
    payment_type: PaymentType,
    status: PaymentStatus,
    /// `u128` as decimal text.
    amount: String,
    fees: String,
    timestamp: u64,
    method: PaymentMethod,
    withdraw_tx_id: Option<String>,
    spark: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
struct LightningRow {
    invoice: String,
    payment_hash: String,
    destination_pubkey: String,
    description: Option<String>,
    preimage: Option<String>,
    htlc_status: SparkHtlcStatus,
    htlc_expiry_time: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct TokenRow {
    metadata: TokenMetadata,
    tx_hash: String,
    tx_type: TokenTransactionType,
    invoice_details: Option<SparkInvoicePaymentDetails>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SparkRow {
    invoice_details: Option<SparkInvoicePaymentDetails>,
    htlc_details: Option<SparkHtlcDetails>,
}

#[derive(Clone, Serialize, Deserialize)]
struct DepositRow {
    tx_id: String,
    vout: u32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct MetadataRow {
    parent_payment_id: Option<String>,
    lnurl_pay_info: Option<LnurlPayInfo>,
    lnurl_withdraw_info: Option<LnurlWithdrawInfo>,
    lnurl_description: Option<String>,
    conversion_info: Option<ConversionInfo>,
    conversion_status: Option<ConversionStatus>,
}

#[derive(Clone, Serialize, Deserialize)]
struct LnurlReceiveRow {
    nostr_zap_request: Option<String>,
    nostr_zap_receipt: Option<String>,
    sender_comment: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct WatchedRow {
    issued_at: u64,
    seen: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct OutgoingRow {
    record_type: String,
    data_id: String,
    schema_version: String,
    updated_fields: HashMap<String, String>,
    revision: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct RecordRow {
    record_type: String,
    data_id: String,
    schema_version: String,
    data: HashMap<String, String>,
    revision: u64,
}

impl RecordRow {
    fn of(record: &Record) -> Self {
        Self {
            record_type: record.id.r#type.clone(),
            data_id: record.id.data_id.clone(),
            schema_version: record.schema_version.clone(),
            data: record.data.clone(),
            revision: record.revision,
        }
    }

    fn record(&self) -> Record {
        Record {
            id: RecordId::new(self.record_type.clone(), self.data_id.clone()),
            revision: self.revision,
            schema_version: self.schema_version.clone(),
            data: self.data.clone(),
        }
    }

    fn same_id(&self, record_type: &str, data_id: &str) -> bool {
        self.record_type == record_type && self.data_id == data_id
    }
}

fn parse_u128(text: &str) -> Result<u128, StorageError> {
    text.parse()
        .map_err(|_| StorageError::Serialization(format!("invalid amount {text:?}")))
}

impl State {
    fn parent_state(&self, record_type: &str, data_id: &str) -> Option<Record> {
        self.sync_state
            .iter()
            .find(|row| row.same_id(record_type, data_id))
            .map(RecordRow::record)
    }

    fn upsert_state(&mut self, record: &Record) {
        let row = RecordRow::of(record);
        match self
            .sync_state
            .iter_mut()
            .find(|existing| existing.same_id(&row.record_type, &row.data_id))
        {
            Some(existing) => *existing = row,
            None => self.sync_state.push(row),
        }
        self.sync_revision = self.sync_revision.max(record.revision);
    }

    fn outgoing(&self, row: &OutgoingRow) -> OutgoingChange {
        OutgoingChange {
            change: RecordChange {
                id: RecordId::new(row.record_type.clone(), row.data_id.clone()),
                schema_version: row.schema_version.clone(),
                updated_fields: row.updated_fields.clone(),
                local_revision: row.revision,
            },
            parent: self.parent_state(&row.record_type, &row.data_id),
        }
    }

    /// `insert_payment_in_tx`: the payment row and its detail rows, keeping
    /// detail values a new update leaves out.
    fn insert_payment(&mut self, payment: Payment) {
        let (withdraw_tx_id, spark) = match &payment.details {
            Some(PaymentDetails::Withdraw { tx_id }) => (Some(tx_id.clone()), None),
            Some(PaymentDetails::Spark { .. }) => (None, Some(true)),
            _ => (None, None),
        };
        let row = match self.payments.get(&payment.id) {
            Some(existing) => existing.row,
            None => {
                self.next_row += 1;
                self.next_row
            }
        };
        self.payments.insert(
            payment.id.clone(),
            PaymentRow {
                row,
                payment_type: payment.payment_type,
                status: payment.status,
                amount: payment.amount.to_string(),
                fees: payment.fees.to_string(),
                timestamp: payment.timestamp,
                method: payment.method,
                withdraw_tx_id,
                spark,
            },
        );
        let id = payment.id;
        match payment.details {
            Some(PaymentDetails::Spark {
                invoice_details,
                htlc_details,
                ..
            }) => {
                if invoice_details.is_some() || htlc_details.is_some() {
                    let entry = self.spark.entry(id).or_insert(SparkRow {
                        invoice_details: None,
                        htlc_details: None,
                    });
                    if invoice_details.is_some() {
                        entry.invoice_details = invoice_details;
                    }
                    if htlc_details.is_some() {
                        entry.htlc_details = htlc_details;
                    }
                }
            }
            Some(PaymentDetails::Token {
                metadata,
                tx_hash,
                tx_type,
                invoice_details,
                ..
            }) => {
                let invoice_details = invoice_details.or_else(|| {
                    self.token
                        .get(&id)
                        .and_then(|row| row.invoice_details.clone())
                });
                self.token.insert(
                    id,
                    TokenRow {
                        metadata,
                        tx_hash,
                        tx_type,
                        invoice_details,
                    },
                );
            }
            Some(PaymentDetails::Lightning {
                invoice,
                destination_pubkey,
                description,
                htlc_details,
                ..
            }) => {
                let preimage = htlc_details
                    .preimage
                    .or_else(|| self.lightning.get(&id).and_then(|row| row.preimage.clone()));
                self.lightning.insert(
                    id,
                    LightningRow {
                        invoice,
                        payment_hash: htlc_details.payment_hash,
                        destination_pubkey,
                        description,
                        preimage,
                        htlc_status: htlc_details.status,
                        htlc_expiry_time: htlc_details.expiry_time,
                    },
                );
            }
            Some(PaymentDetails::Deposit { tx_id, vout }) => {
                self.deposit.insert(id, DepositRow { tx_id, vout });
            }
            Some(PaymentDetails::Withdraw { .. }) | None => {}
        }
    }

    /// `map_payment`: a payment as the SDK reads it, from its rows.
    fn payment(&self, id: &str) -> Result<Payment, StorageError> {
        let row = self.payments.get(id).ok_or(StorageError::NotFound)?;
        let metadata = self.metadata.get(id);
        let conversion_info = || metadata.and_then(|m| m.conversion_info.clone());
        let details = if let Some(lightning) = self.lightning.get(id) {
            let lnurl_receive_metadata =
                self.lnurl_receive
                    .get(&lightning.payment_hash)
                    .map(|lrm| LnurlReceiveMetadata {
                        nostr_zap_request: lrm.nostr_zap_request.clone(),
                        nostr_zap_receipt: lrm.nostr_zap_receipt.clone(),
                        sender_comment: lrm.sender_comment.clone(),
                    });
            Some(PaymentDetails::Lightning {
                invoice: lightning.invoice.clone(),
                destination_pubkey: lightning.destination_pubkey.clone(),
                description: lightning
                    .description
                    .clone()
                    .or_else(|| metadata.and_then(|m| m.lnurl_description.clone())),
                htlc_details: SparkHtlcDetails {
                    payment_hash: lightning.payment_hash.clone(),
                    preimage: lightning.preimage.clone(),
                    expiry_time: lightning.htlc_expiry_time,
                    status: lightning.htlc_status,
                },
                lnurl_pay_info: metadata.and_then(|m| m.lnurl_pay_info.clone()),
                lnurl_withdraw_info: metadata.and_then(|m| m.lnurl_withdraw_info.clone()),
                lnurl_receive_metadata,
                conversion_info: conversion_info(),
            })
        } else if let Some(tx_id) = &row.withdraw_tx_id {
            Some(PaymentDetails::Withdraw {
                tx_id: tx_id.clone(),
            })
        } else if let Some(deposit) = self.deposit.get(id) {
            Some(PaymentDetails::Deposit {
                tx_id: deposit.tx_id.clone(),
                vout: deposit.vout,
            })
        } else if row.spark.is_some() {
            let spark = self.spark.get(id);
            Some(PaymentDetails::Spark {
                invoice_details: spark.and_then(|s| s.invoice_details.clone()),
                htlc_details: spark.and_then(|s| s.htlc_details.clone()),
                conversion_info: conversion_info(),
            })
        } else {
            self.token.get(id).map(|token| PaymentDetails::Token {
                metadata: token.metadata.clone(),
                tx_hash: token.tx_hash.clone(),
                tx_type: token.tx_type.clone(),
                invoice_details: token.invoice_details.clone(),
                conversion_info: conversion_info(),
            })
        };
        let conversion_details = metadata
            .and_then(|m| m.conversion_status.clone())
            .map(|status| breez_sdk_spark::ConversionDetails {
                status,
                conversions: vec![],
            });
        Ok(Payment {
            id: id.to_owned(),
            payment_type: row.payment_type,
            status: row.status,
            amount: parse_u128(&row.amount)?,
            fees: parse_u128(&row.fees)?,
            timestamp: row.timestamp,
            method: row.method,
            details,
            conversion_details,
        })
    }

    /// Whether the payment `id` passes one payment details filter, or `None`
    /// when the filter adds no condition (SQLite drops it from the `OR`).
    fn details_match(&self, id: &str, filter: &StoragePaymentDetailsFilter) -> Option<bool> {
        let row = &self.payments[id];
        let mut clauses: Vec<bool> = Vec::new();
        match filter {
            StoragePaymentDetailsFilter::Spark {
                htlc_status: Some(statuses),
                ..
            } if !statuses.is_empty() => {
                let status = self
                    .spark
                    .get(id)
                    .and_then(|s| s.htlc_details.as_ref())
                    .map(|h| h.status);
                clauses.push(status.is_some_and(|s| statuses.contains(&s)));
            }
            StoragePaymentDetailsFilter::Lightning {
                htlc_status: Some(statuses),
                ..
            } if !statuses.is_empty() => {
                let status = self.lightning.get(id).map(|l| l.htlc_status);
                clauses.push(status.is_some_and(|s| statuses.contains(&s)));
            }
            _ => {}
        }
        match filter {
            StoragePaymentDetailsFilter::Spark { .. } => clauses.push(row.spark == Some(true)),
            StoragePaymentDetailsFilter::Token { .. } => clauses.push(row.spark.is_none()),
            StoragePaymentDetailsFilter::Lightning { .. } => {}
        }
        let conversion_filter = match filter {
            StoragePaymentDetailsFilter::Spark {
                conversion_filter, ..
            }
            | StoragePaymentDetailsFilter::Token {
                conversion_filter, ..
            }
            | StoragePaymentDetailsFilter::Lightning {
                conversion_filter, ..
            } => conversion_filter.as_ref(),
        };
        if let Some(conversion_filter) = conversion_filter {
            let info = self
                .metadata
                .get(id)
                .and_then(|m| m.conversion_info.as_ref())
                .and_then(|info| serde_json::to_value(info).ok());
            let field = |value: &serde_json::Value, name: &str| {
                value.get(name).and_then(|v| v.as_str()).map(str::to_owned)
            };
            let pending = |value: &serde_json::Value, kind: &str| {
                field(value, "type").as_deref() == Some(kind)
                    && field(value, "status").is_some_and(|status| {
                        !matches!(status.as_str(), "Completed" | "Failed" | "Refunded")
                    })
            };
            clauses.push(info.is_some_and(|info| match conversion_filter {
                ConversionFilter::AmmRefundNeeded => {
                    field(&info, "type").as_deref() == Some("amm")
                        && field(&info, "status").as_deref() == Some("RefundNeeded")
                }
                ConversionFilter::OrchestraPending => pending(&info, "orchestra"),
                ConversionFilter::BoltzPending => pending(&info, "boltz"),
            }));
        }
        if let StoragePaymentDetailsFilter::Token {
            tx_hash: Some(tx_hash),
            ..
        } = filter
        {
            clauses.push(self.token.get(id).is_some_and(|t| &t.tx_hash == tx_hash));
        }
        if let StoragePaymentDetailsFilter::Token {
            tx_type: Some(tx_type),
            ..
        } = filter
        {
            clauses.push(
                self.token
                    .get(id)
                    .is_some_and(|t| t.tx_type.to_string() == tx_type.to_string()),
            );
        }
        (!clauses.is_empty()).then(|| clauses.iter().all(|c| *c))
    }

    fn listed(&self, id: &str, request: &StorageListPaymentsRequest) -> bool {
        let row = &self.payments[id];
        if let Some(types) = &request.type_filter
            && !types.is_empty()
            && !types.contains(&row.payment_type)
        {
            return false;
        }
        if let Some(statuses) = &request.status_filter
            && !statuses.is_empty()
            && !statuses.contains(&row.status)
        {
            return false;
        }
        if request
            .from_timestamp
            .is_some_and(|from| row.timestamp < from)
        {
            return false;
        }
        if request.to_timestamp.is_some_and(|to| row.timestamp >= to) {
            return false;
        }
        if let Some(asset) = &request.asset_filter {
            let token = self.token.get(id);
            let passes = match asset {
                AssetFilter::Bitcoin => token.is_none(),
                AssetFilter::Token { token_identifier } => token.is_some_and(|token| {
                    token_identifier
                        .as_ref()
                        .is_none_or(|identifier| &token.metadata.identifier == identifier)
                }),
            };
            if !passes {
                return false;
            }
        }
        if let Some(filters) = &request.payment_details_filter {
            let results: Vec<bool> = filters
                .iter()
                .filter_map(|filter| self.details_match(id, filter))
                .collect();
            if !results.is_empty() && !results.iter().any(|r| *r) {
                return false;
            }
        }
        self.metadata
            .get(id)
            .is_none_or(|m| m.parent_payment_id.is_none())
    }
}

#[async_trait::async_trait]
impl Storage for FileStorage {
    async fn delete_cached_item(&self, key: String) -> Result<(), StorageError> {
        self.change(|state| {
            state.settings.remove(&key);
            Ok(())
        })
    }

    async fn get_cached_item(&self, key: String) -> Result<Option<String>, StorageError> {
        Ok(self.lock().settings.get(&key).cloned())
    }

    async fn set_cached_item(&self, key: String, value: String) -> Result<(), StorageError> {
        self.change(|state| {
            state.settings.insert(key, value);
            Ok(())
        })
    }

    async fn list_payments(
        &self,
        request: StorageListPaymentsRequest,
    ) -> Result<Vec<Payment>, StorageError> {
        let state = self.lock();
        let mut rows: Vec<(&String, &PaymentRow)> = state
            .payments
            .iter()
            .filter(|(id, _)| state.listed(id, &request))
            .collect();
        rows.sort_by_key(|(_, row)| row.row);
        if request.sort_ascending.unwrap_or(false) {
            rows.sort_by_key(|(_, row)| row.timestamp);
        } else {
            rows.sort_by_key(|(_, row)| std::cmp::Reverse(row.timestamp));
        }
        let offset = request.offset.unwrap_or(0) as usize;
        let limit = request.limit.unwrap_or(u32::MAX) as usize;
        rows.into_iter()
            .skip(offset)
            .take(limit)
            .map(|(id, _)| state.payment(id))
            .collect()
    }

    async fn apply_payment_update(&self, payment: Payment) -> Result<bool, StorageError> {
        self.change(|state| {
            let stored = state.payments.get(&payment.id).map(|row| row.status);
            if let Some(stored) = stored
                && stored.is_final()
                && stored != payment.status
            {
                return Ok(false);
            }
            let same_status = stored == Some(payment.status);
            state.insert_payment(payment);
            Ok(!same_status)
        })
    }

    async fn insert_payment_metadata(
        &self,
        payment_id: String,
        metadata: PaymentMetadata,
    ) -> Result<(), StorageError> {
        self.change(|state| {
            let row = state.metadata.entry(payment_id).or_default();
            if metadata.parent_payment_id.is_some() {
                row.parent_payment_id = metadata.parent_payment_id;
            }
            if metadata.lnurl_pay_info.is_some() {
                row.lnurl_pay_info = metadata.lnurl_pay_info;
            }
            if metadata.lnurl_withdraw_info.is_some() {
                row.lnurl_withdraw_info = metadata.lnurl_withdraw_info;
            }
            if metadata.lnurl_description.is_some() {
                row.lnurl_description = metadata.lnurl_description;
            }
            if metadata.conversion_info.is_some() {
                row.conversion_info = metadata.conversion_info;
            }
            if metadata.conversion_status.is_some() {
                row.conversion_status = metadata.conversion_status;
            }
            Ok(())
        })
    }

    async fn get_payment_by_id(&self, id: String) -> Result<Payment, StorageError> {
        self.lock().payment(&id)
    }

    async fn get_payment_by_invoice(
        &self,
        invoice: String,
    ) -> Result<Option<Payment>, StorageError> {
        let state = self.lock();
        let found = state
            .lightning
            .iter()
            .filter(|(id, row)| row.invoice == invoice && state.payments.contains_key(*id))
            .min_by_key(|(id, _)| state.payments[*id].row)
            .map(|(id, _)| id.clone());
        found.map(|id| state.payment(&id)).transpose()
    }

    async fn get_payments_by_parent_ids(
        &self,
        parent_payment_ids: Vec<String>,
    ) -> Result<HashMap<String, Vec<Payment>>, StorageError> {
        let state = self.lock();
        let mut children: Vec<(&String, &PaymentRow, &String)> = state
            .payments
            .iter()
            .filter_map(|(id, row)| {
                let parent = state.metadata.get(id)?.parent_payment_id.as_ref()?;
                parent_payment_ids
                    .contains(parent)
                    .then_some((id, row, parent))
            })
            .collect();
        children.sort_by_key(|(_, row, _)| (row.timestamp, row.row));
        let mut result: HashMap<String, Vec<Payment>> = HashMap::new();
        for (id, _, parent) in children {
            result
                .entry(parent.clone())
                .or_default()
                .push(state.payment(id)?);
        }
        Ok(result)
    }

    async fn add_deposit(
        &self,
        txid: String,
        vout: u32,
        amount_sats: u64,
        is_mature: bool,
    ) -> Result<(), StorageError> {
        self.change(|state| {
            match state
                .unclaimed_deposits
                .iter_mut()
                .find(|d| d.txid == txid && d.vout == vout)
            {
                Some(deposit) => {
                    deposit.amount_sats = amount_sats;
                    deposit.is_mature = is_mature;
                }
                None => state.unclaimed_deposits.push(DepositInfo {
                    txid,
                    vout,
                    amount_sats,
                    is_mature,
                    refund_tx: None,
                    refund_tx_id: None,
                    refund_state: None,
                    claim_error: None,
                    instant_claim_status: None,
                    max_claim_fee: None,
                }),
            }
            Ok(())
        })
    }

    async fn delete_deposit(&self, txid: String, vout: u32) -> Result<(), StorageError> {
        self.change(|state| {
            state
                .unclaimed_deposits
                .retain(|d| !(d.txid == txid && d.vout == vout));
            Ok(())
        })
    }

    async fn list_deposits(&self) -> Result<Vec<DepositInfo>, StorageError> {
        Ok(self.lock().unclaimed_deposits.clone())
    }

    async fn update_deposit(
        &self,
        txid: String,
        vout: u32,
        payload: UpdateDepositPayload,
    ) -> Result<(), StorageError> {
        self.change(|state| {
            let Some(deposit) = state
                .unclaimed_deposits
                .iter_mut()
                .find(|d| d.txid == txid && d.vout == vout)
            else {
                return Ok(());
            };
            match payload {
                UpdateDepositPayload::ClaimError { error } => deposit.claim_error = Some(error),
                UpdateDepositPayload::Refund {
                    refund_txid,
                    refund_tx,
                    state,
                } => {
                    deposit.refund_tx = Some(refund_tx);
                    deposit.refund_tx_id = Some(refund_txid);
                    deposit.refund_state = Some(state);
                    deposit.claim_error = None;
                }
                UpdateDepositPayload::InstantClaim { status } => {
                    deposit.instant_claim_status = Some(status);
                }
                UpdateDepositPayload::RefundBroadcastState { refund_txid, state } => {
                    if deposit.refund_tx_id.as_deref() == Some(refund_txid.as_str()) {
                        deposit.refund_state = Some(state);
                    }
                }
                UpdateDepositPayload::MaxClaimFee { max_fee } => deposit.max_claim_fee = max_fee,
            }
            Ok(())
        })
    }

    async fn list_watched_deposit_addresses(
        &self,
    ) -> Result<Vec<WatchedDepositAddress>, StorageError> {
        let mut watched: Vec<WatchedDepositAddress> = self
            .lock()
            .watched
            .iter()
            .map(|(address, row)| WatchedDepositAddress {
                address: address.clone(),
                issued_at: row.issued_at,
                seen: row.seen,
            })
            .collect();
        watched.sort_by_key(|w| std::cmp::Reverse(w.issued_at));
        Ok(watched)
    }

    async fn update_watched_deposit_address(
        &self,
        address: String,
        payload: UpdateWatchedAddressPayload,
    ) -> Result<(), StorageError> {
        self.change(|state| {
            match payload {
                UpdateWatchedAddressPayload::Watch { issued_at } => {
                    state.watched.insert(
                        address,
                        WatchedRow {
                            issued_at,
                            seen: false,
                        },
                    );
                }
                UpdateWatchedAddressPayload::Seen => {
                    if let Some(row) = state.watched.get_mut(&address) {
                        row.seen = true;
                    }
                }
                UpdateWatchedAddressPayload::Unwatch { issued_at } => {
                    if state
                        .watched
                        .get(&address)
                        .is_some_and(|row| row.issued_at == issued_at)
                    {
                        state.watched.remove(&address);
                    }
                }
            }
            Ok(())
        })
    }

    async fn set_lnurl_metadata(
        &self,
        metadata: Vec<SetLnurlMetadataItem>,
    ) -> Result<(), StorageError> {
        self.change(|state| {
            for item in metadata {
                state.lnurl_receive.insert(
                    item.payment_hash,
                    LnurlReceiveRow {
                        nostr_zap_request: item.nostr_zap_request,
                        nostr_zap_receipt: item.nostr_zap_receipt,
                        sender_comment: item.sender_comment,
                    },
                );
            }
            Ok(())
        })
    }

    async fn list_contacts(
        &self,
        request: ListContactsRequest,
    ) -> Result<Vec<Contact>, StorageError> {
        let mut contacts = self.lock().contacts.clone();
        contacts.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(contacts
            .into_iter()
            .skip(request.offset.unwrap_or(0) as usize)
            .take(request.limit.unwrap_or(u32::MAX) as usize)
            .collect())
    }

    async fn get_contact(&self, id: String) -> Result<Contact, StorageError> {
        self.lock()
            .contacts
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or(StorageError::NotFound)
    }

    async fn insert_contact(&self, contact: Contact) -> Result<(), StorageError> {
        self.change(|state| {
            match state.contacts.iter_mut().find(|c| c.id == contact.id) {
                Some(existing) => {
                    existing.name = contact.name;
                    existing.payment_identifier = contact.payment_identifier;
                    existing.updated_at = contact.updated_at;
                }
                None => state.contacts.push(contact),
            }
            Ok(())
        })
    }

    async fn delete_contact(&self, id: String) -> Result<(), StorageError> {
        self.change(|state| {
            state.contacts.retain(|c| c.id != id);
            Ok(())
        })
    }

    async fn set_cross_chain_swap(&self, swap: StoredCrossChainSwap) -> Result<(), StorageError> {
        self.change(|state| {
            match state
                .swaps
                .iter_mut()
                .find(|s| s.provider == swap.provider && s.id == swap.id)
            {
                Some(existing) => *existing = swap,
                None => state.swaps.push(swap),
            }
            Ok(())
        })
    }

    async fn get_cross_chain_swap(
        &self,
        provider: String,
        id: String,
    ) -> Result<Option<StoredCrossChainSwap>, StorageError> {
        Ok(self
            .lock()
            .swaps
            .iter()
            .find(|s| s.provider == provider && s.id == id)
            .cloned())
    }

    async fn list_active_cross_chain_swaps(
        &self,
        provider: String,
    ) -> Result<Vec<StoredCrossChainSwap>, StorageError> {
        Ok(self
            .lock()
            .swaps
            .iter()
            .filter(|s| s.provider == provider && !s.is_terminal)
            .cloned()
            .collect())
    }

    async fn add_outgoing_change(
        &self,
        record: UnversionedRecordChange,
    ) -> Result<u64, StorageError> {
        self.change(|state| {
            let revision = state
                .sync_outgoing
                .iter()
                .map(|row| row.revision)
                .max()
                .unwrap_or(0)
                + 1;
            state.sync_outgoing.push(OutgoingRow {
                record_type: record.id.r#type,
                data_id: record.id.data_id,
                schema_version: record.schema_version,
                updated_fields: record.updated_fields,
                revision,
            });
            Ok(revision)
        })
    }

    async fn complete_outgoing_sync(
        &self,
        record: Record,
        local_revision: u64,
    ) -> Result<(), StorageError> {
        self.change(|state| {
            state.sync_outgoing.retain(|row| {
                !(row.record_type == record.id.r#type
                    && row.data_id == record.id.data_id
                    && row.revision == local_revision)
            });
            state.upsert_state(&record);
            Ok(())
        })
    }

    async fn get_pending_outgoing_changes(
        &self,
        limit: u32,
    ) -> Result<Vec<OutgoingChange>, StorageError> {
        let state = self.lock();
        let mut rows: Vec<&OutgoingRow> = state.sync_outgoing.iter().collect();
        rows.sort_by_key(|row| row.revision);
        Ok(rows
            .into_iter()
            .take(limit as usize)
            .map(|row| state.outgoing(row))
            .collect())
    }

    async fn get_last_revision(&self) -> Result<u64, StorageError> {
        Ok(self.lock().sync_revision)
    }

    async fn insert_incoming_records(&self, records: Vec<Record>) -> Result<(), StorageError> {
        if records.is_empty() {
            return Ok(());
        }
        self.change(|state| {
            for record in &records {
                let row = RecordRow::of(record);
                match state.sync_incoming.iter_mut().find(|existing| {
                    existing.same_id(&row.record_type, &row.data_id)
                        && existing.revision == row.revision
                }) {
                    Some(existing) => *existing = row,
                    None => state.sync_incoming.push(row),
                }
            }
            Ok(())
        })
    }

    async fn delete_incoming_record(&self, record: Record) -> Result<(), StorageError> {
        self.change(|state| {
            state.sync_incoming.retain(|row| {
                !(row.same_id(&record.id.r#type, &record.id.data_id)
                    && row.revision == record.revision)
            });
            Ok(())
        })
    }

    async fn get_incoming_records(&self, limit: u32) -> Result<Vec<IncomingChange>, StorageError> {
        let state = self.lock();
        let mut rows: Vec<&RecordRow> = state.sync_incoming.iter().collect();
        rows.sort_by_key(|row| row.revision);
        Ok(rows
            .into_iter()
            .take(limit as usize)
            .map(|row| IncomingChange {
                new_state: row.record(),
                old_state: state.parent_state(&row.record_type, &row.data_id),
            })
            .collect())
    }

    async fn get_latest_outgoing_change(&self) -> Result<Option<OutgoingChange>, StorageError> {
        let state = self.lock();
        Ok(state
            .sync_outgoing
            .iter()
            .max_by_key(|row| row.revision)
            .map(|row| state.outgoing(row)))
    }

    async fn update_record_from_incoming(&self, record: Record) -> Result<(), StorageError> {
        self.change(|state| {
            state.upsert_state(&record);
            Ok(())
        })
    }
}
