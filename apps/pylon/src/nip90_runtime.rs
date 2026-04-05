use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nostr::nip89::{HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO, PricingInfo};
use nostr::nip90::JobRequest;
use nostr::{Event, EventTemplate, finalize_event};
use nostr_client::{PoolConfig, RelayConfig, RelayMessage, RelayPool};
use openagents_provider_substrate::ProviderAdvertisedProduct;
use serde::Serialize;
use serde_json::json;

use crate::{
    LocalGemmaChatBackend, LocalGemmaChatEvent, LocalGemmaChatTarget, PylonConfig,
    PylonLedgerAnnouncement, PylonLedgerJob, PylonRelayActivity, ensure_identity,
    load_config_and_status, load_ledger, mutate_ledger, products_from_status,
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
    pub result_preview: Option<String>,
    pub error_detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderRunReport {
    pub seconds: u64,
    pub provider_pubkey: String,
    pub local_model: Option<String>,
    pub accepted_count: usize,
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
    let mut known_statuses = load_ledger(config_path)?
        .jobs
        .into_iter()
        .map(|job| (job.id, job.status))
        .collect::<BTreeMap<_, _>>();
    let mut report_entries = Vec::new();
    let mut accepted_count = 0usize;
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
                result_preview: None,
                error_detail: Some("job already handled locally".to_string()),
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
                result_preview: None,
                error_detail: observed.entry.drop_reason.clone(),
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
                result_preview: None,
                error_detail: Some("provider_not_online".to_string()),
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
                result_preview: None,
                error_detail: Some("no_local_supply".to_string()),
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
                    result_preview: None,
                    error_detail: Some("invalid_request".to_string()),
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
                result_preview: None,
                error_detail: Some("model_unavailable".to_string()),
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
                result_preview: None,
                error_detail: Some("missing_text_input".to_string()),
            });
            continue;
        };

        accepted_count += 1;
        persist_provider_run_state(
            config_path,
            collected.provider_pubkey.as_str(),
            &observed.entry,
            "accepted_local",
            None,
            None,
        )?;
        persist_provider_run_state(
            config_path,
            collected.provider_pubkey.as_str(),
            &observed.entry,
            "processing_local",
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
                completed_count += 1;
                let result_preview = preview_text(output.as_str(), 280);
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "completed_local",
                    None,
                    Some(result_preview.as_str()),
                )?;
                known_statuses.insert(request_event_id.clone(), "completed_local".to_string());
                report_entries.push(ProviderRunEntry {
                    request_event_id: observed.entry.request_event_id.clone(),
                    requester_pubkey: observed.entry.requester_pubkey.clone(),
                    relay_url: observed.entry.relay_url.clone(),
                    status: "completed".to_string(),
                    prompt_preview: Some(preview_text(prompt.as_str(), 72)),
                    model: Some(target.model.clone()),
                    bid_msats: observed.entry.bid_msats,
                    result_preview: Some(result_preview),
                    error_detail: None,
                });
            }
            Err(error) => {
                failed_count += 1;
                let error_string = error.to_string();
                persist_provider_run_state(
                    config_path,
                    collected.provider_pubkey.as_str(),
                    &observed.entry,
                    "failed_local",
                    Some(error_string.as_str()),
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
                    result_preview: None,
                    error_detail: Some(error_string),
                });
            }
        }
    }

    Ok(ProviderRunReport {
        seconds,
        provider_pubkey: collected.provider_pubkey,
        local_model: local_target.map(|target| target.model),
        accepted_count,
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
        if let Some(result_preview) = entry.result_preview.as_deref() {
            lines.push(format!("result_preview: {result_preview}"));
        }
        if let Some(error_detail) = entry.error_detail.as_deref() {
            lines.push(format!("error_detail: {error_detail}"));
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
            | "payment_required"
            | "settled"
            | "result_published"
    )
}

fn persist_provider_run_state(
    config_path: &Path,
    provider_pubkey: &str,
    entry: &ProviderIntakeEntry,
    status: &str,
    error_detail: Option<&str>,
    result_preview: Option<&str>,
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
        job.status = status.to_string();
        job.error_detail = error_detail.map(ToString::to_string);
        if let Some(result_preview) = result_preview {
            job.result_preview = Some(result_preview.to_string());
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
        Ok(())
    })?;
    Ok(())
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
