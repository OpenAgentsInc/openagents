use std::path::PathBuf;

use codex_client::{ThreadResumeParams, ThreadStartParams, TurnStartParams, UserInput};
use nostr::nip90::{JobFeedback, JobStatus, create_job_feedback_event};
use nostr::{Event, EventTemplate, NostrIdentity};
use openagents_spark::PaymentSummary;
use openagents_kernel_core::authority::{
    CreateAccessGrantRequest, KernelAuthority, RegisterDataAssetRequest,
};

use crate::app_state::{
    AutopilotRole, DataSellerCodexSessionPhase, DataSellerSkillAttachment, RenderState,
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

fn sign_event_template(identity: &NostrIdentity, template: &EventTemplate) -> Result<Event, String> {
    let private_key = parse_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| format!("Cannot sign NIP-90 event template: {error}"))
}

fn sync_data_seller_nip90_profile(state: &mut RenderState) {
    let profile = state
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
            }) || expected_payment_hash.as_deref().is_some_and(|expected_payment_hash| {
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
            state.data_seller.last_error = Some(format!(
                "Failed to auto-provision seller skills: {error}"
            ));
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

pub(crate) fn submit_data_seller_prompt(state: &mut RenderState) -> bool {
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

    let prompt = state.data_seller_inputs.composer.get_value().trim().to_string();
    if prompt.is_empty() {
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
            state
                .autopilot_chat
                .append_cached_thread_message(&thread_id, AutopilotRole::User, prompt);
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
            state.data_seller.last_error = Some(format!(
                "Failed to queue Data Seller turn: {error}"
            ));
        }
    }
    true
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
        state
            .data_seller
            .note_payment_invoice_failed(request_id, format!("Failed to queue Spark invoice creation: {error}"));
        return true;
    }

    state.provider_runtime.last_result = Some(format!(
        "queued seller Lightning invoice creation for data request {}",
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
        .ok_or_else(|| format!("Request {} does not have a non-zero quoted price", request_id))?;
    let bolt11 = request
        .pending_bolt11
        .as_deref()
        .ok_or_else(|| format!("Request {} is missing the pending Lightning invoice", request_id))?
        .to_string();
    let identity = state
        .nostr_identity
        .as_ref()
        .ok_or_else(|| "Cannot publish payment-required feedback: Nostr identity unavailable".to_string())?;
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
            if let Err(error) = queue_data_seller_payment_required_feedback(state, request_id.as_str()) {
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
        .filter(|request| request.payment_state == crate::app_state::DataSellerPaymentState::AwaitingPayment)
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
    if outcome.role != ProviderNip90PublishRole::Feedback {
        return false;
    }
    let Some(request) = state.data_seller.request_by_id(outcome.request_id.as_str()) else {
        return false;
    };
    if request.payment_state != crate::app_state::DataSellerPaymentState::PublishingFeedback {
        return false;
    }

    let published = outcome.accepted_relays > 0;
    let handled = state.data_seller.note_payment_feedback_publish_outcome(
        outcome.request_id.as_str(),
        published,
        published.then_some(outcome.event_id.as_str()),
        outcome.first_error.as_deref(),
    );
    if handled && published {
        state.provider_runtime.last_result = Some(format!(
            "seller requested Lightning payment for data request {}",
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

    let preview_payload = match state.data_seller.active_draft.last_previewed_asset_payload.clone() {
        Some(payload) => payload,
        None => {
            state.data_seller.last_error = Some(
                "Publish is armed but the exact preview payload is missing.".to_string(),
            );
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

    let response = match crate::kernel_control::run_kernel_call(client.register_data_asset(request))
    {
        Ok(response) => response,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Kernel authority rejected the asset publication.".to_string();
            return true;
        }
    };
    let asset_id = response.asset.asset_id.clone();
    let receipt_id = Some(response.receipt.receipt_id.clone());
    let readback_asset = match crate::kernel_control::run_kernel_call(client.get_data_asset(
        asset_id.as_str(),
    )) {
        Ok(asset) => asset,
        Err(error) => {
            state
                .data_seller
                .note_asset_published(response.asset, receipt_id);
            if let Some(asset) = state.data_seller.last_published_asset.clone() {
                state
                    .data_market
                    .note_published_asset(asset, current_epoch_ms());
            }
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
    sync_data_seller_nip90_profile(state);
    true
}

pub(crate) fn publish_data_seller_grant(state: &mut RenderState) -> bool {
    state.data_seller.request_publish_grant();
    if !state.data_seller.grant_publish_is_armed() {
        return true;
    }

    let preview_payload = match state.data_seller.active_draft.last_previewed_grant_payload.clone()
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
                "Grant publish blocked because the preview payload is no longer valid."
                    .to_string();
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

    let response = match crate::kernel_control::run_kernel_call(client.create_access_grant(request))
    {
        Ok(response) => response,
        Err(error) => {
            state.data_seller.last_error = Some(error);
            state.data_seller.status_line =
                "Kernel authority rejected the grant publication.".to_string();
            return true;
        }
    };
    let grant_id = response.grant.grant_id.clone();
    let receipt_id = Some(response.receipt.receipt_id.clone());
    let readback_grant = match crate::kernel_control::run_kernel_call(client.get_access_grant(
        grant_id.as_str(),
    )) {
        Ok(grant) => grant,
        Err(error) => {
            state
                .data_seller
                .note_grant_published(response.grant, receipt_id);
            if let Some(grant) = state.data_seller.last_published_grant.clone() {
                state
                    .data_market
                    .note_published_grant(grant, current_epoch_ms());
            }
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
    sync_data_seller_nip90_profile(state);
    true
}
