use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nostr::nip89::{HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO, PricingInfo};
use nostr::nip90::{
    JobFeedback, JobRequest, JobResult, JobStatus, create_job_feedback_event,
    create_job_result_event,
};
use nostr::{Event, EventTemplate, finalize_event};
use nostr_client::{PoolConfig, RelayConfig, RelayMessage, RelayPool};
use openagents_provider_substrate::ProviderAdvertisedProduct;
use serde::Serialize;
use serde_json::json;

use crate::{
    LocalGemmaChatBackend, LocalGemmaChatEvent, LocalGemmaChatTarget, PylonConfig,
    PylonLedgerAnnouncement, PylonLedgerJob, PylonRelayActivity, WalletInvoiceReport,
    create_wallet_invoice_report, ensure_identity, load_config_and_status, load_ledger,
    mutate_ledger, products_from_status,
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

#[cfg(test)]
type TestWalletInvoiceHook =
    Box<dyn Fn(u64, Option<String>, Option<u32>) -> Result<WalletInvoiceReport> + Send + Sync>;

#[cfg(test)]
static TEST_WALLET_INVOICE_HOOK: std::sync::OnceLock<
    std::sync::Mutex<Option<TestWalletInvoiceHook>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
static TEST_RUNTIME_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn set_test_wallet_invoice_hook(hook: Option<TestWalletInvoiceHook>) {
    let slot = TEST_WALLET_INVOICE_HOOK.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().expect("test wallet invoice hook lock") = hook;
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
    let mut known_statuses = load_ledger(config_path)?
        .jobs
        .into_iter()
        .map(|job| (job.id, job.status))
        .collect::<BTreeMap<_, _>>();
    let mut report_entries = Vec::new();
    let mut accepted_count = 0usize;
    let mut payment_required_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;
    let mut dropped_count = 0usize;

    for observed in collected.observed {
        let request_event_id = observed.entry.request_event_id.clone();
        if known_statuses
            .get(request_event_id.as_str())
            .is_some_and(|status| provider_job_blocks_reintake(status.as_str()))
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
            known_statuses.insert(request_event_id.clone(), "observed_drop".to_string());
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
            known_statuses.insert(request_event_id.clone(), "rejected_policy".to_string());
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
            known_statuses.insert(request_event_id.clone(), "rejected_supply".to_string());
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
                known_statuses.insert(request_event_id.clone(), "rejected_input".to_string());
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
            known_statuses.insert(request_event_id.clone(), "rejected_model".to_string());
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
            known_statuses.insert(request_event_id.clone(), "rejected_input".to_string());
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
                result_preview: None,
                error_detail: Some("missing_text_input".to_string()),
                feedback_event_ids: Vec::new(),
                result_event_id: None,
            });
            continue;
        };

        if let Some(price_msats) = price_msats
            .filter(|value| *value > 0)
            .filter(|_| observed.entry.bid_msats.is_some())
        {
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
                known_statuses.insert(request_event_id.clone(), "rejected_policy".to_string());
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
                    known_statuses.insert(request_event_id.clone(), "invoice_failed".to_string());
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
                    known_statuses.insert(request_event_id.clone(), "publish_failed".to_string());
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
            known_statuses.insert(request_event_id.clone(), "payment_required".to_string());
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
                result_preview: None,
                error_detail: None,
                feedback_event_ids: vec![payment_event.id],
                result_event_id: None,
            });
            continue;
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
                known_statuses.insert(request_event_id.clone(), "publish_failed".to_string());
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "failed".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: None,
                    bolt11: None,
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
        known_statuses.insert(request_event_id.clone(), "processing_local".to_string());

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
                        known_statuses
                            .insert(request_event_id.clone(), "publish_failed".to_string());
                        report_entries.push(ProviderRunEntry {
                            request_event_id: observed.entry.request_event_id.clone(),
                            requester_pubkey: observed.entry.requester_pubkey.clone(),
                            relay_url: observed.entry.relay_url.clone(),
                            status: "failed".to_string(),
                            prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                            model: Some(target.model.clone()),
                            bid_msats: observed.entry.bid_msats,
                            amount_msats: None,
                            bolt11: None,
                            result_preview: Some(result_preview),
                            error_detail: Some(error_string),
                            feedback_event_ids: vec![processing_event.id.clone()],
                            result_event_id: None,
                        });
                        continue;
                    }
                };
                completed_count += 1;
                known_statuses.insert(request_event_id.clone(), "completed_local".to_string());
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
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "completed".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: None,
                    bolt11: None,
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
                known_statuses.insert(request_event_id.clone(), "failed_local".to_string());
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "failed".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    amount_msats: None,
                    bolt11: None,
                    result_preview: None,
                    error_detail: Some(error_string),
                    feedback_event_ids: error_feedback
                        .map(|event| vec![processing_event.id.clone(), event.id])
                        .unwrap_or_else(|| vec![processing_event.id.clone()]),
                    result_event_id: None,
                });
            }
        }
    }

    Ok(ProviderRunReport {
        seconds,
        provider_pubkey: collected.provider_pubkey,
        local_model: local_target.map(|target| target.model),
        accepted_count,
        payment_required_count,
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
            | "payment_required"
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
            if let Some(hook) = slot
                .lock()
                .expect("test wallet invoice hook lock")
                .as_ref()
            {
                return hook(msats_to_sats_rounded_up(amount_msats), description, expiry_seconds);
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
