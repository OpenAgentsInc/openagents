use std::path::PathBuf;
use std::str::FromStr;

use codex_client::{ThreadResumeParams, ThreadStartParams, TurnStartParams, UserInput};
use nostr::nip90::{
    DataVendingDeliveryMode, DataVendingPreviewPosture, DataVendingResult, JobFeedback, JobStatus,
    create_data_vending_result_event, create_job_feedback_event,
};
use nostr::{Event, EventTemplate, NostrIdentity};
use openagents_kernel_core::authority::{
    AcceptAccessGrantRequest, CreateAccessGrantRequest, IssueDeliveryBundleRequest,
    KernelAuthority, RegisterDataAssetRequest, RevokeAccessGrantRequest,
};
use openagents_kernel_core::data::{
    AccessGrantStatus, DeliveryBundle, DeliveryBundleStatus, RevocationReceipt,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::{
    Asset, EvidenceRef, Money, MoneyAmount, PolicyContext, ReceiptHints, TraceContext,
};
use openagents_spark::PaymentSummary;
use serde_json::json;

use crate::app_state::{
    AutopilotRole, DataMarketLifecycleEntry, DataSellerCodexSessionPhase,
    DataSellerIncomingRequest, DataSellerRevocationAction, DataSellerSkillAttachment, RenderState,
};
use crate::codex_lane::CodexLaneCommand;
use crate::provider_nip90_lane::{
    ProviderNip90DataVendingProfile, ProviderNip90LaneCommand, ProviderNip90PublishOutcome,
    ProviderNip90PublishRole,
};
use crate::spark_wallet::{
    SparkWalletCommand, decode_lightning_invoice_payment_hash, is_settled_wallet_payment_status,
    normalize_lightning_invoice_ref,
};

fn current_session_cwd() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|value| value.into_os_string().into_string().ok())
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(i64::MAX as u128) as i64
        })
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn is_kernel_idempotency_conflict(error: &str) -> bool {
    error.contains("kernel_idempotency_conflict")
}

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid identity private_key_hex: {error}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "invalid identity private_key_hex length {}, expected 32 bytes",
            key_bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

fn sign_event_template(
    identity: &NostrIdentity,
    template: &EventTemplate,
) -> Result<Event, String> {
    let private_key = parse_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| format!("Cannot sign NIP-90 event template: {error}"))
}

fn sync_data_seller_nip90_profile(state: &mut RenderState) {
    let profile =
        state
            .data_seller
            .derived_nip90_profile()
            .map(|profile| ProviderNip90DataVendingProfile {
                profile_id: profile.profile_id,
                request_kind: profile.request_kind,
                result_kind: profile.result_kind,
                kind_posture: profile.kind_posture,
                targeting_posture: profile.targeting_posture,
                asset_families: profile.asset_families,
                delivery_modes: profile.delivery_modes,
                preview_postures: profile.preview_postures,
            });
    let _ = state.queue_provider_nip90_lane_command(
        ProviderNip90LaneCommand::ConfigureDataVendingProfile { profile },
    );
}

fn build_data_seller_payment_required_feedback_event(
    identity: &NostrIdentity,
    request_id: &str,
    requester: &str,
    quoted_price_sats: u64,
    bolt11: &str,
) -> Result<Event, String> {
    let feedback = JobFeedback::new(JobStatus::PaymentRequired, request_id, requester)
        .with_status_extra("lightning settlement required")
        .with_content("Pay the attached Lightning invoice before delivery can proceed.".to_string())
        .with_amount(
            quoted_price_sats.saturating_mul(1000),
            Some(bolt11.to_string()),
        );
    let template = create_job_feedback_event(&feedback);
    sign_event_template(identity, &template)
}

fn canonical_component(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    trimmed
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn sats_money(amount_sats: u64) -> Money {
    Money {
        asset: Asset::Btc,
        amount: MoneyAmount::AmountSats(amount_sats),
    }
}

fn delivery_mode_for_request(request: &DataSellerIncomingRequest) -> DataVendingDeliveryMode {
    request
        .delivery_mode
        .as_deref()
        .and_then(|value| DataVendingDeliveryMode::from_str(value).ok())
        .unwrap_or_default()
}

fn preview_posture_for_request(request: &DataSellerIncomingRequest) -> DataVendingPreviewPosture {
    if request
        .delivery_draft
        .preview_text
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        DataVendingPreviewPosture::InlinePreview
    } else {
        request
            .preview_posture
            .as_deref()
            .and_then(|value| DataVendingPreviewPosture::from_str(value).ok())
            .unwrap_or(DataVendingPreviewPosture::MetadataOnly)
    }
}

fn build_delivery_result_content(
    request: &DataSellerIncomingRequest,
    delivery: &DeliveryBundle,
) -> String {
    if let Some(preview_text) = request
        .delivery_draft
        .preview_text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        preview_text.to_string()
    } else {
        json!({
            "delivery_bundle_id": delivery.delivery_bundle_id,
            "delivery_ref": delivery.delivery_ref,
            "delivery_digest": delivery.delivery_digest,
            "grant_id": delivery.grant_id,
            "asset_id": delivery.asset_id,
        })
        .to_string()
    }
}

fn build_data_seller_delivery_result_event(
    identity: &NostrIdentity,
    request: &DataSellerIncomingRequest,
    delivery: &DeliveryBundle,
) -> Result<Event, String> {
    let asset_ref = request
        .asset_ref
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(delivery.asset_id.as_str());
    let mut result = DataVendingResult::new(
        request.request_kind,
        request.request_id.as_str(),
        request.requester.as_str(),
        asset_ref,
        delivery.delivery_bundle_id.as_str(),
        build_delivery_result_content(request, delivery),
    )
    .map_err(|error| format!("Cannot build data-vending result: {error}"))?
    .with_delivery_mode(delivery_mode_for_request(request))
    .with_preview_posture(preview_posture_for_request(request))
    .with_grant_id(delivery.grant_id.clone())
    .with_delivery_ref(delivery.delivery_ref.clone());
    if request.encrypted
        || matches!(
            delivery_mode_for_request(request),
            DataVendingDeliveryMode::EncryptedPointer
        )
    {
        result = result.with_encrypted_content();
    }
    let template = create_data_vending_result_event(&result)
        .map_err(|error| format!("Cannot build NIP-90 delivery result event: {error}"))?;
    sign_event_template(identity, &template)
}

fn build_accept_access_grant_request(
    request: &DataSellerIncomingRequest,
    grant_id: &str,
    session_id: Option<&str>,
    accepted_at_ms: i64,
) -> AcceptAccessGrantRequest {
    let payment_amount_sats = request
        .payment_amount_sats
        .or(request.required_price_sats)
        .or((request.price_sats > 0).then_some(request.price_sats));
    let payment_pointer = request
        .payment_pointer
        .as_deref()
        .unwrap_or("missing_payment_pointer");
    AcceptAccessGrantRequest {
        idempotency_key: format!(
            "accept_access_grant:{}",
            sha256_prefixed_text(
                format!(
                    "{grant_id}|{}|{payment_pointer}|{}",
                    request.request_id,
                    payment_amount_sats.unwrap_or_default()
                )
                .as_str()
            )
        ),
        trace: TraceContext {
            session_id: session_id.map(str::to_string),
            trajectory_hash: session_id.map(sha256_prefixed_text),
            claim_id: Some(format!(
                "claim.access_grant.accept.{}",
                canonical_component(grant_id)
            )),
            ..Default::default()
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.data_market.delivery.mvp".to_string(),
            policy_version: "v0".to_string(),
            approved_by: "openagents.data_market.seller_pane".to_string(),
        },
        grant_id: grant_id.to_string(),
        consumer_id: request.effective_consumer_id().to_string(),
        accepted_at_ms,
        settlement_price: payment_amount_sats.map(sats_money),
        metadata: json!({
            "request_id": request.request_id,
            "payment_feedback_event_id": request.payment_feedback_event_id,
            "payment_pointer": request.payment_pointer,
            "payment_amount_sats": request.payment_amount_sats,
        }),
        evidence: {
            let mut evidence = vec![EvidenceRef::new(
                "nip90_request_event",
                format!("nostr:event:{}", request.request_id),
                request.request_id.as_str(),
            )];
            if let Some(payment_pointer) = request
                .payment_pointer
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "payment_pointer",
                    format!(
                        "oa://autopilot/payments/{}",
                        canonical_component(payment_pointer)
                    ),
                    payment_pointer,
                ));
            }
            evidence
        },
        hints: ReceiptHints::default(),
    }
}

fn build_issue_delivery_bundle_request(
    request: &DataSellerIncomingRequest,
    grant_id: &str,
    asset_id: &str,
    provider_id: &str,
    consumer_id: &str,
    session_id: Option<&str>,
    created_at_ms: i64,
) -> IssueDeliveryBundleRequest {
    let delivery_ref = request
        .delivery_draft
        .delivery_ref
        .clone()
        .unwrap_or_else(|| {
            format!(
                "oa://deliveries/{}",
                canonical_component(request.request_id.as_str())
            )
        });
    let delivery_bundle_id = format!(
        "delivery_bundle.{}.{}.{}",
        canonical_component(provider_id),
        canonical_component(grant_id),
        canonical_component(request.request_id.as_str())
    );
    let expires_at_ms = request.delivery_draft.expires_in_hours.map(|hours| {
        created_at_ms.saturating_add(
            i64::try_from(hours)
                .unwrap_or(i64::MAX)
                .saturating_mul(60)
                .saturating_mul(60)
                .saturating_mul(1000),
        )
    });
    IssueDeliveryBundleRequest {
        idempotency_key: format!(
            "issue_delivery_bundle:{}",
            sha256_prefixed_text(
                format!(
                    "{grant_id}|{}|{}|{}",
                    request.request_id,
                    delivery_ref,
                    request
                        .delivery_draft
                        .delivery_digest
                        .as_deref()
                        .unwrap_or("missing_delivery_digest")
                )
                .as_str()
            )
        ),
        trace: TraceContext {
            session_id: session_id.map(str::to_string),
            trajectory_hash: session_id.map(sha256_prefixed_text),
            claim_id: Some(format!(
                "claim.data.delivery.issue.{}",
                canonical_component(request.request_id.as_str())
            )),
            ..Default::default()
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.data_market.delivery.mvp".to_string(),
            policy_version: "v0".to_string(),
            approved_by: "openagents.data_market.seller_pane".to_string(),
        },
        delivery_bundle: DeliveryBundle {
            delivery_bundle_id,
            asset_id: asset_id.to_string(),
            grant_id: grant_id.to_string(),
            provider_id: provider_id.to_string(),
            consumer_id: consumer_id.to_string(),
            created_at_ms,
            delivery_ref,
            delivery_digest: request.delivery_draft.delivery_digest.clone(),
            bundle_size_bytes: request.delivery_draft.bundle_size_bytes,
            manifest_refs: request.delivery_draft.manifest_refs.clone(),
            expires_at_ms,
            status: DeliveryBundleStatus::Issued,
            metadata: json!({
                "request_id": request.request_id,
                "payment_pointer": request.payment_pointer,
                "payment_amount_sats": request.payment_amount_sats,
                "delivery_mode": request.delivery_mode,
                "preview_posture": request.preview_posture,
                "preview_text": request.delivery_draft.preview_text,
            }),
        },
        evidence: {
            let mut evidence = vec![EvidenceRef::new(
                "nip90_request_event",
                format!("nostr:event:{}", request.request_id),
                request.request_id.as_str(),
            )];
            if let Some(payment_pointer) = request
                .payment_pointer
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "payment_pointer",
                    format!(
                        "oa://autopilot/payments/{}",
                        canonical_component(payment_pointer)
                    ),
                    payment_pointer,
                ));
            }
            if let Some(delivery_digest) = request
                .delivery_draft
                .delivery_digest
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "delivery_digest",
                    format!(
                        "oa://autopilot/data_deliveries/{}/digest",
                        canonical_component(request.request_id.as_str())
                    ),
                    delivery_digest,
                ));
            }
            evidence
        },
        hints: ReceiptHints::default(),
    }
}

fn default_revocation_reason_code(action: DataSellerRevocationAction) -> &'static str {
    match action {
        DataSellerRevocationAction::Revoke => "seller_revoked_access",
        DataSellerRevocationAction::Expire => "access_window_expired",
    }
}

fn explicit_revocation_reason_code(
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
) -> String {
    reason_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_revocation_reason_code(action).to_string())
}

fn build_revoke_access_grant_request(
    request: &DataSellerIncomingRequest,
    grant_id: &str,
    asset_id: &str,
    provider_id: &str,
    consumer_id: Option<&str>,
    revoked_delivery_bundle_ids: Vec<String>,
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
    session_id: Option<&str>,
    created_at_ms: i64,
) -> RevokeAccessGrantRequest {
    let canonical_reason_code = explicit_revocation_reason_code(action, reason_code);
    RevokeAccessGrantRequest {
        idempotency_key: format!(
            "revoke_access_grant:{}",
            sha256_prefixed_text(
                format!(
                    "{grant_id}|{}|{}|{}",
                    request.request_id,
                    action.label(),
                    canonical_reason_code
                )
                .as_str()
            )
        ),
        trace: TraceContext {
            session_id: session_id.map(str::to_string),
            trajectory_hash: session_id.map(sha256_prefixed_text),
            claim_id: Some(format!(
                "claim.data.access.{}.{}",
                action.label(),
                canonical_component(request.request_id.as_str())
            )),
            ..Default::default()
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.data_market.revocation.mvp".to_string(),
            policy_version: "v0".to_string(),
            approved_by: "openagents.data_market.seller_pane".to_string(),
        },
        revocation: RevocationReceipt {
            revocation_id: format!(
                "revocation.{}.{}.{}.{}",
                canonical_component(provider_id),
                canonical_component(grant_id),
                canonical_component(request.request_id.as_str()),
                action.label()
            ),
            asset_id: asset_id.to_string(),
            grant_id: grant_id.to_string(),
            provider_id: provider_id.to_string(),
            consumer_id: consumer_id.map(str::to_string),
            created_at_ms,
            reason_code: canonical_reason_code.clone(),
            refund_amount: None,
            revoked_delivery_bundle_ids: revoked_delivery_bundle_ids.clone(),
            replacement_delivery_bundle_id: None,
            status: Default::default(),
            metadata: json!({
                "request_id": request.request_id,
                "control_action": action.label(),
                "delivery_bundle_ids": revoked_delivery_bundle_ids,
                "payment_pointer": request.payment_pointer,
                "payment_amount_sats": request.payment_amount_sats,
            }),
        },
        evidence: {
            let mut evidence = vec![EvidenceRef::new(
                "nip90_request_event",
                format!("nostr:event:{}", request.request_id),
                request.request_id.as_str(),
            )];
            if let Some(payment_pointer) = request
                .payment_pointer
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                evidence.push(EvidenceRef::new(
                    "payment_pointer",
                    format!(
                        "oa://autopilot/payments/{}",
                        canonical_component(payment_pointer)
                    ),
                    payment_pointer,
                ));
            }
            for delivery_bundle_id in &request
                .delivery_bundle_id
                .iter()
                .cloned()
                .chain(revoked_delivery_bundle_ids.iter().cloned())
                .collect::<Vec<_>>()
            {
                evidence.push(EvidenceRef::new(
                    "delivery_bundle",
                    format!(
                        "oa://autopilot/data_deliveries/{}",
                        canonical_component(delivery_bundle_id.as_str())
                    ),
                    delivery_bundle_id.as_str(),
                ));
            }
            evidence
        },
        hints: ReceiptHints::default(),
    }
}

fn record_data_market_lifecycle_entry(
    state: &mut RenderState,
    occurred_at_ms: i64,
    stage: impl Into<String>,
    status: impl Into<String>,
    subject_id: impl Into<String>,
    counterparty: Option<String>,
    policy_id: Option<String>,
    receipt_id: Option<String>,
    summary: impl Into<String>,
) {
    state
        .data_market
        .record_lifecycle_entry(DataMarketLifecycleEntry {
            occurred_at_ms,
            stage: stage.into(),
            status: status.into(),
            subject_id: subject_id.into(),
            counterparty,
            policy_id,
            receipt_id,
            summary: summary.into(),
        });
}

fn matched_settled_receive_payment<'a>(
    invoice: Option<&str>,
    payment_hash: Option<&str>,
    recent_payments: &'a [PaymentSummary],
) -> Option<&'a PaymentSummary> {
    let expected_invoice = invoice.and_then(normalize_lightning_invoice_ref);
    let expected_payment_hash = payment_hash
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .or_else(|| invoice.and_then(decode_lightning_invoice_payment_hash));

    recent_payments
        .iter()
        .filter(|payment| {
            payment.direction.eq_ignore_ascii_case("receive")
                && is_settled_wallet_payment_status(payment.status.as_str())
                && !payment.id.trim().is_empty()
        })
        .filter(|payment| {
            expected_invoice.as_deref().is_some_and(|expected_invoice| {
                payment
                    .invoice
                    .as_deref()
                    .and_then(normalize_lightning_invoice_ref)
                    .is_some_and(|candidate| candidate == expected_invoice)
            }) || expected_payment_hash
                .as_deref()
                .is_some_and(|expected_payment_hash| {
                    payment
                        .payment_hash
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_ascii_lowercase)
                        .is_some_and(|candidate| candidate == expected_payment_hash)
                })
        })
        .max_by_key(|payment| payment.timestamp)
}

pub(crate) fn ensure_data_seller_codex_session(state: &mut RenderState) -> bool {
    if matches!(
        state.data_seller.codex_session_phase,
        DataSellerCodexSessionPhase::Starting | DataSellerCodexSessionPhase::Resuming
    ) {
        return true;
    }

    let cwd = current_session_cwd();
    match crate::skill_autoload::ensure_required_data_market_skills() {
        Ok(skills) => state.data_seller.set_required_skill_attachments(
            skills
                .into_iter()
                .map(|skill| DataSellerSkillAttachment {
                    name: skill.name,
                    path: skill.path,
                })
                .collect(),
        ),
        Err(error) => {
            tracing::warn!(
                "failed to auto-provision managed Data Market skills before seller session: {}",
                error
            );
            state.data_seller.last_error =
                Some(format!("Failed to auto-provision seller skills: {error}"));
        }
    }
    let model = state.autopilot_chat.selected_model_override();
    let service_tier = state.autopilot_chat.service_tier.request_value();
    let approval_policy = Some(state.autopilot_chat.approval_mode);
    let sandbox = Some(state.autopilot_chat.sandbox_mode);
    let personality = state.data_seller.codex_profile.personality.request_value();

    let command = if let Some(thread_id) = state.data_seller.codex_thread_id.clone() {
        state.data_seller.begin_codex_session_resume(cwd.clone());
        CodexLaneCommand::ThreadResume(ThreadResumeParams {
            thread_id,
            path: None,
            model,
            model_provider: None,
            service_tier,
            cwd,
            approval_policy,
            sandbox,
            personality,
        })
    } else {
        state.data_seller.begin_codex_session_start(cwd.clone());
        CodexLaneCommand::ThreadStart(ThreadStartParams {
            model,
            model_provider: None,
            service_tier,
            cwd,
            approval_policy,
            sandbox,
            personality,
            ephemeral: None,
            dynamic_tools: Some(crate::openagents_dynamic_tools::openagents_dynamic_tool_specs()),
        })
    };

    if let Err(error) = state.queue_codex_command(command) {
        state.data_seller.record_codex_session_error(format!(
            "Failed to queue Data Seller Codex session: {error}"
        ));
    }
    true
}

pub(crate) fn submit_data_seller_prompt_text(state: &mut RenderState, prompt: String) -> bool {
    if state.data_seller.codex_thread_id.is_none() {
        ensure_data_seller_codex_session(state);
    }
    let Some(thread_id) = state.data_seller.codex_thread_id.clone() else {
        state.data_seller.last_error = Some(
            "Data Seller session is still starting. Wait for the dedicated thread, then retry."
                .to_string(),
        );
        return true;
    };

    if prompt.trim().is_empty() {
        return false;
    }

    let mut input = vec![UserInput::Text {
        text: prompt.clone(),
        text_elements: Vec::new(),
    }];
    for skill in &state.data_seller.required_skill_attachments {
        input.push(UserInput::Skill {
            name: skill.name.clone(),
            path: PathBuf::from(skill.path.clone()),
        });
    }

    let command = CodexLaneCommand::TurnStart(TurnStartParams {
        thread_id: thread_id.clone(),
        input,
        cwd: state
            .data_seller
            .codex_session_cwd
            .clone()
            .map(PathBuf::from),
        approval_policy: None,
        sandbox_policy: None,
        model: None,
        service_tier: None,
        effort: None,
        summary: None,
        personality: state.data_seller.codex_profile.personality.request_value(),
        output_schema: None,
        collaboration_mode: None,
    });

    match state.queue_codex_command(command) {
        Ok(command_seq) => {
            state.autopilot_chat.append_cached_thread_message(
                &thread_id,
                AutopilotRole::User,
                prompt,
            );
            state.data_seller_inputs.composer.set_value(String::new());
            state.data_seller.last_error = None;
            state.data_seller.last_action = Some(format!(
                "Queued Data Seller turn on thread {thread_id} (command #{command_seq})"
            ));
            state.data_seller.status_line =
                "Seller prompt sent. Waiting for Codex to normalize the draft or ask follow-up questions."
                    .to_string();
            state.data_seller.set_codex_thread_status("running");
        }
        Err(error) => {
            state.data_seller.last_error =
                Some(format!("Failed to queue Data Seller turn: {error}"));
        }
    }
    true
}

pub(crate) fn submit_data_seller_prompt(state: &mut RenderState) -> bool {
    let prompt = state
        .data_seller_inputs
        .composer
        .get_value()
        .trim()
        .to_string();
    if prompt.is_empty() {
        return false;
    }
    submit_data_seller_prompt_text(state, prompt)
}

pub(crate) fn request_data_seller_preview(state: &mut RenderState) -> bool {
    let provider_id = crate::kernel_control::provider_id_for_state(state);
    state
        .data_seller
        .request_preview(provider_id.as_str(), current_epoch_ms());
    true
}

pub(crate) fn confirm_data_seller_preview(state: &mut RenderState) -> bool {
    state.data_seller.confirm_asset_preview();
    true
}

pub(crate) fn request_data_seller_grant_preview(state: &mut RenderState) -> bool {
    let provider_id = crate::kernel_control::provider_id_for_state(state);
    state
        .data_seller
        .request_grant_preview(provider_id.as_str(), current_epoch_ms());
    true
}

pub(crate) fn request_data_seller_payment_required(
    state: &mut RenderState,
    request_id: &str,
) -> bool {
    if state.active_job.payment_required_invoice_requested {
        state.data_seller.last_error = Some(
            "A compute-market settlement invoice is already being created. Wait for it to finish before starting a data-market payment quote."
                .to_string(),
        );
        return true;
    }

    let amount_sats = match state.data_seller.request_payment_required_quote(request_id) {
        Ok(amount_sats) => amount_sats,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            return true;
        }
    };

    state.spark_wallet.last_error = None;
    if let Err(error) = state
        .spark_worker
        .enqueue(SparkWalletCommand::CreateBolt11Invoice {
            amount_sats,
            description: Some(format!("OpenAgents data access {request_id}")),
            expiry_seconds: Some(3600),
        })
    {
        state.data_seller.note_payment_invoice_failed(
            request_id,
            format!("Failed to queue Spark invoice creation: {error}"),
        );
        return true;
    }

    state.provider_runtime.last_result = Some(format!(
        "queued seller Lightning invoice creation for data request {}",
        request_id
    ));
    true
}

pub(crate) fn issue_data_seller_delivery(state: &mut RenderState, request_id: &str) -> bool {
    if let Err(error) = state.data_seller.request_issue_delivery(request_id) {
        state.data_seller.last_error = Some(error);
        return true;
    }

    let Some(request) = state.data_seller.request_by_id(request_id).cloned() else {
        state.data_seller.last_error = Some(format!(
            "Unknown data-access request {request_id} after delivery issue start."
        ));
        return true;
    };
    let Some(grant_id) = request.matched_grant_id.clone() else {
        state.data_seller.note_delivery_issue_failed(
            request_id,
            "Matched grant is missing for delivery issuance.",
        );
        return true;
    };

    let client = match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => client,
        Err(error) => {
            state
                .data_seller
                .note_delivery_issue_failed(request_id, error);
            return true;
        }
    };

    let mut grant =
        match crate::kernel_control::run_kernel_call(client.get_access_grant(grant_id.as_str())) {
            Ok(grant) => grant,
            Err(error) => {
                state.data_seller.note_delivery_issue_failed(
                    request_id,
                    format!("Failed to load grant {grant_id} before delivery: {error}"),
                );
                return true;
            }
        };

    if grant.status == AccessGrantStatus::Offered {
        let accepted_at_ms = current_epoch_ms();
        let accept_request = build_accept_access_grant_request(
            &request,
            grant_id.as_str(),
            state.data_seller.codex_thread_id.as_deref(),
            accepted_at_ms,
        );
        let accept_response = match crate::kernel_control::run_kernel_call(
            client.accept_access_grant(accept_request),
        ) {
            Ok(response) => response,
            Err(error) => {
                state.data_seller.note_delivery_issue_failed(
                    request_id,
                    format!("Failed to accept grant {grant_id} before delivery: {error}"),
                );
                return true;
            }
        };
        grant = match crate::kernel_control::run_kernel_call(
            client.get_access_grant(grant_id.as_str()),
        ) {
            Ok(grant) => grant,
            Err(_) => accept_response.grant,
        };
    } else if !matches!(
        grant.status,
        AccessGrantStatus::Accepted | AccessGrantStatus::Delivered
    ) {
        state.data_seller.note_delivery_issue_failed(
            request_id,
            format!(
                "Grant {} is not ready for delivery. Current status is {}.",
                grant_id,
                grant.status.label()
            ),
        );
        return true;
    }
    state.data_seller.note_grant_state_reconciled(grant.clone());
    state
        .data_market
        .note_published_grant(grant.clone(), current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);

    let delivery = if let Some(existing_delivery_bundle_id) = request.delivery_bundle_id.as_deref()
    {
        match crate::kernel_control::run_kernel_call(
            client.get_delivery_bundle(existing_delivery_bundle_id),
        ) {
            Ok(delivery) => delivery,
            Err(error) => {
                state.data_seller.note_delivery_issue_failed(
                    request_id,
                    format!(
                        "Failed to reload previously issued delivery {}: {error}",
                        existing_delivery_bundle_id
                    ),
                );
                return true;
            }
        }
    } else {
        if let Err(error) = state.data_seller.note_delivery_bundle_issuing(request_id) {
            state
                .data_seller
                .note_delivery_issue_failed(request_id, error);
            return true;
        }
        let created_at_ms = current_epoch_ms();
        let consumer_id = grant
            .consumer_id
            .clone()
            .unwrap_or_else(|| request.requester.clone());
        let issue_request = build_issue_delivery_bundle_request(
            &request,
            grant.grant_id.as_str(),
            grant.asset_id.as_str(),
            grant.provider_id.as_str(),
            consumer_id.as_str(),
            state.data_seller.codex_thread_id.as_deref(),
            created_at_ms,
        );
        let issue_response = match crate::kernel_control::run_kernel_call(
            client.issue_delivery_bundle(issue_request),
        ) {
            Ok(response) => response,
            Err(error) => {
                state.data_seller.note_delivery_issue_failed(
                    request_id,
                    format!("Kernel authority rejected delivery issuance: {error}"),
                );
                return true;
            }
        };
        let receipt_id = Some(issue_response.receipt.receipt_id.clone());
        let delivery_bundle_id = issue_response.delivery_bundle.delivery_bundle_id.clone();
        let readback_delivery = match crate::kernel_control::run_kernel_call(
            client.get_delivery_bundle(delivery_bundle_id.as_str()),
        ) {
            Ok(delivery) => delivery,
            Err(_) => issue_response.delivery_bundle,
        };
        if let Err(error) = state.data_seller.note_delivery_bundle_issued(
            request_id,
            readback_delivery.clone(),
            receipt_id,
        ) {
            state
                .data_seller
                .note_delivery_issue_failed(request_id, error);
            return true;
        }
        state
            .data_market
            .note_published_delivery(readback_delivery.clone(), current_epoch_ms());
        state.data_buyer.sync_selection(&state.data_market);
        record_data_market_lifecycle_entry(
            state,
            readback_delivery.created_at_ms,
            "delivery_issued",
            readback_delivery.status.label(),
            readback_delivery.delivery_bundle_id.clone(),
            Some(readback_delivery.consumer_id.clone()),
            Some(grant.permission_policy.policy_id.clone()),
            state.data_seller.last_delivery_publish_receipt_id.clone(),
            format!(
                "Issued delivery for grant {} with ref {}.",
                readback_delivery.grant_id, readback_delivery.delivery_ref
            ),
        );
        readback_delivery
    };
    state
        .data_market
        .note_published_delivery(delivery.clone(), current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);

    let identity = match state.nostr_identity.as_ref() {
        Some(identity) => identity,
        None => {
            state.data_seller.note_delivery_issue_failed(
                request_id,
                "Cannot publish delivery result: Nostr identity unavailable.",
            );
            return true;
        }
    };
    let event = match build_data_seller_delivery_result_event(identity, &request, &delivery) {
        Ok(event) => event,
        Err(error) => {
            state
                .data_seller
                .note_delivery_issue_failed(request_id, error);
            return true;
        }
    };
    if let Err(error) =
        state.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id: request_id.to_string(),
            role: ProviderNip90PublishRole::Result,
            event: Box::new(event),
        })
    {
        state.data_seller.note_delivery_issue_failed(
            request_id,
            format!("Cannot queue NIP-90 delivery result publish: {error}"),
        );
        return true;
    }

    if let Some(request) = state.data_seller.request_by_id_mut(request_id) {
        request.delivery_state = crate::app_state::DataSellerDeliveryState::PublishingResult;
        request.delivery_error = None;
    }
    state.data_seller.last_error = None;
    state.data_seller.last_action = Some(format!(
        "Queued NIP-90 delivery result publication for data request {}",
        request_id
    ));
    state.data_seller.status_line = format!(
        "Publishing NIP-90 delivery result for request {}.",
        request_id
    );

    state.provider_runtime.last_result = Some(format!(
        "queued delivery result publication for data request {}",
        request_id
    ));
    true
}

pub(crate) fn revoke_data_seller_access(
    state: &mut RenderState,
    request_id: &str,
    action: DataSellerRevocationAction,
    reason_code: Option<&str>,
) -> bool {
    if let Err(error) = state.data_seller.request_revoke_access(request_id, action) {
        state.data_seller.last_error = Some(error);
        return true;
    }

    let Some(request) = state.data_seller.request_by_id(request_id).cloned() else {
        state.data_seller.last_error = Some(format!(
            "Unknown data-access request {request_id} after revocation start."
        ));
        return true;
    };
    let Some(grant_id) = request.matched_grant_id.clone() else {
        state
            .data_seller
            .note_revocation_failed(request_id, "Matched grant is missing for revocation.");
        return true;
    };

    let client = match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => client,
        Err(error) => {
            state.data_seller.note_revocation_failed(request_id, error);
            return true;
        }
    };

    let grant =
        match crate::kernel_control::run_kernel_call(client.get_access_grant(grant_id.as_str())) {
            Ok(grant) => grant,
            Err(error) => {
                state.data_seller.note_revocation_failed(
                    request_id,
                    format!("Failed to load grant {grant_id} before revocation: {error}"),
                );
                return true;
            }
        };
    if matches!(
        grant.status,
        AccessGrantStatus::Revoked | AccessGrantStatus::Refunded | AccessGrantStatus::Expired
    ) {
        state.data_seller.note_revocation_failed(
            request_id,
            format!(
                "Grant {} is already in terminal status {}.",
                grant_id,
                grant.status.label()
            ),
        );
        return true;
    }

    let deliveries = match crate::kernel_control::run_kernel_call(client.list_delivery_bundles(
        None,
        Some(grant_id.as_str()),
        None,
        None,
        None,
    )) {
        Ok(deliveries) => deliveries,
        Err(error) => {
            state.data_seller.note_revocation_failed(
                request_id,
                format!("Failed to load deliveries for grant {grant_id}: {error}"),
            );
            return true;
        }
    };

    let now_ms = current_epoch_ms();
    if action == DataSellerRevocationAction::Expire {
        let grant_expired = now_ms >= grant.expires_at_ms;
        let delivery_expired = deliveries.iter().any(|delivery| {
            delivery
                .expires_at_ms
                .is_some_and(|expires_at_ms| now_ms >= expires_at_ms)
        });
        if !grant_expired && !delivery_expired {
            state.data_seller.note_revocation_failed(
                request_id,
                format!(
                    "Request {} cannot be expired yet because neither the grant nor any delivery bundle is past its expiry window.",
                    request_id
                ),
            );
            return true;
        }
    }

    let revoke_request = build_revoke_access_grant_request(
        &request,
        grant.grant_id.as_str(),
        grant.asset_id.as_str(),
        grant.provider_id.as_str(),
        grant.consumer_id.as_deref(),
        deliveries
            .iter()
            .map(|delivery| delivery.delivery_bundle_id.clone())
            .collect(),
        action,
        reason_code,
        state.data_seller.codex_thread_id.as_deref(),
        now_ms,
    );
    let revoke_response =
        match crate::kernel_control::run_kernel_call(client.revoke_access_grant(revoke_request)) {
            Ok(response) => response,
            Err(error) => {
                state.data_seller.note_revocation_failed(
                    request_id,
                    format!("Kernel authority rejected revocation control: {error}"),
                );
                return true;
            }
        };

    let revocation = match crate::kernel_control::run_kernel_call(
        client.get_revocation(revoke_response.revocation.revocation_id.as_str()),
    ) {
        Ok(revocation) => revocation,
        Err(_) => revoke_response.revocation,
    };
    let grant =
        match crate::kernel_control::run_kernel_call(client.get_access_grant(grant_id.as_str())) {
            Ok(grant) => grant,
            Err(error) => {
                state.data_seller.note_revocation_failed(
                    request_id,
                    format!("Failed to reload grant {grant_id} after revocation: {error}"),
                );
                return true;
            }
        };
    let deliveries = match crate::kernel_control::run_kernel_call(client.list_delivery_bundles(
        None,
        Some(grant_id.as_str()),
        None,
        None,
        None,
    )) {
        Ok(deliveries) => deliveries,
        Err(error) => {
            state.data_seller.note_revocation_failed(
                request_id,
                format!("Failed to reload deliveries for grant {grant_id}: {error}"),
            );
            return true;
        }
    };

    let reflected_at_ms = current_epoch_ms();
    if let Err(error) = state.data_seller.note_revocation_recorded(
        request_id,
        action,
        revocation.clone(),
        Some(revoke_response.receipt.receipt_id.clone()),
        grant.clone(),
        deliveries.as_slice(),
    ) {
        state.data_seller.note_revocation_failed(request_id, error);
        return true;
    }
    state
        .data_market
        .note_published_grant(grant, reflected_at_ms);
    state.data_buyer.sync_selection(&state.data_market);
    for delivery in deliveries {
        state
            .data_market
            .note_published_delivery(delivery, reflected_at_ms);
        state.data_buyer.sync_selection(&state.data_market);
    }
    state
        .data_market
        .note_published_revocation(revocation, reflected_at_ms);
    state.data_buyer.sync_selection(&state.data_market);
    if let Some((
        revocation_state,
        revocation_id,
        requester,
        policy_id,
        receipt_id,
        grant_id,
        reason_code,
    )) = state.data_seller.request_by_id(request_id).map(|request| {
        (
            request.revocation_state,
            request
                .revocation_id
                .clone()
                .unwrap_or_else(|| format!("revocation_for_{request_id}")),
            request.requester.clone(),
            state
                .data_seller
                .last_published_grant
                .as_ref()
                .map(|grant| grant.permission_policy.policy_id.clone()),
            request.revocation_receipt_id.clone(),
            request
                .matched_grant_id
                .clone()
                .unwrap_or_else(|| "unknown_grant".to_string()),
            request
                .revocation_reason_code
                .clone()
                .unwrap_or_else(|| "unspecified".to_string()),
        )
    }) {
        record_data_market_lifecycle_entry(
            state,
            reflected_at_ms,
            format!("access_{}", action.past_tense_label()),
            revocation_state.label(),
            revocation_id,
            Some(requester),
            policy_id,
            receipt_id,
            format!(
                "{} access for grant {} with reason {}.",
                action.past_tense_label(),
                grant_id,
                reason_code
            ),
        );
    }
    state.provider_runtime.last_result = Some(format!(
        "seller {} access for data request {}",
        action.past_tense_label(),
        request_id
    ));
    true
}

fn queue_data_seller_payment_required_feedback(
    state: &mut RenderState,
    request_id: &str,
) -> Result<(), String> {
    let request = state
        .data_seller
        .request_by_id(request_id)
        .ok_or_else(|| format!("Unknown data-access request {request_id}"))?;
    let request_id = request.request_id.clone();
    let requester = request.requester.clone();
    let quoted_price_sats = request
        .required_price_sats
        .or((request.price_sats > 0).then_some(request.price_sats))
        .ok_or_else(|| {
            format!(
                "Request {} does not have a non-zero quoted price",
                request_id
            )
        })?;
    let bolt11 = request
        .pending_bolt11
        .as_deref()
        .ok_or_else(|| {
            format!(
                "Request {} is missing the pending Lightning invoice",
                request_id
            )
        })?
        .to_string();
    let identity = state.nostr_identity.as_ref().ok_or_else(|| {
        "Cannot publish payment-required feedback: Nostr identity unavailable".to_string()
    })?;
    let event = build_data_seller_payment_required_feedback_event(
        identity,
        request_id.as_str(),
        requester.as_str(),
        quoted_price_sats,
        bolt11.as_str(),
    )?;
    state
        .queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id: request_id.clone(),
            role: ProviderNip90PublishRole::Feedback,
            event: Box::new(event),
        })
        .map_err(|error| format!("Cannot queue data-market payment-required feedback: {error}"))?;
    state.provider_runtime.last_result = Some(format!(
        "queued payment-required feedback for data request {}",
        request_id
    ));
    Ok(())
}

pub(crate) fn reconcile_data_seller_wallet_update(
    state: &mut RenderState,
    previous_invoice: Option<&str>,
    previous_error: Option<&str>,
) {
    if let Some(request_id) = state
        .data_seller
        .pending_payment_invoice_request_id()
        .map(str::to_string)
    {
        if state.spark_wallet.last_invoice.as_deref() != previous_invoice
            && let Some(invoice) = state.spark_wallet.last_invoice.as_deref()
        {
            if let Err(error) = state.data_seller.note_payment_invoice_created(
                request_id.as_str(),
                invoice,
                state.spark_wallet.last_invoice_created_at_epoch_seconds,
            ) {
                state
                    .data_seller
                    .note_payment_invoice_failed(request_id.as_str(), error);
                return;
            }
            if let Err(error) =
                queue_data_seller_payment_required_feedback(state, request_id.as_str())
            {
                state
                    .data_seller
                    .note_payment_invoice_failed(request_id.as_str(), error);
                return;
            }
        }

        if state.spark_wallet.last_error.as_deref() != previous_error
            && let Some(error) = state.spark_wallet.last_error.as_deref()
        {
            state
                .data_seller
                .note_payment_invoice_failed(request_id.as_str(), error);
        }
    }

    let now_epoch_seconds = current_epoch_seconds();
    let settled = state
        .data_seller
        .incoming_requests
        .iter()
        .filter(|request| {
            request.payment_state == crate::app_state::DataSellerPaymentState::AwaitingPayment
        })
        .filter(|request| request.payment_pointer.is_none())
        .filter_map(|request| {
            matched_settled_receive_payment(
                request.pending_bolt11.as_deref(),
                request.settlement_payment_hash.as_deref(),
                state.spark_wallet.recent_payments.as_slice(),
            )
            .map(|payment| {
                (
                    request.request_id.clone(),
                    payment.id.clone(),
                    payment.amount_sats,
                    payment.timestamp.max(now_epoch_seconds),
                )
            })
        })
        .collect::<Vec<_>>();
    for (request_id, payment_pointer, amount_sats, observed_at_epoch_seconds) in settled {
        if state.data_seller.note_payment_observed(
            request_id.as_str(),
            payment_pointer.as_str(),
            amount_sats,
            observed_at_epoch_seconds,
        ) {
            if let Some((request_id_owned, requester, payment_state, policy_id)) = state
                .data_seller
                .request_by_id(request_id.as_str())
                .map(|request| {
                    (
                        request.request_id.clone(),
                        request.requester.clone(),
                        request.payment_state,
                        state
                            .data_seller
                            .last_published_grant
                            .as_ref()
                            .map(|grant| grant.permission_policy.policy_id.clone()),
                    )
                })
            {
                record_data_market_lifecycle_entry(
                    state,
                    i64::try_from(observed_at_epoch_seconds).unwrap_or(i64::MAX) * 1000,
                    "payment_settled",
                    payment_state.label(),
                    request_id_owned.clone(),
                    Some(requester),
                    policy_id,
                    Some(payment_pointer.clone()),
                    format!(
                        "Settled {} sats for data request {}.",
                        amount_sats, request_id_owned
                    ),
                );
            }
            state.provider_runtime.last_result = Some(format!(
                "seller payment settled for data request {}",
                request_id
            ));
        }
    }
}

pub(crate) fn apply_data_seller_publish_outcome(
    state: &mut RenderState,
    outcome: &ProviderNip90PublishOutcome,
) -> bool {
    let Some((payment_state, delivery_state)) = state
        .data_seller
        .request_by_id(outcome.request_id.as_str())
        .map(|request| (request.payment_state, request.delivery_state))
    else {
        return false;
    };
    let published = outcome.accepted_relays > 0;
    let handled = match outcome.role {
        ProviderNip90PublishRole::Feedback => {
            if payment_state != crate::app_state::DataSellerPaymentState::PublishingFeedback {
                return false;
            }
            let handled = state.data_seller.note_payment_feedback_publish_outcome(
                outcome.request_id.as_str(),
                published,
                published.then_some(outcome.event_id.as_str()),
                outcome.first_error.as_deref(),
            );
            if handled && published {
                if let Some((request_id_owned, requester, payment_state, policy_id, quoted_sats)) =
                    state
                        .data_seller
                        .request_by_id(outcome.request_id.as_str())
                        .map(|request| {
                            (
                                request.request_id.clone(),
                                request.requester.clone(),
                                request.payment_state,
                                state
                                    .data_seller
                                    .last_published_grant
                                    .as_ref()
                                    .map(|grant| grant.permission_policy.policy_id.clone()),
                                request.required_price_sats.unwrap_or(request.price_sats),
                            )
                        })
                {
                    record_data_market_lifecycle_entry(
                        state,
                        current_epoch_ms(),
                        "payment_required_published",
                        payment_state.label(),
                        request_id_owned,
                        Some(requester),
                        policy_id,
                        Some(outcome.event_id.clone()),
                        format!(
                            "Published payment-required feedback for {} sats.",
                            quoted_sats
                        ),
                    );
                }
                state.provider_runtime.last_result = Some(format!(
                    "seller requested Lightning payment for data request {}",
                    outcome.request_id
                ));
            }
            handled
        }
        ProviderNip90PublishRole::Result => {
            if delivery_state != crate::app_state::DataSellerDeliveryState::PublishingResult {
                return false;
            }
            let handled = state.data_seller.note_delivery_result_publish_outcome(
                outcome.request_id.as_str(),
                published,
                published.then_some(outcome.event_id.as_str()),
                outcome.first_error.as_deref(),
            );
            if handled && published {
                state.provider_runtime.last_result = Some(format!(
                    "seller published delivery result for data request {}",
                    outcome.request_id
                ));
            }
            handled
        }
        _ => return false,
    };
    if handled && !published {
        state.provider_runtime.last_result = Some(format!(
            "seller publish failed for data request {}",
            outcome.request_id
        ));
    }
    handled
}

pub(crate) fn publish_data_seller_asset(state: &mut RenderState) -> bool {
    state.data_seller.request_publish();
    if !state.data_seller.publish_is_armed() {
        return true;
    }

    let preview_payload = match state
        .data_seller
        .active_draft
        .last_previewed_asset_payload
        .clone()
    {
        Some(payload) => payload,
        None => {
            state.data_seller.last_error =
                Some("Publish is armed but the exact preview payload is missing.".to_string());
            return true;
        }
    };
    let request: RegisterDataAssetRequest = match serde_json::from_value(preview_payload) {
        Ok(request) => request,
        Err(error) => {
            state.data_seller.last_error = Some(format!(
                "Failed to decode the exact preview payload into RegisterDataAssetRequest: {error}"
            ));
            state.data_seller.status_line =
                "Publish blocked because the preview payload is no longer valid.".to_string();
            return true;
        }
    };

    let client = match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => client,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Publish blocked because kernel authority is unavailable.".to_string();
            return true;
        }
    };

    let expected_asset_id = request.asset.asset_id.clone();
    let response = match crate::kernel_control::run_kernel_call(client.register_data_asset(request))
    {
        Ok(response) => response,
        Err(error) => {
            if is_kernel_idempotency_conflict(error.as_str()) {
                let readback_asset = match crate::kernel_control::run_kernel_call(
                    client.get_data_asset(expected_asset_id.as_str()),
                ) {
                    Ok(asset) => asset,
                    Err(readback_error) => {
                        state.data_seller.last_error = Some(format!(
                            "{error}; existing asset read-back failed: {readback_error}"
                        ));
                        state.data_seller.status_line =
                            "Kernel authority reported an asset replay conflict and read-back failed."
                                .to_string();
                        return true;
                    }
                };
                state
                    .data_seller
                    .note_asset_published(readback_asset.clone(), None);
                state
                    .data_market
                    .note_published_asset(readback_asset, current_epoch_ms());
                state.data_buyer.sync_selection(&state.data_market);
                if let Some(asset) = state.data_seller.last_published_asset.clone() {
                    record_data_market_lifecycle_entry(
                        state,
                        asset.created_at_ms,
                        "asset_published",
                        asset.status.label(),
                        asset.asset_id.clone(),
                        Some(asset.provider_id.clone()),
                        asset
                            .default_policy
                            .as_ref()
                            .map(|policy| policy.policy_id.clone()),
                        None,
                        format!(
                            "Re-synced existing asset {} from kernel after idempotent replay.",
                            asset.title
                        ),
                    );
                }
                sync_data_seller_nip90_profile(state);
                return true;
            }
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Kernel authority rejected the asset publication.".to_string();
            return true;
        }
    };
    let asset_id = response.asset.asset_id.clone();
    let receipt_id = Some(response.receipt.receipt_id.clone());
    let readback_asset =
        match crate::kernel_control::run_kernel_call(client.get_data_asset(asset_id.as_str())) {
            Ok(asset) => asset,
            Err(error) => {
                let fallback_asset = response.asset.clone();
                state
                    .data_seller
                    .note_asset_published(response.asset, receipt_id);
                if let Some(asset) = state.data_seller.last_published_asset.clone() {
                    state
                        .data_market
                        .note_published_asset(asset, current_epoch_ms());
                    state.data_buyer.sync_selection(&state.data_market);
                }
                record_data_market_lifecycle_entry(
                    state,
                    current_epoch_ms(),
                    "asset_published",
                    "published",
                    asset_id,
                    Some(fallback_asset.provider_id.clone()),
                    fallback_asset
                        .default_policy
                        .as_ref()
                        .map(|policy| policy.policy_id.clone()),
                    state.data_seller.last_publish_receipt_id.clone(),
                    format!("Published asset {} from seller lane.", fallback_asset.title),
                );
                sync_data_seller_nip90_profile(state);
                state.data_seller.last_error = Some(format!(
                    "Asset was published but the immediate kernel read-back failed: {error}"
                ));
                state.data_seller.status_line =
                    "Asset published, but immediate kernel read-back failed.".to_string();
                return true;
            }
        };

    state
        .data_seller
        .note_asset_published(readback_asset.clone(), receipt_id);
    state
        .data_market
        .note_published_asset(readback_asset, current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);
    if let Some(asset) = state.data_seller.last_published_asset.clone() {
        record_data_market_lifecycle_entry(
            state,
            asset.created_at_ms,
            "asset_published",
            asset.status.label(),
            asset.asset_id.clone(),
            Some(asset.provider_id.clone()),
            asset
                .default_policy
                .as_ref()
                .map(|policy| policy.policy_id.clone()),
            state.data_seller.last_publish_receipt_id.clone(),
            format!("Published asset {} from seller lane.", asset.title),
        );
    }
    sync_data_seller_nip90_profile(state);
    true
}

pub(crate) fn publish_data_seller_grant(state: &mut RenderState) -> bool {
    state.data_seller.request_publish_grant();
    if !state.data_seller.grant_publish_is_armed() {
        return true;
    }

    let preview_payload = match state
        .data_seller
        .active_draft
        .last_previewed_grant_payload
        .clone()
    {
        Some(payload) => payload,
        None => {
            state.data_seller.last_error = Some(
                "Grant publish is armed but the exact preview payload is missing.".to_string(),
            );
            return true;
        }
    };
    let request: CreateAccessGrantRequest = match serde_json::from_value(preview_payload) {
        Ok(request) => request,
        Err(error) => {
            state.data_seller.last_error = Some(format!(
                "Failed to decode the exact grant preview payload into CreateAccessGrantRequest: {error}"
            ));
            state.data_seller.status_line =
                "Grant publish blocked because the preview payload is no longer valid.".to_string();
            return true;
        }
    };

    let client = match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => client,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Grant publish blocked because kernel authority is unavailable.".to_string();
            return true;
        }
    };

    let expected_grant_id = request.grant.grant_id.clone();
    let response = match crate::kernel_control::run_kernel_call(client.create_access_grant(request))
    {
        Ok(response) => response,
        Err(error) => {
            if is_kernel_idempotency_conflict(error.as_str()) {
                let readback_grant = match crate::kernel_control::run_kernel_call(
                    client.get_access_grant(expected_grant_id.as_str()),
                ) {
                    Ok(grant) => grant,
                    Err(readback_error) => {
                        state.data_seller.last_error = Some(format!(
                            "{error}; existing grant read-back failed: {readback_error}"
                        ));
                        state.data_seller.status_line =
                            "Kernel authority reported a grant replay conflict and read-back failed."
                                .to_string();
                        return true;
                    }
                };
                state
                    .data_seller
                    .note_grant_published(readback_grant.clone(), None);
                state
                    .data_market
                    .note_published_grant(readback_grant, current_epoch_ms());
                state.data_buyer.sync_selection(&state.data_market);
                if let Some(grant) = state.data_seller.last_published_grant.clone() {
                    record_data_market_lifecycle_entry(
                        state,
                        grant.created_at_ms,
                        "grant_published",
                        grant.status.label(),
                        grant.grant_id.clone(),
                        grant.consumer_id.clone(),
                        Some(grant.permission_policy.policy_id.clone()),
                        None,
                        format!(
                            "Re-synced existing grant {} from kernel after idempotent replay.",
                            grant.grant_id
                        ),
                    );
                }
                sync_data_seller_nip90_profile(state);
                return true;
            }
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Kernel authority rejected the grant publication.".to_string();
            return true;
        }
    };
    let grant_id = response.grant.grant_id.clone();
    let receipt_id = Some(response.receipt.receipt_id.clone());
    let readback_grant =
        match crate::kernel_control::run_kernel_call(client.get_access_grant(grant_id.as_str())) {
            Ok(grant) => grant,
            Err(error) => {
                let fallback_grant = response.grant.clone();
                state
                    .data_seller
                    .note_grant_published(response.grant, receipt_id);
                if let Some(grant) = state.data_seller.last_published_grant.clone() {
                    state
                        .data_market
                        .note_published_grant(grant, current_epoch_ms());
                    state.data_buyer.sync_selection(&state.data_market);
                }
                record_data_market_lifecycle_entry(
                    state,
                    current_epoch_ms(),
                    "grant_published",
                    fallback_grant.status.label(),
                    grant_id,
                    fallback_grant.consumer_id.clone(),
                    Some(fallback_grant.permission_policy.policy_id.clone()),
                    state.data_seller.last_grant_publish_receipt_id.clone(),
                    format!(
                        "Published grant for asset {} with expiry {}.",
                        fallback_grant.asset_id, fallback_grant.expires_at_ms
                    ),
                );
                sync_data_seller_nip90_profile(state);
                state.data_seller.last_error = Some(format!(
                    "Grant was published but the immediate kernel read-back failed: {error}"
                ));
                state.data_seller.status_line =
                    "Grant published, but immediate kernel read-back failed.".to_string();
                return true;
            }
        };

    state
        .data_seller
        .note_grant_published(readback_grant.clone(), receipt_id);
    state
        .data_market
        .note_published_grant(readback_grant, current_epoch_ms());
    state.data_buyer.sync_selection(&state.data_market);
    if let Some(grant) = state.data_seller.last_published_grant.clone() {
        record_data_market_lifecycle_entry(
            state,
            grant.created_at_ms,
            "grant_published",
            grant.status.label(),
            grant.grant_id.clone(),
            grant.consumer_id.clone(),
            Some(grant.permission_policy.policy_id.clone()),
            state.data_seller.last_grant_publish_receipt_id.clone(),
            format!(
                "Published grant for asset {} with expiry {}.",
                grant.asset_id, grant.expires_at_ms
            ),
        );
    }
    sync_data_seller_nip90_profile(state);
    true
}

#[cfg(test)]
mod tests {
    use super::is_kernel_idempotency_conflict;

    #[test]
    fn detects_kernel_idempotency_conflicts() {
        assert!(is_kernel_idempotency_conflict(
            "kernel authority call failed: status=409 error=kernel_error reason=kernel_idempotency_conflict"
        ));
        assert!(!is_kernel_idempotency_conflict(
            "kernel authority call failed: status=500 error=kernel_error reason=storage_unavailable"
        ));
    }
}
