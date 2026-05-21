use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const LEDGER_SCHEMA_VERSION: u32 = 1;
const PROCESSED_PROVIDER_REQUESTS_SCHEMA_VERSION: u32 = 1;
const MAX_RELAY_ACTIVITY: usize = 256;
const MAX_ANNOUNCEMENTS: usize = 32;
const MAX_JOBS: usize = 256;
const MAX_INVOICES: usize = 128;
const MAX_PAYMENTS: usize = 256;
const MAX_PAYOUTS: usize = 128;
const MAX_SETTLEMENTS: usize = 256;
const MAX_PROCESSED_PROVIDER_REQUESTS: usize = 16_384;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonLedgerSummary {
    pub relay_count: usize,
    pub relay_activity_count: usize,
    pub announcement_count: usize,
    pub job_count: usize,
    pub invoice_count: usize,
    pub payment_count: usize,
    pub settlement_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonProcessedProviderRequestRecord {
    pub request_event_id: String,
    pub status: String,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonProcessedProviderRequestStore {
    pub schema_version: u32,
    #[serde(
        serialize_with = "serialize_requests_as_vec",
        deserialize_with = "deserialize_requests_from_vec"
    )]
    pub requests: HashMap<String, PylonProcessedProviderRequestRecord>,
}

impl Default for PylonProcessedProviderRequestStore {
    fn default() -> Self {
        Self {
            schema_version: PROCESSED_PROVIDER_REQUESTS_SCHEMA_VERSION,
            requests: HashMap::new(),
        }
    }
}

impl PylonProcessedProviderRequestStore {
    pub fn remember(&mut self, request_event_id: impl Into<String>, status: impl Into<String>) {
        let request_event_id = request_event_id.into();
        let status = status.into();
        let updated_at_ms = now_epoch_ms();
        let entry = self
            .requests
            .entry(request_event_id.clone())
            .or_insert_with(|| PylonProcessedProviderRequestRecord {
                request_event_id,
                status: String::new(),
                updated_at_ms,
            });
        entry.status = status;
        entry.updated_at_ms = updated_at_ms;
        if self.requests.len() > MAX_PROCESSED_PROVIDER_REQUESTS
            && let Some(oldest_key) = self
                .requests
                .iter()
                .min_by_key(|(_, v)| v.updated_at_ms)
                .map(|(k, _)| k.clone())
        {
            self.requests.remove(&oldest_key);
        }
    }

    pub fn contains(&self, request_event_id: &str) -> bool {
        self.requests.contains_key(request_event_id)
    }
}

fn serialize_requests_as_vec<S>(
    map: &HashMap<String, PylonProcessedProviderRequestRecord>,
    s: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut vec: Vec<&PylonProcessedProviderRequestRecord> = map.values().collect();
    vec.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    vec.serialize(s)
}

fn deserialize_requests_from_vec<'de, D>(
    d: D,
) -> std::result::Result<HashMap<String, PylonProcessedProviderRequestRecord>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let vec = Vec::<PylonProcessedProviderRequestRecord>::deserialize(d)?;
    Ok(vec
        .into_iter()
        .map(|r| (r.request_event_id.clone(), r))
        .collect())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonRelayConfigSnapshot {
    pub relay_urls: Vec<String>,
    pub connect_timeout_seconds: u64,
    pub last_applied_at_ms: Option<u64>,
}

impl Default for PylonRelayConfigSnapshot {
    fn default() -> Self {
        Self {
            relay_urls: Vec::new(),
            connect_timeout_seconds: 10,
            last_applied_at_ms: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonRelayState {
    pub url: String,
    pub connection_state: String,
    pub auth_state: String,
    pub last_detail: Option<String>,
    pub last_error: Option<String>,
    pub last_connected_at_ms: Option<u64>,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonRelayActivity {
    pub at_ms: u64,
    pub url: Option<String>,
    pub kind: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonLedgerAnnouncement {
    pub announcement_id: String,
    pub event_id: String,
    pub request_kind: u16,
    pub model: Option<String>,
    pub backend: Option<String>,
    pub capabilities: Vec<String>,
    pub price_msats: Option<u64>,
    pub relay_urls: Vec<String>,
    pub fingerprint: String,
    pub published_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonLedgerJob {
    pub id: String,
    pub direction: String,
    pub request_kind: u16,
    pub status: String,
    pub prompt: Option<String>,
    pub relay_url: Option<String>,
    pub provider_pubkey: Option<String>,
    pub customer_pubkey: Option<String>,
    pub model: Option<String>,
    pub bid_msats: Option<u64>,
    pub amount_msats: Option<u64>,
    pub bolt11: Option<String>,
    pub payment_id: Option<String>,
    pub settlement_id: Option<String>,
    pub request_event_id: Option<String>,
    pub result_event_id: Option<String>,
    pub feedback_event_ids: Vec<String>,
    pub result_preview: Option<String>,
    pub error_detail: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl PylonLedgerJob {
    pub fn new(
        id: impl Into<String>,
        direction: impl Into<String>,
        request_kind: u16,
        status: impl Into<String>,
    ) -> Self {
        let now = now_epoch_ms();
        Self {
            id: id.into(),
            direction: direction.into(),
            request_kind,
            status: status.into(),
            prompt: None,
            relay_url: None,
            provider_pubkey: None,
            customer_pubkey: None,
            model: None,
            bid_msats: None,
            amount_msats: None,
            bolt11: None,
            payment_id: None,
            settlement_id: None,
            request_event_id: None,
            result_event_id: None,
            feedback_event_ids: Vec::new(),
            result_preview: None,
            error_detail: None,
            created_at_ms: now,
            updated_at_ms: now,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonWalletInvoiceRecord {
    pub invoice_id: String,
    pub amount_sats: u64,
    pub status: String,
    pub payment_request: String,
    pub description: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonWalletPaymentRecord {
    pub payment_id: String,
    pub direction: String,
    pub status: String,
    pub amount_sats: u64,
    pub fees_sats: u64,
    pub method: String,
    pub description: Option<String>,
    pub invoice: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonWalletCreditSummary {
    pub credited_lifetime_sats: u64,
    pub credited_today_sats: u64,
    pub credited_today_count: u64,
    pub last_credit_at_ms: Option<u64>,
    pub last_full_sync_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonLedgerPayout {
    pub payout_id: String,
    pub payment_id: Option<String>,
    pub status: String,
    pub amount_sats: Option<u64>,
    pub fees_sats: Option<u64>,
    pub invoice: Option<String>,
    pub payout_destination: Option<String>,
    pub detail: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonWalletLedger {
    pub runtime_status: Option<String>,
    pub last_error: Option<String>,
    pub network: Option<String>,
    pub last_balance_sats: Option<u64>,
    pub last_balance_at_ms: Option<u64>,
    pub bitcoin_address: Option<String>,
    #[serde(default)]
    pub credits: PylonWalletCreditSummary,
    pub invoices: Vec<PylonWalletInvoiceRecord>,
    pub payments: Vec<PylonWalletPaymentRecord>,
}

impl Default for PylonWalletLedger {
    fn default() -> Self {
        Self {
            runtime_status: None,
            last_error: None,
            network: None,
            last_balance_sats: None,
            last_balance_at_ms: None,
            bitcoin_address: None,
            credits: PylonWalletCreditSummary::default(),
            invoices: Vec::new(),
            payments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonSettlementRecord {
    pub settlement_id: String,
    pub job_id: String,
    pub direction: String,
    pub status: String,
    pub amount_msats: u64,
    pub payment_reference: Option<String>,
    pub receipt_detail: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonLedger {
    pub schema_version: u32,
    #[serde(default)]
    pub relay_config: PylonRelayConfigSnapshot,
    #[serde(default)]
    pub relay_state: Vec<PylonRelayState>,
    #[serde(default)]
    pub relay_activity: Vec<PylonRelayActivity>,
    #[serde(default)]
    pub announcements: Vec<PylonLedgerAnnouncement>,
    #[serde(default)]
    pub jobs: Vec<PylonLedgerJob>,
    #[serde(default)]
    pub wallet: PylonWalletLedger,
    #[serde(default)]
    pub payouts: Vec<PylonLedgerPayout>,
    #[serde(default)]
    pub settlements: Vec<PylonSettlementRecord>,
}

impl Default for PylonLedger {
    fn default() -> Self {
        Self {
            schema_version: LEDGER_SCHEMA_VERSION,
            relay_config: PylonRelayConfigSnapshot::default(),
            relay_state: Vec::new(),
            relay_activity: Vec::new(),
            announcements: Vec::new(),
            jobs: Vec::new(),
            wallet: PylonWalletLedger::default(),
            payouts: Vec::new(),
            settlements: Vec::new(),
        }
    }
}

impl PylonLedger {
    pub fn summary(&self) -> PylonLedgerSummary {
        PylonLedgerSummary {
            relay_count: self.relay_state.len(),
            relay_activity_count: self.relay_activity.len(),
            announcement_count: self.announcements.len(),
            job_count: self.jobs.len(),
            invoice_count: self.wallet.invoices.len(),
            payment_count: self.wallet.payments.len(),
            settlement_count: self.settlements.len(),
        }
    }

    pub fn set_relay_config(&mut self, relay_urls: Vec<String>, connect_timeout_seconds: u64) {
        let mut relay_urls = relay_urls
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        relay_urls.sort();
        relay_urls.dedup();
        self.relay_config = PylonRelayConfigSnapshot {
            relay_urls,
            connect_timeout_seconds,
            last_applied_at_ms: Some(now_epoch_ms()),
        };
    }

    pub fn upsert_relay_state(&mut self, entry: PylonRelayState) {
        if let Some(existing) = self
            .relay_state
            .iter_mut()
            .find(|existing| existing.url == entry.url)
        {
            *existing = entry;
            return;
        }
        self.relay_state.push(entry);
        self.relay_state
            .sort_by(|left, right| left.url.cmp(&right.url));
    }

    pub fn push_relay_activity(&mut self, entry: PylonRelayActivity) {
        self.relay_activity.push(entry);
        trim_tail(&mut self.relay_activity, MAX_RELAY_ACTIVITY);
    }

    pub fn upsert_announcement(&mut self, mut entry: PylonLedgerAnnouncement) {
        entry.updated_at_ms = now_epoch_ms();
        if let Some(existing) = self
            .announcements
            .iter_mut()
            .find(|existing| existing.announcement_id == entry.announcement_id)
        {
            let published_at_ms = existing.published_at_ms;
            *existing = entry;
            existing.published_at_ms = published_at_ms;
            return;
        }
        self.announcements.push(entry);
        self.announcements
            .sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        trim_tail(&mut self.announcements, MAX_ANNOUNCEMENTS);
    }

    pub fn upsert_job(&mut self, mut job: PylonLedgerJob) {
        job.updated_at_ms = now_epoch_ms();
        if let Some(existing) = self.jobs.iter_mut().find(|existing| existing.id == job.id) {
            let created_at_ms = existing.created_at_ms;
            *existing = job;
            existing.created_at_ms = created_at_ms;
            return;
        }
        self.jobs.push(job);
        self.jobs
            .sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        trim_tail(&mut self.jobs, MAX_JOBS);
    }

    pub fn upsert_wallet_invoice(&mut self, mut invoice: PylonWalletInvoiceRecord) {
        invoice.updated_at_ms = now_epoch_ms();
        if let Some(existing) = self
            .wallet
            .invoices
            .iter_mut()
            .find(|existing| existing.invoice_id == invoice.invoice_id)
        {
            let created_at_ms = existing.created_at_ms;
            *existing = invoice;
            existing.created_at_ms = created_at_ms;
            return;
        }
        self.wallet.invoices.push(invoice);
        self.wallet
            .invoices
            .sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        trim_tail(&mut self.wallet.invoices, MAX_INVOICES);
    }

    pub fn upsert_wallet_payment(&mut self, mut payment: PylonWalletPaymentRecord) {
        payment.updated_at_ms = now_epoch_ms();
        if let Some(existing) = self
            .wallet
            .payments
            .iter_mut()
            .find(|existing| existing.payment_id == payment.payment_id)
        {
            let created_at_ms = existing.created_at_ms;
            *existing = payment;
            existing.created_at_ms = created_at_ms;
            return;
        }
        self.wallet.payments.push(payment);
        self.wallet
            .payments
            .sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        trim_tail(&mut self.wallet.payments, MAX_PAYMENTS);
    }

    pub fn upsert_settlement(&mut self, mut settlement: PylonSettlementRecord) {
        settlement.updated_at_ms = now_epoch_ms();
        if let Some(existing) = self
            .settlements
            .iter_mut()
            .find(|existing| existing.settlement_id == settlement.settlement_id)
        {
            let created_at_ms = existing.created_at_ms;
            *existing = settlement;
            existing.created_at_ms = created_at_ms;
            return;
        }
        self.settlements.push(settlement);
        self.settlements
            .sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        trim_tail(&mut self.settlements, MAX_SETTLEMENTS);
    }

    pub fn upsert_payout(&mut self, mut payout: PylonLedgerPayout) {
        payout.updated_at_ms = now_epoch_ms();
        if let Some(existing) = self
            .payouts
            .iter_mut()
            .find(|existing| existing.payout_id == payout.payout_id)
        {
            let created_at_ms = existing.created_at_ms;
            *existing = payout;
            existing.created_at_ms = created_at_ms;
            return;
        }
        self.payouts.push(payout);
        self.payouts
            .sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        trim_tail(&mut self.payouts, MAX_PAYOUTS);
    }
}

pub fn default_ledger_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("ledger.json")
}

pub fn default_processed_provider_requests_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("processed-provider-requests.json")
}

pub fn ensure_local_ledger(config_path: &Path) -> Result<PathBuf> {
    let path = default_ledger_path(config_path);
    if path.exists() {
        let _ = load_ledger(config_path)?;
        return Ok(path);
    }
    save_ledger(config_path, &PylonLedger::default())?;
    Ok(path)
}

pub fn load_ledger(config_path: &Path) -> Result<PylonLedger> {
    let path = default_ledger_path(config_path);
    if !path.exists() {
        return Ok(PylonLedger::default());
    }
    let payload = std::fs::read_to_string(path.as_path())
        .with_context(|| format!("failed to read pylon ledger {}", path.display()))?;
    let mut ledger: PylonLedger = serde_json::from_str(payload.as_str())
        .with_context(|| format!("failed to parse pylon ledger {}", path.display()))?;
    if ledger.schema_version == 0 {
        ledger.schema_version = LEDGER_SCHEMA_VERSION;
    }
    Ok(ledger)
}

pub fn load_processed_provider_request_store(
    config_path: &Path,
) -> Result<PylonProcessedProviderRequestStore> {
    let path = default_processed_provider_requests_path(config_path);
    if !path.exists() {
        return Ok(PylonProcessedProviderRequestStore::default());
    }
    let payload = std::fs::read_to_string(path.as_path()).with_context(|| {
        format!(
            "failed to read processed provider request store {}",
            path.display()
        )
    })?;
    let mut store: PylonProcessedProviderRequestStore = serde_json::from_str(payload.as_str())
        .with_context(|| {
            format!(
                "failed to parse processed provider request store {}",
                path.display()
            )
        })?;
    if store.schema_version == 0 {
        store.schema_version = PROCESSED_PROVIDER_REQUESTS_SCHEMA_VERSION;
    }
    Ok(store)
}

pub fn save_ledger(config_path: &Path, ledger: &PylonLedger) -> Result<()> {
    let path = default_ledger_path(config_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create pylon ledger dir {}", parent.display()))?;
    }
    write_atomic_json_file(
        path.as_path(),
        format!("{}\n", serde_json::to_string_pretty(ledger)?).as_bytes(),
        "pylon ledger",
    )?;
    Ok(())
}

pub fn save_processed_provider_request_store(
    config_path: &Path,
    store: &PylonProcessedProviderRequestStore,
) -> Result<()> {
    let path = default_processed_provider_requests_path(config_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create processed provider request store dir {}",
                parent.display()
            )
        })?;
    }
    write_atomic_json_file(
        path.as_path(),
        format!("{}\n", serde_json::to_string_pretty(store)?).as_bytes(),
        "processed provider request store",
    )?;
    Ok(())
}

pub fn load_ledger_summary(config_path: &Path) -> Result<PylonLedgerSummary> {
    Ok(load_ledger(config_path)?.summary())
}

pub fn mutate_ledger<T, F>(config_path: &Path, mutator: F) -> Result<T>
where
    F: FnOnce(&mut PylonLedger) -> Result<T>,
{
    let mut ledger = load_ledger(config_path)?;
    let value = mutator(&mut ledger)?;
    save_ledger(config_path, &ledger)?;
    Ok(value)
}

pub fn mutate_processed_provider_request_store<T, F>(config_path: &Path, mutator: F) -> Result<T>
where
    F: FnOnce(&mut PylonProcessedProviderRequestStore) -> Result<T>,
{
    let mut store = load_processed_provider_request_store(config_path)?;
    let value = mutator(&mut store)?;
    save_processed_provider_request_store(config_path, &store)?;
    Ok(value)
}

fn trim_tail<T>(entries: &mut Vec<T>, limit: usize) {
    if entries.len() > limit {
        entries.truncate(limit);
    }
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn write_atomic_json_file(path: &Path, payload: &[u8], label: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("state.json");
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name,
        std::process::id(),
        now_epoch_ms()
    ));
    std::fs::write(temp_path.as_path(), payload).with_context(|| {
        format!(
            "failed to write temporary {} {}",
            label,
            temp_path.display()
        )
    })?;
    if let Err(error) = std::fs::rename(temp_path.as_path(), path) {
        let _ = std::fs::remove_file(temp_path.as_path());
        return Err(error)
            .with_context(|| format!("failed to replace {} {}", label, path.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PylonLedger, PylonLedgerAnnouncement, PylonLedgerJob, PylonRelayActivity, PylonRelayState,
        PylonSettlementRecord, PylonWalletInvoiceRecord, PylonWalletPaymentRecord,
        default_ledger_path, default_processed_provider_requests_path, ensure_local_ledger,
        load_ledger, load_ledger_summary, load_processed_provider_request_store, mutate_ledger,
        mutate_processed_provider_request_store, save_ledger,
    };
    use tempfile::tempdir;

    #[test]
    fn ensure_local_ledger_bootstraps_empty_store() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("config.json");
        let ledger_path = ensure_local_ledger(config_path.as_path()).expect("ensure ledger");
        assert!(ledger_path.exists());
        let ledger = load_ledger(config_path.as_path()).expect("load ledger");
        assert_eq!(ledger, PylonLedger::default());
    }

    #[test]
    fn mutate_ledger_persists_relay_job_wallet_and_settlement_state() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("config.json");
        mutate_ledger(config_path.as_path(), |ledger| {
            ledger.set_relay_config(
                vec![
                    String::from("wss://relay.damus.io"),
                    String::from("wss://nexus.openagents.com"),
                ],
                12,
            );
            ledger.upsert_relay_state(PylonRelayState {
                url: String::from("wss://nexus.openagents.com"),
                connection_state: String::from("connected"),
                auth_state: String::from("ready"),
                last_detail: Some(String::from("connected cleanly")),
                last_error: None,
                last_connected_at_ms: Some(1),
                updated_at_ms: 1,
            });
            ledger.push_relay_activity(PylonRelayActivity {
                at_ms: 2,
                url: Some(String::from("wss://nexus.openagents.com")),
                kind: String::from("connect"),
                detail: String::from("connected"),
            });
            ledger.upsert_announcement(PylonLedgerAnnouncement {
                announcement_id: String::from("handler:5050"),
                event_id: String::from("handler-event-001"),
                request_kind: 5050,
                model: Some(String::from("gemma4-e4b-local:latest")),
                backend: Some(String::from("gpt_oss")),
                capabilities: vec![String::from("nip90.5050")],
                price_msats: Some(42_000),
                relay_urls: vec![String::from("wss://nexus.openagents.com")],
                fingerprint: String::from("fingerprint-001"),
                published_at_ms: 2,
                updated_at_ms: 2,
            });
            let mut job = PylonLedgerJob::new("job-001", "buyer", 5050, "pending");
            job.prompt = Some(String::from("hello"));
            job.relay_url = Some(String::from("wss://nexus.openagents.com"));
            ledger.upsert_job(job);
            ledger.upsert_wallet_invoice(PylonWalletInvoiceRecord {
                invoice_id: String::from("invoice-001"),
                amount_sats: 42,
                status: String::from("open"),
                payment_request: String::from("lnbc1example"),
                description: Some(String::from("test")),
                created_at_ms: 3,
                updated_at_ms: 3,
            });
            ledger.upsert_wallet_payment(PylonWalletPaymentRecord {
                payment_id: String::from("payment-001"),
                direction: String::from("send"),
                status: String::from("completed"),
                amount_sats: 42,
                fees_sats: 1,
                method: String::from("lightning"),
                description: Some(String::from("test")),
                invoice: Some(String::from("lnbc1example")),
                created_at_ms: 4,
                updated_at_ms: 4,
            });
            ledger.upsert_settlement(PylonSettlementRecord {
                settlement_id: String::from("settlement-001"),
                job_id: String::from("job-001"),
                direction: String::from("buyer"),
                status: String::from("settled"),
                amount_msats: 42_000,
                payment_reference: Some(String::from("payment-001")),
                receipt_detail: Some(String::from("paid and accepted")),
                created_at_ms: 5,
                updated_at_ms: 5,
            });
            Ok(())
        })
        .expect("mutate ledger");

        let ledger = load_ledger(config_path.as_path()).expect("load ledger");
        assert_eq!(
            ledger.relay_config.relay_urls,
            vec![
                String::from("wss://nexus.openagents.com"),
                String::from("wss://relay.damus.io"),
            ]
        );
        assert_eq!(ledger.relay_state.len(), 1);
        assert_eq!(ledger.announcements.len(), 1);
        assert_eq!(ledger.jobs.len(), 1);
        assert_eq!(ledger.wallet.invoices.len(), 1);
        assert_eq!(ledger.wallet.payments.len(), 1);
        assert_eq!(ledger.settlements.len(), 1);

        let summary = load_ledger_summary(config_path.as_path()).expect("ledger summary");
        assert_eq!(summary.relay_count, 1);
        assert_eq!(summary.relay_activity_count, 1);
        assert_eq!(summary.announcement_count, 1);
        assert_eq!(summary.job_count, 1);
        assert_eq!(summary.invoice_count, 1);
        assert_eq!(summary.payment_count, 1);
        assert_eq!(summary.settlement_count, 1);
    }

    #[test]
    fn processed_provider_request_store_persists_and_dedupes() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("config.json");
        mutate_processed_provider_request_store(config_path.as_path(), |store| {
            store.remember("request-001", "completed_local");
            store.remember("request-002", "failed_local");
            store.remember("request-001", "settled");
            Ok(())
        })
        .expect("mutate processed provider request store");

        let path = default_processed_provider_requests_path(config_path.as_path());
        assert!(path.exists());

        let store =
            load_processed_provider_request_store(config_path.as_path()).expect("load store");
        assert_eq!(store.schema_version, 1);
        assert_eq!(store.requests.len(), 2);
        assert!(store.contains("request-001"));
        assert!(store.contains("request-002"));
        assert_eq!(
            store
                .requests
                .get("request-001")
                .expect("request-001")
                .status,
            "settled"
        );
    }

    #[test]
    fn save_ledger_replaces_file_without_leaving_temp_artifacts() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("config.json");
        let mut ledger = PylonLedger::default();
        ledger.upsert_job(PylonLedgerJob::new(
            "job-atomic-001",
            "provider",
            5050,
            "completed_local",
        ));

        save_ledger(config_path.as_path(), &ledger).expect("save ledger");

        let ledger_path = default_ledger_path(config_path.as_path());
        assert!(ledger_path.exists());
        let entries = std::fs::read_dir(dir.path())
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert!(
            entries
                .iter()
                .all(|entry| !entry.contains(".ledger.json.tmp-")),
            "atomic save should not leak temporary files"
        );
        assert_eq!(
            load_ledger(config_path.as_path())
                .expect("load ledger")
                .jobs
                .len(),
            1
        );
    }
}
