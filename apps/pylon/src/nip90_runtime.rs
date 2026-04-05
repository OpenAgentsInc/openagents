use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nostr::nip89::{HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO, PricingInfo};
use nostr::nip90::{
    InputType, JobFeedback, JobInput, JobRequest, JobResult, JobStatus, create_job_feedback_event,
    create_job_request_event, create_job_result_event,
};
use nostr::{Event, EventTemplate, finalize_event};
use nostr_client::{PoolConfig, RelayConfig, RelayMessage, RelayPool};
use openagents_provider_substrate::ProviderAdvertisedProduct;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::{
    LocalGemmaChatBackend, LocalGemmaChatEvent, LocalGemmaChatTarget, PylonConfig,
    PylonLedgerAnnouncement, PylonLedgerJob, PylonRelayActivity, PylonSettlementRecord,
    PylonWalletPaymentRecord, WalletHistoryReport, WalletInvoiceReport,
    create_wallet_invoice_report, ensure_identity, load_config_and_status, load_ledger,
    load_wallet_history_report, mutate_ledger, now_epoch_ms, products_from_status,
};

const ANNOUNCEMENT_KIND_TEXT_GENERATION: u16 = nostr::nip90::KIND_JOB_TEXT_GENERATION;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnouncementAction {
    Show,
    Publish,
    Refresh,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderIntakeEntry {
    pub request_event_id: String,
    pub requester_pubkey: String,
    pub relay_url: Option<String>,
    pub targeted: bool,
    pub decision: String,
    pub drop_reason: Option<String>,
    pub prompt_preview: Option<String>,
    pub model: Option<String>,
    pub bid_msats: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderIntakeReport {
    pub seconds: u64,
    pub provider_pubkey: String,
    pub local_ready: bool,
    pub local_model: Option<String>,
    pub matched_count: usize,
    pub dropped_count: usize,
    pub entries: Vec<ProviderIntakeEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderRunEntry {
    pub request_event_id: String,
    pub requester_pubkey: String,
    pub relay_url: Option<String>,
    pub status: String,
    pub prompt_preview: Option<String>,
    pub model: Option<String>,
    pub bid_msats: Option<u64>,
    pub amount_msats: Option<u64>,
    pub bolt11: Option<String>,
    pub payment_id: Option<String>,
    pub settlement_id: Option<String>,
    pub result_preview: Option<String>,
    pub error_detail: Option<String>,
    pub feedback_event_ids: Vec<String>,
    pub result_event_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderRunReport {
    pub seconds: u64,
    pub provider_pubkey: String,
    pub local_model: Option<String>,
    pub accepted_count: usize,
    pub payment_required_count: usize,
    pub settled_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub dropped_count: usize,
    pub entries: Vec<ProviderRunEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnnouncementReport {
    pub node_label: String,
    pub provider_pubkey: String,
    pub status: String,
    pub request_kind: u16,
    pub handler_event_id: Option<String>,
    pub model: Option<String>,
    pub backend: Option<String>,
    pub capabilities: Vec<String>,
    pub price_msats: Option<u64>,
    pub relay_urls: Vec<String>,
    pub published_at_ms: Option<u64>,
    pub fingerprint: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
struct AnnouncementSpec {
    model: String,
    backend: String,
    capabilities: Vec<String>,
    price_msats: Option<u64>,
    fingerprint: String,
}

#[derive(Clone, Debug)]
struct ObservedProviderRequest {
    event: Event,
    entry: ProviderIntakeEntry,
}

#[derive(Clone, Debug)]
struct ProviderRequestCollection {
    provider_pubkey: String,
    desired_mode: openagents_provider_substrate::ProviderDesiredMode,
    spec: Option<AnnouncementSpec>,
    observed: Vec<ObservedProviderRequest>,
}

#[derive(Clone, Debug)]
struct ProviderPaymentRequirement {
    bolt11: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuyerJobSubmitRequest {
    pub prompt: Option<String>,
    pub request_json: Option<String>,
    pub bid_msats: Option<u64>,
    pub model: Option<String>,
    pub provider_pubkey: Option<String>,
    pub output_mime: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuyerJobSubmitReport {
    pub job_id: String,
    pub request_event_id: String,
    pub customer_pubkey: String,
    pub relay_urls: Vec<String>,
    pub provider_pubkey: Option<String>,
    pub model: Option<String>,
    pub bid_msats: Option<u64>,
    pub output_mime: Option<String>,
    pub prompt_preview: Option<String>,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuyerJobWatchEntry {
    pub request_event_id: String,
    pub relay_url: Option<String>,
    pub event_id: String,
    pub event_kind: String,
    pub status: String,
    pub amount_msats: Option<u64>,
    pub bolt11: Option<String>,
    pub result_preview: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuyerJobWatchReport {
    pub seconds: u64,
    pub tracked_request_ids: Vec<String>,
    pub feedback_count: usize,
    pub result_count: usize,
    pub entries: Vec<BuyerJobWatchEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
struct StructuredBuyerJobPayload {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    inputs: Vec<StructuredBuyerJobInput>,
    #[serde(default)]
    params: BTreeMap<String, String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    bid_msats: Option<u64>,
    #[serde(default)]
    output: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
struct StructuredBuyerJobInput {
    #[serde(alias = "type")]
    input_type: String,
    data: String,
    #[serde(default)]
    relay: Option<String>,
    #[serde(default)]
    marker: Option<String>,
}

#[cfg(test)]
type TestWalletInvoiceHook =
    Box<dyn Fn(u64, Option<String>, Option<u32>) -> Result<WalletInvoiceReport> + Send + Sync>;

#[cfg(test)]
type TestWalletPaymentsHook = Box<dyn Fn() -> Result<Vec<PylonWalletPaymentRecord>> + Send + Sync>;

#[cfg(test)]
static TEST_WALLET_INVOICE_HOOK: std::sync::OnceLock<
    std::sync::Mutex<Option<TestWalletInvoiceHook>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
static TEST_WALLET_PAYMENTS_HOOK: std::sync::OnceLock<
    std::sync::Mutex<Option<TestWalletPaymentsHook>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
static TEST_RUNTIME_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn set_test_wallet_invoice_hook(hook: Option<TestWalletInvoiceHook>) {
    let slot = TEST_WALLET_INVOICE_HOOK.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().expect("test wallet invoice hook lock") = hook;
}

#[cfg(test)]
pub(crate) fn set_test_wallet_payments_hook(hook: Option<TestWalletPaymentsHook>) {
    let slot = TEST_WALLET_PAYMENTS_HOOK.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().expect("test wallet payments hook lock") = hook;
}

#[cfg(test)]
pub(crate) fn lock_test_runtime() -> std::sync::MutexGuard<'static, ()> {
    TEST_RUNTIME_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("test runtime lock")
}

pub async fn load_announcement_report(config_path: &Path) -> Result<AnnouncementReport> {
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let (_, status) = load_config_and_status(config_path).await?;
    let ledger = load_ledger(config_path)?;
    let latest = ledger.announcements.first().cloned();
    let spec = announcement_spec(&config, &status);
    Ok(build_announcement_report(
        &config,
        identity.public_key_hex.as_str(),
        latest.as_ref(),
        spec.as_ref(),
        None,
    ))
}

pub async fn publish_announcement_report(
    config_path: &Path,
    refresh_only: bool,
) -> Result<AnnouncementReport> {
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let (_, status) = load_config_and_status(config_path).await?;
    let spec = announcement_spec(&config, &status)
        .ok_or_else(|| anyhow::anyhow!("no eligible local text-generation supply is ready"))?;
    let existing = load_ledger(config_path)?.announcements.first().cloned();
    if refresh_only
        && existing
            .as_ref()
            .is_some_and(|existing| existing.fingerprint == spec.fingerprint)
    {
        return Ok(build_announcement_report(
            &config,
            identity.public_key_hex.as_str(),
            existing.as_ref(),
            Some(&spec),
            Some("announcement already matches the current local supply".to_string()),
        ));
    }

    let event = publish_announcement_event(&config, &identity, &spec).await?;
    let now = crate::now_epoch_ms() as u64;
    let record = PylonLedgerAnnouncement {
        announcement_id: format!("handler:{}", ANNOUNCEMENT_KIND_TEXT_GENERATION),
        event_id: event.id.clone(),
        request_kind: ANNOUNCEMENT_KIND_TEXT_GENERATION,
        model: Some(spec.model.clone()),
        backend: Some(spec.backend.clone()),
        capabilities: spec.capabilities.clone(),
        price_msats: spec.price_msats,
        relay_urls: config.relay_urls.clone(),
        fingerprint: spec.fingerprint.clone(),
        published_at_ms: now,
        updated_at_ms: now,
    };
    mutate_ledger(config_path, |ledger| {
        ledger.upsert_announcement(record.clone());
        ledger.push_relay_activity(crate::PylonRelayActivity {
            at_ms: now,
            url: None,
            kind: "announcement.published".to_string(),
            detail: format!(
                "published kind:{} handler {} for model {}",
                ANNOUNCEMENT_KIND_TEXT_GENERATION, event.id, spec.model
            ),
        });
        Ok(())
    })?;

    Ok(build_announcement_report(
        &config,
        identity.public_key_hex.as_str(),
        Some(&record),
        Some(&spec),
        Some(format!("published handler event {}", event.id)),
    ))
}

pub fn render_announcement_report(report: &AnnouncementReport) -> String {
    let mut lines = vec![
        format!("node_label: {}", report.node_label),
        format!("provider_pubkey: {}", report.provider_pubkey),
        format!("status: {}", report.status),
        format!("request_kind: {}", report.request_kind),
        format!(
            "handler_event_id: {}",
            report.handler_event_id.as_deref().unwrap_or("none")
        ),
        format!("model: {}", report.model.as_deref().unwrap_or("none")),
        format!("backend: {}", report.backend.as_deref().unwrap_or("none")),
        format!(
            "capabilities: {}",
            comma_or_none(report.capabilities.as_slice())
        ),
        format!(
            "price_msats: {}",
            report
                .price_msats
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("relays: {}", comma_or_none(report.relay_urls.as_slice())),
        format!(
            "published_at_ms: {}",
            report
                .published_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("fingerprint: {}", report.fingerprint),
    ];
    if let Some(detail) = report.detail.as_deref() {
        lines.push(format!("detail: {detail}"));
    }
    lines.join("\n")
}

pub async fn scan_provider_requests(
    config_path: &Path,
    seconds: u64,
) -> Result<ProviderIntakeReport> {
    let collected = collect_provider_requests(config_path, seconds).await?;
    let report = ProviderIntakeReport {
        seconds,
        provider_pubkey: collected.provider_pubkey.clone(),
        local_ready: collected.spec.is_some(),
        local_model: collected.spec.as_ref().map(|spec| spec.model.clone()),
        matched_count: collected
            .observed
            .iter()
            .map(|observed| &observed.entry)
            .filter(|entry| entry.decision == "match")
            .count(),
        dropped_count: collected
            .observed
            .iter()
            .map(|observed| &observed.entry)
            .filter(|entry| entry.decision == "drop")
            .count(),
        entries: collected
            .observed
            .into_iter()
            .map(|observed| observed.entry)
            .collect(),
    };
    persist_provider_intake(config_path, &report)?;
    Ok(report)
}

pub fn render_provider_intake_report(report: &ProviderIntakeReport) -> String {
    let mut lines = vec![
        format!("provider_pubkey: {}", report.provider_pubkey),
        format!("scan_seconds: {}", report.seconds),
        format!("local_ready: {}", report.local_ready),
        format!(
            "local_model: {}",
            report.local_model.as_deref().unwrap_or("none")
        ),
        format!("matched_count: {}", report.matched_count),
        format!("dropped_count: {}", report.dropped_count),
    ];
    for entry in &report.entries {
        lines.push(String::new());
        lines.push(format!("request_event_id: {}", entry.request_event_id));
        lines.push(format!("requester_pubkey: {}", entry.requester_pubkey));
        lines.push(format!(
            "relay_url: {}",
            entry.relay_url.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!("targeted: {}", entry.targeted));
        lines.push(format!("decision: {}", entry.decision));
        if let Some(reason) = entry.drop_reason.as_deref() {
            lines.push(format!("drop_reason: {reason}"));
        }
        if let Some(prompt_preview) = entry.prompt_preview.as_deref() {
            lines.push(format!("prompt_preview: {prompt_preview}"));
        }
        if let Some(model) = entry.model.as_deref() {
            lines.push(format!("model: {model}"));
        }
        if let Some(bid_msats) = entry.bid_msats {
            lines.push(format!("bid_msats: {bid_msats}"));
        }
    }
    lines.join("\n")
}

pub async fn run_provider_requests(config_path: &Path, seconds: u64) -> Result<ProviderRunReport> {
    let collected = collect_provider_requests(config_path, seconds).await?;
    let local_target = collected.spec.as_ref().map(spec_to_local_target);
    let price_msats = collected.spec.as_ref().and_then(|spec| spec.price_msats);
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let signer_key = decode_private_key_hex(identity.private_key_hex.as_str())?;
    let mut publish_pool: Option<RelayPool> = None;
    let known_jobs = load_ledger(config_path)?
        .jobs
        .into_iter()
        .map(|job| (job.id.clone(), job))
        .collect::<BTreeMap<_, _>>();
    let mut wallet_payments: Option<Vec<PylonWalletPaymentRecord>> = None;
    let mut report_entries = Vec::new();
    let mut accepted_count = 0usize;
    let mut payment_required_count = 0usize;
    let mut settled_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;
    let mut dropped_count = 0usize;

    for observed in collected.observed {
        let request_event_id = observed.entry.request_event_id.clone();
        let existing_job = known_jobs.get(request_event_id.as_str()).cloned();
        if existing_job
            .as_ref()
            .is_some_and(|job| provider_job_blocks_reintake(job.status.as_str()))
        {
            report_entries.push(ProviderRunEntry {
                request_event_id: observed.entry.request_event_id.clone(),
                requester_pubkey: observed.entry.requester_pubkey.clone(),
                relay_url: observed.entry.relay_url.clone(),
                status: "skipped_duplicate".to_string(),
                prompt_preview: observed.entry.prompt_preview.clone(),
                model: observed.entry.model.clone(),
                bid_msats: observed.entry.bid_msats,
                amount_msats: None,
                bolt11: None,
                payment_id: existing_job.as_ref().and_then(|job| job.payment_id.clone()),
                settlement_id: existing_job
                    .as_ref()
                    .and_then(|job| job.settlement_id.clone()),
                result_preview: None,
                error_detail: Some("job already handled locally".to_string()),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        }

        if observed.entry.decision != "match" {
            dropped_count += 1;
            persist_provider_run_state(
                config_path,
                collected.provider_pubkey.as_str(),
                &observed.entry,
                "observed_drop",
                observed.entry.drop_reason.as_deref(),
                None,
                None,
                None,
                None,
                None,
            )?;
            report_entries.push(ProviderRunEntry {
                request_event_id: observed.entry.request_event_id.clone(),
                requester_pubkey: observed.entry.requester_pubkey.clone(),
                relay_url: observed.entry.relay_url.clone(),
                status: "dropped".to_string(),
                prompt_preview: observed.entry.prompt_preview.clone(),
                model: observed.entry.model.clone(),
                bid_msats: observed.entry.bid_msats,
                amount_msats: None,
                bolt11: None,
                payment_id: None,
                settlement_id: None,
                result_preview: None,
                error_detail: observed.entry.drop_reason.clone(),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        }

        if collected.desired_mode != openagents_provider_substrate::ProviderDesiredMode::Online {
            dropped_count += 1;
            persist_provider_run_state(
                config_path,
                collected.provider_pubkey.as_str(),
                &observed.entry,
                "rejected_policy",
                Some("provider_not_online"),
                None,
                None,
                None,
                None,
                None,
            )?;
            report_entries.push(ProviderRunEntry {
                request_event_id: observed.entry.request_event_id.clone(),
                requester_pubkey: observed.entry.requester_pubkey.clone(),
                relay_url: observed.entry.relay_url.clone(),
                status: "dropped".to_string(),
                prompt_preview: observed.entry.prompt_preview.clone(),
                model: observed.entry.model.clone(),
                bid_msats: observed.entry.bid_msats,
                amount_msats: None,
                bolt11: None,
                payment_id: None,
                settlement_id: None,
                result_preview: None,
                error_detail: Some("provider_not_online".to_string()),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        }

        let Some(target) = local_target.clone() else {
            dropped_count += 1;
            persist_provider_run_state(
                config_path,
                collected.provider_pubkey.as_str(),
                &observed.entry,
                "rejected_supply",
                Some("no_local_supply"),
                None,
                None,
                None,
                None,
                None,
            )?;
            report_entries.push(ProviderRunEntry {
                request_event_id: observed.entry.request_event_id.clone(),
                requester_pubkey: observed.entry.requester_pubkey.clone(),
                relay_url: observed.entry.relay_url.clone(),
                status: "dropped".to_string(),
                prompt_preview: observed.entry.prompt_preview.clone(),
                model: observed.entry.model.clone(),
                bid_msats: observed.entry.bid_msats,
                amount_msats: None,
                bolt11: None,
                payment_id: None,
                settlement_id: None,
                result_preview: None,
                error_detail: Some("no_local_supply".to_string()),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        };

        let request = match JobRequest::from_event(&observed.event) {
            Ok(request) => request,
            Err(_) => {
                dropped_count += 1;
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "rejected_input",
                    Some("invalid_request"),
                    None,
                    None,
                    None,
                    None,
                    None,
                )?;
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "dropped".to_string(),
                    prompt_preview: observed.entry.prompt_preview.clone(),
                    model: observed.entry.model.clone(),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: None,
                    bolt11: None,
                    payment_id: None,
                    settlement_id: None,
                    result_preview: None,
                    error_detail: Some("invalid_request".to_string()),
                    feedback_event_ids: Vec::new(),
                    result_event_id: None,
                });
                continue;
            }
        };

        if !request_model_matches_target(observed.entry.model.as_deref(), target.model.as_str()) {
            dropped_count += 1;
            persist_provider_run_state(
                config_path,
                collected.provider_pubkey.as_str(),
                &observed.entry,
                "rejected_model",
                Some("model_unavailable"),
                None,
                None,
                None,
                None,
                None,
            )?;
            report_entries.push(ProviderRunEntry {
                request_event_id: observed.entry.request_event_id.clone(),
                requester_pubkey: observed.entry.requester_pubkey.clone(),
                relay_url: observed.entry.relay_url.clone(),
                status: "dropped".to_string(),
                prompt_preview: observed.entry.prompt_preview.clone(),
                model: Some(target.model.clone()),
                bid_msats: observed.entry.bid_msats,
                amount_msats: None,
                bolt11: None,
                payment_id: None,
                settlement_id: None,
                result_preview: None,
                error_detail: Some("model_unavailable".to_string()),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        }

        let Some(prompt) = request_prompt(&request) else {
            dropped_count += 1;
            persist_provider_run_state(
                config_path,
                collected.provider_pubkey.as_str(),
                &observed.entry,
                "rejected_input",
                Some("missing_text_input"),
                None,
                None,
                None,
                None,
                None,
            )?;
            report_entries.push(ProviderRunEntry {
                request_event_id: observed.entry.request_event_id.clone(),
                requester_pubkey: observed.entry.requester_pubkey.clone(),
                relay_url: observed.entry.relay_url.clone(),
                status: "dropped".to_string(),
                prompt_preview: observed.entry.prompt_preview.clone(),
                model: Some(target.model.clone()),
                bid_msats: observed.entry.bid_msats,
                amount_msats: None,
                bolt11: None,
                payment_id: None,
                settlement_id: None,
                result_preview: None,
                error_detail: Some("missing_text_input".to_string()),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        };

        let mut settled_payment = None::<PylonWalletPaymentRecord>;
        let mut settlement_id = None::<String>;

        if let Some(price_msats) = price_msats
            .filter(|value| *value > 0)
            .filter(|_| observed.entry.bid_msats.is_some())
        {
            if let Some(existing_job) = existing_job
                .as_ref()
                .filter(|job| job.status == "payment_required")
            {
                let Some(bolt11) = existing_job
                    .bolt11
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                else {
                    failed_count += 1;
                    persist_provider_run_state(
                        config_path,
                        collected.provider_pubkey.as_str(),
                        &observed.entry,
                        "invoice_failed",
                        Some("payment_required_job_missing_invoice"),
                        None,
                        None,
                        None,
                        Some(price_msats),
                        None,
                    )?;
                    report_entries.push(ProviderRunEntry {
                        request_event_id: observed.entry.request_event_id.clone(),
                        requester_pubkey: observed.entry.requester_pubkey.clone(),
                        relay_url: observed.entry.relay_url.clone(),
                        status: "failed".to_string(),
                        prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                        model: Some(target.model.clone()),
                        bid_msats: observed.entry.bid_msats,
                        amount_msats: Some(price_msats),
                        bolt11: None,
                        payment_id: None,
                        settlement_id: None,
                        result_preview: None,
                        error_detail: Some("payment_required_job_missing_invoice".to_string()),
                        feedback_event_ids: existing_job.feedback_event_ids.clone(),
                        result_event_id: existing_job.result_event_id.clone(),
                    });
                    continue;
                };
                let payments = match wallet_payments.as_ref() {
                    Some(payments) => payments.clone(),
                    None => {
                        let payments = load_provider_wallet_payments(config_path).await?;
                        wallet_payments = Some(payments.clone());
                        payments
                    }
                };
                if let Some(payment) = find_settled_receive_payment(payments.as_slice(), bolt11) {
                    settlement_id = Some(record_provider_payment_received(
                        config_path,
                        &observed.entry,
                        payment.clone(),
                        price_msats,
                    )?);
                    settled_payment = Some(payment);
                } else {
                    report_entries.push(ProviderRunEntry {
                        request_event_id: observed.entry.request_event_id.clone(),
                        requester_pubkey: observed.entry.requester_pubkey.clone(),
                        relay_url: observed.entry.relay_url.clone(),
                        status: "payment_required".to_string(),
                        prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                        model: Some(target.model.clone()),
                        bid_msats: observed.entry.bid_msats,
                        amount_msats: Some(existing_job.amount_msats.unwrap_or(price_msats)),
                        bolt11: Some(bolt11.to_string()),
                        payment_id: existing_job.payment_id.clone(),
                        settlement_id: existing_job.settlement_id.clone(),
                        result_preview: None,
                        error_detail: Some("awaiting_payment".to_string()),
                        feedback_event_ids: existing_job.feedback_event_ids.clone(),
                        result_event_id: existing_job.result_event_id.clone(),
                    });
                    continue;
                }
            } else {
                if observed
                    .entry
                    .bid_msats
                    .is_some_and(|bid_msats| bid_msats < price_msats)
                {
                    dropped_count += 1;
                    persist_provider_run_state(
                        config_path,
                        collected.provider_pubkey.as_str(),
                        &observed.entry,
                        "rejected_policy",
                        Some("bid_below_price_floor"),
                        None,
                        None,
                        None,
                        None,
                        None,
                    )?;
                    report_entries.push(ProviderRunEntry {
                        request_event_id: observed.entry.request_event_id.clone(),
                        requester_pubkey: observed.entry.requester_pubkey.clone(),
                        relay_url: observed.entry.relay_url.clone(),
                        status: "dropped".to_string(),
                        prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                        model: Some(target.model.clone()),
                        bid_msats: observed.entry.bid_msats,
                        amount_msats: Some(price_msats),
                        bolt11: None,
                        payment_id: None,
                        settlement_id: None,
                        result_preview: None,
                        error_detail: Some("bid_below_price_floor".to_string()),
                        feedback_event_ids: Vec::new(),
                        result_event_id: None,
                    });
                    continue;
                }

                accepted_count += 1;
                let payment_requirement = match create_provider_payment_requirement(
                    config_path,
                    &observed.entry,
                    price_msats,
                )
                .await
                {
                    Ok(requirement) => requirement,
                    Err(error) => {
                        failed_count += 1;
                        let error_string = error.to_string();
                        persist_provider_run_state(
                            config_path,
                            collected.provider_pubkey.as_str(),
                            &observed.entry,
                            "invoice_failed",
                            Some(error_string.as_str()),
                            None,
                            None,
                            None,
                            Some(price_msats),
                            None,
                        )?;
                        report_entries.push(ProviderRunEntry {
                            request_event_id: observed.entry.request_event_id.clone(),
                            requester_pubkey: observed.entry.requester_pubkey.clone(),
                            relay_url: observed.entry.relay_url.clone(),
                            status: "failed".to_string(),
                            prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                            model: Some(target.model.clone()),
                            bid_msats: observed.entry.bid_msats,
                            amount_msats: Some(price_msats),
                            bolt11: None,
                            payment_id: None,
                            settlement_id: None,
                            result_preview: None,
                            error_detail: Some(error_string),
                            feedback_event_ids: Vec::new(),
                            result_event_id: None,
                        });
                        continue;
                    }
                };
                let pool = match publish_pool.as_ref() {
                    Some(pool) => pool,
                    None => {
                        publish_pool = Some(build_relay_pool(&config, &identity).await?);
                        publish_pool.as_ref().expect("publish pool should exist")
                    }
                };
                let payment_event = match publish_payment_required_feedback(
                    pool,
                    &signer_key,
                    &observed.event,
                    observed.entry.relay_url.as_deref(),
                    price_msats,
                    payment_requirement.bolt11.as_str(),
                )
                .await
                {
                    Ok(event) => event,
                    Err(error) => {
                        failed_count += 1;
                        let error_string = error.to_string();
                        persist_provider_run_state(
                            config_path,
                            collected.provider_pubkey.as_str(),
                            &observed.entry,
                            "publish_failed",
                            Some(error_string.as_str()),
                            None,
                            None,
                            None,
                            Some(price_msats),
                            Some(payment_requirement.bolt11.as_str()),
                        )?;
                        report_entries.push(ProviderRunEntry {
                            request_event_id: observed.entry.request_event_id.clone(),
                            requester_pubkey: observed.entry.requester_pubkey.clone(),
                            relay_url: observed.entry.relay_url.clone(),
                            status: "failed".to_string(),
                            prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                            model: Some(target.model.clone()),
                            bid_msats: observed.entry.bid_msats,
                            amount_msats: Some(price_msats),
                            bolt11: Some(payment_requirement.bolt11.clone()),
                            payment_id: None,
                            settlement_id: None,
                            result_preview: None,
                            error_detail: Some(error_string),
                            feedback_event_ids: Vec::new(),
                            result_event_id: None,
                        });
                        continue;
                    }
                };
                payment_required_count += 1;
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "payment_required",
                    None,
                    None,
                    Some(payment_event.id.as_str()),
                    None,
                    Some(price_msats),
                    Some(payment_requirement.bolt11.as_str()),
                )?;
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "payment_required".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: Some(price_msats),
                    bolt11: Some(payment_requirement.bolt11),
                    payment_id: None,
                    settlement_id: None,
                    result_preview: None,
                    error_detail: None,
                    feedback_event_ids: vec![payment_event.id],
                    result_event_id: None,
                });
                continue;
            }
        }

        accepted_count += 1;
        persist_provider_run_state(
            config_path,
            collected.provider_pubkey.as_str(),
            &observed.entry,
            "accepted_local",
            None,
            None,
            None,
            None,
            None,
            None,
        )?;
        let pool = match publish_pool.as_ref() {
            Some(pool) => pool,
            None => {
                publish_pool = Some(build_relay_pool(&config, &identity).await?);
                publish_pool.as_ref().expect("publish pool should exist")
            }
        };
        let processing_event = match publish_processing_feedback(
            pool,
            &signer_key,
            &observed.event,
            observed.entry.relay_url.as_deref(),
            target.model.as_str(),
        )
        .await
        {
            Ok(event) => event,
            Err(error) => {
                failed_count += 1;
                let error_string = error.to_string();
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "publish_failed",
                    Some(error_string.as_str()),
                    None,
                    None,
                    None,
                    None,
                    None,
                )?;
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "failed".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: settled_payment.as_ref().map(|_| price_msats.unwrap_or(0)),
                    bolt11: existing_job.as_ref().and_then(|job| job.bolt11.clone()),
                    payment_id: settled_payment
                        .as_ref()
                        .map(|payment| payment.payment_id.clone()),
                    settlement_id: settlement_id.clone(),
                    result_preview: None,
                    error_detail: Some(error_string),
                    feedback_event_ids: Vec::new(),
                    result_event_id: None,
                });
                continue;
            }
        };
        persist_provider_run_state(
            config_path,
            collected.provider_pubkey.as_str(),
            &observed.entry,
            "processing_local",
            None,
            None,
            Some(processing_event.id.as_str()),
            None,
            None,
            None,
        )?;

        let mut output = String::new();
        let result =
            crate::stream_local_gemma_chat_target(config_path, &target, prompt.as_str(), |event| {
                if let LocalGemmaChatEvent::Delta(delta) = event {
                    output.push_str(delta.as_str());
                }
            })
            .await;
        match result {
            Ok(_) => {
                let result_preview = preview_text(output.as_str(), 280);
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "completed_local",
                    None,
                    Some(result_preview.as_str()),
                    Some(processing_event.id.as_str()),
                    None,
                    None,
                    None,
                )?;
                let result_event = match publish_job_result(
                    pool,
                    &signer_key,
                    &observed.event,
                    &request,
                    observed.entry.relay_url.as_deref(),
                    output.as_str(),
                )
                .await
                {
                    Ok(event) => event,
                    Err(error) => {
                        failed_count += 1;
                        let error_string = error.to_string();
                        persist_provider_run_state(
                            config_path,
                            collected.provider_pubkey.as_str(),
                            &observed.entry,
                            "publish_failed",
                            Some(error_string.as_str()),
                            Some(result_preview.as_str()),
                            Some(processing_event.id.as_str()),
                            None,
                            None,
                            None,
                        )?;
                        report_entries.push(ProviderRunEntry {
                            request_event_id: observed.entry.request_event_id.clone(),
                            requester_pubkey: observed.entry.requester_pubkey.clone(),
                            relay_url: observed.entry.relay_url.clone(),
                            status: "failed".to_string(),
                            prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                            model: Some(target.model.clone()),
                            bid_msats: observed.entry.bid_msats,
                            amount_msats: settled_payment
                                .as_ref()
                                .map(|_| price_msats.unwrap_or(0)),
                            bolt11: existing_job.as_ref().and_then(|job| job.bolt11.clone()),
                            payment_id: settled_payment
                                .as_ref()
                                .map(|payment| payment.payment_id.clone()),
                            settlement_id: settlement_id.clone(),
                            result_preview: Some(result_preview),
                            error_detail: Some(error_string.clone()),
                            feedback_event_ids: vec![processing_event.id.clone()],
                            result_event_id: None,
                        });
                        if let Some(payment) = settled_payment.as_ref() {
                            let _ = record_provider_settlement_outcome(
                                config_path,
                                &observed.entry,
                                payment,
                                price_msats.unwrap_or(0),
                                settlement_id.clone(),
                                "delivery_failed_after_payment",
                                error_string.as_str(),
                            );
                        }
                        continue;
                    }
                };
                completed_count += 1;
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "completed_local",
                    None,
                    Some(result_preview.as_str()),
                    Some(processing_event.id.as_str()),
                    Some(result_event.id.as_str()),
                    None,
                    None,
                )?;
                let final_status = if let Some(payment) = settled_payment.as_ref() {
                    let recorded_settlement_id = record_provider_settlement_outcome(
                        config_path,
                        &observed.entry,
                        payment,
                        price_msats.unwrap_or(0),
                        settlement_id.clone(),
                        "settled",
                        format!("published result {}", result_event.id).as_str(),
                    )?;
                    settlement_id = Some(recorded_settlement_id);
                    settled_count += 1;
                    "settled"
                } else {
                    "completed"
                };
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: final_status.to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: settled_payment.as_ref().map(|_| price_msats.unwrap_or(0)),
                    bolt11: existing_job.as_ref().and_then(|job| job.bolt11.clone()),
                    payment_id: settled_payment
                        .as_ref()
                        .map(|payment| payment.payment_id.clone()),
                    settlement_id: settlement_id.clone(),
                    result_preview: Some(result_preview),
                    error_detail: None,
                    feedback_event_ids: vec![processing_event.id.clone()],
                    result_event_id: Some(result_event.id),
                });
            }
            Err(error) => {
                failed_count += 1;
                let error_string = error.to_string();
                let error_feedback = publish_error_feedback(
                    pool,
                    &signer_key,
                    &observed.event,
                    observed.entry.relay_url.as_deref(),
                    error_string.as_str(),
                )
                .await
                .ok();
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "failed_local",
                    Some(error_string.as_str()),
                    None,
                    error_feedback.as_ref().map(|event| event.id.as_str()),
                    None,
                    None,
                    None,
                )?;
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "failed".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: settled_payment.as_ref().map(|_| price_msats.unwrap_or(0)),
                    bolt11: existing_job.as_ref().and_then(|job| job.bolt11.clone()),
                    payment_id: settled_payment
                        .as_ref()
                        .map(|payment| payment.payment_id.clone()),
                    settlement_id: settlement_id.clone(),
                    result_preview: None,
                    error_detail: Some(error_string.clone()),
                    feedback_event_ids: error_feedback
                        .map(|event| vec![processing_event.id.clone(), event.id])
                        .unwrap_or_else(|| vec![processing_event.id.clone()]),
                    result_event_id: None,
                });
                if let Some(payment) = settled_payment.as_ref() {
                    let _ = record_provider_settlement_outcome(
                        config_path,
                        &observed.entry,
                        payment,
                        price_msats.unwrap_or(0),
                        settlement_id.clone(),
                        "delivery_failed_after_payment",
                        error_string.as_str(),
                    );
                }
            }
        }
    }

    Ok(ProviderRunReport {
        seconds,
        provider_pubkey: collected.provider_pubkey,
        local_model: local_target.map(|target| target.model),
        accepted_count,
        payment_required_count,
        settled_count,
        completed_count,
        failed_count,
        dropped_count,
        entries: report_entries,
    })
}

pub fn render_provider_run_report(report: &ProviderRunReport) -> String {
    let mut lines = vec![
        format!("provider_pubkey: {}", report.provider_pubkey),
        format!("run_seconds: {}", report.seconds),
        format!(
            "local_model: {}",
            report.local_model.as_deref().unwrap_or("none")
        ),
        format!("accepted_count: {}", report.accepted_count),
        format!("payment_required_count: {}", report.payment_required_count),
        format!("settled_count: {}", report.settled_count),
        format!("completed_count: {}", report.completed_count),
        format!("failed_count: {}", report.failed_count),
        format!("dropped_count: {}", report.dropped_count),
    ];
    for entry in &report.entries {
        lines.push(String::new());
        lines.push(format!("request_event_id: {}", entry.request_event_id));
        lines.push(format!("requester_pubkey: {}", entry.requester_pubkey));
        lines.push(format!(
            "relay_url: {}",
            entry.relay_url.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!("status: {}", entry.status));
        if let Some(prompt_preview) = entry.prompt_preview.as_deref() {
            lines.push(format!("prompt_preview: {prompt_preview}"));
        }
        if let Some(model) = entry.model.as_deref() {
            lines.push(format!("model: {model}"));
        }
        if let Some(bid_msats) = entry.bid_msats {
            lines.push(format!("bid_msats: {bid_msats}"));
        }
        if let Some(amount_msats) = entry.amount_msats {
            lines.push(format!("amount_msats: {amount_msats}"));
        }
        if let Some(bolt11) = entry.bolt11.as_deref() {
            lines.push(format!("bolt11: {bolt11}"));
        }
        if let Some(payment_id) = entry.payment_id.as_deref() {
            lines.push(format!("payment_id: {payment_id}"));
        }
        if let Some(settlement_id) = entry.settlement_id.as_deref() {
            lines.push(format!("settlement_id: {settlement_id}"));
        }
        if let Some(result_preview) = entry.result_preview.as_deref() {
            lines.push(format!("result_preview: {result_preview}"));
        }
        if let Some(error_detail) = entry.error_detail.as_deref() {
            lines.push(format!("error_detail: {error_detail}"));
        }
        if !entry.feedback_event_ids.is_empty() {
            lines.push(format!(
                "feedback_event_ids: {}",
                entry.feedback_event_ids.join(", ")
            ));
        }
        if let Some(result_event_id) = entry.result_event_id.as_deref() {
            lines.push(format!("result_event_id: {result_event_id}"));
        }
    }
    lines.join("\n")
}

pub async fn submit_buyer_job(
    config_path: &Path,
    request: BuyerJobSubmitRequest,
) -> Result<BuyerJobSubmitReport> {
    let config = crate::ensure_local_setup(config_path)?;
    if config.relay_urls.is_empty() {
        bail!("no relays are configured for buyer job submission");
    }
    let identity = ensure_identity(config.identity_path.as_path())?;
    let signer_key = decode_private_key_hex(identity.private_key_hex.as_str())?;
    let job_request = build_buyer_job_request(&config, &request)?;
    let prompt_preview =
        request_prompt(&job_request).map(|prompt| preview_text(prompt.as_str(), 72));
    let model = job_request
        .params
        .iter()
        .find(|param| param.key == "model")
        .map(|param| param.value.clone());
    let provider_pubkey = job_request.service_providers.first().cloned();
    let output_mime = job_request.output.clone();
    let pool = build_relay_pool(&config, &identity).await?;
    let event = publish_signed_event(
        &pool,
        &signer_key,
        create_job_request_event(&job_request),
        "buyer job request",
    )
    .await?;
    persist_buyer_job_submission(
        config_path,
        identity.public_key_hex.as_str(),
        config.relay_urls.as_slice(),
        &request,
        &event,
        prompt_preview.as_deref(),
        model.as_deref(),
        provider_pubkey.as_deref(),
        job_request.bid,
    )?;
    Ok(BuyerJobSubmitReport {
        job_id: event.id.clone(),
        request_event_id: event.id,
        customer_pubkey: identity.public_key_hex,
        relay_urls: config.relay_urls.clone(),
        provider_pubkey,
        model,
        bid_msats: job_request.bid,
        output_mime,
        prompt_preview,
        status: "submitted".to_string(),
        detail: Some("published retained kind:5050 request to configured relays".to_string()),
    })
}

pub fn render_buyer_job_submit_report(report: &BuyerJobSubmitReport) -> String {
    let mut lines = vec![
        format!("job_id: {}", report.job_id),
        format!("request_event_id: {}", report.request_event_id),
        format!("customer_pubkey: {}", report.customer_pubkey),
        format!("status: {}", report.status),
        format!("relays: {}", comma_or_none(report.relay_urls.as_slice())),
        format!(
            "provider_pubkey: {}",
            report.provider_pubkey.as_deref().unwrap_or("none")
        ),
        format!("model: {}", report.model.as_deref().unwrap_or("none")),
        format!(
            "bid_msats: {}",
            report
                .bid_msats
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "output: {}",
            report.output_mime.as_deref().unwrap_or("none")
        ),
    ];
    if let Some(prompt_preview) = report.prompt_preview.as_deref() {
        lines.push(format!("prompt_preview: {prompt_preview}"));
    }
    if let Some(detail) = report.detail.as_deref() {
        lines.push(format!("detail: {detail}"));
    }
    lines.join("\n")
}

pub async fn watch_buyer_jobs<F>(
    config_path: &Path,
    request_event_id: Option<&str>,
    seconds: u64,
    mut on_event: F,
) -> Result<BuyerJobWatchReport>
where
    F: FnMut(BuyerJobWatchEntry),
{
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let tracked_request_ids = tracked_buyer_request_ids(config_path, request_event_id)?;
    if tracked_request_ids.is_empty() {
        bail!("no retained buyer jobs are available to watch");
    }
    let pool = build_relay_pool(&config, &identity).await?;
    let subscription_id = format!("pylon-buyer-watch-{}", crate::now_epoch_ms());
    pool.subscribe_filters(
        subscription_id.as_str(),
        vec![json!({
            "kinds": [nostr::nip90::KIND_JOB_FEEDBACK, ANNOUNCEMENT_KIND_TEXT_GENERATION + 1000],
            "#e": tracked_request_ids.clone(),
        })],
    )
    .await
    .context("failed to subscribe for buyer job tracking")?;

    let tracked = tracked_request_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let deadline = std::time::Instant::now() + Duration::from_secs(seconds.max(1));
    let poll_step = Duration::from_millis(150);
    let mut seen_event_ids = std::collections::BTreeSet::new();
    let mut entries = Vec::new();
    let mut feedback_count = 0usize;
    let mut result_count = 0usize;

    while std::time::Instant::now() < deadline {
        let relays = pool.relays().await;
        for relay in relays {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let wait = remaining.min(poll_step);
            let recv = tokio::time::timeout(wait, relay.recv()).await;
            let message = match recv {
                Ok(Ok(Some(message))) => message,
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => continue,
            };
            let RelayMessage::Event(_, event) = message else {
                continue;
            };
            if !seen_event_ids.insert(event.id.clone()) {
                continue;
            }
            let Some(entry) = classify_buyer_job_event(relay.url(), &tracked, &event) else {
                continue;
            };
            persist_buyer_job_event(config_path, &entry)?;
            if entry.event_kind == "feedback" {
                feedback_count += 1;
            } else if entry.event_kind == "result" {
                result_count += 1;
            }
            on_event(entry.clone());
            entries.push(entry);
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let _ = pool.unsubscribe(subscription_id.as_str()).await;

    Ok(BuyerJobWatchReport {
        seconds,
        tracked_request_ids,
        feedback_count,
        result_count,
        entries,
    })
}

pub fn render_buyer_job_watch_report(report: &BuyerJobWatchReport) -> String {
    let mut lines = vec![
        format!("watch_seconds: {}", report.seconds),
        format!(
            "tracked_request_ids: {}",
            comma_or_none(report.tracked_request_ids.as_slice())
        ),
        format!("feedback_count: {}", report.feedback_count),
        format!("result_count: {}", report.result_count),
    ];
    for entry in &report.entries {
        lines.push(String::new());
        lines.push(format!("request_event_id: {}", entry.request_event_id));
        lines.push(format!(
            "relay_url: {}",
            entry.relay_url.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!("event_id: {}", entry.event_id));
        lines.push(format!("event_kind: {}", entry.event_kind));
        lines.push(format!("status: {}", entry.status));
        if let Some(amount_msats) = entry.amount_msats {
            lines.push(format!("amount_msats: {amount_msats}"));
        }
        if let Some(bolt11) = entry.bolt11.as_deref() {
            lines.push(format!("bolt11: {bolt11}"));
        }
        if let Some(result_preview) = entry.result_preview.as_deref() {
            lines.push(format!("result_preview: {result_preview}"));
        }
        if let Some(detail) = entry.detail.as_deref() {
            lines.push(format!("detail: {detail}"));
        }
    }
    lines.join("\n")
}

fn build_buyer_job_request(
    config: &PylonConfig,
    request: &BuyerJobSubmitRequest,
) -> Result<JobRequest> {
    let structured = match request.request_json.as_deref() {
        Some(raw) => Some(
            serde_json::from_str::<StructuredBuyerJobPayload>(raw)
                .context("failed to parse buyer job structured payload")?,
        ),
        None => None,
    };
    if request.prompt.is_some() && structured.is_some() {
        bail!("buyer job submit accepts either prompt text or --request-json, not both");
    }

    let mut job_request = JobRequest::new(ANNOUNCEMENT_KIND_TEXT_GENERATION)
        .context("failed to build retained kind:5050 request")?;
    for relay_url in &config.relay_urls {
        job_request = job_request.add_relay(relay_url.clone());
    }

    if let Some(structured) = structured.as_ref() {
        if let Some(prompt) = structured
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            job_request = job_request.add_input(JobInput::text(prompt.to_string()));
        }
        for input in &structured.inputs {
            let input_type = InputType::from_str(input.input_type.as_str()).with_context(|| {
                format!("unsupported buyer job input type `{}`", input.input_type)
            })?;
            job_request = job_request.add_input(JobInput {
                data: input.data.clone(),
                input_type,
                relay: input.relay.clone(),
                marker: input.marker.clone(),
            });
        }
        for (key, value) in &structured.params {
            job_request = job_request.add_param(key.clone(), value.clone());
        }
    }

    if let Some(prompt) = request
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        job_request = job_request.add_input(JobInput::text(prompt.to_string()));
    }

    if job_request.inputs.is_empty() {
        bail!("buyer job submit requires prompt text or structured inputs");
    }

    if let Some(output) = request
        .output_mime
        .as_deref()
        .or_else(|| {
            structured
                .as_ref()
                .and_then(|payload| payload.output.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        job_request = job_request.with_output(output.to_string());
    }
    if let Some(model) = request
        .model
        .as_deref()
        .or_else(|| {
            structured
                .as_ref()
                .and_then(|payload| payload.model.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        job_request = job_request.add_param("model", model.to_string());
    }
    if let Some(provider_pubkey) = request
        .provider_pubkey
        .as_deref()
        .or_else(|| {
            structured
                .as_ref()
                .and_then(|payload| payload.provider.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        job_request = job_request.add_service_provider(provider_pubkey.to_string());
    }
    if let Some(bid_msats) = request
        .bid_msats
        .or_else(|| structured.as_ref().and_then(|payload| payload.bid_msats))
    {
        job_request = job_request.with_bid(bid_msats);
    }
    Ok(job_request)
}

fn tracked_buyer_request_ids(
    config_path: &Path,
    request_event_id: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(request_event_id) = request_event_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(vec![request_event_id.to_string()]);
    }
    let ledger = load_ledger(config_path)?;
    Ok(ledger
        .jobs
        .iter()
        .filter(|job| job.direction == "buyer")
        .filter_map(|job| {
            job.request_event_id
                .clone()
                .or_else(|| Some(job.id.clone()))
        })
        .collect())
}

fn classify_buyer_job_event(
    relay_url: &str,
    tracked_request_ids: &std::collections::BTreeSet<String>,
    event: &Event,
) -> Option<BuyerJobWatchEntry> {
    if event.kind == nostr::nip90::KIND_JOB_FEEDBACK {
        let feedback = JobFeedback::from_event(event).ok()?;
        if !tracked_request_ids.contains(feedback.request_id.as_str()) {
            return None;
        }
        return Some(BuyerJobWatchEntry {
            request_event_id: feedback.request_id,
            relay_url: Some(relay_url.to_string()),
            event_id: event.id.clone(),
            event_kind: "feedback".to_string(),
            status: feedback.status.as_str().to_string(),
            amount_msats: feedback.amount,
            bolt11: feedback.bolt11,
            result_preview: (!feedback.content.trim().is_empty())
                .then(|| preview_text(feedback.content.as_str(), 72)),
            detail: feedback.status_extra,
        });
    }
    if event.kind == ANNOUNCEMENT_KIND_TEXT_GENERATION + 1000 {
        let result = JobResult::from_event(event).ok()?;
        if !tracked_request_ids.contains(result.request_id.as_str()) {
            return None;
        }
        return Some(BuyerJobWatchEntry {
            request_event_id: result.request_id,
            relay_url: Some(relay_url.to_string()),
            event_id: event.id.clone(),
            event_kind: "result".to_string(),
            status: "result_received".to_string(),
            amount_msats: result.amount,
            bolt11: result.bolt11,
            result_preview: Some(preview_text(result.content.as_str(), 72)),
            detail: Some("received retained kind:6050 result".to_string()),
        });
    }
    None
}

fn build_announcement_report(
    config: &PylonConfig,
    provider_pubkey: &str,
    existing: Option<&PylonLedgerAnnouncement>,
    spec: Option<&AnnouncementSpec>,
    detail: Option<String>,
) -> AnnouncementReport {
    let status = match (existing, spec) {
        (Some(_), Some(_)) => "ready".to_string(),
        (Some(_), None) => "stale".to_string(),
        (None, Some(_)) => "unpublished".to_string(),
        (None, None) => "not_ready".to_string(),
    };
    AnnouncementReport {
        node_label: config.node_label.clone(),
        provider_pubkey: provider_pubkey.to_string(),
        status,
        request_kind: ANNOUNCEMENT_KIND_TEXT_GENERATION,
        handler_event_id: existing.map(|entry| entry.event_id.clone()),
        model: spec
            .map(|entry| entry.model.clone())
            .or_else(|| existing.and_then(|entry| entry.model.clone())),
        backend: spec
            .map(|entry| entry.backend.clone())
            .or_else(|| existing.and_then(|entry| entry.backend.clone())),
        capabilities: spec
            .map(|entry| entry.capabilities.clone())
            .unwrap_or_else(|| {
                existing
                    .map(|entry| entry.capabilities.clone())
                    .unwrap_or_default()
            }),
        price_msats: spec
            .map(|entry| entry.price_msats)
            .unwrap_or_else(|| existing.and_then(|entry| entry.price_msats)),
        relay_urls: config.relay_urls.clone(),
        published_at_ms: existing.map(|entry| entry.published_at_ms),
        fingerprint: spec
            .map(|entry| entry.fingerprint.clone())
            .unwrap_or_else(|| {
                existing
                    .map(|entry| entry.fingerprint.clone())
                    .unwrap_or_else(|| "none".to_string())
            }),
        detail,
    }
}

fn announcement_spec(
    config: &PylonConfig,
    status: &openagents_provider_substrate::ProviderStatusResponse,
) -> Option<AnnouncementSpec> {
    let products = products_from_status(config, status);
    let product = first_text_generation_product(products.as_slice())?;
    let backend = product.product.backend_label().to_string();
    let model = match backend.as_str() {
        "gpt_oss" => status
            .snapshot
            .as_ref()?
            .availability
            .gpt_oss
            .ready_model
            .clone()?,
        "apple_foundation_models" => status
            .snapshot
            .as_ref()?
            .availability
            .apple_foundation_models
            .ready_model
            .clone()?,
        _ => return None,
    };
    let price_msats = product.price_floor_sats.checked_mul(1000);
    let capabilities = vec![
        "nip90.5050".to_string(),
        "text_generation".to_string(),
        format!("backend:{backend}"),
        format!("model:{model}"),
        format!("product:{}", product.product.product_id()),
    ];
    let fingerprint = [
        ANNOUNCEMENT_KIND_TEXT_GENERATION.to_string(),
        backend.clone(),
        model.clone(),
        price_msats.unwrap_or(0).to_string(),
    ]
    .join("|");
    Some(AnnouncementSpec {
        model,
        backend,
        capabilities,
        price_msats,
        fingerprint,
    })
}

fn first_text_generation_product(
    products: &[ProviderAdvertisedProduct],
) -> Option<ProviderAdvertisedProduct> {
    products
        .iter()
        .find(|product| {
            product.eligible
                && product.product.compute_family_label() == "inference"
                && matches!(
                    product.product.backend_label(),
                    "gpt_oss" | "apple_foundation_models"
                )
        })
        .cloned()
}

async fn publish_announcement_event(
    config: &PylonConfig,
    identity: &nostr::NostrIdentity,
    spec: &AnnouncementSpec,
) -> Result<Event> {
    let metadata = HandlerMetadata::new(
        format!("Pylon {}", config.node_label),
        format!(
            "Standalone OpenAgents compute provider for local {} text generation",
            spec.model
        ),
    );
    let mut info = HandlerInfo::new(
        identity.public_key_hex.as_str(),
        HandlerType::ComputeProvider,
        metadata.clone(),
    )
    .add_custom_tag(
        "d",
        format!(
            "pylon:{}:{}",
            config.node_label, ANNOUNCEMENT_KIND_TEXT_GENERATION
        ),
    )
    .add_custom_tag("k", ANNOUNCEMENT_KIND_TEXT_GENERATION.to_string())
    .add_custom_tag("model", spec.model.clone())
    .add_custom_tag("backend", spec.backend.clone())
    .add_custom_tag("node", config.node_label.clone());
    for capability in &spec.capabilities {
        info = info.add_capability(capability.clone());
    }
    if let Some(price_msats) = spec.price_msats {
        info = info.with_pricing(
            PricingInfo::new(price_msats)
                .with_model("per_request")
                .with_currency("msats"),
        );
    }
    let template = EventTemplate {
        kind: KIND_HANDLER_INFO,
        tags: info.to_tags(),
        content: serde_json::to_string(&metadata)?,
        created_at: nostr::nip01::unix_now_secs().unwrap_or(0),
    };
    let event = finalize_event(
        &template,
        &decode_private_key_hex(identity.private_key_hex.as_str())?,
    )
    .context("failed to sign Pylon handler announcement")?;
    let pool = build_relay_pool(config, identity).await?;
    let confirmations = pool.publish(&event).await?;
    if !confirmations
        .iter()
        .any(|confirmation| confirmation.accepted)
    {
        let detail = confirmations
            .iter()
            .map(|confirmation| format!("{}:{}", confirmation.relay_url, confirmation.message))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("no relay accepted the announcement event: {detail}");
    }
    Ok(event)
}

async fn build_relay_pool(
    config: &PylonConfig,
    identity: &nostr::NostrIdentity,
) -> Result<RelayPool> {
    let relay_config = RelayConfig {
        connect_timeout: Duration::from_secs(config.relay_connect_timeout_seconds),
        nip42_identity: config
            .relay_auth_enabled
            .then(|| nostr_client::RelayAuthIdentity {
                private_key_hex: identity.private_key_hex.clone(),
            }),
    };
    let pool = RelayPool::new(PoolConfig {
        max_relays: config.relay_urls.len().max(1),
        relay_config,
    });
    for relay in &config.relay_urls {
        pool.add_relay(relay.as_str())
            .await
            .with_context(|| format!("failed to add relay {}", relay))?;
    }
    pool.connect_all()
        .await
        .context("failed to connect relays")?;
    Ok(pool)
}

async fn collect_provider_requests(
    config_path: &Path,
    seconds: u64,
) -> Result<ProviderRequestCollection> {
    let config = crate::ensure_local_setup(config_path)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let (_, status) = load_config_and_status(config_path).await?;
    let spec = announcement_spec(&config, &status);
    let pool = build_relay_pool(&config, &identity).await?;
    let subscription_id = format!("pylon-provider-{}", crate::now_epoch_ms());
    pool.subscribe_filters(
        subscription_id.as_str(),
        vec![json!({
            "kinds": [ANNOUNCEMENT_KIND_TEXT_GENERATION],
        })],
    )
    .await
    .context("failed to subscribe for provider intake")?;

    let deadline = std::time::Instant::now() + Duration::from_secs(seconds.max(1));
    let poll_step = Duration::from_millis(150);
    let mut observed = BTreeMap::<String, ObservedProviderRequest>::new();
    while std::time::Instant::now() < deadline {
        let relays = pool.relays().await;
        for relay in relays {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let wait = remaining.min(poll_step);
            let recv = tokio::time::timeout(wait, relay.recv()).await;
            let message = match recv {
                Ok(Ok(Some(message))) => message,
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => continue,
            };
            if let RelayMessage::Event(_, event) = message {
                let entry = classify_provider_request(
                    relay.url(),
                    &identity.public_key_hex,
                    spec.as_ref(),
                    &event,
                );
                observed
                    .entry(entry.request_event_id.clone())
                    .or_insert(ObservedProviderRequest { event, entry });
            }
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let _ = pool.unsubscribe(subscription_id.as_str()).await;

    Ok(ProviderRequestCollection {
        provider_pubkey: identity.public_key_hex,
        desired_mode: status.desired_mode,
        spec,
        observed: observed.into_values().collect(),
    })
}

fn classify_provider_request(
    relay_url: &str,
    provider_pubkey: &str,
    spec: Option<&AnnouncementSpec>,
    event: &Event,
) -> ProviderIntakeEntry {
    let default = ProviderIntakeEntry {
        request_event_id: event.id.clone(),
        requester_pubkey: event.pubkey.clone(),
        relay_url: Some(relay_url.to_string()),
        targeted: false,
        decision: "drop".to_string(),
        drop_reason: Some("invalid_request".to_string()),
        prompt_preview: None,
        model: None,
        bid_msats: None,
    };
    let Ok(request) = JobRequest::from_event(event) else {
        return default;
    };
    let targeted = !request.service_providers.is_empty();
    let prompt_preview = request
        .inputs
        .iter()
        .find(|input| input.input_type.as_str() == "text")
        .map(|input| preview_text(input.data.as_str(), 72));
    let model = request
        .params
        .iter()
        .find(|param| param.key == "model")
        .map(|param| param.value.clone());
    if targeted
        && !request
            .service_providers
            .iter()
            .any(|pubkey| pubkey == provider_pubkey)
    {
        return ProviderIntakeEntry {
            request_event_id: event.id.clone(),
            requester_pubkey: event.pubkey.clone(),
            relay_url: Some(relay_url.to_string()),
            targeted,
            decision: "drop".to_string(),
            drop_reason: Some("targeted_elsewhere".to_string()),
            prompt_preview,
            model,
            bid_msats: request.bid,
        };
    }
    if spec.is_none() {
        return ProviderIntakeEntry {
            request_event_id: event.id.clone(),
            requester_pubkey: event.pubkey.clone(),
            relay_url: Some(relay_url.to_string()),
            targeted,
            decision: "drop".to_string(),
            drop_reason: Some("no_local_supply".to_string()),
            prompt_preview,
            model,
            bid_msats: request.bid,
        };
    }
    ProviderIntakeEntry {
        request_event_id: event.id.clone(),
        requester_pubkey: event.pubkey.clone(),
        relay_url: Some(relay_url.to_string()),
        targeted,
        decision: "match".to_string(),
        drop_reason: None,
        prompt_preview,
        model,
        bid_msats: request.bid,
    }
}

fn spec_to_local_target(spec: &AnnouncementSpec) -> LocalGemmaChatTarget {
    LocalGemmaChatTarget {
        backend: match spec.backend.as_str() {
            "apple_foundation_models" => LocalGemmaChatBackend::OpenAiCompat,
            _ => LocalGemmaChatBackend::Ollama,
        },
        model: spec.model.clone(),
    }
}

fn request_prompt(request: &JobRequest) -> Option<String> {
    let prompts = request
        .inputs
        .iter()
        .filter(|input| input.input_type.as_str() == "text")
        .map(|input| input.data.trim())
        .filter(|input| !input.is_empty())
        .collect::<Vec<_>>();
    if prompts.is_empty() {
        None
    } else {
        Some(prompts.join("\n\n"))
    }
}

fn request_model_matches_target(requested: Option<&str>, target_model: &str) -> bool {
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let requested = requested.to_ascii_lowercase();
    let target = target_model.trim().to_ascii_lowercase();
    target == requested
        || target.contains(requested.as_str())
        || requested.contains(target.as_str())
        || (requested.contains("gemma") && target.contains("gemma"))
}

fn provider_job_blocks_reintake(status: &str) -> bool {
    matches!(
        status,
        "accepted_local"
            | "processing_local"
            | "completed_local"
            | "failed_local"
            | "settled"
            | "result_published"
            | "publish_failed"
    )
}

fn persist_provider_run_state(
    config_path: &Path,
    provider_pubkey: &str,
    entry: &ProviderIntakeEntry,
    status: &str,
    error_detail: Option<&str>,
    result_preview: Option<&str>,
    feedback_event_id: Option<&str>,
    result_event_id: Option<&str>,
    amount_msats: Option<u64>,
    bolt11: Option<&str>,
) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        let mut job = ledger
            .jobs
            .iter()
            .find(|job| job.id == entry.request_event_id)
            .cloned()
            .unwrap_or_else(|| {
                PylonLedgerJob::new(
                    entry.request_event_id.clone(),
                    "provider",
                    ANNOUNCEMENT_KIND_TEXT_GENERATION,
                    status,
                )
            });
        job.request_event_id = Some(entry.request_event_id.clone());
        job.customer_pubkey = Some(entry.requester_pubkey.clone());
        job.provider_pubkey = Some(provider_pubkey.to_string());
        job.relay_url = entry.relay_url.clone();
        job.prompt = entry.prompt_preview.clone();
        job.model = entry.model.clone();
        job.bid_msats = entry.bid_msats;
        if let Some(amount_msats) = amount_msats {
            job.amount_msats = Some(amount_msats);
        }
        if let Some(bolt11) = bolt11 {
            job.bolt11 = Some(bolt11.to_string());
        }
        job.status = status.to_string();
        job.error_detail = error_detail.map(ToString::to_string);
        if let Some(result_preview) = result_preview {
            job.result_preview = Some(result_preview.to_string());
        }
        if let Some(feedback_event_id) = feedback_event_id {
            if !job
                .feedback_event_ids
                .iter()
                .any(|existing| existing == feedback_event_id)
            {
                job.feedback_event_ids.push(feedback_event_id.to_string());
            }
        }
        if let Some(result_event_id) = result_event_id {
            job.result_event_id = Some(result_event_id.to_string());
        }
        ledger.upsert_job(job);
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: crate::now_epoch_ms() as u64,
            url: entry.relay_url.clone(),
            kind: match status {
                "accepted_local" => "nip90.job_accepted",
                "processing_local" => "nip90.job_processing",
                "completed_local" => "nip90.job_completed",
                "failed_local" => "nip90.job_failed",
                "payment_required" => "nip90.job_payment_required",
                "invoice_failed" => "nip90.job_invoice_failed",
                "rejected_policy" | "rejected_supply" | "rejected_model" | "rejected_input" => {
                    "nip90.job_rejected"
                }
                "observed_drop" => "nip90.request_dropped",
                _ => "nip90.job_updated",
            }
            .to_string(),
            detail: match status {
                "accepted_local" => format!("accepted request {}", entry.request_event_id),
                "processing_local" => format!("processing request {}", entry.request_event_id),
                "completed_local" => format!("completed request {}", entry.request_event_id),
                "failed_local" => format!(
                    "failed request {} ({})",
                    entry.request_event_id,
                    error_detail.unwrap_or("unknown")
                ),
                "payment_required" => format!(
                    "payment required for request {} ({})",
                    entry.request_event_id,
                    bolt11.unwrap_or("missing_invoice")
                ),
                "invoice_failed" => format!(
                    "invoice failed for request {} ({})",
                    entry.request_event_id,
                    error_detail.unwrap_or("unknown")
                ),
                "rejected_policy" | "rejected_supply" | "rejected_model" | "rejected_input" => {
                    format!(
                        "rejected request {} ({})",
                        entry.request_event_id,
                        error_detail.unwrap_or("unknown")
                    )
                }
                "observed_drop" => format!(
                    "dropped request {} ({})",
                    entry.request_event_id,
                    error_detail.unwrap_or("unknown")
                ),
                _ => format!("updated request {}", entry.request_event_id),
            },
        });
        if let Some(feedback_event_id) = feedback_event_id {
            ledger.push_relay_activity(PylonRelayActivity {
                at_ms: crate::now_epoch_ms() as u64,
                url: entry.relay_url.clone(),
                kind: "nip90.feedback_published".to_string(),
                detail: format!(
                    "published feedback {} for request {}",
                    feedback_event_id, entry.request_event_id
                ),
            });
        }
        if let Some(result_event_id) = result_event_id {
            ledger.push_relay_activity(PylonRelayActivity {
                at_ms: crate::now_epoch_ms() as u64,
                url: entry.relay_url.clone(),
                kind: "nip90.result_published".to_string(),
                detail: format!(
                    "published result {} for request {}",
                    result_event_id, entry.request_event_id
                ),
            });
        }
        Ok(())
    })?;
    Ok(())
}

fn record_provider_payment_received(
    config_path: &Path,
    entry: &ProviderIntakeEntry,
    payment: PylonWalletPaymentRecord,
    amount_msats: u64,
) -> Result<String> {
    let settlement_id = format!("provider-settlement:{}", entry.request_event_id);
    mutate_ledger(config_path, |ledger| {
        let mut settlement = ledger
            .settlements
            .iter()
            .find(|existing| existing.settlement_id == settlement_id)
            .cloned()
            .unwrap_or(PylonSettlementRecord {
                settlement_id: settlement_id.clone(),
                job_id: entry.request_event_id.clone(),
                direction: "provider".to_string(),
                status: "payment_received".to_string(),
                amount_msats,
                payment_reference: Some(payment.payment_id.clone()),
                receipt_detail: Some("invoice completed in local wallet".to_string()),
                created_at_ms: now_epoch_ms() as u64,
                updated_at_ms: now_epoch_ms() as u64,
            });
        settlement.status = "payment_received".to_string();
        settlement.amount_msats = amount_msats;
        settlement.payment_reference = Some(payment.payment_id.clone());
        settlement.receipt_detail = Some("invoice completed in local wallet".to_string());
        ledger.upsert_settlement(settlement);
        ledger.upsert_wallet_payment(payment.clone());
        if let Some(invoice) = payment.invoice.as_deref() {
            if let Some(existing_invoice) = ledger
                .wallet
                .invoices
                .iter()
                .find(|existing| existing.payment_request == invoice)
                .cloned()
            {
                let mut updated_invoice = existing_invoice;
                updated_invoice.status = payment.status.clone();
                ledger.upsert_wallet_invoice(updated_invoice);
            }
        }
        if let Some(job) = ledger
            .jobs
            .iter()
            .find(|existing| existing.id == entry.request_event_id)
            .cloned()
        {
            let mut updated_job = job;
            updated_job.payment_id = Some(payment.payment_id.clone());
            updated_job.settlement_id = Some(settlement_id.clone());
            updated_job.status = "payment_settled".to_string();
            ledger.upsert_job(updated_job);
        }
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: entry.relay_url.clone(),
            kind: "nip90.payment_settled".to_string(),
            detail: format!(
                "invoice settled for request {} via payment {}",
                entry.request_event_id, payment.payment_id
            ),
        });
        Ok(())
    })?;
    Ok(settlement_id)
}

fn record_provider_settlement_outcome(
    config_path: &Path,
    entry: &ProviderIntakeEntry,
    payment: &PylonWalletPaymentRecord,
    amount_msats: u64,
    settlement_id: Option<String>,
    status: &str,
    detail: &str,
) -> Result<String> {
    let settlement_id =
        settlement_id.unwrap_or_else(|| format!("provider-settlement:{}", entry.request_event_id));
    mutate_ledger(config_path, |ledger| {
        let mut settlement = ledger
            .settlements
            .iter()
            .find(|existing| existing.settlement_id == settlement_id)
            .cloned()
            .unwrap_or(PylonSettlementRecord {
                settlement_id: settlement_id.clone(),
                job_id: entry.request_event_id.clone(),
                direction: "provider".to_string(),
                status: status.to_string(),
                amount_msats,
                payment_reference: Some(payment.payment_id.clone()),
                receipt_detail: Some(detail.to_string()),
                created_at_ms: now_epoch_ms() as u64,
                updated_at_ms: now_epoch_ms() as u64,
            });
        settlement.status = status.to_string();
        settlement.amount_msats = amount_msats;
        settlement.payment_reference = Some(payment.payment_id.clone());
        settlement.receipt_detail = Some(detail.to_string());
        ledger.upsert_settlement(settlement);
        if let Some(job) = ledger
            .jobs
            .iter()
            .find(|existing| existing.id == entry.request_event_id)
            .cloned()
        {
            let mut updated_job = job;
            updated_job.payment_id = Some(payment.payment_id.clone());
            updated_job.settlement_id = Some(settlement_id.clone());
            if status.eq_ignore_ascii_case("settled") {
                updated_job.status = "settled".to_string();
            }
            ledger.upsert_job(updated_job);
        }
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: entry.relay_url.clone(),
            kind: "nip90.settlement_updated".to_string(),
            detail: format!(
                "settlement {} for request {} ({detail})",
                status, entry.request_event_id
            ),
        });
        Ok(())
    })?;
    Ok(settlement_id)
}

async fn load_provider_wallet_payments(
    config_path: &Path,
) -> Result<Vec<PylonWalletPaymentRecord>> {
    #[cfg(test)]
    {
        if let Some(slot) = TEST_WALLET_PAYMENTS_HOOK.get() {
            if let Some(hook) = slot
                .lock()
                .expect("test wallet payments hook lock")
                .as_ref()
            {
                return hook();
            }
        }
    }
    let report: WalletHistoryReport = load_wallet_history_report(config_path, Some(100)).await?;
    Ok(report.payments)
}

fn find_settled_receive_payment(
    payments: &[PylonWalletPaymentRecord],
    bolt11: &str,
) -> Option<PylonWalletPaymentRecord> {
    payments.iter().find_map(|payment| {
        if payment.direction.eq_ignore_ascii_case("receive")
            && is_settled_wallet_payment_status(payment.status.as_str())
            && payment
                .invoice
                .as_deref()
                .is_some_and(|invoice| invoice == bolt11)
        {
            Some(payment.clone())
        } else {
            None
        }
    })
}

fn is_settled_wallet_payment_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "success" | "settled" | "completed" | "confirmed"
    )
}

async fn create_provider_payment_requirement(
    config_path: &Path,
    entry: &ProviderIntakeEntry,
    amount_msats: u64,
) -> Result<ProviderPaymentRequirement> {
    let description = Some(format!("pylon nip90 {}", entry.request_event_id));
    let report =
        create_provider_invoice_report(config_path, amount_msats, description, None).await?;
    Ok(ProviderPaymentRequirement {
        bolt11: report.invoice.payment_request,
    })
}

async fn publish_processing_feedback(
    pool: &RelayPool,
    signer_key: &[u8; 32],
    request_event: &Event,
    relay_url: Option<&str>,
    model: &str,
) -> Result<Event> {
    let mut feedback = JobFeedback::new(
        JobStatus::Processing,
        request_event.id.clone(),
        request_event.pubkey.clone(),
    )
    .with_status_extra(format!("processing with local {}", model));
    if let Some(relay_url) = relay_url {
        feedback = feedback.with_request_relay(relay_url.to_string());
    }
    publish_signed_event(
        pool,
        signer_key,
        create_job_feedback_event(&feedback),
        "processing feedback",
    )
    .await
}

async fn publish_payment_required_feedback(
    pool: &RelayPool,
    signer_key: &[u8; 32],
    request_event: &Event,
    relay_url: Option<&str>,
    amount_msats: u64,
    bolt11: &str,
) -> Result<Event> {
    let mut feedback = JobFeedback::new(
        JobStatus::PaymentRequired,
        request_event.id.clone(),
        request_event.pubkey.clone(),
    )
    .with_status_extra("lightning settlement required".to_string())
    .with_amount(amount_msats, Some(bolt11.to_string()));
    if let Some(relay_url) = relay_url {
        feedback = feedback.with_request_relay(relay_url.to_string());
    }
    publish_signed_event(
        pool,
        signer_key,
        create_job_feedback_event(&feedback),
        "payment-required feedback",
    )
    .await
}

async fn publish_error_feedback(
    pool: &RelayPool,
    signer_key: &[u8; 32],
    request_event: &Event,
    relay_url: Option<&str>,
    error_detail: &str,
) -> Result<Event> {
    let mut feedback = JobFeedback::new(
        JobStatus::Error,
        request_event.id.clone(),
        request_event.pubkey.clone(),
    )
    .with_status_extra(error_detail.to_string());
    if let Some(relay_url) = relay_url {
        feedback = feedback.with_request_relay(relay_url.to_string());
    }
    publish_signed_event(
        pool,
        signer_key,
        create_job_feedback_event(&feedback),
        "error feedback",
    )
    .await
}

async fn publish_job_result(
    pool: &RelayPool,
    signer_key: &[u8; 32],
    request_event: &Event,
    request: &JobRequest,
    relay_url: Option<&str>,
    output: &str,
) -> Result<Event> {
    let mut result = JobResult::new(
        request.kind,
        request_event.id.clone(),
        request_event.pubkey.clone(),
        output,
    )?;
    if let Some(relay_url) = relay_url {
        result = result.with_request_relay(relay_url.to_string());
    }
    for input in &request.inputs {
        result = result.add_input(input.clone());
    }
    publish_signed_event(
        pool,
        signer_key,
        create_job_result_event(&result),
        "job result",
    )
    .await
}

async fn publish_signed_event(
    pool: &RelayPool,
    signer_key: &[u8; 32],
    template: nostr::nip01::EventTemplate,
    label: &str,
) -> Result<Event> {
    let event = finalize_event(&template, signer_key)
        .with_context(|| format!("failed to sign Pylon {label}"))?;
    let confirmations = pool.publish(&event).await?;
    if !confirmations
        .iter()
        .any(|confirmation| confirmation.accepted)
    {
        let detail = confirmations
            .iter()
            .map(|confirmation| format!("{}:{}", confirmation.relay_url, confirmation.message))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("no relay accepted the {label}: {detail}");
    }
    Ok(event)
}

fn persist_provider_intake(config_path: &Path, report: &ProviderIntakeReport) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        for entry in &report.entries {
            let mut job = ledger
                .jobs
                .iter()
                .find(|job| job.id == entry.request_event_id)
                .cloned()
                .unwrap_or_else(|| {
                    PylonLedgerJob::new(
                        entry.request_event_id.clone(),
                        "provider",
                        ANNOUNCEMENT_KIND_TEXT_GENERATION,
                        if entry.decision == "match" {
                            "observed_match"
                        } else {
                            "observed_drop"
                        },
                    )
                });
            job.request_event_id = Some(entry.request_event_id.clone());
            job.customer_pubkey = Some(entry.requester_pubkey.clone());
            job.provider_pubkey = Some(report.provider_pubkey.clone());
            job.relay_url = entry.relay_url.clone();
            job.prompt = entry.prompt_preview.clone();
            job.model = entry.model.clone();
            job.bid_msats = entry.bid_msats;
            job.status = if entry.decision == "match" {
                "observed_match".to_string()
            } else {
                "observed_drop".to_string()
            };
            job.error_detail = entry.drop_reason.clone();
            ledger.upsert_job(job);
            ledger.push_relay_activity(PylonRelayActivity {
                at_ms: crate::now_epoch_ms() as u64,
                url: entry.relay_url.clone(),
                kind: if entry.decision == "match" {
                    "nip90.request_matched".to_string()
                } else {
                    "nip90.request_dropped".to_string()
                },
                detail: if entry.decision == "match" {
                    format!("matched request {}", entry.request_event_id)
                } else {
                    format!(
                        "dropped request {} ({})",
                        entry.request_event_id,
                        entry.drop_reason.as_deref().unwrap_or("unknown")
                    )
                },
            });
        }
        Ok(())
    })?;
    Ok(())
}

fn persist_buyer_job_submission(
    config_path: &Path,
    customer_pubkey: &str,
    relay_urls: &[String],
    request: &BuyerJobSubmitRequest,
    event: &Event,
    prompt_preview: Option<&str>,
    model: Option<&str>,
    provider_pubkey: Option<&str>,
    bid_msats: Option<u64>,
) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        let mut job = PylonLedgerJob::new(
            event.id.clone(),
            "buyer",
            ANNOUNCEMENT_KIND_TEXT_GENERATION,
            "submitted",
        );
        job.request_event_id = Some(event.id.clone());
        job.customer_pubkey = Some(customer_pubkey.to_string());
        job.provider_pubkey = provider_pubkey.map(ToString::to_string);
        job.relay_url = relay_urls.first().cloned();
        job.prompt = Some(
            request
                .prompt
                .clone()
                .or_else(|| request.request_json.clone())
                .unwrap_or_default(),
        );
        job.model = model.map(ToString::to_string);
        job.bid_msats = bid_msats;
        job.result_preview = prompt_preview.map(ToString::to_string);
        job.error_detail = None;
        ledger.upsert_job(job);
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: None,
            kind: "nip90.job_submitted".to_string(),
            detail: format!(
                "submitted buyer request {} to {}",
                event.id,
                comma_or_none(relay_urls)
            ),
        });
        Ok(())
    })?;
    Ok(())
}

fn persist_buyer_job_event(config_path: &Path, entry: &BuyerJobWatchEntry) -> Result<()> {
    mutate_ledger(config_path, |ledger| {
        let mut job = ledger
            .jobs
            .iter()
            .find(|job| {
                job.direction == "buyer"
                    && job
                        .request_event_id
                        .as_deref()
                        .is_some_and(|value| value == entry.request_event_id)
            })
            .cloned()
            .unwrap_or_else(|| {
                let mut job = PylonLedgerJob::new(
                    entry.request_event_id.clone(),
                    "buyer",
                    ANNOUNCEMENT_KIND_TEXT_GENERATION,
                    "observed",
                );
                job.request_event_id = Some(entry.request_event_id.clone());
                job
            });
        job.relay_url = entry.relay_url.clone().or(job.relay_url);
        if entry.event_kind == "feedback" {
            if !job
                .feedback_event_ids
                .iter()
                .any(|id| id == &entry.event_id)
            {
                job.feedback_event_ids.push(entry.event_id.clone());
            }
            job.status = entry.status.replace('-', "_");
            job.amount_msats = entry.amount_msats.or(job.amount_msats);
            job.bolt11 = entry.bolt11.clone().or(job.bolt11.clone());
            if let Some(preview) = entry.result_preview.as_deref() {
                job.result_preview = Some(preview.to_string());
            }
            job.error_detail = entry.detail.clone();
        } else if entry.event_kind == "result" {
            job.status = "result_received".to_string();
            job.result_event_id = Some(entry.event_id.clone());
            job.amount_msats = entry.amount_msats.or(job.amount_msats);
            job.bolt11 = entry.bolt11.clone().or(job.bolt11.clone());
            job.result_preview = entry.result_preview.clone();
            job.error_detail = entry.detail.clone();
        }
        ledger.upsert_job(job);
        ledger.push_relay_activity(PylonRelayActivity {
            at_ms: now_epoch_ms() as u64,
            url: entry.relay_url.clone(),
            kind: if entry.event_kind == "result" {
                "nip90.result_received".to_string()
            } else {
                "nip90.feedback_received".to_string()
            },
            detail: format!(
                "{} for buyer request {}",
                entry.status, entry.request_event_id
            ),
        });
        Ok(())
    })?;
    Ok(())
}

fn decode_private_key_hex(value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value).with_context(|| "invalid pylon private key hex")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32-byte private key"))?;
    Ok(bytes)
}

fn comma_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    trimmed.chars().take(max_chars).collect::<String>() + "..."
}

async fn create_provider_invoice_report(
    config_path: &Path,
    amount_msats: u64,
    description: Option<String>,
    expiry_seconds: Option<u32>,
) -> Result<WalletInvoiceReport> {
    #[cfg(test)]
    {
        if let Some(slot) = TEST_WALLET_INVOICE_HOOK.get() {
            if let Some(hook) = slot.lock().expect("test wallet invoice hook lock").as_ref() {
                return hook(
                    msats_to_sats_rounded_up(amount_msats),
                    description,
                    expiry_seconds,
                );
            }
        }
    }
    create_wallet_invoice_report(
        config_path,
        msats_to_sats_rounded_up(amount_msats),
        description,
        expiry_seconds,
    )
    .await
}

fn msats_to_sats_rounded_up(amount_msats: u64) -> u64 {
    amount_msats.saturating_add(999) / 1000
}
