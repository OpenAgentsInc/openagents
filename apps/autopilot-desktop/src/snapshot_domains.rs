use std::fmt::Write as _;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use openagents_kernel_core::ids::sha256_prefixed_text;

use crate::app_state::{AutopilotChatState, AutopilotMessage, DesktopPane, RenderState};

const SIGNATURE_JOB_HISTORY_LIMIT: usize = 16;
const SIGNATURE_LOG_TAIL_LIMIT: usize = 8;
const SIGNATURE_TERMINAL_LINE_LIMIT: usize = 6;

struct SignatureBuilder {
    payload: String,
}

impl SignatureBuilder {
    fn new(namespace: &str) -> Self {
        Self {
            payload: namespace.to_string(),
        }
    }

    fn field(&mut self, key: &str, value: impl std::fmt::Display) {
        let _ = write!(self.payload, "\n{key}={value}");
    }

    fn bool(&mut self, key: &str, value: bool) {
        self.field(key, u8::from(value));
    }

    fn opt_str(&mut self, key: &str, value: Option<&str>) {
        self.field(key, value.unwrap_or(""));
    }

    fn opt_i64(&mut self, key: &str, value: Option<i64>) {
        match value {
            Some(value) => self.field(key, value),
            None => self.field(key, ""),
        }
    }

    fn opt_u64(&mut self, key: &str, value: Option<u64>) {
        match value {
            Some(value) => self.field(key, value),
            None => self.field(key, ""),
        }
    }

    fn hashed_text(&mut self, key: &str, value: &str) {
        self.field(key, sha256_prefixed_text(value));
    }

    fn finish(self) -> String {
        sha256_prefixed_text(self.payload.as_str())
    }
}

pub(crate) fn provider_admin_signature(state: &RenderState) -> String {
    let mut builder = SignatureBuilder::new("provider-admin");
    append_provider_runtime_signature(&mut builder, state);
    append_wallet_signature(&mut builder, state);
    append_provider_inventory_signature(&mut builder, state);
    append_provider_job_history_signature(&mut builder, state);
    builder.finish()
}

pub(crate) fn codex_remote_signature(state: &RenderState) -> String {
    let mut builder = SignatureBuilder::new("codex-remote");
    append_codex_session_signature(&mut builder, state);
    append_codex_thread_signature(&mut builder, &state.autopilot_chat);
    append_codex_message_signature(&mut builder, &state.autopilot_chat);
    append_codex_artifact_signature(&mut builder, &state.autopilot_chat);
    append_wallet_signature(&mut builder, state);
    append_provider_remote_summary_signature(&mut builder, state);
    builder.finish()
}

pub(crate) fn desktop_control_signature(state: &RenderState) -> String {
    let mut builder = SignatureBuilder::new("desktop-control");
    append_provider_runtime_signature(&mut builder, state);
    append_wallet_signature(&mut builder, state);
    append_provider_inventory_signature(&mut builder, state);
    append_desktop_control_shell_signature(&mut builder, state);
    append_desktop_control_buy_mode_signature(&mut builder, state);
    append_desktop_control_compute_history_signature(&mut builder, state);
    append_desktop_control_remote_training_signature(&mut builder, state);
    append_desktop_control_log_signature(&mut builder, state);
    builder.finish()
}

fn append_provider_runtime_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    let provider = &state.provider_runtime;
    builder.field("provider_mode", provider.mode.label());
    builder.opt_str("provider_last_result", provider.last_result.as_deref());
    builder.opt_str(
        "provider_inventory_last_action",
        provider.inventory_last_action.as_deref(),
    );
    builder.opt_str(
        "provider_last_error_detail",
        provider.last_error_detail.as_deref(),
    );
    builder.opt_str(
        "provider_degraded_reason_code",
        provider.degraded_reason_code.as_deref(),
    );
    builder.opt_str(
        "provider_authoritative_status",
        provider.last_authoritative_status.as_deref(),
    );
    builder.opt_str(
        "provider_authoritative_event_id",
        provider.last_authoritative_event_id.as_deref(),
    );
    builder.field(
        "provider_authoritative_error_class",
        format!("{:?}", provider.last_authoritative_error_class),
    );
    builder.field("provider_queue_depth", provider.queue_depth);
    builder.opt_i64(
        "provider_inventory_session_started_at_ms",
        provider.inventory_session_started_at_ms,
    );
    builder.opt_i64(
        "provider_last_completed_job_at_ms",
        provider
            .last_completed_job_at
            .and_then(approx_epoch_ms_for_instant),
    );
    builder.opt_i64(
        "provider_online_since_ms",
        provider.online_since.and_then(approx_epoch_ms_for_instant),
    );
    builder.field("provider_execution_lane", provider.execution_lane_label());
    builder.field(
        "provider_execution_backend",
        provider.execution_backend_label(),
    );
    builder.field(
        "provider_control_authority",
        provider.control_authority_label(false),
    );
    builder.field(
        "provider_settlement_truth",
        provider.settlement_truth_label(),
    );
    for relay_url in state.configured_provider_relay_urls() {
        builder.field("provider_relay_url", relay_url);
    }
    builder.opt_str(
        "hosted_control_base_url",
        state.hosted_control_base_url.as_deref(),
    );
    builder.opt_str(
        "provider_admin_listen_addr",
        state.provider_admin_listen_addr.as_deref(),
    );
    builder.opt_str(
        "provider_admin_last_error",
        state.provider_admin_last_error.as_deref(),
    );
    if let Some(identity) = state.nostr_identity.as_ref() {
        builder.opt_str("nostr_npub", Some(identity.npub.as_str()));
        builder.opt_str("nostr_pubkey", Some(identity.public_key_hex.as_str()));
    } else {
        builder.opt_str("nostr_npub", None);
        builder.opt_str("nostr_pubkey", None);
    }
    for blocker in state.provider_blockers() {
        builder.field("provider_blocker", blocker.code());
    }
    builder.bool("gpt_oss_reachable", provider.gpt_oss.reachable);
    builder.opt_str(
        "gpt_oss_configured_model",
        provider.gpt_oss.configured_model.as_deref(),
    );
    builder.opt_str(
        "gpt_oss_ready_model",
        provider.gpt_oss.ready_model.as_deref(),
    );
    builder.opt_str("gpt_oss_last_error", provider.gpt_oss.last_error.as_deref());
    builder.opt_str(
        "gpt_oss_last_action",
        provider.gpt_oss.last_action.as_deref(),
    );
    builder.opt_str(
        "gpt_oss_last_request_id",
        provider.gpt_oss.last_request_id.as_deref(),
    );
    for model in &provider.gpt_oss.available_models {
        builder.field("gpt_oss_available_model", model);
    }
    for model in &provider.gpt_oss.loaded_models {
        builder.field("gpt_oss_loaded_model", model);
    }
    builder.bool("apple_fm_reachable", provider.apple_fm.reachable);
    builder.bool(
        "apple_fm_model_available",
        provider.apple_fm.model_available,
    );
    builder.field(
        "apple_fm_model_id",
        provider.apple_fm.system_model.id.as_str(),
    );
    builder.field(
        "apple_fm_use_case",
        format!("{:?}", provider.apple_fm.system_model.use_case),
    );
    builder.field(
        "apple_fm_guardrails",
        format!("{:?}", provider.apple_fm.system_model.guardrails),
    );
    builder.opt_str(
        "apple_fm_ready_model",
        provider.apple_fm.ready_model.as_deref(),
    );
    builder.opt_str(
        "apple_fm_last_error",
        provider.apple_fm.last_error.as_deref(),
    );
    builder.opt_str(
        "apple_fm_last_action",
        provider.apple_fm.last_action.as_deref(),
    );
    builder.opt_str(
        "apple_fm_last_request_id",
        provider.apple_fm.last_request_id.as_deref(),
    );
    builder.opt_str(
        "apple_fm_bridge_status",
        provider.apple_fm.bridge_status.as_deref(),
    );
    builder.opt_str(
        "apple_fm_availability_message",
        provider.apple_fm.availability_message.as_deref(),
    );
    builder.field(
        "apple_fm_unavailable_reason",
        format!("{:?}", provider.apple_fm.unavailable_reason),
    );
    for model in &provider.apple_fm.available_models {
        builder.field("apple_fm_available_model", model);
    }
    for use_case in &provider.apple_fm.supported_use_cases {
        builder.field("apple_fm_supported_use_case", format!("{use_case:?}"));
    }
    for guardrail in &provider.apple_fm.supported_guardrails {
        builder.field("apple_fm_supported_guardrail", format!("{guardrail:?}"));
    }
    for runtime in &provider.sandbox.runtimes {
        builder.field("sandbox_runtime_kind", runtime.runtime_kind.label());
        builder.bool("sandbox_runtime_detected", runtime.detected);
        builder.bool("sandbox_runtime_ready", runtime.ready);
        builder.opt_str("sandbox_binary_name", runtime.binary_name.as_deref());
        builder.opt_str("sandbox_binary_path", runtime.binary_path.as_deref());
        builder.opt_str(
            "sandbox_runtime_version",
            runtime.runtime_version.as_deref(),
        );
        builder.opt_str("sandbox_runtime_error", runtime.last_error.as_deref());
        for class in &runtime.supported_execution_classes {
            builder.field("sandbox_runtime_class", class.product_id());
        }
    }
    for profile in &provider.sandbox.profiles {
        builder.field("sandbox_profile_id", profile.profile_id.as_str());
        builder.field("sandbox_profile_digest", profile.profile_digest.as_str());
        builder.field(
            "sandbox_profile_class",
            profile.execution_class.product_id(),
        );
        builder.field(
            "sandbox_profile_runtime_family",
            profile.runtime_family.as_str(),
        );
        builder.field(
            "sandbox_profile_runtime_version",
            profile.runtime_version.as_str(),
        );
        builder.field("sandbox_profile_engine", profile.sandbox_engine.as_str());
        builder.field("sandbox_profile_os", profile.os_family.as_str());
        builder.field("sandbox_profile_arch", profile.arch.as_str());
        builder.field("sandbox_profile_cpu_limit", profile.cpu_limit);
        builder.field("sandbox_profile_memory_limit_mb", profile.memory_limit_mb);
        builder.field("sandbox_profile_disk_limit_mb", profile.disk_limit_mb);
        builder.field("sandbox_profile_timeout_limit_s", profile.timeout_limit_s);
        builder.field(
            "sandbox_profile_network_mode",
            profile.network_mode.as_str(),
        );
        builder.field(
            "sandbox_profile_filesystem_mode",
            profile.filesystem_mode.as_str(),
        );
        builder.field(
            "sandbox_profile_workspace_mode",
            profile.workspace_mode.as_str(),
        );
        builder.field(
            "sandbox_profile_artifact_output_mode",
            profile.artifact_output_mode.as_str(),
        );
        builder.field(
            "sandbox_profile_secrets_mode",
            profile.secrets_mode.as_str(),
        );
        builder.opt_str(
            "sandbox_profile_container_image",
            profile.container_image.as_deref(),
        );
        builder.opt_str(
            "sandbox_profile_runtime_image_digest",
            profile.runtime_image_digest.as_deref(),
        );
        builder.opt_str(
            "sandbox_profile_accelerator_policy",
            profile.accelerator_policy.as_deref(),
        );
        builder.field("sandbox_profile_runtime_kind", profile.runtime_kind.label());
        builder.bool("sandbox_profile_runtime_ready", profile.runtime_ready);
        builder.opt_str(
            "sandbox_profile_runtime_binary_path",
            profile.runtime_binary_path.as_deref(),
        );
        builder.field(
            "sandbox_profile_capability_summary",
            profile.capability_summary.as_str(),
        );
        for allowed in &profile.allowed_binaries {
            builder.field("sandbox_profile_allowed_binary", allowed);
        }
        for toolchain in &profile.toolchain_inventory {
            builder.field("sandbox_profile_toolchain", toolchain);
        }
    }
    builder.opt_str(
        "sandbox_last_scan_error",
        provider.sandbox.last_scan_error.as_deref(),
    );
    builder.opt_str(
        "presence_last_published_mode",
        provider.autopilot_presence.last_published_mode.as_deref(),
    );
    builder.opt_str(
        "presence_last_published_event_id",
        provider
            .autopilot_presence
            .last_published_event_id
            .as_deref(),
    );
    builder.opt_u64(
        "presence_last_published_at_epoch_seconds",
        provider.autopilot_presence.last_published_at_epoch_seconds,
    );
    builder.opt_u64(
        "presence_last_expires_at_epoch_seconds",
        provider.autopilot_presence.last_expires_at_epoch_seconds,
    );
    builder.opt_str(
        "presence_pending_mode",
        provider.autopilot_presence.pending_mode.as_deref(),
    );
    builder.opt_str(
        "presence_pending_event_id",
        provider.autopilot_presence.pending_event_id.as_deref(),
    );
    builder.opt_str(
        "presence_last_action",
        provider.autopilot_presence.last_action.as_deref(),
    );
    builder.opt_str(
        "presence_last_error",
        provider.autopilot_presence.last_error.as_deref(),
    );
}

fn append_wallet_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    builder.opt_u64(
        "wallet_total_balance_sats",
        state.spark_wallet.total_balance_sats(),
    );
    builder.bool(
        "wallet_balance_reconciling",
        state.spark_wallet.balance_reconciling(),
    );
    builder.field("wallet_network", state.spark_wallet.network_name());
    builder.field(
        "wallet_network_status",
        state.spark_wallet.network_status_label(),
    );
    builder.opt_str(
        "wallet_last_action",
        state.spark_wallet.last_action.as_deref(),
    );
    builder.opt_str(
        "wallet_last_error",
        state.spark_wallet.last_error.as_deref(),
    );
}

fn append_provider_inventory_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    let controls = &state.provider_runtime.inventory_controls;
    builder.bool(
        "inventory_gpt_oss_inference_enabled",
        controls.gpt_oss_inference_enabled,
    );
    builder.bool(
        "inventory_gpt_oss_embeddings_enabled",
        controls.gpt_oss_embeddings_enabled,
    );
    builder.bool(
        "inventory_apple_fm_inference_enabled",
        controls.apple_fm_inference_enabled,
    );
    builder.bool(
        "inventory_sandbox_container_exec_enabled",
        controls.sandbox_container_exec_enabled,
    );
    builder.bool(
        "inventory_sandbox_python_exec_enabled",
        controls.sandbox_python_exec_enabled,
    );
    builder.bool(
        "inventory_sandbox_node_exec_enabled",
        controls.sandbox_node_exec_enabled,
    );
    builder.bool(
        "inventory_sandbox_posix_exec_enabled",
        controls.sandbox_posix_exec_enabled,
    );
    for row in &state.provider_runtime.inventory_rows {
        builder.field("inventory_target", row.target.product_id());
        builder.bool("inventory_enabled", row.enabled);
        builder.bool("inventory_backend_ready", row.backend_ready);
        builder.bool("inventory_eligible", row.eligible);
        builder.field(
            "inventory_capability_summary",
            row.capability_summary.as_str(),
        );
        builder.field("inventory_source_badge", row.source_badge.as_str());
        builder.opt_str("inventory_capacity_lot_id", row.capacity_lot_id.as_deref());
        builder.field("inventory_total_quantity", row.total_quantity);
        builder.field("inventory_reserved_quantity", row.reserved_quantity);
        builder.field("inventory_available_quantity", row.available_quantity);
        builder.field("inventory_delivery_state", row.delivery_state.as_str());
        builder.field("inventory_price_floor_sats", row.price_floor_sats);
        builder.field("inventory_terms_label", row.terms_label.as_str());
        builder.opt_str(
            "inventory_forward_capacity_lot_id",
            row.forward_capacity_lot_id.as_deref(),
        );
        builder.opt_str(
            "inventory_forward_delivery_window_label",
            row.forward_delivery_window_label.as_deref(),
        );
        builder.field(
            "inventory_forward_total_quantity",
            row.forward_total_quantity,
        );
        builder.field(
            "inventory_forward_reserved_quantity",
            row.forward_reserved_quantity,
        );
        builder.field(
            "inventory_forward_available_quantity",
            row.forward_available_quantity,
        );
        builder.opt_str(
            "inventory_forward_terms_label",
            row.forward_terms_label.as_deref(),
        );
    }
}

fn append_provider_job_history_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    builder.field("job_history_len", state.job_history.rows.len());
    for row in state
        .job_history
        .rows
        .iter()
        .take(SIGNATURE_JOB_HISTORY_LIMIT)
    {
        builder.field("job_id", row.job_id.as_str());
        builder.field("job_status", format!("{:?}", row.status));
        builder.field("job_demand_source", format!("{:?}", row.demand_source));
        builder.field(
            "job_completed_at_epoch_seconds",
            row.completed_at_epoch_seconds,
        );
        builder.opt_str(
            "job_requester_nostr_pubkey",
            row.requester_nostr_pubkey.as_deref(),
        );
        builder.opt_str(
            "job_provider_nostr_pubkey",
            row.provider_nostr_pubkey.as_deref(),
        );
        builder.opt_str("job_skill_scope_id", row.skill_scope_id.as_deref());
        builder.opt_str("job_delivery_proof_id", row.delivery_proof_id.as_deref());
        builder.opt_str(
            "job_delivery_metering_rule_id",
            row.delivery_metering_rule_id.as_deref(),
        );
        builder.opt_str(
            "job_delivery_proof_status_label",
            row.delivery_proof_status_label.as_deref(),
        );
        builder.opt_u64(
            "job_delivery_metered_quantity",
            row.delivery_metered_quantity,
        );
        builder.opt_u64(
            "job_delivery_accepted_quantity",
            row.delivery_accepted_quantity,
        );
        builder.opt_str(
            "job_delivery_variance_reason_label",
            row.delivery_variance_reason_label.as_deref(),
        );
        builder.opt_str(
            "job_delivery_rejection_reason_label",
            row.delivery_rejection_reason_label.as_deref(),
        );
        builder.field("job_payout_sats", row.payout_sats);
        builder.field("job_payment_pointer", row.payment_pointer.as_str());
        builder.field("job_result_hash", row.result_hash.as_str());
        builder.opt_str("job_failure_reason", row.failure_reason.as_deref());
        if let Some(provenance) = row.execution_provenance.as_ref() {
            builder.field("job_exec_backend", provenance.backend.as_str());
            builder.opt_str(
                "job_exec_requested_model",
                provenance.requested_model.as_deref(),
            );
            builder.field("job_exec_served_model", provenance.served_model.as_str());
            builder.field(
                "job_exec_prompt_digest",
                provenance.normalized_prompt_digest.as_str(),
            );
            builder.field(
                "job_exec_options_digest",
                provenance.normalized_options_digest.as_str(),
            );
            builder.field("job_exec_base_url", provenance.base_url.as_str());
        }
    }
}

fn append_codex_session_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    builder.field(
        "codex_connection_status",
        state.autopilot_chat.connection_status.as_str(),
    );
    builder.field(
        "codex_readiness_summary",
        state.codex_account.readiness_summary.as_str(),
    );
    builder.field(
        "codex_account_summary",
        state.codex_account.account_summary.as_str(),
    );
    builder.opt_str(
        "codex_constraint_summary",
        state.codex_account.config_constraint_summary.as_deref(),
    );
    builder.field("codex_current_model", state.autopilot_chat.current_model());
    builder.opt_str(
        "codex_reasoning_effort",
        state.autopilot_chat.reasoning_effort.as_deref(),
    );
    builder.field(
        "codex_service_tier",
        state.autopilot_chat.service_tier.label(),
    );
    builder.field(
        "codex_approval_mode",
        format!("{:?}", state.autopilot_chat.approval_mode),
    );
    builder.field(
        "codex_sandbox_mode",
        format!("{:?}", state.autopilot_chat.sandbox_mode),
    );
    builder.field(
        "codex_personality",
        state.autopilot_chat.personality.label(),
    );
    builder.field(
        "codex_collaboration_mode",
        state.autopilot_chat.collaboration_mode.label(),
    );
    builder.opt_str(
        "codex_last_turn_status",
        state.autopilot_chat.last_turn_status.as_deref(),
    );
    builder.field(
        "codex_pending_auth_refresh_len",
        state.autopilot_chat.pending_auth_refresh.len(),
    );
    if let Some(usage) = state.autopilot_chat.token_usage.as_ref() {
        builder.field("codex_input_tokens", usage.input_tokens);
        builder.field("codex_cached_input_tokens", usage.cached_input_tokens);
        builder.field("codex_output_tokens", usage.output_tokens);
    } else {
        builder.field("codex_input_tokens", 0_u64);
        builder.field("codex_cached_input_tokens", 0_u64);
        builder.field("codex_output_tokens", 0_u64);
    }
    builder.bool(
        "codex_models_include_hidden",
        state.codex_models.include_hidden,
    );
    for entry in &state.codex_models.entries {
        builder.field("codex_model_entry", entry.model.as_str());
        builder.field("codex_model_display_name", entry.display_name.as_str());
        builder.field("codex_model_description", entry.description.as_str());
        builder.bool("codex_model_hidden", entry.hidden);
        builder.bool("codex_model_is_default", entry.is_default);
        builder.field(
            "codex_model_default_reasoning_effort",
            entry.default_reasoning_effort.as_str(),
        );
        for effort in &entry.supported_reasoning_efforts {
            builder.field("codex_model_reasoning_effort", effort);
        }
    }
}

fn append_codex_thread_signature(builder: &mut SignatureBuilder, chat: &AutopilotChatState) {
    builder.opt_str("codex_active_thread_id", chat.active_thread_id.as_deref());
    builder.field("codex_thread_count", chat.threads.len());
    for thread_id in &chat.threads {
        builder.field("codex_thread_id", thread_id.as_str());
        if let Some(metadata) = chat.thread_metadata.get(thread_id) {
            builder.opt_str("codex_thread_name", metadata.thread_name.as_deref());
            match metadata.preview.as_deref() {
                Some(preview) => builder.hashed_text("codex_thread_preview", preview),
                None => builder.opt_str("codex_thread_preview", None),
            }
            builder.opt_str("codex_thread_status", metadata.status.as_deref());
            builder.bool("codex_thread_loaded", metadata.loaded);
            builder.opt_i64("codex_thread_created_at", metadata.created_at);
            builder.opt_i64("codex_thread_updated_at", metadata.updated_at);
            builder.opt_str(
                "codex_thread_workspace_root",
                metadata.workspace_root.as_deref(),
            );
            builder.opt_str("codex_thread_cwd", metadata.cwd.as_deref());
            builder.opt_str("codex_thread_path", metadata.path.as_deref());
            builder.opt_str("codex_thread_project_id", metadata.project_id.as_deref());
            builder.opt_str(
                "codex_thread_project_name",
                metadata.project_name.as_deref(),
            );
            builder.opt_str("codex_thread_git_branch", metadata.git_branch.as_deref());
            builder.field(
                "codex_thread_git_dirty",
                metadata.git_dirty.unwrap_or(false),
            );
        }
    }
}

fn append_codex_message_signature(builder: &mut SignatureBuilder, chat: &AutopilotChatState) {
    builder.field("codex_message_count", chat.messages.len());
    for message in &chat.messages {
        append_codex_message(builder, message);
    }
    builder.field(
        "codex_pending_command_approvals",
        chat.pending_command_approvals.len(),
    );
    for request in &chat.pending_command_approvals {
        builder.field(
            "codex_command_request_id",
            app_server_request_id_value(&request.request_id),
        );
        builder.field("codex_command_thread_id", request.thread_id.as_str());
        builder.field("codex_command_turn_id", request.turn_id.as_str());
        builder.field("codex_command_item_id", request.item_id.as_str());
        builder.opt_str("codex_command_reason", request.reason.as_deref());
        builder.opt_str("codex_command", request.command.as_deref());
        builder.opt_str("codex_command_cwd", request.cwd.as_deref());
    }
    builder.field(
        "codex_pending_file_change_approvals",
        chat.pending_file_change_approvals.len(),
    );
    for request in &chat.pending_file_change_approvals {
        builder.field(
            "codex_file_request_id",
            app_server_request_id_value(&request.request_id),
        );
        builder.field("codex_file_thread_id", request.thread_id.as_str());
        builder.field("codex_file_turn_id", request.turn_id.as_str());
        builder.field("codex_file_item_id", request.item_id.as_str());
        builder.opt_str("codex_file_reason", request.reason.as_deref());
        builder.opt_str("codex_file_grant_root", request.grant_root.as_deref());
    }
    builder.field(
        "codex_pending_tool_user_input",
        chat.pending_tool_user_input.len(),
    );
    for request in &chat.pending_tool_user_input {
        builder.field(
            "codex_tool_request_id",
            app_server_request_id_value(&request.request_id),
        );
        builder.field("codex_tool_thread_id", request.thread_id.as_str());
        builder.field("codex_tool_turn_id", request.turn_id.as_str());
        builder.field("codex_tool_item_id", request.item_id.as_str());
        for question in &request.questions {
            builder.field("codex_tool_question_id", question.id.as_str());
            builder.field("codex_tool_question_header", question.header.as_str());
            builder.field("codex_tool_question", question.question.as_str());
            for option in &question.options {
                builder.field("codex_tool_option", option.as_str());
            }
        }
    }
    if let Some(session) = chat.active_terminal_session() {
        builder.field("codex_terminal_thread_id", session.thread_id.as_str());
        builder.field(
            "codex_terminal_workspace_root",
            session.workspace_root.as_str(),
        );
        builder.field("codex_terminal_shell", session.shell.as_str());
        builder.opt_u64("codex_terminal_pid", session.pid.map(u64::from));
        builder.field("codex_terminal_cols", session.cols);
        builder.field("codex_terminal_rows", session.rows);
        builder.field("codex_terminal_status", format!("{:?}", session.status));
        builder.opt_i64("codex_terminal_exit_code", session.exit_code.map(i64::from));
        builder.field(
            "codex_terminal_created_at_epoch_ms",
            session.created_at_epoch_ms,
        );
        builder.field(
            "codex_terminal_updated_at_epoch_ms",
            session.updated_at_epoch_ms,
        );
        builder.opt_str("codex_terminal_last_error", session.last_error.as_deref());
        builder.field("codex_terminal_line_count", session.lines.len());
        for line in session
            .lines
            .iter()
            .rev()
            .take(SIGNATURE_TERMINAL_LINE_LIMIT)
        {
            builder.field("codex_terminal_stream", format!("{:?}", line.stream));
            builder.hashed_text("codex_terminal_text", line.text.as_str());
            builder.opt_str("codex_terminal_key", line.key.as_deref());
        }
    }
}

fn append_codex_message(builder: &mut SignatureBuilder, message: &AutopilotMessage) {
    builder.field("codex_message_id", message.id);
    builder.field("codex_message_role", format!("{:?}", message.role));
    builder.field("codex_message_status", format!("{:?}", message.status));
    if message.content.trim().is_empty() {
        if let Some(structured) = message.structured.as_ref() {
            builder.hashed_text("codex_message_reasoning", structured.reasoning.as_str());
            builder.hashed_text("codex_message_answer", structured.answer.as_str());
            builder.opt_str(
                "codex_message_structured_status",
                structured.status.as_deref(),
            );
            builder.field(
                "codex_message_structured_event_count",
                structured.events.len(),
            );
            builder.field(
                "codex_message_structured_progress_block_count",
                structured.progress_blocks.len(),
            );
        } else {
            builder.hashed_text("codex_message_content", "");
        }
    } else {
        builder.hashed_text("codex_message_content", message.content.as_str());
    }
}

fn append_codex_artifact_signature(builder: &mut SignatureBuilder, chat: &AutopilotChatState) {
    if let Some(artifact) = chat.active_plan_artifact() {
        builder.opt_str("codex_plan_explanation", artifact.explanation.as_deref());
        builder.field(
            "codex_plan_updated_at_epoch_ms",
            artifact.updated_at_epoch_ms,
        );
        builder.field("codex_plan_step_count", artifact.steps.len());
        for step in &artifact.steps {
            builder.field("codex_plan_step", step.step.as_str());
            builder.field("codex_plan_step_status", step.status.as_str());
        }
    }
    if let Some(artifact) = chat.active_diff_artifact() {
        builder.field(
            "codex_diff_updated_at_epoch_ms",
            artifact.updated_at_epoch_ms,
        );
        builder.field("codex_diff_added_line_count", artifact.added_line_count);
        builder.field("codex_diff_removed_line_count", artifact.removed_line_count);
        builder.field("codex_diff_file_count", artifact.files.len());
        builder.hashed_text("codex_raw_diff", artifact.raw_diff.as_str());
        for file in &artifact.files {
            builder.field("codex_diff_file_path", file.path.as_str());
            builder.field("codex_diff_file_added_line_count", file.added_line_count);
            builder.field(
                "codex_diff_file_removed_line_count",
                file.removed_line_count,
            );
        }
    }
    if let Some(artifact) = chat.active_review_artifact() {
        builder.field(
            "codex_review_updated_at_epoch_ms",
            artifact.updated_at_epoch_ms,
        );
        builder.field("codex_review_status", artifact.status.as_str());
        builder.field("codex_review_delivery", artifact.delivery.as_str());
        builder.field("codex_review_target", artifact.target.as_str());
        builder.opt_str("codex_review_summary", artifact.summary.as_deref());
        builder.bool(
            "codex_review_restored_from_thread_read",
            artifact.restored_from_thread_read,
        );
    }
    if let Some(artifact) = chat.active_compaction_artifact() {
        builder.field(
            "codex_compaction_updated_at_epoch_ms",
            artifact.updated_at_epoch_ms,
        );
        builder.field(
            "codex_compaction_source_turn_id",
            artifact.source_turn_id.as_str(),
        );
        builder.bool(
            "codex_compaction_restored_from_thread_read",
            artifact.restored_from_thread_read,
        );
    }
}

fn append_provider_remote_summary_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    builder.field("remote_provider_mode", state.provider_runtime.mode.label());
    builder.opt_str(
        "remote_provider_last_action",
        state
            .provider_runtime
            .last_result
            .as_deref()
            .or(state.provider_runtime.inventory_last_action.as_deref()),
    );
    builder.opt_str(
        "remote_provider_last_error",
        state.provider_runtime.last_error_detail.as_deref(),
    );
}

fn append_desktop_control_shell_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    builder.bool(
        "desktop_buy_mode_surface_enabled",
        state.mission_control_buy_mode_enabled(),
    );
    builder.opt_str(
        "desktop_mission_control_last_action",
        state.mission_control.last_action.as_deref(),
    );
    builder.opt_str(
        "desktop_mission_control_last_error",
        state.mission_control.last_error.as_deref(),
    );
    builder.bool(
        "desktop_mission_control_can_go_online",
        state.mission_control_go_online_enabled(),
    );
    builder.field("desktop_pane_count", state.panes.len());
    for pane in &state.panes {
        append_pane_signature(builder, pane);
    }
    builder.field(
        "desktop_managed_chat_projection_revision",
        state
            .autopilot_chat
            .managed_chat_projection
            .projection_revision(),
    );
    builder.field(
        "desktop_chat_browse_mode",
        format!("{:?}", state.autopilot_chat.chat_browse_mode()),
    );
    builder.opt_str(
        "desktop_active_group_id",
        state
            .autopilot_chat
            .active_managed_chat_group()
            .map(|group| group.group_id.as_str()),
    );
    builder.opt_str(
        "desktop_active_channel_id",
        state
            .autopilot_chat
            .active_managed_chat_channel()
            .map(|channel| channel.channel_id.as_str()),
    );
    builder.field(
        "desktop_managed_group_count",
        state
            .autopilot_chat
            .managed_chat_projection
            .snapshot
            .groups
            .len(),
    );
    builder.field(
        "desktop_managed_channel_count",
        state
            .autopilot_chat
            .managed_chat_projection
            .snapshot
            .channels
            .len(),
    );
    builder.field(
        "desktop_managed_message_count",
        state
            .autopilot_chat
            .managed_chat_projection
            .snapshot
            .messages
            .len(),
    );
}

fn append_desktop_control_buy_mode_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    let now = Instant::now();
    builder.bool(
        "desktop_buy_mode_loop_enabled",
        state.buy_mode_payments.buy_mode_loop_enabled,
    );
    builder.opt_u64(
        "desktop_buy_mode_next_dispatch_countdown_millis",
        state
            .buy_mode_payments
            .buy_mode_next_dispatch_countdown_millis(now),
    );
    builder.opt_u64(
        "desktop_buy_mode_next_dispatch_countdown_seconds",
        state
            .buy_mode_payments
            .buy_mode_next_dispatch_countdown_seconds(now),
    );
    builder.field(
        "desktop_network_quote_mode",
        format!("{:?}", state.network_requests.quote_mode),
    );
    builder.opt_str(
        "desktop_selected_spot_quote_id",
        state.network_requests.selected_spot_quote_id.as_deref(),
    );
    builder.opt_str(
        "desktop_selected_forward_quote_id",
        state.network_requests.selected_forward_quote_id.as_deref(),
    );
    builder.field(
        "desktop_spot_quote_candidate_count",
        state.network_requests.spot_quote_candidates.len(),
    );
    builder.field(
        "desktop_forward_quote_candidate_count",
        state.network_requests.forward_quote_candidates.len(),
    );
    builder.field(
        "desktop_accepted_spot_order_count",
        state.network_requests.accepted_spot_orders.len(),
    );
    builder.field(
        "desktop_accepted_forward_order_count",
        state.network_requests.accepted_forward_orders.len(),
    );
    builder.field(
        "desktop_submitted_request_count",
        state.network_requests.submitted.len(),
    );
    builder.opt_str(
        "desktop_pending_auto_payment_request_id",
        state
            .network_requests
            .pending_auto_payment_request_id
            .as_deref(),
    );
    for request in &state.network_requests.submitted {
        builder.field("desktop_request_id", request.request_id.as_str());
        builder.field("desktop_request_type", request.request_type.as_str());
        builder.field("desktop_request_status", format!("{:?}", request.status));
        builder.field(
            "desktop_request_resolution_mode",
            format!("{:?}", request.resolution_mode),
        );
        builder.opt_str(
            "desktop_request_selected_provider_pubkey",
            request.last_provider_pubkey.as_deref(),
        );
        builder.opt_str(
            "desktop_request_result_provider_pubkey",
            request.result_provider_pubkey.as_deref(),
        );
        builder.opt_str(
            "desktop_request_invoice_provider_pubkey",
            request.invoice_provider_pubkey.as_deref(),
        );
        builder.opt_str(
            "desktop_request_payable_provider_pubkey",
            request.winning_provider_pubkey.as_deref(),
        );
        builder.opt_str(
            "desktop_request_payment_blocker_summary",
            request.payment_notice.as_deref(),
        );
    }
    builder.field("desktop_presence_epoch_seconds", current_epoch_seconds());
    builder.opt_str(
        "desktop_active_job_id",
        state.active_job.job.as_ref().map(|job| job.job_id.as_str()),
    );
    builder.opt_str(
        "desktop_active_request_id",
        state
            .active_job
            .job
            .as_ref()
            .map(|job| job.request_id.as_str()),
    );
    builder.opt_str(
        "desktop_active_capability",
        state
            .active_job
            .job
            .as_ref()
            .map(|job| job.capability.as_str()),
    );
    builder.field(
        "desktop_active_job_stage",
        state
            .active_job
            .job
            .as_ref()
            .map(|job| format!("{:?}", &job.stage))
            .unwrap_or_default(),
    );
    builder.bool(
        "desktop_result_publish_in_flight",
        state.active_job.result_publish_in_flight,
    );
    builder.opt_str(
        "desktop_pending_result_publish_event_id",
        state.active_job.pending_result_publish_event_id.as_deref(),
    );
    builder.field(
        "desktop_result_publish_attempt_count",
        state.active_job.result_publish_attempt_count,
    );
    builder.bool(
        "desktop_payment_required_invoice_requested",
        state.active_job.payment_required_invoice_requested,
    );
    builder.bool(
        "desktop_payment_required_feedback_in_flight",
        state.active_job.payment_required_feedback_in_flight,
    );
    builder.bool(
        "desktop_payment_required_failed",
        state.active_job.payment_required_failed,
    );
    builder.opt_str(
        "desktop_pending_bolt11",
        state.active_job.pending_bolt11.as_deref(),
    );
}

fn append_desktop_control_compute_history_signature(
    builder: &mut SignatureBuilder,
    state: &RenderState,
) {
    let history = &state.desktop_control.compute_history;
    builder.opt_str(
        "desktop_compute_history_provider_id",
        history.provider_id.as_deref(),
    );
    builder.field(
        "desktop_compute_delivery_proof_count",
        history.delivery_proofs.len(),
    );
    builder.field(
        "desktop_compute_capacity_instrument_count",
        history.capacity_instruments.len(),
    );
    builder.field(
        "desktop_compute_structured_capacity_instrument_count",
        history.structured_capacity_instruments.len(),
    );
    builder.field(
        "desktop_compute_validator_challenge_count",
        history.validator_challenges.len(),
    );
    builder.opt_u64(
        "desktop_compute_last_refreshed_at_epoch_ms",
        history.last_refreshed_at_epoch_ms,
    );
    builder.opt_str("desktop_compute_last_error", history.last_error.as_deref());
    builder.opt_str(
        "desktop_compute_last_action",
        history.last_action.as_deref(),
    );
    for proof in &history.delivery_proofs {
        builder.field(
            "desktop_delivery_proof_id",
            proof.delivery_proof_id.as_str(),
        );
        builder.field(
            "desktop_delivery_proof_status",
            format!("{:?}", proof.status),
        );
        builder.field(
            "desktop_delivery_proof_bundle_digest",
            proof.attestation_digest.as_deref().unwrap_or(""),
        );
    }
    for instrument in &history.capacity_instruments {
        builder.field(
            "desktop_capacity_instrument_id",
            instrument.instrument_id.as_str(),
        );
        builder.field(
            "desktop_capacity_instrument_kind",
            format!("{:?}", instrument.kind),
        );
        builder.field(
            "desktop_capacity_instrument_delivery_product",
            instrument.product_id.as_str(),
        );
    }
    for instrument in &history.structured_capacity_instruments {
        builder.field(
            "desktop_structured_capacity_instrument_id",
            instrument.structured_instrument_id.as_str(),
        );
        builder.field(
            "desktop_structured_capacity_instrument_kind",
            format!("{:?}", instrument.kind),
        );
        builder.field(
            "desktop_structured_capacity_instrument_status",
            format!("{:?}", instrument.status),
        );
    }
    for challenge in &history.validator_challenges {
        builder.field(
            "desktop_validator_challenge_id",
            challenge.request.context.challenge_id.as_str(),
        );
        builder.field(
            "desktop_validator_challenge_status",
            format!("{:?}", challenge.status),
        );
        builder.opt_str(
            "desktop_validator_challenge_reason_code",
            challenge
                .final_result
                .as_ref()
                .and_then(|result| result.reason_code.as_ref())
                .map(|reason| reason.label()),
        );
    }
}

fn append_desktop_control_remote_training_signature(
    builder: &mut SignatureBuilder,
    state: &RenderState,
) {
    let remote_training = &state.desktop_control.remote_training;
    builder.opt_str(
        "desktop_remote_training_source_root_hint",
        remote_training
            .source_root_hint
            .as_ref()
            .and_then(|path| path.to_str()),
    );
    builder.opt_str(
        "desktop_remote_training_source_root",
        remote_training
            .source_root
            .as_ref()
            .and_then(|path| path.to_str()),
    );
    builder.opt_str(
        "desktop_remote_training_source_index_path",
        remote_training
            .source_index_path
            .as_ref()
            .and_then(|path| path.to_str()),
    );
    builder.opt_u64(
        "desktop_remote_training_last_refreshed_at_epoch_ms",
        remote_training.last_refreshed_at_epoch_ms,
    );
    builder.opt_u64(
        "desktop_remote_training_last_successful_sync_at_epoch_ms",
        remote_training.last_successful_sync_at_epoch_ms,
    );
    builder.field(
        "desktop_remote_training_refresh_interval_ms",
        remote_training.refresh_interval_ms,
    );
    builder.bool(
        "desktop_remote_training_using_cached_mirror",
        remote_training.using_cached_mirror,
    );
    builder.opt_str(
        "desktop_remote_training_selected_run_id",
        remote_training.selected_run_id.as_deref(),
    );
    builder.opt_str(
        "desktop_remote_training_last_error",
        remote_training.last_error.as_deref(),
    );
    builder.opt_str(
        "desktop_remote_training_last_action",
        remote_training.last_action.as_deref(),
    );
    if let Some(index) = &remote_training.run_index {
        builder.field(
            "desktop_remote_training_index_digest",
            index.index_digest.as_str(),
        );
        builder.field("desktop_remote_training_index_id", index.index_id.as_str());
    }
    for (run_id, bundle) in &remote_training.bundles {
        builder.field("desktop_remote_training_bundle_run_id", run_id.as_str());
        builder.field(
            "desktop_remote_training_bundle_digest",
            bundle.bundle_digest.as_str(),
        );
        builder.field(
            "desktop_remote_training_bundle_heartbeat_seq",
            bundle.refresh_contract.heartbeat_seq,
        );
        builder.opt_u64(
            "desktop_remote_training_bundle_last_heartbeat_at_ms",
            bundle.refresh_contract.last_heartbeat_at_ms,
        );
        builder.field(
            "desktop_remote_training_bundle_series_status",
            format!("{:?}", bundle.series_status),
        );
    }
}

fn append_desktop_control_log_signature(builder: &mut SignatureBuilder, state: &RenderState) {
    let lines = state
        .log_stream
        .terminal
        .recent_lines(usize::MAX)
        .iter()
        .rev()
        .take(SIGNATURE_LOG_TAIL_LIMIT)
        .collect::<Vec<_>>();
    builder.field(
        "desktop_log_line_count",
        state.log_stream.terminal.recent_lines(usize::MAX).len(),
    );
    for line in lines.into_iter().rev() {
        builder.field("desktop_log_stream", format!("{:?}", line.stream));
        builder.hashed_text("desktop_log_text", line.text.as_str());
        builder.opt_str("desktop_log_key", line.key.as_deref());
    }
    builder.opt_str(
        "desktop_last_command_summary",
        state.desktop_control.last_command_summary.as_deref(),
    );
    builder.opt_str(
        "desktop_last_command_error",
        state.desktop_control.last_command_error.as_deref(),
    );
    builder.opt_u64(
        "desktop_last_command_completed_at_epoch_ms",
        state.desktop_control.last_command_completed_at_epoch_ms,
    );
    builder.opt_str(
        "desktop_last_snapshot_signature",
        state.desktop_control.last_snapshot_signature.as_deref(),
    );
}

fn append_pane_signature(builder: &mut SignatureBuilder, pane: &DesktopPane) {
    builder.field("desktop_pane_id", pane.id);
    builder.field("desktop_pane_kind", format!("{:?}", pane.kind));
    builder.field("desktop_pane_title", pane.title.as_str());
    builder.field("desktop_pane_z_index", pane.z_index);
    builder.field(
        "desktop_pane_presentation",
        format!("{:?}", pane.presentation),
    );
}

fn approx_epoch_ms_for_instant(target: Instant) -> Option<i64> {
    let now = Instant::now();
    let age = now.checked_duration_since(target)?;
    let observed = SystemTime::now().checked_sub(age)?;
    observed
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

fn current_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn app_server_request_id_value(value: &codex_client::AppServerRequestId) -> String {
    match value {
        codex_client::AppServerRequestId::String(value) => value.clone(),
        codex_client::AppServerRequestId::Integer(value) => value.to_string(),
    }
}
