use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const LEDGER_SCHEMA_VERSION: u32 = 1;
const MAX_RELAY_ACTIVITY: usize = 256;
const MAX_ANNOUNCEMENTS: usize = 32;
const MAX_JOBS: usize = 256;
const MAX_INVOICES: usize = 128;
const MAX_PAYMENTS: usize = 256;
const MAX_PAYOUTS: usize = 128;
const MAX_SETTLEMENTS: usize = 256;

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
    pub spark_address: Option<String>,
    pub bitcoin_address: Option<String>,
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
            spark_address: None,
            bitcoin_address: None,
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

pub fn save_ledger(config_path: &Path, ledger: &PylonLedger) -> Result<()> {
    let path = default_ledger_path(config_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create pylon ledger dir {}", parent.display()))?;
    }
    std::fs::write(
        path.as_path(),
        format!("{}\n", serde_json::to_string_pretty(ledger)?),
    )
    .with_context(|| format!("failed to write pylon ledger {}", path.display()))?;
    Ok(())
}

pub fn load_ledger_summary(config_path: &Path) -> Result<PylonLedgerSummary> {
    Ok(load_ledger(config_path)?.summary())
}

pub fn mutate_ledger<T, F>(config_path: &Path, mutator: F) -> Result<T>
where
    F: FnOnce(&mut PylonLedger) -> Result<T>,
{
    let _ = ensure_local_ledger(config_path)?;
    let mut ledger = load_ledger(config_path)?;
    let value = mutator(&mut ledger)?;
    save_ledger(config_path, &ledger)?;
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

#[cfg(test)]
mod tests {
    use super::{
        PylonLedger, PylonLedgerAnnouncement, PylonLedgerJob, PylonRelayActivity, PylonRelayState,
        PylonSettlementRecord, PylonWalletInvoiceRecord, PylonWalletPaymentRecord,
        ensure_local_ledger, load_ledger, load_ledger_summary, mutate_ledger,
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
}
