use std::sync::Arc;

use crate::app_state::{
    ActiveJobRecord, ActiveJobState, ActivityEventDomain, ActivityFeedFilter, ActivityFeedState,
    AgentProfileStatePaneState, AgentScheduleTickPaneState, AlertSeverity, AlertsRecoveryState,
    AppleAdapterTrainingPaneInputs, AppleAdapterTrainingPaneState, AppleFmWorkbenchPaneInputs,
    AppleFmWorkbenchPaneState, AttnResLabPaneState, AutopilotChatState, BuyModePaymentsPaneState,
    CadDemoPaneState, CalculatorPaneInputs, CastControlPaneState, ChatPaneInputs,
    CodexAccountPaneState, CodexAppsPaneState, CodexConfigPaneState, CodexDiagnosticsPaneState,
    CodexLabsPaneState, CodexMcpPaneState, CodexModelsPaneState, CreateInvoicePaneInputs,
    CredentialsPaneInputs, CredentialsState, CreditDeskPaneState, CreditSettlementLedgerPaneState,
    DataMarketPaneState, DataSellerPaneState, DesktopPane, EarnJobLifecycleProjectionState,
    EarningsScoreboardState, FrameDebuggerPaneState, JobHistoryPaneInputs, JobHistoryState,
    JobInboxState, JobLifecycleStage, LocalInferencePaneInputs, LocalInferencePaneState,
    LogStreamPaneState, MissionControlLocalRuntimeLane, MissionControlPaneState,
    NetworkRequestsPaneInputs, NetworkRequestsState, Nip90SentPaymentsPaneState, NostrSecretState,
    PaneKind, PaneLoadState, PanePaintTimingSample, PayInvoicePaneInputs, PresentationPaneState,
    PresentationRuntimeState, ProjectOpsPaneState, ProviderBlocker, ProviderControlHudRuntimeState,
    ProviderControlPaneState, ProviderRuntimeState, ReciprocalLoopState,
    RelayConnectionsPaneInputs, RelayConnectionsState, RivePreviewPaneState,
    RivePreviewRuntimeState, SettingsPaneInputs, SettingsState, SkillRegistryPaneState,
    SkillTrustRevocationPaneState, SparkPaneInputs, SparkReplayPaneState, StarterJobStatus,
    StarterJobsState, SyncHealthState, TassadarLabPaneState, TrajectoryAuditPaneState,
    mission_control_local_runtime_is_ready, mission_control_local_runtime_lane,
    mission_control_show_local_model_button,
};
use crate::apple_fm_bridge::AppleFmBridgeSnapshot;
use crate::bitcoin_display::{format_mission_control_amount, format_sats_amount};
use crate::desktop_control::DesktopControlTrainingStatus;
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::local_runtime_capabilities::local_runtime_capability_surface_for_lane;
use crate::pane_system::{
    PANE_TITLE_HEIGHT, active_job_abort_button_bounds, active_job_advance_button_bounds,
    active_job_copy_button_bounds, active_job_scroll_viewport_bounds,
    activity_feed_detail_viewport_bounds, activity_feed_details_bounds,
    activity_feed_filter_button_bounds, activity_feed_next_page_button_bounds,
    activity_feed_prev_page_button_bounds, activity_feed_refresh_button_bounds,
    activity_feed_row_bounds, activity_feed_visible_row_count, alerts_recovery_ack_button_bounds,
    alerts_recovery_recover_button_bounds, alerts_recovery_resolve_button_bounds,
    alerts_recovery_row_bounds, alerts_recovery_visible_row_count,
    credentials_add_custom_button_bounds, credentials_delete_button_bounds,
    credentials_import_button_bounds, credentials_name_input_bounds,
    credentials_reload_button_bounds, credentials_row_bounds, credentials_save_value_button_bounds,
    credentials_scope_codex_button_bounds, credentials_scope_global_button_bounds,
    credentials_scope_skills_button_bounds, credentials_scope_spark_button_bounds,
    credentials_toggle_enabled_button_bounds, credentials_value_input_bounds,
    credentials_visible_row_count, go_online_toggle_button_bounds,
    job_history_next_page_button_bounds, job_history_prev_page_button_bounds,
    job_history_search_input_bounds, job_history_status_button_bounds,
    job_history_time_button_bounds, job_inbox_accept_button_bounds, job_inbox_reject_button_bounds,
    job_inbox_row_bounds, job_inbox_visible_row_count, mission_control_alert_dismiss_button_bounds,
    mission_control_buy_mode_button_bounds, mission_control_buy_mode_history_button_bounds,
    mission_control_copy_log_stream_button_bounds,
    mission_control_copy_seed_button_bounds_for_scroll, mission_control_layout_for_mode,
    mission_control_load_funds_layout_with_scroll,
    mission_control_load_funds_scroll_viewport_bounds, mission_control_local_fm_test_button_bounds,
    mission_control_local_model_button_bounds, mission_control_sell_scroll_viewport_bounds,
    mission_control_send_invoice_input_bounds_for_scroll,
    mission_control_send_lightning_button_bounds_for_scroll,
    mission_control_wallet_refresh_button_bounds, network_requests_accept_button_bounds,
    network_requests_budget_input_bounds, network_requests_credit_envelope_input_bounds,
    network_requests_max_price_input_bounds, network_requests_payload_input_bounds,
    network_requests_quote_row_bounds, network_requests_skill_scope_input_bounds,
    network_requests_submit_button_bounds, network_requests_timeout_input_bounds,
    network_requests_type_input_bounds, network_requests_visible_quote_count,
    nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds, nostr_reveal_button_bounds,
    pane_content_bounds_for_pane, provider_inventory_toggle_button_bounds,
    reciprocal_loop_reset_button_bounds, reciprocal_loop_start_button_bounds,
    reciprocal_loop_stop_button_bounds, settings_provider_queue_input_bounds,
    settings_relay_input_bounds, settings_reset_button_bounds, settings_save_button_bounds,
    settings_wallet_default_input_bounds, starter_jobs_complete_button_bounds,
    starter_jobs_kill_switch_button_bounds, starter_jobs_row_bounds,
    starter_jobs_visible_row_count, sync_health_rebootstrap_button_bounds,
};
use crate::panes::{
    agent as agent_pane, apple_adapter_training as apple_adapter_training_pane,
    apple_fm_workbench as apple_fm_workbench_pane, attnres_lab as attnres_lab_pane,
    buy_mode as buy_mode_pane, buyer_race_matrix as buyer_race_matrix_pane, cad as cad_pane,
    calculator as calculator_pane, cast as cast_pane, chat as chat_pane, codex as codex_pane,
    credit as credit_pane, data_market as data_market_pane, data_seller as data_seller_pane,
    earnings_jobs as earnings_jobs_pane, frame_debugger as frame_debugger_pane,
    key_ledger as key_ledger_pane, local_inference as local_inference_pane,
    log_stream as log_stream_pane, nip90_sent_payments as nip90_sent_payments_pane,
    presentation as presentation_pane, project_ops as project_ops_pane,
    provider_control as provider_control_pane, psionic_viz as psionic_viz_pane,
    relay_choreography as relay_choreography_pane, relay_connections as relay_connections_pane,
    rive as rive_pane, seller_earnings_timeline as seller_earnings_timeline_pane,
    settlement_atlas as settlement_atlas_pane, settlement_ladder as settlement_ladder_pane,
    skill as skill_pane, spark_replay as spark_replay_pane, tassadar_lab as tassadar_lab_pane,
    wallet as wallet_pane,
};
use crate::spark_wallet::{SparkInvoiceState, SparkPaneState};
use crate::state::job_inbox::JobInboxRequest;
use crate::state::nip90_payment_facts::Nip90PaymentFactLedgerState;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, SvgQuad, theme};

pub struct PaneRenderer;

pub struct PanePaintReport {
    pub next_layer: u32,
    pub pane_paint_samples: Vec<PanePaintTimingSample>,
}

const INACTIVE_PANE_OVERLAY_ALPHA: f32 = 0.2;
const ACTIVE_PANE_FOCUS_CLEARANCE: f32 = 8.0;
const MISSION_CONTROL_LABEL_ROW_LINE_HEIGHT: f32 = 18.0;
const MISSION_CONTROL_BODY_BLOCK_LINE_HEIGHT: f32 = 16.0;
const MISSION_CONTROL_ROW_DIVIDER_TOP_GAP: f32 = 10.0;
const MISSION_CONTROL_ROW_DIVIDER_HEIGHT: f32 = 1.0;
const MISSION_CONTROL_ROW_DIVIDER_BOTTOM_GAP: f32 = 10.0;
const MISSION_CONTROL_LABEL_ROW_TRAILING_GAP: f32 = 20.0;
const MISSION_CONTROL_BODY_BLOCK_LABEL_GAP: f32 = 16.0;
const MISSION_CONTROL_BODY_BLOCK_BOTTOM_GAP: f32 = 4.0;
const MISSION_CONTROL_BODY_BLOCK_TRAILING_GAP: f32 = 12.0;
const MISSION_CONTROL_LOAD_FUNDS_CONTENT_BOTTOM_PADDING: f32 = 8.0;
const MISSION_CONTROL_REFRESH_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M129.9 292.5C143.2 199.5 223.3 128 320 128C373 128 421 149.5 455.8 184.2C456 184.4 456.2 184.6 456.4 184.8L464 192L416.1 192C398.4 192 384.1 206.3 384.1 224C384.1 241.7 398.4 256 416.1 256L544.1 256C561.8 256 576.1 241.7 576.1 224L576.1 96C576.1 78.3 561.8 64 544.1 64C526.4 64 512.1 78.3 512.1 96L512.1 149.4L500.8 138.7C454.5 92.6 390.5 64 320 64C191 64 84.3 159.4 66.6 283.5C64.1 301 76.2 317.2 93.7 319.7C111.2 322.2 127.4 310 129.9 292.6zM573.4 356.5C575.9 339 563.7 322.8 546.3 320.3C528.9 317.8 512.6 330 510.1 347.4C496.8 440.4 416.7 511.9 320 511.9C267 511.9 219 490.4 184.2 455.7C184 455.5 183.8 455.3 183.6 455.1L176 447.9L223.9 447.9C241.6 447.9 255.9 433.6 255.9 415.9C255.9 398.2 241.6 383.9 223.9 383.9L96 384C87.5 384 79.3 387.4 73.3 393.5C67.3 399.6 63.9 407.7 64 416.3L65 543.3C65.1 561 79.6 575.2 97.3 575C115 574.8 129.2 560.4 129 542.7L128.6 491.2L139.3 501.3C185.6 547.4 249.5 576 320 576C449 576 555.7 480.6 573.4 356.5z"/></svg>"##;
const MISSION_CONTROL_COPY_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M480 400L288 400C279.2 400 272 392.8 272 384L272 128C272 119.2 279.2 112 288 112L421.5 112C425.7 112 429.8 113.7 432.8 116.7L491.3 175.2C494.3 178.2 496 182.3 496 186.5L496 384C496 392.8 488.8 400 480 400zM288 448L480 448C515.3 448 544 419.3 544 384L544 186.5C544 169.5 537.3 153.2 525.3 141.2L466.7 82.7C454.7 70.7 438.5 64 421.5 64L288 64C252.7 64 224 92.7 224 128L224 384C224 419.3 252.7 448 288 448zM160 192C124.7 192 96 220.7 96 256L96 512C96 547.3 124.7 576 160 576L352 576C387.3 576 416 547.3 416 512L416 496L368 496L368 512C368 520.8 360.8 528 352 528L160 528C151.2 528 144 520.8 144 512L144 256C144 247.2 151.2 240 160 240L176 240L176 192L160 192z"/></svg>"##;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InactivePaneRenderPolicy {
    Full,
    Summary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InactivePanePreviewState {
    source_badge: String,
    load_state: PaneLoadState,
    summary: String,
    last_action: Option<String>,
    last_error: Option<String>,
    detail_lines: Vec<String>,
}

impl PaneRenderer {
    #[expect(
        clippy::too_many_arguments,
        reason = "Pane rendering orchestrates all per-pane state until pane modules are split."
    )]
    pub fn paint(
        panes: &mut [DesktopPane],
        canvas_bounds: Bounds,
        active_id: Option<u64>,
        cursor_position: Point,
        desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
        buy_mode_enabled: bool,
        backend_kernel_authority: bool,
        nostr_identity: Option<&nostr::NostrIdentity>,
        nostr_identity_error: Option<&str>,
        nostr_secret_state: &NostrSecretState,
        autopilot_chat: &AutopilotChatState,
        project_ops: &ProjectOpsPaneState,
        spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
        codex_account: &CodexAccountPaneState,
        codex_models: &CodexModelsPaneState,
        codex_config: &CodexConfigPaneState,
        codex_mcp: &CodexMcpPaneState,
        codex_apps: &CodexAppsPaneState,
        codex_labs: &CodexLabsPaneState,
        codex_remote: &crate::app_state::CodexRemoteState,
        codex_diagnostics: &CodexDiagnosticsPaneState,
        sa_lane: &crate::runtime_lanes::SaLaneSnapshot,
        skl_lane: &crate::runtime_lanes::SklLaneSnapshot,
        ac_lane: &crate::runtime_lanes::AcLaneSnapshot,
        provider_runtime: &ProviderRuntimeState,
        local_inference_runtime: &LocalInferenceExecutionSnapshot,
        apple_fm_execution: &AppleFmBridgeSnapshot,
        local_inference: &LocalInferencePaneState,
        attnres_lab: &AttnResLabPaneState,
        tassadar_lab: &TassadarLabPaneState,
        rive_preview: &mut RivePreviewPaneState,
        rive_preview_runtime: &mut RivePreviewRuntimeState,
        presentation: &mut PresentationPaneState,
        presentation_runtime: &mut PresentationRuntimeState,
        provider_control_hud_runtime: &mut ProviderControlHudRuntimeState,
        frame_debugger: &FrameDebuggerPaneState,
        apple_fm_workbench: &mut AppleFmWorkbenchPaneState,
        apple_adapter_training: &mut AppleAdapterTrainingPaneState,
        training_status: &DesktopControlTrainingStatus,
        provider_blockers: &[ProviderBlocker],
        earnings_scoreboard: &EarningsScoreboardState,
        relay_connections: &RelayConnectionsState,
        sync_health: &SyncHealthState,
        network_requests: &NetworkRequestsState,
        nip90_buyer_payment_attempts: &crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentAttemptLedgerState,
        nip90_payment_facts: &Nip90PaymentFactLedgerState,
        starter_jobs: &StarterJobsState,
        reciprocal_loop: &ReciprocalLoopState,
        activity_feed: &ActivityFeedState,
        alerts_recovery: &AlertsRecoveryState,
        settings: &SettingsState,
        credentials: &CredentialsState,
        job_inbox: &JobInboxState,
        active_job: &ActiveJobState,
        job_history: &JobHistoryState,
        earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
        agent_profile_state: &AgentProfileStatePaneState,
        agent_schedule_tick: &AgentScheduleTickPaneState,
        trajectory_audit: &TrajectoryAuditPaneState,
        cast_control: &CastControlPaneState,
        skill_registry: &SkillRegistryPaneState,
        skill_trust_revocation: &SkillTrustRevocationPaneState,
        credit_desk: &CreditDeskPaneState,
        credit_settlement_ledger: &CreditSettlementLedgerPaneState,
        cad_demo: &CadDemoPaneState,
        spark_wallet: &SparkPaneState,
        provider_inventory: &crate::provider_inventory::DesktopControlInventoryStatus,
        spark_inputs: &mut SparkPaneInputs,
        pay_invoice_inputs: &mut PayInvoicePaneInputs,
        create_invoice_inputs: &mut CreateInvoicePaneInputs,
        relay_connections_inputs: &mut RelayConnectionsPaneInputs,
        network_requests_inputs: &mut NetworkRequestsPaneInputs,
        local_inference_inputs: &mut LocalInferencePaneInputs,
        apple_fm_workbench_inputs: &mut AppleFmWorkbenchPaneInputs,
        apple_adapter_training_inputs: &mut AppleAdapterTrainingPaneInputs,
        settings_inputs: &mut SettingsPaneInputs,
        credentials_inputs: &mut CredentialsPaneInputs,
        job_history_inputs: &mut JobHistoryPaneInputs,
        chat_inputs: &mut ChatPaneInputs,
        calculator_inputs: &mut CalculatorPaneInputs,
        mission_control: &mut MissionControlPaneState,
        provider_control: &mut ProviderControlPaneState,
        log_stream_last_action: Option<&str>,
        log_stream_last_error: Option<&str>,
        log_stream: &mut LogStreamPaneState,
        buy_mode_payments: &mut BuyModePaymentsPaneState,
        nip90_sent_payments: &mut Nip90SentPaymentsPaneState,
        data_seller: &DataSellerPaneState,
        data_market: &DataMarketPaneState,
        spark_replay: &mut SparkReplayPaneState,
        paint: &mut PaintContext,
    ) -> PanePaintReport {
        log_stream.sync_log_stream(
            log_stream_last_action,
            log_stream_last_error,
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
            provider_blockers,
            earn_job_lifecycle_projection,
            spark_wallet,
            network_requests,
            job_inbox,
            active_job,
        );
        let mut indices: Vec<usize> = (0..panes.len()).collect();
        indices.sort_by_key(|idx| panes[*idx].z_index);
        let dim_inactive_panes = panes.len() > 1 && active_id.is_some();

        let mut next_layer: u32 = 1;
        let mut pane_paint_samples = Vec::with_capacity(panes.len());
        for idx in indices {
            paint.scene.set_layer(next_layer);
            next_layer = next_layer.saturating_add(1);

            let pane = &mut panes[idx];
            let pane_is_active = active_id == Some(pane.id);
            let pane_kind_label = format!("{:?}", pane.kind);
            let pane_title = pane.title.clone();
            let pane_paint_start = std::time::Instant::now();

            paint
                .scene
                .draw_quad(Quad::new(pane.bounds).with_background(theme::bg::APP));

            let content_bounds = pane_content_bounds_for_pane(pane);
            if pane.presentation.uses_window_chrome() {
                pane.frame.set_title(&pane.title);
                pane.frame.set_active(pane_is_active);
                pane.frame.set_title_height(PANE_TITLE_HEIGHT);
                pane.frame.set_header_action(match pane.kind {
                    PaneKind::Presentation => {
                        Some(wgpui::components::hud::PaneHeaderAction::Fullscreen)
                    }
                    _ => None,
                });
                pane.frame.paint(pane.bounds, paint);

                paint.scene.draw_quad(
                    Quad::new(content_bounds)
                        .with_background(theme::bg::SURFACE)
                        .with_corner_radius(6.0),
                );
            } else {
                paint
                    .scene
                    .draw_quad(Quad::new(content_bounds).with_background(theme::bg::SURFACE));
            }

            if !pane_is_active
                && paint_inactive_pane_preview_if_needed(
                    inactive_pane_render_policy(pane.kind),
                    pane.title.as_str(),
                    pane.kind,
                    content_bounds,
                    desktop_shell_mode,
                    buy_mode_enabled,
                    backend_kernel_authority,
                    autopilot_chat,
                    codex_diagnostics,
                    provider_runtime,
                    local_inference_runtime,
                    provider_blockers,
                    provider_control,
                    log_stream,
                    buy_mode_payments,
                    network_requests,
                    nip90_buyer_payment_attempts,
                    nip90_payment_facts,
                    relay_connections,
                    nip90_sent_payments,
                    data_seller,
                    data_market,
                    spark_replay,
                    spark_wallet,
                    frame_debugger,
                    paint,
                )
            {
                pane_paint_samples.push(PanePaintTimingSample {
                    pane_kind: pane_kind_label,
                    pane_title,
                    render_mode: "summary".to_string(),
                    active: false,
                    elapsed_ms: pane_paint_start.elapsed().as_secs_f32() * 1_000.0,
                });
                continue;
            }

            match pane.kind {
                PaneKind::Empty => paint_empty_pane(content_bounds, paint),
                PaneKind::AutopilotChat => {
                    paint_autopilot_chat_pane(
                        content_bounds,
                        autopilot_chat,
                        codex_account,
                        spacetime_presence,
                        chat_inputs,
                        paint,
                    );
                }
                PaneKind::ProjectOps => {
                    project_ops_pane::paint_project_ops_pane(content_bounds, project_ops, paint);
                }
                PaneKind::Calculator => {
                    calculator_pane::paint(content_bounds, calculator_inputs, paint);
                }
                PaneKind::CodexAccount => {
                    codex_pane::paint_account_pane(content_bounds, codex_account, paint);
                }
                PaneKind::CodexModels => {
                    codex_pane::paint_models_pane(content_bounds, codex_models, paint);
                }
                PaneKind::CodexConfig => {
                    codex_pane::paint_config_pane(content_bounds, codex_config, paint);
                }
                PaneKind::CodexMcp => {
                    codex_pane::paint_mcp_pane(content_bounds, codex_mcp, paint);
                }
                PaneKind::CodexApps => {
                    codex_pane::paint_apps_pane(content_bounds, codex_apps, paint);
                }
                PaneKind::CodexLabs => {
                    codex_pane::paint_labs_pane(content_bounds, codex_labs, codex_remote, paint);
                }
                PaneKind::CodexDiagnostics => {
                    codex_pane::paint_diagnostics_pane(content_bounds, codex_diagnostics, paint);
                }
                PaneKind::GoOnline => {
                    paint_go_online_pane(
                        content_bounds,
                        pane_is_active,
                        cursor_position,
                        desktop_shell_mode,
                        buy_mode_enabled,
                        autopilot_chat,
                        nostr_identity,
                        mission_control,
                        provider_control,
                        provider_runtime,
                        local_inference_runtime,
                        log_stream,
                        buy_mode_payments,
                        earn_job_lifecycle_projection,
                        sa_lane,
                        skl_lane,
                        ac_lane,
                        backend_kernel_authority,
                        provider_blockers,
                        earnings_scoreboard,
                        spark_wallet,
                        network_requests,
                        job_inbox,
                        active_job,
                        paint,
                    );
                }
                PaneKind::ProviderControl => {
                    provider_control_pane::paint_provider_control_pane(
                        content_bounds,
                        provider_control,
                        provider_control_hud_runtime,
                        desktop_shell_mode,
                        provider_runtime,
                        local_inference_runtime,
                        provider_blockers,
                        backend_kernel_authority,
                        spark_wallet,
                        provider_inventory,
                        paint,
                    );
                }
                PaneKind::ProviderStatus => {
                    paint_provider_status_pane(
                        content_bounds,
                        provider_runtime,
                        earn_job_lifecycle_projection,
                        backend_kernel_authority,
                        provider_blockers,
                        paint,
                    );
                }
                PaneKind::LocalInference => {
                    let capability_surface = local_runtime_capability_surface_for_lane(
                        MissionControlLocalRuntimeLane::GptOss,
                        provider_runtime,
                        local_inference_runtime,
                    );
                    local_inference_pane::paint(
                        content_bounds,
                        local_inference,
                        &capability_surface,
                        local_inference_runtime,
                        local_inference_inputs,
                        paint,
                    );
                }
                PaneKind::PsionicViz => {
                    psionic_viz_pane::paint(
                        content_bounds,
                        local_inference,
                        local_inference_runtime,
                        paint,
                    );
                }
                PaneKind::AttnResLab => {
                    attnres_lab_pane::paint(content_bounds, attnres_lab, paint);
                }
                PaneKind::TassadarLab => {
                    tassadar_lab_pane::paint(content_bounds, tassadar_lab, paint);
                }
                PaneKind::RivePreview => {
                    rive_pane::paint(content_bounds, rive_preview, rive_preview_runtime, paint);
                }
                PaneKind::Presentation => {
                    presentation_pane::paint(
                        content_bounds,
                        presentation,
                        presentation_runtime,
                        paint,
                    );
                }
                PaneKind::FrameDebugger => {
                    frame_debugger_pane::paint(content_bounds, frame_debugger, paint);
                }
                PaneKind::AppleFmWorkbench => {
                    let capability_surface = local_runtime_capability_surface_for_lane(
                        MissionControlLocalRuntimeLane::AppleFoundationModels,
                        provider_runtime,
                        local_inference_runtime,
                    );
                    apple_fm_workbench_pane::paint(
                        content_bounds,
                        apple_fm_workbench,
                        &capability_surface,
                        apple_fm_execution,
                        apple_fm_workbench_inputs,
                        paint,
                    );
                }
                PaneKind::AppleAdapterTraining => {
                    apple_adapter_training_pane::paint(
                        content_bounds,
                        apple_adapter_training,
                        training_status,
                        apple_adapter_training_inputs,
                        paint,
                    );
                }
                PaneKind::EarningsScoreboard => {
                    paint_earnings_scoreboard_pane(
                        content_bounds,
                        pane_is_active,
                        desktop_shell_mode,
                        earnings_scoreboard,
                        provider_runtime,
                        local_inference_runtime,
                        job_inbox,
                        active_job,
                        job_history,
                        earn_job_lifecycle_projection,
                        spark_wallet,
                        paint,
                    );
                }
                PaneKind::RelayConnections => {
                    paint_relay_connections_pane(
                        content_bounds,
                        relay_connections,
                        relay_connections_inputs,
                        paint,
                    );
                }
                PaneKind::SyncHealth => {
                    paint_sync_health_pane(content_bounds, sync_health, paint);
                }
                PaneKind::NetworkRequests => {
                    paint_network_requests_pane(
                        content_bounds,
                        network_requests,
                        network_requests_inputs,
                        paint,
                    );
                }
                PaneKind::StarterJobs => {
                    paint_starter_jobs_pane(content_bounds, starter_jobs, paint);
                }
                PaneKind::ReciprocalLoop => {
                    paint_reciprocal_loop_pane(
                        content_bounds,
                        reciprocal_loop,
                        provider_runtime,
                        spark_wallet,
                        paint,
                    );
                }
                PaneKind::ActivityFeed => {
                    paint_activity_feed_pane(content_bounds, activity_feed, paint);
                }
                PaneKind::AlertsRecovery => {
                    paint_alerts_recovery_pane(content_bounds, alerts_recovery, paint);
                }
                PaneKind::Settings => {
                    paint_settings_pane(content_bounds, settings, settings_inputs, paint);
                }
                PaneKind::Credentials => {
                    paint_credentials_pane(content_bounds, credentials, credentials_inputs, paint);
                }
                PaneKind::JobInbox => {
                    paint_job_inbox_pane(content_bounds, job_inbox, provider_runtime, paint);
                }
                PaneKind::ActiveJob => {
                    paint_active_job_pane(
                        content_bounds,
                        active_job,
                        earn_job_lifecycle_projection,
                        spark_wallet,
                        paint,
                    );
                }
                PaneKind::JobHistory => {
                    paint_job_history_pane(
                        content_bounds,
                        job_history,
                        earn_job_lifecycle_projection,
                        job_history_inputs,
                        paint,
                    );
                }
                PaneKind::LogStream => {
                    log_stream_pane::paint(content_bounds, log_stream, paint);
                }
                PaneKind::BuyModePayments => {
                    buy_mode_pane::paint(
                        content_bounds,
                        buy_mode_enabled,
                        autopilot_chat,
                        buy_mode_payments,
                        network_requests,
                        nip90_payment_facts,
                        spark_wallet,
                        paint,
                    );
                }
                PaneKind::Nip90SentPayments => {
                    nip90_sent_payments_pane::paint(
                        content_bounds,
                        nip90_sent_payments,
                        nip90_buyer_payment_attempts,
                        relay_connections,
                        paint,
                    );
                }
                PaneKind::DataSeller => {
                    data_seller_pane::paint(content_bounds, data_seller, paint);
                }
                PaneKind::DataMarket => {
                    data_market_pane::paint(content_bounds, data_market, paint);
                }
                PaneKind::BuyerRaceMatrix => {
                    buyer_race_matrix_pane::paint(
                        content_bounds,
                        network_requests,
                        spark_wallet,
                        paint,
                    );
                }
                PaneKind::SellerEarningsTimeline => {
                    seller_earnings_timeline_pane::paint(
                        content_bounds,
                        nip90_payment_facts,
                        paint,
                    );
                }
                PaneKind::SettlementLadder => {
                    settlement_ladder_pane::paint(content_bounds, nip90_payment_facts, paint);
                }
                PaneKind::KeyLedger => {
                    key_ledger_pane::paint(content_bounds, nip90_payment_facts, paint);
                }
                PaneKind::SettlementAtlas => {
                    settlement_atlas_pane::paint(content_bounds, nip90_payment_facts, paint);
                }
                PaneKind::SparkReplay => {
                    spark_replay_pane::paint(
                        content_bounds,
                        spark_replay,
                        nip90_payment_facts,
                        paint,
                    );
                }
                PaneKind::RelayChoreography => {
                    relay_choreography_pane::paint(
                        content_bounds,
                        relay_connections,
                        nip90_payment_facts,
                        paint,
                    );
                }
                PaneKind::AgentProfileState => {
                    agent_pane::paint_agent_profile_state_pane(
                        content_bounds,
                        agent_profile_state,
                        paint,
                    );
                }
                PaneKind::AgentScheduleTick => {
                    agent_pane::paint_agent_schedule_tick_pane(
                        content_bounds,
                        agent_schedule_tick,
                        paint,
                    );
                }
                PaneKind::TrajectoryAudit => {
                    agent_pane::paint_trajectory_audit_pane(
                        content_bounds,
                        trajectory_audit,
                        paint,
                    );
                }
                PaneKind::CastControl => {
                    cast_pane::paint_cast_control_pane(content_bounds, cast_control, paint);
                }
                PaneKind::SkillRegistry => {
                    skill_pane::paint_skill_registry_pane(content_bounds, skill_registry, paint);
                }
                PaneKind::SkillTrustRevocation => {
                    skill_pane::paint_skill_trust_revocation_pane(
                        content_bounds,
                        skill_trust_revocation,
                        paint,
                    );
                }
                PaneKind::CreditDesk => {
                    credit_pane::paint_credit_desk_pane(content_bounds, credit_desk, paint);
                }
                PaneKind::CreditSettlementLedger => {
                    credit_pane::paint_credit_settlement_ledger_pane(
                        content_bounds,
                        credit_settlement_ledger,
                        paint,
                    );
                }
                PaneKind::CadDemo => {
                    cad_pane::paint_cad_demo_placeholder_pane(content_bounds, cad_demo, paint);
                }
                PaneKind::NostrIdentity => {
                    paint_nostr_identity_pane(
                        content_bounds,
                        nostr_identity,
                        nostr_identity_error,
                        nostr_secret_state,
                        paint,
                    );
                }
                PaneKind::SparkWallet => {
                    paint_spark_wallet_pane(content_bounds, spark_wallet, spark_inputs, paint);
                }
                PaneKind::SparkCreateInvoice => {
                    paint_create_invoice_pane(
                        content_bounds,
                        spark_wallet,
                        create_invoice_inputs,
                        paint,
                    );
                }
                PaneKind::SparkPayInvoice => {
                    paint_pay_invoice_pane(content_bounds, spark_wallet, pay_invoice_inputs, paint);
                }
            }

            pane_paint_samples.push(PanePaintTimingSample {
                pane_kind: pane_kind_label,
                pane_title,
                render_mode: "full".to_string(),
                active: pane_is_active,
                elapsed_ms: pane_paint_start.elapsed().as_secs_f32() * 1_000.0,
            });
        }

        if dim_inactive_panes {
            let Some(active_bounds) = panes
                .iter()
                .find(|pane| Some(pane.id) == active_id)
                .map(|pane| pane.bounds)
            else {
                return PanePaintReport {
                    next_layer,
                    pane_paint_samples,
                };
            };

            paint.scene.set_layer(next_layer);
            next_layer = next_layer.saturating_add(1);

            let overlay = theme::bg::APP.with_alpha(INACTIVE_PANE_OVERLAY_ALPHA);
            let cutout = Bounds::new(
                active_bounds.origin.x - ACTIVE_PANE_FOCUS_CLEARANCE,
                active_bounds.origin.y - ACTIVE_PANE_FOCUS_CLEARANCE,
                active_bounds.size.width + ACTIVE_PANE_FOCUS_CLEARANCE * 2.0,
                active_bounds.size.height + ACTIVE_PANE_FOCUS_CLEARANCE * 2.0,
            );

            let x0 = cutout
                .origin
                .x
                .clamp(canvas_bounds.origin.x, canvas_bounds.max_x());
            let x1 = cutout
                .max_x()
                .clamp(canvas_bounds.origin.x, canvas_bounds.max_x());
            let y0 = cutout
                .origin
                .y
                .clamp(canvas_bounds.origin.y, canvas_bounds.max_y());
            let y1 = cutout
                .max_y()
                .clamp(canvas_bounds.origin.y, canvas_bounds.max_y());

            if x1 <= x0 || y1 <= y0 {
                paint
                    .scene
                    .draw_quad(Quad::new(canvas_bounds).with_background(overlay));
                return PanePaintReport {
                    next_layer,
                    pane_paint_samples,
                };
            }

            if y0 > canvas_bounds.origin.y {
                paint.scene.draw_quad(
                    Quad::new(Bounds::new(
                        canvas_bounds.origin.x,
                        canvas_bounds.origin.y,
                        canvas_bounds.size.width,
                        y0 - canvas_bounds.origin.y,
                    ))
                    .with_background(overlay),
                );
            }
            if x0 > canvas_bounds.origin.x {
                paint.scene.draw_quad(
                    Quad::new(Bounds::new(
                        canvas_bounds.origin.x,
                        y0,
                        x0 - canvas_bounds.origin.x,
                        (y1 - y0).max(0.0),
                    ))
                    .with_background(overlay),
                );
            }
            if x1 < canvas_bounds.max_x() {
                paint.scene.draw_quad(
                    Quad::new(Bounds::new(
                        x1,
                        y0,
                        canvas_bounds.max_x() - x1,
                        (y1 - y0).max(0.0),
                    ))
                    .with_background(overlay),
                );
            }
            if y1 < canvas_bounds.max_y() {
                paint.scene.draw_quad(
                    Quad::new(Bounds::new(
                        canvas_bounds.origin.x,
                        y1,
                        canvas_bounds.size.width,
                        canvas_bounds.max_y() - y1,
                    ))
                    .with_background(overlay),
                );
            }
        }

        PanePaintReport {
            next_layer,
            pane_paint_samples,
        }
    }
}

fn paint_empty_pane(content_bounds: Bounds, paint: &mut PaintContext) {
    let empty = paint.text.layout(
        "Empty pane",
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 16.0,
        ),
        12.0,
        theme::text::MUTED,
    );
    paint.scene.draw_text(empty);
}

fn inactive_pane_render_policy(kind: PaneKind) -> InactivePaneRenderPolicy {
    match kind {
        PaneKind::AutopilotChat
        | PaneKind::GoOnline
        | PaneKind::ProviderControl
        | PaneKind::CodexDiagnostics
        | PaneKind::FrameDebugger
        | PaneKind::LogStream
        | PaneKind::BuyModePayments
        | PaneKind::Nip90SentPayments
        | PaneKind::DataSeller
        | PaneKind::DataMarket
        | PaneKind::BuyerRaceMatrix
        | PaneKind::SellerEarningsTimeline
        | PaneKind::SettlementLadder
        | PaneKind::KeyLedger
        | PaneKind::SettlementAtlas
        | PaneKind::SparkReplay
        | PaneKind::RelayChoreography => InactivePaneRenderPolicy::Summary,
        _ => InactivePaneRenderPolicy::Full,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Inactive pane summaries read across multiple app-owned pane states."
)]
fn paint_inactive_pane_preview_if_needed(
    policy: InactivePaneRenderPolicy,
    pane_title: &str,
    kind: PaneKind,
    content_bounds: Bounds,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    buy_mode_enabled: bool,
    backend_kernel_authority: bool,
    autopilot_chat: &AutopilotChatState,
    codex_diagnostics: &CodexDiagnosticsPaneState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    provider_control: &ProviderControlPaneState,
    log_stream: &LogStreamPaneState,
    buy_mode_payments: &BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    nip90_buyer_payment_attempts: &crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentAttemptLedgerState,
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
    relay_connections: &RelayConnectionsState,
    nip90_sent_payments: &Nip90SentPaymentsPaneState,
    data_seller: &DataSellerPaneState,
    data_market: &DataMarketPaneState,
    spark_replay: &SparkReplayPaneState,
    spark_wallet: &SparkPaneState,
    frame_debugger: &FrameDebuggerPaneState,
    paint: &mut PaintContext,
) -> bool {
    if policy == InactivePaneRenderPolicy::Full {
        return false;
    }
    let Some(preview) = inactive_pane_preview_state(
        kind,
        desktop_shell_mode,
        buy_mode_enabled,
        backend_kernel_authority,
        autopilot_chat,
        codex_diagnostics,
        provider_runtime,
        local_inference_runtime,
        provider_blockers,
        provider_control,
        log_stream,
        buy_mode_payments,
        network_requests,
        nip90_buyer_payment_attempts,
        nip90_payment_facts,
        relay_connections,
        nip90_sent_payments,
        data_seller,
        data_market,
        spark_replay,
        spark_wallet,
        frame_debugger,
    ) else {
        return false;
    };

    paint_inactive_pane_preview(content_bounds, pane_title, &preview, paint);
    true
}

#[expect(
    clippy::too_many_arguments,
    reason = "Inactive pane summaries intentionally stay app-owned and context-rich."
)]
fn inactive_pane_preview_state(
    kind: PaneKind,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    buy_mode_enabled: bool,
    backend_kernel_authority: bool,
    autopilot_chat: &AutopilotChatState,
    codex_diagnostics: &CodexDiagnosticsPaneState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    provider_control: &ProviderControlPaneState,
    log_stream: &LogStreamPaneState,
    buy_mode_payments: &BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    nip90_buyer_payment_attempts: &crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentAttemptLedgerState,
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
    relay_connections: &RelayConnectionsState,
    nip90_sent_payments: &Nip90SentPaymentsPaneState,
    data_seller: &DataSellerPaneState,
    data_market: &DataMarketPaneState,
    spark_replay: &SparkReplayPaneState,
    spark_wallet: &SparkPaneState,
    frame_debugger: &FrameDebuggerPaneState,
) -> Option<InactivePanePreviewState> {
    match kind {
        PaneKind::GoOnline | PaneKind::ProviderControl => {
            Some(provider_control_inactive_preview_state(
                desktop_shell_mode,
                backend_kernel_authority,
                provider_runtime,
                local_inference_runtime,
                provider_blockers,
                provider_control,
                spark_wallet,
            ))
        }
        PaneKind::AutopilotChat => Some(autopilot_chat_inactive_preview_state(autopilot_chat)),
        PaneKind::CodexDiagnostics => {
            Some(codex_diagnostics_inactive_preview_state(codex_diagnostics))
        }
        PaneKind::FrameDebugger => Some(frame_debugger_inactive_preview_state(frame_debugger)),
        PaneKind::LogStream => Some(log_stream_inactive_preview_state(log_stream)),
        PaneKind::BuyModePayments => Some(buy_mode_payments_inactive_preview_state(
            buy_mode_enabled,
            autopilot_chat,
            buy_mode_payments,
            network_requests,
            nip90_payment_facts,
            spark_wallet,
        )),
        PaneKind::Nip90SentPayments => Some(nip90_sent_payments_inactive_preview_state(
            nip90_sent_payments,
            nip90_buyer_payment_attempts,
            relay_connections,
        )),
        PaneKind::DataSeller => Some(data_seller_inactive_preview_state(data_seller)),
        PaneKind::DataMarket => Some(data_market_inactive_preview_state(data_market)),
        PaneKind::BuyerRaceMatrix => Some(buyer_race_matrix_inactive_preview_state(
            network_requests,
            spark_wallet,
        )),
        PaneKind::SellerEarningsTimeline => Some(seller_earnings_timeline_inactive_preview_state(
            nip90_payment_facts,
        )),
        PaneKind::SettlementLadder => Some(settlement_ladder_inactive_preview_state(
            nip90_payment_facts,
        )),
        PaneKind::KeyLedger => Some(key_ledger_inactive_preview_state(nip90_payment_facts)),
        PaneKind::SettlementAtlas => {
            Some(settlement_atlas_inactive_preview_state(nip90_payment_facts))
        }
        PaneKind::SparkReplay => Some(spark_replay_inactive_preview_state(
            spark_replay,
            nip90_payment_facts,
        )),
        PaneKind::RelayChoreography => Some(relay_choreography_inactive_preview_state(
            relay_connections,
            nip90_payment_facts,
        )),
        _ => None,
    }
}

fn paint_inactive_pane_preview(
    content_bounds: Bounds,
    pane_title: &str,
    preview: &InactivePanePreviewState,
    paint: &mut PaintContext,
) {
    let shell_bounds = Bounds::new(
        content_bounds.origin.x + 10.0,
        content_bounds.origin.y + 10.0,
        (content_bounds.size.width - 20.0).max(0.0),
        (content_bounds.size.height - 20.0).max(0.0),
    );
    paint.scene.draw_quad(
        Quad::new(shell_bounds)
            .with_background(theme::bg::APP.with_alpha(0.22))
            .with_border(theme::border::DEFAULT.with_alpha(0.72), 1.0)
            .with_corner_radius(10.0),
    );
    paint_source_badge(content_bounds, preview.source_badge.as_str(), paint);
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "{} // INACTIVE PREVIEW",
            pane_title.trim().to_ascii_uppercase()
        ),
        Point::new(shell_bounds.origin.x + 12.0, shell_bounds.origin.y + 14.0),
        11.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Activate pane for live controls and the full detail surface.",
        Point::new(shell_bounds.origin.x + 12.0, shell_bounds.origin.y + 32.0),
        10.0,
        theme::text::MUTED,
    ));

    let mut y = paint_state_summary(
        paint,
        shell_bounds.origin.x + 12.0,
        shell_bounds.origin.y + 52.0,
        preview.load_state,
        preview.summary.as_str(),
        preview.last_action.as_deref(),
        preview.last_error.as_deref(),
    );

    for line in preview.detail_lines.iter().take(4) {
        if y + 14.0 > shell_bounds.max_y() - 18.0 {
            break;
        }
        paint.scene.draw_text(paint.text.layout_mono(
            line,
            Point::new(shell_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::SECONDARY,
        ));
        y += 14.0;
    }
}

fn provider_control_inactive_preview_state(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    backend_kernel_authority: bool,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    provider_control: &ProviderControlPaneState,
    spark_wallet: &SparkPaneState,
) -> InactivePanePreviewState {
    let runtime_view = crate::app_state::mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    let preflight = if provider_blockers.is_empty() {
        "clear".to_string()
    } else {
        format!("{} blocker(s)", provider_blockers.len())
    };
    let mut detail_lines = vec![
        format!("preflight {preflight}"),
        format!(
            "control {}",
            provider_runtime.control_authority_label(backend_kernel_authority)
        ),
        format!("load {}", runtime_view.load_label),
        format!("inventory {} rows", provider_runtime.inventory_rows.len()),
    ];
    if let Some(blocker) = provider_blockers.first().copied() {
        detail_lines.push(format!(
            "blocker {}",
            mission_control_blocker_detail(blocker, spark_wallet, provider_runtime)
        ));
    } else if provider_control.local_fm_summary_is_pending() {
        detail_lines.push("local-fm summary streaming".to_string());
    } else if !provider_control.local_fm_summary_text.trim().is_empty() {
        detail_lines.push(format!(
            "local-fm {}",
            compact_mission_control_id(provider_control.local_fm_summary_text.trim())
        ));
    }
    InactivePanePreviewState {
        source_badge: "inactive runtime".to_string(),
        load_state: if provider_control.last_error.is_some()
            || provider_runtime.last_error_detail.is_some()
        {
            PaneLoadState::Error
        } else if provider_runtime.mode == crate::app_state::ProviderMode::Connecting {
            PaneLoadState::Loading
        } else {
            PaneLoadState::Ready
        },
        summary: format!(
            "{} // {} // {}",
            provider_runtime.mode.label(),
            runtime_view.model_label,
            runtime_view.backend_label
        ),
        last_action: provider_control
            .last_action
            .clone()
            .or_else(|| provider_runtime.last_result.clone()),
        last_error: provider_control
            .last_error
            .clone()
            .or_else(|| provider_runtime.last_error_detail.clone()),
        detail_lines,
    }
}

fn autopilot_chat_inactive_preview_state(
    autopilot_chat: &AutopilotChatState,
) -> InactivePanePreviewState {
    let active_thread_label = autopilot_chat
        .active_thread_id
        .as_ref()
        .map(|thread_id| {
            autopilot_chat
                .thread_metadata
                .get(thread_id)
                .and_then(|metadata| metadata.thread_name.as_ref())
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| compact_mission_control_id(thread_id))
        })
        .unwrap_or_else(|| "none".to_string());
    let active_preview = autopilot_chat
        .active_thread_preview()
        .map(|value| compact_preview_text(value, 48))
        .unwrap_or_else(|| "No active transcript loaded".to_string());
    let pending_tool_inputs = autopilot_chat.pending_tool_user_input.len();
    let pending_approvals = autopilot_chat.pending_command_approvals.len()
        + autopilot_chat.pending_file_change_approvals.len();
    InactivePanePreviewState {
        source_badge: "inactive chat".to_string(),
        load_state: if autopilot_chat.last_error.is_some() {
            PaneLoadState::Error
        } else if autopilot_chat.connection_status != "ready" {
            PaneLoadState::Loading
        } else {
            PaneLoadState::Ready
        },
        summary: format!(
            "{} // {} threads // active {}",
            autopilot_chat.connection_status,
            autopilot_chat.threads.len(),
            active_thread_label
        ),
        last_action: autopilot_chat.last_turn_status.clone(),
        last_error: autopilot_chat.last_error.clone(),
        detail_lines: vec![
            format!(
                "thread_status {} // loaded {}",
                autopilot_chat.active_thread_status().unwrap_or("idle"),
                if autopilot_chat.active_thread_loaded().unwrap_or(false) {
                    "yes"
                } else {
                    "no"
                }
            ),
            format!(
                "model {} // tool_inputs {}",
                autopilot_chat.current_model(),
                pending_tool_inputs
            ),
            format!(
                "approvals {} // auth_refresh {}",
                pending_approvals,
                autopilot_chat.pending_auth_refresh.len()
            ),
            active_preview,
        ],
    }
}

fn codex_diagnostics_inactive_preview_state(
    codex_diagnostics: &CodexDiagnosticsPaneState,
) -> InactivePanePreviewState {
    let notification_total = codex_diagnostics
        .notification_counts
        .iter()
        .map(|entry| entry.count)
        .sum::<u64>();
    let request_total = codex_diagnostics
        .server_request_counts
        .iter()
        .map(|entry| entry.count)
        .sum::<u64>();
    let mut detail_lines = vec![
        format!(
            "wire_log {} // path {}",
            if codex_diagnostics.wire_log_enabled {
                "enabled"
            } else {
                "disabled"
            },
            codex_diagnostics.wire_log_path
        ),
        format!(
            "notifications {} methods // requests {} methods",
            codex_diagnostics.notification_counts.len(),
            codex_diagnostics.server_request_counts.len()
        ),
    ];
    if let Some(error) = codex_diagnostics.last_command_failure.as_deref() {
        detail_lines.push(format!(
            "command_failure {}",
            compact_preview_text(error, 48)
        ));
    }
    if let Some(error) = codex_diagnostics.last_snapshot_error.as_deref() {
        detail_lines.push(format!(
            "snapshot_error {}",
            compact_preview_text(error, 48)
        ));
    }
    InactivePanePreviewState {
        source_badge: "inactive codex".to_string(),
        load_state: codex_diagnostics.load_state,
        summary: format!(
            "{notification_total} notifications // {request_total} server requests // {} raw events",
            codex_diagnostics.raw_events.len()
        ),
        last_action: codex_diagnostics.last_action.clone(),
        last_error: codex_diagnostics.last_error.clone(),
        detail_lines,
    }
}

fn frame_debugger_inactive_preview_state(
    frame_debugger: &FrameDebuggerPaneState,
) -> InactivePanePreviewState {
    let summary = frame_debugger
        .rolling_fps
        .zip(frame_debugger.last_report.as_ref())
        .map(|(fps, report)| format!("{fps:.1} fps // {:.2} ms cpu", report.total_cpu_ms))
        .unwrap_or_else(|| "Waiting for first frame sample".to_string());
    let top_pane = frame_debugger
        .top_pane_paint_summaries(1)
        .into_iter()
        .next()
        .map(|entry| {
            format!(
                "pane {} [{}] {:.2}ms",
                entry.pane_title, entry.render_mode, entry.total_ms
            )
        })
        .unwrap_or_else(|| "pane timings warming".to_string());
    let top_runtime = frame_debugger
        .top_runtime_pump_summaries(1)
        .into_iter()
        .next()
        .map(|entry| format!("pump {} {:.2}ms", entry.operation, entry.total_ms))
        .unwrap_or_else(|| "runtime timings warming".to_string());
    let top_snapshot = frame_debugger
        .top_snapshot_timing_summaries(1)
        .into_iter()
        .next()
        .map(|entry| {
            format!(
                "snapshot {}:{} {:.2}ms",
                entry.subsystem, entry.phase, entry.total_ms
            )
        })
        .unwrap_or_else(|| "snapshot timings warming".to_string());
    InactivePanePreviewState {
        source_badge: "inactive perf".to_string(),
        load_state: frame_debugger.load_state,
        summary,
        last_action: frame_debugger.last_action.clone(),
        last_error: frame_debugger.last_error.clone(),
        detail_lines: vec![
            format!("redraw {}", frame_debugger.redraw_pressure.reason_summary()),
            format!(
                "samples {} // redraw_requests {}",
                frame_debugger.samples().len(),
                frame_debugger.redraw_requests
            ),
            top_pane,
            top_runtime,
            top_snapshot,
        ],
    }
}

fn log_stream_inactive_preview_state(log_stream: &LogStreamPaneState) -> InactivePanePreviewState {
    let recent_lines = log_stream.terminal.recent_lines(3);
    let mut detail_lines = if recent_lines.is_empty() {
        vec!["No runtime logs buffered yet".to_string()]
    } else {
        recent_lines
            .iter()
            .map(|line| compact_preview_text(line.text.as_str(), 64))
            .collect::<Vec<_>>()
    };
    detail_lines.push(format!(
        "buffered {} lines",
        log_stream.terminal.recent_lines(usize::MAX).len()
    ));
    InactivePanePreviewState {
        source_badge: "inactive logs".to_string(),
        load_state: PaneLoadState::Ready,
        summary: format!(
            "{} buffered runtime lines",
            log_stream.terminal.recent_lines(usize::MAX).len()
        ),
        last_action: None,
        last_error: None,
        detail_lines,
    }
}

fn buy_mode_payments_inactive_preview_state(
    buy_mode_enabled: bool,
    autopilot_chat: &AutopilotChatState,
    buy_mode_payments: &BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
    spark_wallet: &SparkPaneState,
) -> InactivePanePreviewState {
    let now = std::time::Instant::now();
    let panel = mission_control_buy_mode_panel_state(
        buy_mode_enabled,
        autopilot_chat,
        buy_mode_payments,
        network_requests,
        spark_wallet,
        now,
    );
    let mut detail_lines = crate::app_state::buy_mode_payments_status_lines(
        buy_mode_payments,
        network_requests,
        spark_wallet,
        now,
    );
    detail_lines.truncate(3);
    detail_lines.push(format!("fact rows {}", nip90_payment_facts.facts.len()));
    InactivePanePreviewState {
        source_badge: "inactive buy+facts".to_string(),
        load_state: if buy_mode_payments.last_error.is_some() {
            PaneLoadState::Error
        } else {
            PaneLoadState::Ready
        },
        summary: panel
            .as_ref()
            .map(|panel| panel.summary.clone())
            .unwrap_or_else(|| "Buy Mode is disabled for this session.".to_string()),
        last_action: buy_mode_payments.last_action.clone(),
        last_error: buy_mode_payments.last_error.clone(),
        detail_lines,
    }
}

fn nip90_sent_payments_inactive_preview_state(
    pane_state: &Nip90SentPaymentsPaneState,
    buyer_payment_attempts: &crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentAttemptLedgerState,
    relay_connections: &RelayConnectionsState,
) -> InactivePanePreviewState {
    match crate::panes::nip90_sent_payments::build_view(
        pane_state,
        buyer_payment_attempts,
        relay_connections,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
    ) {
        Ok(view) => InactivePanePreviewState {
            source_badge: "inactive sent-payments".to_string(),
            load_state: if pane_state.last_error.is_some() {
                PaneLoadState::Error
            } else {
                PaneLoadState::Ready
            },
            summary: format!(
                "{} // {} sats // relays {}",
                view.report.payment_count,
                format_sats_amount(view.report.total_sats_sent),
                view.connected_relay_count
            ),
            last_action: pane_state.last_action.clone(),
            last_error: pane_state.last_error.clone(),
            detail_lines: vec![
                format!("window {}", view.window_label),
                format!("requests {}", view.report.deduped_request_count),
                format!("degraded {}", view.report.degraded_binding_count),
            ],
        },
        Err(error) => InactivePanePreviewState {
            source_badge: "inactive sent-payments".to_string(),
            load_state: PaneLoadState::Error,
            summary: "window configuration error".to_string(),
            last_action: pane_state.last_action.clone(),
            last_error: Some(error),
            detail_lines: vec![format!("selected {}", pane_state.selected_window.label())],
        },
    }
}

fn data_market_inactive_preview_state(
    data_market: &DataMarketPaneState,
) -> InactivePanePreviewState {
    let summary = if data_market.has_snapshot() {
        format!(
            "{} assets // {} grants // {} deliveries",
            data_market.assets.len(),
            data_market.grants.len(),
            data_market.deliveries.len()
        )
    } else {
        "No Data Market snapshot loaded yet".to_string()
    };
    let refreshed = data_market
        .last_refreshed_at_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "never".to_string());
    InactivePanePreviewState {
        source_badge: "inactive kernel.data".to_string(),
        load_state: data_market.load_state,
        summary,
        last_action: data_market.last_action.clone(),
        last_error: data_market.last_error.clone(),
        detail_lines: vec![
            format!("revocations {}", data_market.revocations.len()),
            format!("refresh_ms {}", refreshed),
            format!("state {}", data_market.load_state.label()),
        ],
    }
}

fn data_seller_inactive_preview_state(
    data_seller: &DataSellerPaneState,
) -> InactivePanePreviewState {
    InactivePanePreviewState {
        source_badge: "inactive codex.data_seller".to_string(),
        load_state: data_seller.load_state,
        summary: "Conversational seller shell".to_string(),
        last_action: data_seller.last_action.clone(),
        last_error: data_seller.last_error.clone(),
        detail_lines: vec![
            format!(
                "preview {}",
                if data_seller.preview_enabled {
                    "armed"
                } else {
                    "blocked"
                }
            ),
            format!(
                "publish {}",
                if data_seller.publish_enabled {
                    "armed"
                } else {
                    "blocked"
                }
            ),
            format!(
                "posture {}",
                data_seller.active_draft.preview_posture.label()
            ),
            format!("session {}", data_seller.codex_session_phase.label()),
            format!(
                "blockers {}",
                data_seller.active_draft.readiness_blockers.len()
            ),
        ],
    }
}

fn buyer_race_matrix_inactive_preview_state(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> InactivePanePreviewState {
    let request = network_requests
        .submitted
        .iter()
        .find(|request| {
            !request.provider_observation_history.is_empty()
                || !request.provider_observations.is_empty()
                || !request.target_provider_pubkeys.is_empty()
        })
        .or_else(|| network_requests.submitted.first());

    let (summary, detail_lines) = if let Some(request) = request {
        let snapshot =
            crate::nip90_compute_flow::build_buyer_request_flow_snapshot(request, spark_wallet);
        (
            format!(
                "req {} // phase {} // next {}",
                compact_mission_control_id(request.request_id.as_str()),
                snapshot.phase.as_str(),
                snapshot.next_expected_event
            ),
            vec![
                format!(
                    "targets {} // history {}",
                    request.target_provider_pubkeys.len(),
                    snapshot.provider_observation_history.len()
                ),
                format!(
                    "selected {} // payable {}",
                    snapshot
                        .selected_provider_pubkey()
                        .map(compact_mission_control_id)
                        .unwrap_or_else(|| "none".to_string()),
                    snapshot
                        .payable_provider_pubkey
                        .as_deref()
                        .map(compact_mission_control_id)
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "result {} // invoice {}",
                    snapshot
                        .result_provider_pubkey()
                        .map(compact_mission_control_id)
                        .unwrap_or_else(|| "none".to_string()),
                    snapshot
                        .invoice_provider_pubkey()
                        .map(compact_mission_control_id)
                        .unwrap_or_else(|| "none".to_string())
                ),
            ],
        )
    } else {
        (
            "No active buyer race request observed".to_string(),
            vec![
                "Open Buyer Race Matrix on an in-flight request for full race detail.".to_string(),
            ],
        )
    };

    InactivePanePreviewState {
        source_badge: "inactive buy+race".to_string(),
        load_state: PaneLoadState::Ready,
        summary,
        last_action: None,
        last_error: None,
        detail_lines,
    }
}

fn seller_earnings_timeline_inactive_preview_state(
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
) -> InactivePanePreviewState {
    let seller_facts = nip90_payment_facts
        .facts
        .iter()
        .filter(|fact| {
            fact.seller_wallet_confirmed_at.is_some()
                || fact.seller_settlement_feedback_at.is_some()
                || fact.seller_payment_pointer.is_some()
        })
        .collect::<Vec<_>>();
    let settled_sats = seller_facts
        .iter()
        .filter(|fact| fact.seller_wallet_confirmed_at.is_some())
        .map(|fact| fact.amount_sats.unwrap_or_default())
        .sum::<u64>();
    let pending_sats = seller_facts
        .iter()
        .filter(|fact| fact.seller_wallet_confirmed_at.is_none())
        .map(|fact| fact.amount_sats.unwrap_or_default())
        .sum::<u64>();
    let latest_request = seller_facts
        .iter()
        .max_by_key(|fact| fact.latest_event_epoch_seconds().unwrap_or_default())
        .map(|fact| compact_mission_control_id(fact.request_id.as_str()))
        .unwrap_or_else(|| "none".to_string());
    InactivePanePreviewState {
        source_badge: "inactive payment facts".to_string(),
        load_state: nip90_payment_facts.load_state,
        summary: if seller_facts.is_empty() {
            "No seller settlements observed yet".to_string()
        } else {
            format!(
                "settled {} // pending {}",
                format_sats_amount(settled_sats),
                format_sats_amount(pending_sats)
            )
        },
        last_action: nip90_payment_facts.last_action.clone(),
        last_error: nip90_payment_facts.last_error.clone(),
        detail_lines: vec![
            format!("rows {} // latest {}", seller_facts.len(), latest_request),
            format!(
                "wallet confirmed {} // pending {}",
                seller_facts
                    .iter()
                    .filter(|fact| fact.seller_wallet_confirmed_at.is_some())
                    .count(),
                seller_facts
                    .iter()
                    .filter(|fact| fact.seller_wallet_confirmed_at.is_none())
                    .count()
            ),
        ],
    }
}

fn settlement_ladder_inactive_preview_state(
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
) -> InactivePanePreviewState {
    let latest = nip90_payment_facts
        .facts
        .iter()
        .max_by_key(|fact| fact.latest_event_epoch_seconds().unwrap_or_default());
    let (summary, detail_lines) = if let Some(fact) = latest {
        let lit_count = usize::from(fact.request_event_id.is_some())
            + usize::from(fact.result_event_id.is_some())
            + usize::from(fact.invoice_event_id.is_some())
            + usize::from(fact.buyer_payment_pointer.is_some())
            + usize::from(
                fact.seller_settlement_feedback_at.is_some()
                    || fact.seller_wallet_confirmed_at.is_some(),
            )
            + usize::from(fact.buyer_wallet_confirmed_at.is_some());
        (
            format!(
                "req {} // {lit_count}/6 settlement proofs",
                compact_mission_control_id(fact.request_id.as_str())
            ),
            vec![
                format!("status {}", fact.status.label()),
                format!("authority {}", fact.settlement_authority),
                format!(
                    "amount {}",
                    format_sats_amount(fact.amount_sats.unwrap_or_default())
                ),
            ],
        )
    } else {
        (
            "No settlement ladder facts yet".to_string(),
            vec!["Waiting for request, result, invoice, and wallet proof edges.".to_string()],
        )
    };
    InactivePanePreviewState {
        source_badge: "inactive payment facts".to_string(),
        load_state: nip90_payment_facts.load_state,
        summary,
        last_action: nip90_payment_facts.last_action.clone(),
        last_error: nip90_payment_facts.last_error.clone(),
        detail_lines,
    }
}

fn key_ledger_inactive_preview_state(
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
) -> InactivePanePreviewState {
    let total_sent_sats = nip90_payment_facts
        .facts
        .iter()
        .map(|fact| {
            fact.total_debit_sats
                .unwrap_or(fact.amount_sats.unwrap_or_default())
        })
        .sum::<u64>();
    let total_received_sats = nip90_payment_facts
        .facts
        .iter()
        .map(|fact| fact.amount_sats.unwrap_or_default())
        .sum::<u64>();
    let nostr_actor_count = nip90_payment_facts
        .actors
        .iter()
        .filter(|actor| {
            actor.namespace == crate::state::nip90_payment_facts::Nip90ActorNamespace::Nostr
        })
        .count();
    let lightning_actor_count = nip90_payment_facts
        .actors
        .iter()
        .filter(|actor| {
            actor.namespace
                == crate::state::nip90_payment_facts::Nip90ActorNamespace::LightningDestination
        })
        .count();
    InactivePanePreviewState {
        source_badge: "inactive payment facts".to_string(),
        load_state: nip90_payment_facts.load_state,
        summary: format!(
            "actors {} // recv {} // sent {}",
            nip90_payment_facts.actors.len(),
            format_sats_amount(total_received_sats),
            format_sats_amount(total_sent_sats)
        ),
        last_action: nip90_payment_facts.last_action.clone(),
        last_error: nip90_payment_facts.last_error.clone(),
        detail_lines: vec![
            format!("nostr actors {nostr_actor_count}"),
            format!("lightning destinations {lightning_actor_count}"),
            format!(
                "settlement failures {}",
                nip90_payment_facts
                    .facts
                    .iter()
                    .filter(|fact| {
                        fact.status
                            == crate::state::nip90_payment_facts::Nip90PaymentFactStatus::Failed
                    })
                    .count()
            ),
        ],
    }
}

fn settlement_atlas_inactive_preview_state(
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
) -> InactivePanePreviewState {
    let mut buyers = std::collections::BTreeSet::<String>::new();
    let mut providers = std::collections::BTreeSet::<String>::new();
    let mut edges = std::collections::BTreeSet::<(String, String)>::new();
    let mut total_volume_sats = 0u64;
    let mut degraded_fact_count = 0usize;
    for fact in &nip90_payment_facts.facts {
        total_volume_sats = total_volume_sats.saturating_add(fact.amount_sats.unwrap_or_default());
        let buyer = fact
            .buyer_nostr_pubkey
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let provider = fact
            .provider_nostr_pubkey
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        match (buyer, provider) {
            (Some(buyer), Some(provider)) => {
                buyers.insert(buyer.to_string());
                providers.insert(provider.to_string());
                edges.insert((buyer.to_string(), provider.to_string()));
            }
            _ => degraded_fact_count = degraded_fact_count.saturating_add(1),
        }
    }
    InactivePanePreviewState {
        source_badge: "inactive payment facts".to_string(),
        load_state: nip90_payment_facts.load_state,
        summary: format!(
            "buyers {} // providers {} // volume {}",
            buyers.len(),
            providers.len(),
            format_sats_amount(total_volume_sats)
        ),
        last_action: nip90_payment_facts.last_action.clone(),
        last_error: nip90_payment_facts.last_error.clone(),
        detail_lines: vec![
            format!("edges {}", edges.len()),
            format!("degraded hidden {degraded_fact_count}"),
            format!("facts {}", nip90_payment_facts.facts.len()),
        ],
    }
}

fn spark_replay_inactive_preview_state(
    spark_replay: &SparkReplayPaneState,
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
) -> InactivePanePreviewState {
    let step_count = spark_replay_pane::replay_step_count(nip90_payment_facts);
    InactivePanePreviewState {
        source_badge: "inactive replay".to_string(),
        load_state: nip90_payment_facts.load_state,
        summary: if step_count == 0 {
            "No payment replay steps available yet".to_string()
        } else {
            format!(
                "{} replay steps // cursor {}/{}",
                step_count,
                spark_replay.cursor_step.min(step_count.saturating_sub(1)) + 1,
                step_count
            )
        },
        last_action: spark_replay.last_action.clone(),
        last_error: spark_replay.last_error.clone(),
        detail_lines: vec![
            format!(
                "auto_follow {}",
                if spark_replay.auto_follow {
                    "on"
                } else {
                    "off"
                }
            ),
            format!(
                "request {}",
                spark_replay
                    .last_request_id
                    .as_deref()
                    .map(compact_mission_control_id)
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!("facts {}", nip90_payment_facts.facts.len()),
        ],
    }
}

fn relay_choreography_inactive_preview_state(
    relay_connections: &RelayConnectionsState,
    nip90_payment_facts: &Nip90PaymentFactLedgerState,
) -> InactivePanePreviewState {
    let connected = relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == crate::state::operations::RelayConnectionStatus::Connected)
        .count();
    let errored = relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == crate::state::operations::RelayConnectionStatus::Error)
        .count();
    let latest_request = nip90_payment_facts
        .facts
        .iter()
        .max_by_key(|fact| fact.latest_event_epoch_seconds().unwrap_or_default())
        .map(|fact| compact_mission_control_id(fact.request_id.as_str()))
        .unwrap_or_else(|| "none".to_string());
    InactivePanePreviewState {
        source_badge: "inactive relays".to_string(),
        load_state: relay_connections.load_state,
        summary: format!(
            "live relays {connected}/{} // relay hops {}",
            relay_connections.relays.len(),
            nip90_payment_facts.relay_hops.len()
        ),
        last_action: relay_connections.last_action.clone(),
        last_error: relay_connections
            .last_error
            .clone()
            .or_else(|| nip90_payment_facts.last_error.clone()),
        detail_lines: vec![
            format!("errored relays {errored}"),
            format!("facts {}", nip90_payment_facts.facts.len()),
            format!("latest request {latest_request}"),
        ],
    }
}

fn compact_preview_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "n/a".to_string();
    }
    if trimmed.chars().count() <= max_chars.max(8) {
        return trimmed.to_string();
    }
    let keep = max_chars.max(8);
    let head = (keep / 2).max(4);
    let tail = (keep / 3).max(4);
    let start = trimmed.chars().take(head).collect::<String>();
    let end = trimmed
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}..{end}")
}

fn paint_autopilot_chat_pane(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    codex_account: &CodexAccountPaneState,
    spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    chat_pane::paint(
        content_bounds,
        autopilot_chat,
        codex_account.account_summary.as_str(),
        spacetime_presence,
        chat_inputs,
        paint,
    );
}

fn paint_go_online_pane(
    content_bounds: Bounds,
    pane_is_active: bool,
    cursor_position: Point,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    buy_mode_enabled: bool,
    autopilot_chat: &AutopilotChatState,
    nostr_identity: Option<&nostr::NostrIdentity>,
    mission_control: &mut MissionControlPaneState,
    provider_control: &ProviderControlPaneState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    log_stream: &mut LogStreamPaneState,
    buy_mode: &BuyModePaymentsPaneState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    sa_lane: &crate::runtime_lanes::SaLaneSnapshot,
    skl_lane: &crate::runtime_lanes::SklLaneSnapshot,
    ac_lane: &crate::runtime_lanes::AcLaneSnapshot,
    backend_kernel_authority: bool,
    provider_blockers: &[ProviderBlocker],
    earnings_scoreboard: &EarningsScoreboardState,
    spark_wallet: &SparkPaneState,
    network_requests: &NetworkRequestsState,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    paint: &mut PaintContext,
) {
    let layout = mission_control_layout_for_mode(content_bounds, buy_mode_enabled);
    let now = std::time::Instant::now();
    let now_epoch_ms = mission_control_now_epoch_millis();
    let status_label = provider_runtime.mode.label().to_ascii_uppercase();
    let status_color = mission_control_mode_color(provider_runtime.mode);
    let wallet_status = if spark_wallet.balance_reconciling() {
        "RECONCILING"
    } else {
        match spark_wallet.network_status_label() {
            "connected" => "CONNECTED",
            "disconnected" => "DISCONNECTED",
            _ => "UNKNOWN",
        }
    };
    let preflight_value = if provider_blockers.is_empty() {
        "CLEAR".to_string()
    } else {
        format!("{} BLOCKER(S)", provider_blockers.len())
    };
    let rail_gap = 10.0;
    let rail_width = ((layout.status_row.size.width - rail_gap * 3.0) / 4.0).max(0.0);
    let alert_message = mission_control_alert_message(
        mission_control,
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
        provider_blockers,
        spark_wallet,
    );
    let alert_visible = mission_control.should_show_alert(alert_message.signature.as_str());
    let alert_dismiss_bounds = mission_control_alert_dismiss_button_bounds(content_bounds);

    paint
        .scene
        .draw_quad(Quad::new(content_bounds).with_background(mission_control_background_color()));
    paint_mission_control_status_cell(
        Bounds::new(
            layout.status_row.origin.x,
            layout.status_row.origin.y,
            rail_width,
            layout.status_row.size.height,
        ),
        "MODE",
        &status_label,
        status_color,
        12.0,
        paint,
    );
    paint_mission_control_status_cell(
        Bounds::new(
            layout.status_row.origin.x + rail_width + rail_gap,
            layout.status_row.origin.y,
            rail_width,
            layout.status_row.size.height,
        ),
        "BACKEND",
        &mission_control_backend_label(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        ),
        mission_control_cyan_color(),
        12.0,
        paint,
    );
    paint_mission_control_status_cell(
        Bounds::new(
            layout.status_row.origin.x + (rail_width + rail_gap) * 2.0,
            layout.status_row.origin.y,
            rail_width,
            layout.status_row.size.height,
        ),
        "WALLET",
        wallet_status,
        if wallet_status == "CONNECTED" {
            mission_control_green_color()
        } else if wallet_status == "RECONCILING" {
            mission_control_cyan_color()
        } else {
            mission_control_amber_color()
        },
        12.0,
        paint,
    );
    paint_mission_control_status_cell(
        Bounds::new(
            layout.status_row.origin.x + (rail_width + rail_gap) * 3.0,
            layout.status_row.origin.y,
            layout.status_row.size.width - rail_width * 3.0 - rail_gap * 3.0,
            layout.status_row.size.height,
        ),
        "PREFLIGHT",
        &preflight_value,
        if provider_blockers.is_empty() {
            mission_control_green_color()
        } else {
            mission_control_orange_color()
        },
        12.0,
        paint,
    );
    paint_mission_control_alert_band(
        layout.alert_band,
        alert_dismiss_bounds,
        &alert_message,
        alert_visible,
        paint,
    );

    paint_mission_control_section_panel(
        layout.sell_panel,
        "SELL COMPUTE",
        mission_control_green_color(),
        matches!(
            provider_runtime.mode,
            crate::app_state::ProviderMode::Offline
        ),
        paint,
    );
    paint_mission_control_section_panel(
        layout.earnings_panel,
        "EARNINGS",
        mission_control_green_color(),
        false,
        paint,
    );
    paint_mission_control_section_panel(
        layout.wallet_panel,
        "WALLET",
        mission_control_green_color(),
        false,
        paint,
    );
    let wallet_refresh_bounds = mission_control_wallet_refresh_button_bounds(content_bounds);
    let pointer_in_pane = pane_is_active && content_bounds.contains(cursor_position);
    let wallet_refresh_hovered = pointer_in_pane && wallet_refresh_bounds.contains(cursor_position);
    let wallet_refresh_clicked = mission_control.wallet_refresh_icon_click_feedback(now_epoch_ms);
    paint_mission_control_wallet_refresh_icon_button(
        wallet_refresh_bounds,
        mission_control_green_color(),
        wallet_refresh_hovered,
        wallet_refresh_clicked,
        paint,
    );
    paint_mission_control_section_panel(
        layout.actions_panel,
        "CONTROL",
        mission_control_orange_color(),
        false,
        paint,
    );
    paint_mission_control_section_panel(
        layout.active_jobs_panel,
        "ACTIVE JOBS",
        status_color,
        false,
        paint,
    );
    if buy_mode_enabled {
        paint_mission_control_section_panel(
            layout.buy_mode_panel,
            "BUY MODE",
            mission_control_cyan_color(),
            false,
            paint,
        );
    }
    paint_mission_control_section_panel(
        layout.load_funds_panel,
        "LOAD FUNDS",
        mission_control_cyan_color(),
        false,
        paint,
    );

    let toggle_bounds = go_online_toggle_button_bounds(content_bounds);
    let wants_online = matches!(
        provider_runtime.mode,
        crate::app_state::ProviderMode::Offline | crate::app_state::ProviderMode::Degraded
    );
    let go_online_enabled = !wants_online
        || mission_control_local_runtime_is_ready(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        );
    let toggle_label = if wants_online {
        "GO ONLINE"
    } else {
        "GO OFFLINE"
    };
    let primary_model_label = mission_control_primary_model_label(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    let backend_label = mission_control_backend_label(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    let load_status_label = mission_control_model_load_status(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    let control_label = provider_runtime
        .control_authority_label(backend_kernel_authority)
        .to_string();
    let sell_viewport = mission_control_sell_scroll_viewport_bounds(content_bounds);
    let sell_value_chunk_len = mission_control_value_chunk_len(layout.sell_panel);
    let mut sell_content_height = 0.0;
    for value in [
        provider_runtime.mode.label().to_string(),
        primary_model_label.clone(),
        backend_label.clone(),
        load_status_label.clone(),
        control_label.clone(),
        preflight_value.clone(),
    ] {
        sell_content_height +=
            mission_control_wrapped_row_height(value.as_str(), sell_value_chunk_len, true);
    }
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        sell_content_height += mission_control_wrapped_row_height(
            format!("{}s", provider_runtime.uptime_seconds(now)).as_str(),
            sell_value_chunk_len,
            false,
        );
    }
    if sa_lane.mode != crate::runtime_lanes::SaRunnerMode::Offline {
        sell_content_height +=
            mission_control_wrapped_row_height(sa_lane.mode.label(), sell_value_chunk_len, false);
    }
    if skl_lane.trust_tier != crate::runtime_lanes::SkillTrustTier::Unknown {
        sell_content_height += mission_control_wrapped_row_height(
            skl_lane.trust_tier.label(),
            sell_value_chunk_len,
            false,
        );
    }
    if ac_lane.credit_available {
        sell_content_height +=
            mission_control_wrapped_row_height("AVAILABLE", sell_value_chunk_len, false);
    }
    let sell_max_scroll =
        mission_control_max_scroll_for_viewport(sell_viewport, sell_content_height);
    let sell_scroll = mission_control.clamp_sell_scroll_offset(sell_max_scroll);
    paint_mission_control_go_online_button(
        toggle_bounds,
        toggle_label,
        go_online_enabled,
        status_color,
        paint,
    );

    paint.scene.push_clip(sell_viewport);
    let mut sell_y = sell_viewport.origin.y - sell_scroll;
    sell_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Mode",
        provider_runtime.mode.label(),
        sell_value_chunk_len,
        layout.sell_panel.size.width - 24.0,
        true,
    );
    sell_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Model",
        &primary_model_label,
        sell_value_chunk_len,
        layout.sell_panel.size.width - 24.0,
        true,
    );
    sell_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Backend",
        &backend_label,
        sell_value_chunk_len,
        layout.sell_panel.size.width - 24.0,
        true,
    );
    sell_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Load",
        &load_status_label,
        sell_value_chunk_len,
        layout.sell_panel.size.width - 24.0,
        true,
    );
    sell_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Control",
        &control_label,
        sell_value_chunk_len,
        layout.sell_panel.size.width - 24.0,
        true,
    );
    sell_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Preflight",
        &preflight_value,
        sell_value_chunk_len,
        layout.sell_panel.size.width - 24.0,
        true,
    );
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        sell_y = paint_wrapped_label_line_mission_control_label(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "Uptime",
            &format!("{}s", provider_runtime.uptime_seconds(now)),
            sell_value_chunk_len,
            layout.sell_panel.size.width - 24.0,
            false,
        );
    }
    if sa_lane.mode != crate::runtime_lanes::SaRunnerMode::Offline {
        sell_y = paint_wrapped_label_line_mission_control_label(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "Runner",
            sa_lane.mode.label(),
            sell_value_chunk_len,
            layout.sell_panel.size.width - 24.0,
            false,
        );
    }
    if skl_lane.trust_tier != crate::runtime_lanes::SkillTrustTier::Unknown {
        sell_y = paint_wrapped_label_line_mission_control_label(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "SKL Trust",
            skl_lane.trust_tier.label(),
            sell_value_chunk_len,
            layout.sell_panel.size.width - 24.0,
            false,
        );
    }
    if ac_lane.credit_available {
        let _ = paint_wrapped_label_line_mission_control_label(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "Credit",
            "AVAILABLE",
            sell_value_chunk_len,
            layout.sell_panel.size.width - 24.0,
            false,
        );
    }
    paint.scene.pop_clip();
    paint_mission_control_scrollbar_for_viewport(
        layout.sell_panel,
        sell_viewport,
        sell_content_height,
        sell_scroll,
        paint,
    );

    let earnings_clip = mission_control_section_clip_bounds(layout.earnings_panel);
    paint.scene.push_clip(earnings_clip);
    const MISSION_CONTROL_PANEL_FONT_SIZE: f32 = 12.0;
    let earnings_content_height = 41.0 + 41.0 + 40.0 + 6.0 + 54.0;
    let earnings_max_scroll =
        mission_control_section_max_scroll(layout.earnings_panel, earnings_content_height);
    let earnings_scroll = mission_control.clamp_earnings_scroll_offset(earnings_max_scroll);
    let mut earnings_y = mission_control_section_content_y(layout.earnings_panel) - earnings_scroll;
    let today_display = earnings_scoreboard_amount_display(
        earnings_scoreboard.load_state,
        format_mission_control_amount(earnings_scoreboard.sats_today),
    );
    let month_display = earnings_scoreboard_amount_display(
        earnings_scoreboard.load_state,
        format_mission_control_amount(earnings_scoreboard.sats_this_month),
    );
    let lifetime_display = earnings_scoreboard_amount_display(
        earnings_scoreboard.load_state,
        format_mission_control_amount(earnings_scoreboard.lifetime_sats),
    );
    earnings_y = paint_mission_control_amount_line(
        paint,
        layout.earnings_panel.origin.x + 12.0,
        earnings_y,
        "Today",
        &today_display,
        mission_control_green_color(),
        MISSION_CONTROL_PANEL_FONT_SIZE,
        layout.earnings_panel.size.width - 24.0,
        true,
    );
    earnings_y = paint_mission_control_amount_line(
        paint,
        layout.earnings_panel.origin.x + 12.0,
        earnings_y,
        "This Month",
        &month_display,
        mission_control_text_color(),
        MISSION_CONTROL_PANEL_FONT_SIZE,
        layout.earnings_panel.size.width - 24.0,
        true,
    );
    let _ = paint_mission_control_amount_line(
        paint,
        layout.earnings_panel.origin.x + 12.0,
        earnings_y,
        "All Time",
        &lifetime_display,
        mission_control_cyan_color(),
        MISSION_CONTROL_PANEL_FONT_SIZE,
        layout.earnings_panel.size.width - 24.0,
        false,
    );
    paint.scene.pop_clip();
    paint_mission_control_section_scrollbar(
        layout.earnings_panel,
        earnings_content_height,
        earnings_scroll,
        paint,
    );

    let wallet_clip = mission_control_section_clip_bounds(layout.wallet_panel);
    paint.scene.push_clip(wallet_clip);
    let now_epoch_seconds = mission_control_now_epoch_seconds();
    let wallet_pending_delta_sats = crate::spark_wallet::pending_wallet_delta_sats(
        &spark_wallet.recent_payments,
        now_epoch_seconds,
    );
    let current_wallet_total_sats = spark_wallet
        .balance
        .as_ref()
        .map(|balance| balance.total_sats());
    let wallet_display_balance_sats = mission_control.mission_control_wallet_display_balance_sats(
        current_wallet_total_sats,
        wallet_pending_delta_sats,
        now_epoch_seconds,
    );
    let wallet_balance = spark_wallet
        .balance
        .as_ref()
        .and(wallet_display_balance_sats)
        .map(format_mission_control_amount)
        .unwrap_or_else(|| "LOADING".to_string());
    let wallet_pending = if spark_wallet.balance.is_some() {
        crate::bitcoin_display::format_mission_control_signed_amount(wallet_pending_delta_sats)
    } else {
        "LOADING".to_string()
    };
    let wallet_address = spark_wallet
        .spark_address
        .as_deref()
        .or(spark_wallet.bitcoin_address.as_deref())
        .map(mask_secret)
        .unwrap_or_else(|| "NOT GENERATED".to_string());
    let wallet_network = spark_wallet.network_name().to_ascii_uppercase();
    let wallet_value_chunk_len = mission_control_value_chunk_len(layout.wallet_panel);
    let wallet_content_height = 41.0
        + 39.0
        + mission_control_wrapped_row_height(&wallet_network, wallet_value_chunk_len, true)
        + mission_control_wrapped_row_height(wallet_status, wallet_value_chunk_len, true)
        + mission_control_wrapped_row_height(&wallet_address, wallet_value_chunk_len, false);
    let wallet_max_scroll =
        mission_control_section_max_scroll(layout.wallet_panel, wallet_content_height);
    let wallet_scroll = mission_control.clamp_wallet_scroll_offset(wallet_max_scroll);
    let mut wallet_y = mission_control_section_content_y(layout.wallet_panel) - wallet_scroll;
    wallet_y = paint_mission_control_amount_line(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Balance (₿)",
        &wallet_balance,
        mission_control_green_color(),
        MISSION_CONTROL_PANEL_FONT_SIZE,
        layout.wallet_panel.size.width - 24.0,
        true,
    );
    wallet_y = paint_mission_control_amount_line(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Pending (₿)",
        &wallet_pending,
        if wallet_pending_delta_sats == 0 {
            mission_control_muted_color()
        } else {
            theme::status::WARNING
        },
        MISSION_CONTROL_PANEL_FONT_SIZE,
        layout.wallet_panel.size.width - 24.0,
        true,
    );
    wallet_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Network",
        &wallet_network,
        wallet_value_chunk_len,
        layout.wallet_panel.size.width - 24.0,
        true,
    );
    wallet_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Status",
        wallet_status,
        wallet_value_chunk_len,
        layout.wallet_panel.size.width - 24.0,
        true,
    );
    let _ = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Target",
        &wallet_address,
        wallet_value_chunk_len,
        layout.wallet_panel.size.width - 24.0,
        false,
    );
    paint.scene.pop_clip();
    paint_mission_control_section_scrollbar(
        layout.wallet_panel,
        wallet_content_height,
        wallet_scroll,
        paint,
    );

    let download_bounds = mission_control_local_model_button_bounds(content_bounds);
    if mission_control_show_local_model_button(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    ) {
        let download_label = mission_control_local_model_button_label(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        );
        paint_mission_control_command_button(
            download_bounds,
            &download_label,
            if mission_control_local_action_enabled(
                desktop_shell_mode,
                provider_runtime,
                local_inference_runtime,
            ) {
                mission_control_orange_color()
            } else {
                mission_control_muted_color()
            },
            mission_control_local_action_enabled(
                desktop_shell_mode,
                provider_runtime,
                local_inference_runtime,
            ),
            paint,
        );
    }
    if mission_control_local_fm_test_button_visible(desktop_shell_mode, local_inference_runtime) {
        let test_bounds = mission_control_local_fm_test_button_bounds(content_bounds);
        let test_enabled = mission_control_local_fm_test_enabled(
            provider_control,
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        );
        paint_mission_control_command_button(
            test_bounds,
            &mission_control_local_fm_test_button_label(
                provider_control,
                provider_runtime,
                local_inference_runtime,
                desktop_shell_mode,
            ),
            if test_enabled {
                mission_control_cyan_color()
            } else {
                mission_control_muted_color()
            },
            test_enabled,
            paint,
        );
    }

    let load_funds_viewport =
        mission_control_load_funds_scroll_viewport_bounds(content_bounds, buy_mode_enabled);
    let lightning_state = spark_wallet.last_invoice_state(mission_control_now_epoch_seconds());
    let lightning_target_text = match lightning_state {
        SparkInvoiceState::Ready => spark_wallet
            .last_invoice
            .as_deref()
            .unwrap_or("Generate a Lightning invoice to fund this wallet.")
            .to_string(),
        SparkInvoiceState::Expired => {
            "Previous Lightning invoice expired. Generate a fresh receive target.".to_string()
        }
        SparkInvoiceState::Empty => "Generate a Lightning invoice to fund this wallet.".to_string(),
    };
    let recent_receive_history = mission_control_recent_receive_history(spark_wallet);
    let load_funds_measurement_layout =
        mission_control_load_funds_layout_with_scroll(content_bounds, buy_mode_enabled, 0.0);
    let load_funds_content_height = mission_control_load_funds_content_height(
        &load_funds_measurement_layout,
        &wallet_network,
        wallet_status,
        mission_control_lightning_receive_state_label(lightning_state),
        &lightning_target_text,
        &recent_receive_history,
    );
    let load_funds_max_scroll =
        mission_control_max_scroll_for_viewport(load_funds_viewport, load_funds_content_height);
    let load_funds_scroll = mission_control.clamp_load_funds_scroll_offset(load_funds_max_scroll);
    let load_funds_layout = mission_control_load_funds_layout_with_scroll(
        content_bounds,
        buy_mode_enabled,
        load_funds_scroll,
    );
    let lightning_amount_valid = mission_control
        .load_funds_amount_sats
        .get_value()
        .trim()
        .parse::<u64>()
        .ok()
        .is_some_and(|value| value > 0);
    paint.scene.push_clip(load_funds_viewport);
    let mut lightning_sats_label = paint.text.layout_mono(
        "LIGHTNING SATS (₿)",
        Point::ZERO,
        MISSION_CONTROL_PANEL_FONT_SIZE,
        mission_control_muted_color(),
    );
    let lightning_sats_label_bounds = lightning_sats_label.bounds();
    let lightning_sats_label_bottom = load_funds_layout.amount_input.origin.y - 8.0;
    lightning_sats_label.origin = Point::new(
        load_funds_layout.amount_input.origin.x - lightning_sats_label_bounds.origin.x,
        lightning_sats_label_bottom
            - lightning_sats_label_bounds.size.height
            - lightning_sats_label_bounds.origin.y,
    );
    paint.scene.draw_text(lightning_sats_label);
    mission_control
        .load_funds_amount_sats
        .set_max_width(load_funds_layout.amount_input.size.width);
    mission_control
        .load_funds_amount_sats
        .paint(load_funds_layout.amount_input, paint);
    paint_mission_control_command_button(
        load_funds_layout.lightning_button,
        "LIGHTNING RECEIVE",
        mission_control_green_color(),
        lightning_amount_valid,
        paint,
    );
    paint_mission_control_command_button(
        load_funds_layout.copy_lightning_button,
        "COPY LIGHTNING",
        mission_control_cyan_color(),
        lightning_state == SparkInvoiceState::Ready,
        paint,
    );
    let send_invoice_bounds = mission_control_send_invoice_input_bounds_for_scroll(
        content_bounds,
        buy_mode_enabled,
        load_funds_scroll,
    );
    let mut lightning_withdraw_label = paint.text.layout_mono(
        "LIGHTNING WITHDRAW",
        Point::ZERO,
        MISSION_CONTROL_PANEL_FONT_SIZE,
        mission_control_muted_color(),
    );
    let lightning_withdraw_label_bounds = lightning_withdraw_label.bounds();
    let lightning_withdraw_label_bottom = send_invoice_bounds.origin.y - 8.0;
    lightning_withdraw_label.origin = Point::new(
        send_invoice_bounds.origin.x - lightning_withdraw_label_bounds.origin.x,
        lightning_withdraw_label_bottom
            - lightning_withdraw_label_bounds.size.height
            - lightning_withdraw_label_bounds.origin.y,
    );
    paint.scene.draw_text(lightning_withdraw_label);
    mission_control
        .send_invoice
        .set_max_width(send_invoice_bounds.size.width);
    mission_control
        .send_invoice
        .paint(send_invoice_bounds, paint);
    paint_mission_control_command_button(
        mission_control_send_lightning_button_bounds_for_scroll(
            content_bounds,
            buy_mode_enabled,
            load_funds_scroll,
        ),
        "LIGHTNING WITHDRAW",
        mission_control_orange_color(),
        !mission_control.send_invoice.get_value().trim().is_empty(),
        paint,
    );
    paint_mission_control_command_button(
        mission_control_copy_seed_button_bounds_for_scroll(
            content_bounds,
            buy_mode_enabled,
            load_funds_scroll,
        ),
        "COPY SEED",
        mission_control_cyan_color(),
        nostr_identity.is_some_and(|identity| !identity.mnemonic.trim().is_empty()),
        paint,
    );
    let load_funds_value_chunk_len =
        mission_control_value_chunk_len(load_funds_layout.details_column);
    let load_funds_body_chunk_len =
        mission_control_body_chunk_len(load_funds_layout.details_column);
    let mut load_funds_y = load_funds_layout.details_column.origin.y;
    load_funds_y = paint_wrapped_label_line_mission_control_label(
        paint,
        load_funds_layout.details_column.origin.x,
        load_funds_y,
        "Network",
        &wallet_network,
        load_funds_value_chunk_len,
        load_funds_layout.details_column.size.width,
        true,
    );
    load_funds_y = paint_wrapped_label_line_mission_control_label(
        paint,
        load_funds_layout.details_column.origin.x,
        load_funds_y,
        "Connection",
        wallet_status,
        load_funds_value_chunk_len,
        load_funds_layout.details_column.size.width,
        true,
    );
    load_funds_y = paint_wrapped_label_line_mission_control_label(
        paint,
        load_funds_layout.details_column.origin.x,
        load_funds_y,
        "Lightning",
        mission_control_lightning_receive_state_label(lightning_state),
        load_funds_value_chunk_len,
        load_funds_layout.details_column.size.width,
        true,
    );
    load_funds_y = paint_mission_control_body_block(
        paint,
        load_funds_layout.details_column.origin.x,
        load_funds_y,
        "Lightning target",
        &lightning_target_text,
        load_funds_body_chunk_len,
        load_funds_layout.details_column.size.width,
        true,
    );
    let _ = paint_mission_control_body_block(
        paint,
        load_funds_layout.details_column.origin.x,
        load_funds_y,
        "Recent receives",
        &recent_receive_history,
        load_funds_body_chunk_len,
        load_funds_layout.details_column.size.width,
        false,
    );
    paint.scene.pop_clip();
    paint_mission_control_scrollbar_for_viewport(
        layout.load_funds_panel,
        load_funds_viewport,
        load_funds_content_height,
        load_funds_scroll,
        paint,
    );

    let active_clip = mission_control_section_clip_bounds(layout.active_jobs_panel);
    paint.scene.push_clip(active_clip);
    let active_panel_state = mission_control_active_jobs_panel_state(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
        job_inbox,
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
    );
    let active_state = match active_panel_state.headline.as_str() {
        "STANDBY" => ("STANDBY", mission_control_orange_color()),
        "FAULT" => ("FAULT", mission_control_red_color()),
        "ACTIVE" => ("ACTIVE", mission_control_green_color()),
        _ => ("SCANNING", mission_control_cyan_color()),
    };
    let active_content_height = 44.0 + active_panel_state.lines.len() as f32 * 17.0;
    let active_jobs_max_scroll =
        mission_control_section_max_scroll(layout.active_jobs_panel, active_content_height);
    let active_jobs_scroll =
        mission_control.clamp_active_jobs_scroll_offset(active_jobs_max_scroll);
    let active_content_y =
        mission_control_section_content_y(layout.active_jobs_panel) - active_jobs_scroll;
    paint.scene.draw_text(paint.text.layout_mono(
        active_state.0,
        Point::new(layout.active_jobs_panel.origin.x + 12.0, active_content_y),
        22.0,
        active_state.1,
    ));
    for (index, line) in active_panel_state.lines.iter().enumerate() {
        paint.scene.draw_text(paint.text.layout_mono(
            line,
            Point::new(
                layout.active_jobs_panel.origin.x + 12.0,
                active_content_y + 30.0 + index as f32 * 17.0,
            ),
            10.0,
            if index == 2 && active_job.job.is_some() {
                mission_control_green_color()
            } else {
                mission_control_text_color()
            },
        ));
    }
    paint.scene.pop_clip();
    paint_mission_control_section_scrollbar(
        layout.active_jobs_panel,
        active_content_height,
        active_jobs_scroll,
        paint,
    );

    if buy_mode_enabled {
        paint_mission_control_buy_mode_panel(
            content_bounds,
            layout.buy_mode_panel,
            autopilot_chat,
            buy_mode,
            network_requests,
            spark_wallet,
            now,
            paint,
        );
    }

    paint_mission_control_section_panel(
        layout.log_stream,
        "LOG STREAM",
        mission_control_orange_color(),
        false,
        paint,
    );
    let log_copy_bounds =
        mission_control_copy_log_stream_button_bounds(content_bounds, buy_mode_enabled);
    let log_copy_hovered = pointer_in_pane && log_copy_bounds.contains(cursor_position);
    let log_copy_clicked = log_stream.copy_button_click_feedback(now_epoch_ms);
    paint_mission_control_log_copy_icon_button(
        log_copy_bounds,
        mission_control_orange_color(),
        log_copy_hovered,
        log_copy_clicked,
        paint,
    );
    let log_body_bounds = mission_control_section_scroll_viewport_bounds(layout.log_stream);
    paint.scene.draw_quad(
        Quad::new(log_body_bounds)
            .with_background(mission_control_background_color().with_alpha(0.75))
            .with_corner_radius(4.0),
    );
    log_stream.terminal.set_title("");
    log_stream.terminal.paint(log_body_bounds, paint);
}

pub(crate) fn paint_mission_control_section_panel(
    bounds: Bounds,
    title: &str,
    accent: Hsla,
    show_moving_header_bar: bool,
    paint: &mut PaintContext,
) {
    let width = bounds.size.width.max(0.0);
    let height = bounds.size.height.max(0.0);
    if width <= 1.0 || height <= 1.0 {
        return;
    }
    let anim_t = mission_control_anim_seconds_f64();
    let pulse_phase =
        (anim_t * 5.0 + bounds.origin.x as f64 * 0.015).rem_euclid(std::f64::consts::TAU);
    let pulse = ((pulse_phase as f32).sin() * 0.5) + 0.5;

    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(mission_control_panel_color())
            .with_border(mission_control_panel_border_color(), 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x - 1.0,
            bounds.origin.y - 1.0,
            bounds.size.width + 2.0,
            bounds.size.height + 2.0,
        ))
        .with_border(accent.with_alpha(0.04 + pulse * 0.06), 1.0)
        .with_corner_radius(7.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            5.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.74 + pulse * 0.18))
        .with_corner_radius(6.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 5.0,
            bounds.origin.y,
            (bounds.size.width - 5.0).max(0.0),
            MISSION_CONTROL_SECTION_HEADER_HEIGHT,
        ))
        .with_background(mission_control_panel_header_color().with_alpha(0.95)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 5.0,
            bounds.origin.y,
            (bounds.size.width - 5.0).max(0.0),
            1.0,
        ))
        .with_background(accent.with_alpha(0.35)),
    );
    if show_moving_header_bar {
        let rail_top = bounds.origin.y + 2.0;
        let rail_bottom = bounds.max_y() - 2.0;
        let rail_height = (rail_bottom - rail_top).max(1.0);
        let shimmer_height = 26.0_f32.min(rail_height);
        let travel = (rail_height - shimmer_height).max(0.0);
        let cycle_seconds = 1.35_f64;
        let phase = (anim_t / cycle_seconds) * std::f64::consts::TAU;
        let ease = (0.5 - 0.5 * phase.cos()) as f32;
        let shimmer_top = rail_top + travel * ease;
        let shimmer_bottom = (shimmer_top + shimmer_height).min(rail_bottom);
        let visible_height = (shimmer_bottom - shimmer_top).max(0.0);
        if visible_height > 0.5 {
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + 1.0,
                    shimmer_top,
                    3.0,
                    visible_height,
                ))
                .with_background(Hsla::from_hex(0x1F8A44).with_alpha(0.92))
                .with_corner_radius(2.0),
            );
        }
    }

    if !title.is_empty() {
        let marker_origin = Point::new(bounds.origin.x + 14.0, bounds.origin.y + 8.0);
        let marker = paint.text.layout_mono("\\\\", marker_origin, 10.0, accent);
        let marker_width = marker.bounds().size.width;
        paint.scene.draw_text(marker);
        paint.scene.draw_text(paint.text.layout_mono(
            title,
            Point::new(marker_origin.x + marker_width + 6.0, marker_origin.y),
            10.0,
            mission_control_text_color(),
        ));
    }
}

fn paint_mission_control_status_cell(
    bounds: Bounds,
    label: &str,
    value: &str,
    value_color: Hsla,
    value_font_size: f32,
    paint: &mut PaintContext,
) {
    let anim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    let blink = ((anim_t * 9.6 + bounds.origin.x * 0.04).sin() * 0.5) + 0.5;
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(mission_control_panel_color())
            .with_border(mission_control_panel_border_color(), 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            18.0,
        ))
        .with_background(mission_control_panel_header_color().with_alpha(0.95)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 8.0,
            bounds.origin.y + 5.0,
            10.0,
            10.0,
        ))
        .with_background(value_color.with_alpha(0.10 + blink * 0.12))
        .with_corner_radius(5.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + 7.0,
            6.0,
            6.0,
        ))
        .with_background(value_color.with_alpha(0.55 + blink * 0.45))
        .with_corner_radius(3.0),
    );
    let label_y = bounds.origin.y + 4.0;
    let value_area_y = bounds.origin.y + 18.0;
    let value_area_h = (bounds.size.height - 18.0).max(0.0);
    let mut value_run = paint
        .text
        .layout_mono(value, Point::ZERO, value_font_size, value_color);
    let value_bounds = value_run.bounds();
    let value_y = value_area_y + ((value_area_h - value_bounds.size.height).max(0.0) * 0.5)
        - value_bounds.origin.y;
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 22.0, label_y),
        9.0,
        mission_control_muted_color(),
    ));
    value_run.origin = Point::new(bounds.origin.x + 14.0 - value_bounds.origin.x, value_y);
    paint.scene.draw_text(value_run);
}

fn paint_mission_control_alert_band(
    bounds: Bounds,
    dismiss_button_bounds: Bounds,
    alert: &MissionControlAlertDescriptor,
    show_alert: bool,
    paint: &mut PaintContext,
) {
    let anim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    let accent = if show_alert {
        alert.accent
    } else {
        mission_control_muted_color()
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(if show_alert { 0.10 } else { 0.06 }))
            .with_border(accent.with_alpha(if show_alert { 0.50 } else { 0.24 }), 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            3.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.95))
        .with_corner_radius(4.0),
    );
    if show_alert {
        paint.scene.draw_text(paint.text.layout_mono(
            "!",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 5.0),
            12.0,
            accent,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            alert.text.as_str(),
            Point::new(bounds.origin.x + 28.0, bounds.origin.y + 8.0),
            11.0,
            mission_control_text_color(),
        ));
        paint.scene.draw_quad(
            Quad::new(dismiss_button_bounds)
                .with_background(accent.with_alpha(0.08))
                .with_border(accent.with_alpha(0.28), 1.0)
                .with_corner_radius(3.0),
        );
        let mut dismiss_run =
            paint
                .text
                .layout_mono("X", Point::ZERO, 11.0, accent.with_alpha(0.9));
        let dismiss_run_bounds = dismiss_run.bounds();
        dismiss_run.origin = Point::new(
            dismiss_button_bounds.origin.x
                + ((dismiss_button_bounds.size.width - dismiss_run_bounds.size.width).max(0.0)
                    * 0.5)
                - dismiss_run_bounds.origin.x,
            dismiss_button_bounds.origin.y
                + ((dismiss_button_bounds.size.height - dismiss_run_bounds.size.height).max(0.0)
                    * 0.5)
                - dismiss_run_bounds.origin.y,
        );
        paint.scene.draw_text(dismiss_run);
    } else {
        paint.scene.draw_text(paint.text.layout_mono(
            "ALERT DISMISSED // NEXT STATE CHANGE REOPENS THIS BAND",
            Point::new(bounds.origin.x + 16.0, bounds.origin.y + 8.0),
            10.0,
            mission_control_muted_color(),
        ));
    }
    let sweep_track = (bounds.size.width - 52.0).max(1.0);
    let sweep = (anim_t * 520.0 + bounds.origin.x * 0.08).rem_euclid(sweep_track);
    let sweep_x = bounds.origin.x + 24.0 + sweep;
    let sweep_width = 16.0_f32.min((bounds.max_x() - 16.0 - sweep_x).max(0.0));
    if show_alert && sweep_width > 0.5 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(sweep_x, bounds.max_y() - 2.0, sweep_width, 1.0))
                .with_background(accent.with_alpha(0.30)),
        );
    }
    paint.scene.draw_text(paint.text.layout_mono(
        mission_control_truth_legend(),
        Point::new(bounds.origin.x + 16.0, bounds.origin.y + 22.0),
        9.0,
        mission_control_muted_color(),
    ));
}

fn mission_control_truth_legend() -> &'static str {
    "LEGEND // PROV=SELECTED PROVIDER // WORK=MARKET FLOW // PAY=WALLET FLOW // NEXT=EXPECTED EVENT"
}

struct MissionControlAlertDescriptor {
    signature: String,
    text: String,
    accent: Hsla,
}

pub(crate) fn mission_control_current_alert_signature(
    mission_control: &MissionControlPaneState,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    spark_wallet: &SparkPaneState,
) -> String {
    mission_control_alert_message(
        mission_control,
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
        provider_blockers,
        spark_wallet,
    )
    .signature
}

fn mission_control_alert_message(
    mission_control: &MissionControlPaneState,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    provider_blockers: &[ProviderBlocker],
    spark_wallet: &SparkPaneState,
) -> MissionControlAlertDescriptor {
    if let Some(error) = mission_control.last_error.as_deref().map(str::trim)
        && !error.is_empty()
    {
        let text = format!("ALERT // {}", error.to_ascii_uppercase());
        return MissionControlAlertDescriptor {
            signature: text.clone(),
            text,
            accent: mission_control_red_color(),
        };
    }

    if let Some(blocker) = provider_blockers.first().copied() {
        let text = format!(
            "PREFLIGHT // {}",
            mission_control_blocker_detail(blocker, spark_wallet, provider_runtime)
                .to_ascii_uppercase()
        );
        return MissionControlAlertDescriptor {
            signature: text.clone(),
            text,
            accent: mission_control_orange_color(),
        };
    }

    let load_hint = mission_control_go_online_hint(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    if !load_hint.trim().is_empty() {
        let text = format!("READY PATH // {}", load_hint.to_ascii_uppercase());
        return MissionControlAlertDescriptor {
            signature: text.clone(),
            text,
            accent: mission_control_amber_color(),
        };
    }

    if let Some(action) = mission_control.last_action.as_deref().map(str::trim)
        && !action.is_empty()
    {
        let text = format!("MISSION // {}", action.to_ascii_uppercase());
        return MissionControlAlertDescriptor {
            signature: text.clone(),
            text,
            accent: mission_control_cyan_color(),
        };
    }

    let text = "MISSION // APPLE FM EARN LOOP ARMED".to_string();
    MissionControlAlertDescriptor {
        signature: text.clone(),
        text,
        accent: mission_control_green_color(),
    }
}

fn paint_mission_control_buy_mode_panel(
    content_bounds: Bounds,
    panel_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    buy_mode: &BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    now: std::time::Instant,
    paint: &mut PaintContext,
) {
    let Some(panel_state) = mission_control_buy_mode_panel_state(
        true,
        autopilot_chat,
        buy_mode,
        network_requests,
        spark_wallet,
        now,
    ) else {
        return;
    };
    let clip = mission_control_section_clip_bounds(panel_bounds);
    paint.scene.push_clip(clip);

    let primary_button_bounds = mission_control_buy_mode_button_bounds(content_bounds, true);
    let history_button_bounds =
        mission_control_buy_mode_history_button_bounds(content_bounds, true);
    let content_y = mission_control_section_content_y(panel_bounds);
    let extra_height = (panel_bounds.size.height - 120.0).max(0.0);
    let summary_to_cells_gap = (14.0 + extra_height * 0.12).clamp(14.0, 30.0);
    let cell_height = (34.0 + extra_height * 0.10).clamp(34.0, 44.0);
    let min_cells_to_buttons_gap = 18.0;
    let max_cell_y = (primary_button_bounds.origin.y - min_cells_to_buttons_gap - cell_height)
        .max(content_y + 14.0);
    paint.scene.draw_text(paint.text.layout_mono(
        panel_state.summary.as_str(),
        Point::new(panel_bounds.origin.x + 12.0, content_y),
        11.0,
        mission_control_text_color(),
    ));

    let cell_gap = 8.0;
    let cell_width = ((panel_bounds.size.width - 24.0 - cell_gap * 4.0) / 5.0).max(0.0);
    let cell_y = (content_y + summary_to_cells_gap).min(max_cell_y);
    let values = [
        ("MODE", panel_state.mode.clone()),
        ("NEXT", panel_state.next.clone()),
        ("PROV", panel_state.provider.clone()),
        ("WORK", panel_state.work.clone()),
        ("PAY", panel_state.payment.clone()),
    ];
    for (index, (label, value)) in values.iter().enumerate() {
        let x = panel_bounds.origin.x + 12.0 + index as f32 * (cell_width + cell_gap);
        paint_mission_control_status_cell(
            Bounds::new(x, cell_y, cell_width, cell_height),
            label,
            value.as_str(),
            mission_control_cyan_color(),
            12.0,
            paint,
        );
    }
    paint.scene.pop_clip();

    paint_mission_control_command_button(
        primary_button_bounds,
        panel_state.button_label.as_str(),
        if panel_state.button_enabled {
            if panel_state.button_active {
                mission_control_green_color()
            } else {
                mission_control_cyan_color()
            }
        } else {
            mission_control_muted_color()
        },
        panel_state.button_enabled,
        paint,
    );
    paint_mission_control_command_button(
        history_button_bounds,
        "PAYMENT HISTORY",
        mission_control_cyan_color(),
        true,
        paint,
    );
}

type MissionControlActiveJobsPanelState = earnings_jobs_pane::MissionControlActiveJobsPanelState;

fn mission_control_active_jobs_panel_state(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
) -> MissionControlActiveJobsPanelState {
    earnings_jobs_pane::active_jobs_panel_state(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
        job_inbox,
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
    )
}

fn active_job_stage_display(
    stage: JobLifecycleStage,
    phase: crate::nip90_compute_flow::Nip90FlowPhase,
) -> String {
    match phase {
        crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment => {
            "delivered (awaiting buyer payment)".to_string()
        }
        crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet => {
            "delivered (seller settled / buyer local wallet pending)".to_string()
        }
        crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment => {
            "delivered (preparing buyer invoice)".to_string()
        }
        crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid => {
            "delivered-unpaid (buyer never paid)".to_string()
        }
        _ => stage.label().to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissionControlBuyModePanelState {
    pub(crate) summary: String,
    pub(crate) mode: String,
    pub(crate) next: String,
    pub(crate) provider: String,
    pub(crate) work: String,
    pub(crate) payment: String,
    pub(crate) button_label: String,
    pub(crate) button_active: bool,
    pub(crate) button_enabled: bool,
}

pub(crate) fn mission_control_buy_mode_panel_state(
    buy_mode_enabled: bool,
    autopilot_chat: &AutopilotChatState,
    buy_mode: &BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    _now: std::time::Instant,
) -> Option<MissionControlBuyModePanelState> {
    if !buy_mode_enabled {
        return None;
    }

    let request = network_requests
        .latest_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE);
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let target_selection = autopilot_chat.select_autopilot_buy_mode_target(now_epoch_seconds);
    let request_snapshot = request.map(|request| {
        crate::nip90_compute_flow::build_buyer_request_flow_snapshot(request, spark_wallet)
    });
    let block_reason = crate::app_state::mission_control_buy_mode_start_block_reason(spark_wallet);
    let blocked_while_idle = request.is_none() && block_reason.is_some();
    let next = if let Some(snapshot) = request_snapshot.as_ref() {
        snapshot.next_expected_event.clone()
    } else if !buy_mode.buy_mode_loop_enabled {
        "off".to_string()
    } else if network_requests
        .has_in_flight_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
    {
        "in-flight".to_string()
    } else if block_reason.is_some() {
        "blocked".to_string()
    } else if target_selection.selected_peer_pubkey.is_some() {
        "dispatch-ready".to_string()
    } else if let Some(blocked_reason_code) = target_selection.blocked_reason_code.as_deref() {
        match blocked_reason_code {
            crate::autopilot_peer_roster::AUTOPILOT_BUY_MODE_TARGET_BLOCK_WAITING_FOR_MAIN_CHANNEL => {
                "waiting-channel".to_string()
            }
            crate::autopilot_peer_roster::AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_PEERS_OBSERVED
            | crate::autopilot_peer_roster::AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_ELIGIBLE_PEERS => {
                "waiting-peer".to_string()
            }
            _ => "blocked".to_string(),
        }
    } else {
        "armed".to_string()
    };
    let provider = request_snapshot
        .as_ref()
        .map(crate::nip90_compute_flow::BuyerRequestFlowSnapshot::provider_label)
        .unwrap_or_else(|| {
            target_selection
                .selected_peer_pubkey
                .as_deref()
                .map(compact_mission_control_id)
                .unwrap_or_else(|| "none".to_string())
        });
    let work = request_snapshot
        .as_ref()
        .map(crate::nip90_compute_flow::BuyerRequestFlowSnapshot::work_label)
        .unwrap_or_else(|| {
            if target_selection.selected_peer_pubkey.is_some() {
                format!(
                    "{}/{}",
                    target_selection.eligible_peer_count, target_selection.observed_peer_count
                )
            } else if target_selection.blocked_reason_code.is_some() {
                "blocked".to_string()
            } else {
                "idle".to_string()
            }
        });
    let payment = request_snapshot
        .as_ref()
        .map(|snapshot| snapshot.wallet_status.clone())
        .unwrap_or_else(|| "idle".to_string());
    Some(MissionControlBuyModePanelState {
        summary: request_snapshot
            .as_ref()
            .map(|snapshot| {
                let mut summary = format!(
                    "req {} // provider {} // {} // {} // phase {} // auth {} // next {} // payment {}",
                    compact_mission_control_id(snapshot.request_id.as_str()),
                    snapshot.provider_label(),
                    snapshot.winner_selection_summary(),
                    snapshot.work_summary(),
                    snapshot.phase.as_str(),
                    snapshot.authority.as_str(),
                    snapshot.next_expected_event,
                    snapshot.payment_summary(),
                );
                if let Some(target_pubkey) = request
                    .as_ref()
                    .and_then(|request| request.target_provider_pubkeys.first())
                {
                    summary.push_str(" // target ");
                    summary.push_str(compact_mission_control_id(target_pubkey).as_str());
                }
                if let Some(loser_summary) = snapshot.loser_reason_summary.as_deref() {
                    summary.push_str(" // ");
                    summary.push_str(loser_summary);
                }
                if let Some(blocker_summary) = snapshot.payment_blocker_summary.as_deref() {
                    summary.push_str(" // blocker ");
                    summary.push_str(blocker_summary);
                }
                summary
            })
            .or_else(|| {
                blocked_while_idle.then(|| {
                    let mut summary = format!(
                        "Buy Mode blocked // {}",
                        block_reason.as_deref().unwrap_or("wallet funding required")
                    );
                    summary.push_str(" // roster ");
                    summary.push_str(
                        format!(
                            "{}/{}",
                            target_selection.eligible_peer_count, target_selection.observed_peer_count
                        )
                        .as_str(),
                    );
                    if let Some(target_pubkey) = target_selection.selected_peer_pubkey.as_deref() {
                        summary.push_str(" // target ");
                        summary.push_str(compact_mission_control_id(target_pubkey).as_str());
                    } else if let Some(blocked_reason) = target_selection.blocked_reason.as_deref() {
                        summary.push_str(" // ");
                        summary.push_str(blocked_reason);
                    }
                    summary
                })
            })
            .unwrap_or_else(|| {
                let mut summary = format!(
                    "{} // 5050 // {} sats // every {} // roster {}/{}",
                    if buy_mode.buy_mode_loop_enabled {
                        "Buy Mode armed"
                    } else {
                        "Buy Mode off"
                    },
                    crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                    crate::app_state::mission_control_buy_mode_interval_label(),
                    target_selection.eligible_peer_count,
                    target_selection.observed_peer_count,
                );
                if let Some(target_pubkey) = target_selection.selected_peer_pubkey.as_deref() {
                    summary.push_str(" // target ");
                    summary.push_str(compact_mission_control_id(target_pubkey).as_str());
                    if let Some(model) = target_selection.selected_ready_model.as_deref() {
                        summary.push_str(" // model ");
                        summary.push_str(model);
                    }
                } else if let Some(blocked_reason) = target_selection.blocked_reason.as_deref() {
                    summary.push_str(" // ");
                    summary.push_str(blocked_reason);
                }
                summary
            }),
        mode: if buy_mode.buy_mode_loop_enabled {
            "on".to_string()
        } else {
            "off".to_string()
        },
        next,
        provider,
        work,
        payment,
        button_label: if buy_mode.buy_mode_loop_enabled {
            "STOP BUY MODE".to_string()
        } else {
            "START BUY MODE".to_string()
        },
        button_active: buy_mode.buy_mode_loop_enabled,
        button_enabled: buy_mode.buy_mode_loop_enabled || block_reason.is_none(),
    })
}

fn mission_control_background_color() -> Hsla {
    Hsla::from_hex(0x070C14)
}

const MISSION_CONTROL_SECTION_HEADER_HEIGHT: f32 = 28.0;
const MISSION_CONTROL_SECTION_HEADER_MARGIN_BOTTOM: f32 = 10.0;
const MISSION_CONTROL_SECTION_BOTTOM_PADDING: f32 = 15.0;
const MISSION_CONTROL_SECTION_CONTENT_TOP: f32 =
    MISSION_CONTROL_SECTION_HEADER_HEIGHT + MISSION_CONTROL_SECTION_HEADER_MARGIN_BOTTOM;

fn mission_control_section_content_y(bounds: Bounds) -> f32 {
    bounds.origin.y + MISSION_CONTROL_SECTION_CONTENT_TOP
}

pub(crate) fn mission_control_panel_color() -> Hsla {
    Hsla::from_hex(0x0D121A)
}

fn mission_control_panel_header_color() -> Hsla {
    Hsla::from_hex(0x121924)
}

pub(crate) fn mission_control_panel_border_color() -> Hsla {
    Hsla::from_hex(0x263245)
}

pub(crate) fn mission_control_text_color() -> Hsla {
    Hsla::from_hex(0xD8DFF0)
}

fn mission_control_muted_color() -> Hsla {
    Hsla::from_hex(0x8A909E)
}

fn mission_control_orange_color() -> Hsla {
    Hsla::from_hex(0xFFA122)
}

fn mission_control_amber_color() -> Hsla {
    Hsla::from_hex(0xF9B84D)
}

pub(crate) fn mission_control_green_color() -> Hsla {
    Hsla::from_hex(0x52E06D)
}

pub(crate) fn mission_control_cyan_color() -> Hsla {
    Hsla::from_hex(0x2FB7F2)
}

fn mission_control_red_color() -> Hsla {
    Hsla::from_hex(0xF46060)
}

fn mission_control_mode_color(mode: crate::app_state::ProviderMode) -> Hsla {
    match mode {
        crate::app_state::ProviderMode::Offline => mission_control_orange_color(),
        crate::app_state::ProviderMode::Connecting => mission_control_cyan_color(),
        crate::app_state::ProviderMode::Online => mission_control_green_color(),
        crate::app_state::ProviderMode::Degraded => mission_control_red_color(),
    }
}

fn mission_control_buy_mode_result_label(
    request: Option<&crate::app_state::SubmittedNetworkRequest>,
) -> &str {
    match request {
        Some(request) if request.last_result_event_id.is_some() => "received",
        Some(request) if request.last_feedback_event_id.is_some() => "feedback",
        Some(_) => "pending",
        None => "waiting",
    }
}

fn mission_control_buy_mode_payment_label(
    request: Option<&crate::app_state::SubmittedNetworkRequest>,
    spark_wallet: &SparkPaneState,
) -> String {
    match request {
        Some(request) => crate::app_state::buy_mode_wallet_state_label(
            request,
            crate::app_state::buy_mode_wallet_payment(request, spark_wallet),
        ),
        None => "idle".to_string(),
    }
}

fn mission_control_buy_mode_payment_summary(
    request: &crate::app_state::SubmittedNetworkRequest,
    spark_wallet: &SparkPaneState,
) -> String {
    let wallet_payment = crate::app_state::buy_mode_wallet_payment(request, spark_wallet);
    if request.payment_sent_at_epoch_seconds.is_some()
        || request.status == crate::state::operations::NetworkRequestStatus::Paid
    {
        return wallet_payment
            .map(|payment| {
                format!(
                    "payment sent ({})",
                    crate::spark_wallet::wallet_payment_amount_summary(payment)
                )
            })
            .unwrap_or_else(|| "payment sent".to_string());
    }
    if let Some(wallet_payment) = wallet_payment {
        let amount_summary = crate::spark_wallet::wallet_payment_amount_summary(wallet_payment);
        if wallet_payment.is_returned_htlc_failure() {
            return format!(
                "payment returned ({amount_summary}); refund should settle back to wallet"
            );
        }
        if crate::spark_wallet::is_terminal_wallet_payment_status(wallet_payment.status.as_str()) {
            let detail = wallet_payment
                .status_detail
                .clone()
                .unwrap_or_else(|| "payment failed".to_string());
            return format!("{detail} ({amount_summary})");
        }
        let detail = wallet_payment
            .status_detail
            .as_deref()
            .unwrap_or("payment pending Spark confirmation");
        if wallet_payment.fees_sats > 0 {
            return format!("{detail} ({amount_summary})");
        }
        return detail.to_string();
    }
    if request.last_payment_pointer.is_some() {
        return "payment pending Spark confirmation".to_string();
    }
    if request.pending_bolt11.is_some() {
        return "payment queued".to_string();
    }
    if request.payment_required_at_epoch_seconds.is_some() {
        return "invoice received".to_string();
    }
    if request.payment_error.is_some() {
        return "payment failed".to_string();
    }
    "payment idle".to_string()
}

fn mission_control_buy_mode_provider_label(
    request: Option<&crate::app_state::SubmittedNetworkRequest>,
) -> String {
    request
        .and_then(|request| {
            request
                .winning_provider_pubkey
                .as_deref()
                .or(request.last_provider_pubkey.as_deref())
        })
        .map(compact_mission_control_id)
        .unwrap_or_else(|| "none".to_string())
}

fn mission_control_buy_mode_provider_summary(
    request: &crate::app_state::SubmittedNetworkRequest,
) -> String {
    request
        .winning_provider_pubkey
        .as_deref()
        .or(request.last_provider_pubkey.as_deref())
        .map(|provider| format!("provider {}", compact_mission_control_id(provider)))
        .unwrap_or_else(|| "awaiting provider".to_string())
}

fn mission_control_buy_mode_work_label(
    request: Option<&crate::app_state::SubmittedNetworkRequest>,
) -> String {
    let Some(request) = request else {
        return "idle".to_string();
    };
    if request.status == crate::state::operations::NetworkRequestStatus::Failed {
        return "fault".to_string();
    }
    if request.last_result_event_id.is_some() {
        return "done".to_string();
    }
    match request.last_feedback_status.as_deref() {
        Some(status) if status.eq_ignore_ascii_case("processing") => "working".to_string(),
        Some(status) if status.eq_ignore_ascii_case("payment-required") => "invoice".to_string(),
        Some(_) => "feedback".to_string(),
        None if request.published_request_event_id.is_some() => "searching".to_string(),
        None => "queued".to_string(),
    }
}

fn mission_control_buy_mode_work_summary(
    request: &crate::app_state::SubmittedNetworkRequest,
) -> String {
    if request.status == crate::state::operations::NetworkRequestStatus::Failed {
        return "request failed".to_string();
    }
    if request.last_result_event_id.is_some() {
        return "result received".to_string();
    }
    match request.last_feedback_status.as_deref() {
        Some(status) if status.eq_ignore_ascii_case("processing") => "provider working".to_string(),
        Some(status) if status.eq_ignore_ascii_case("payment-required") => {
            "invoice received".to_string()
        }
        Some(_) => format!(
            "feedback {}",
            mission_control_buy_mode_result_label(Some(request))
        ),
        None if request.published_request_event_id.is_some() => "request published".to_string(),
        None => "queued locally".to_string(),
    }
}

fn compact_mission_control_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        return trimmed.to_string();
    }
    format!("{}..{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
}

fn mission_control_section_clip_bounds(bounds: Bounds) -> Bounds {
    mission_control_section_scroll_viewport_bounds(bounds)
}

fn mission_control_section_scroll_viewport_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 8.0,
        mission_control_section_content_y(bounds),
        (bounds.size.width - 16.0).max(0.0),
        (bounds.size.height
            - MISSION_CONTROL_SECTION_CONTENT_TOP
            - MISSION_CONTROL_SECTION_BOTTOM_PADDING)
            .max(0.0),
    )
}

fn mission_control_section_max_scroll(bounds: Bounds, content_height: f32) -> f32 {
    mission_control_max_scroll_for_viewport(
        mission_control_section_scroll_viewport_bounds(bounds),
        content_height,
    )
}

fn mission_control_max_scroll_for_viewport(viewport: Bounds, content_height: f32) -> f32 {
    (content_height - viewport.size.height).max(0.0)
}

fn paint_mission_control_section_scrollbar(
    bounds: Bounds,
    content_height: f32,
    scroll_offset: f32,
    paint: &mut PaintContext,
) {
    paint_mission_control_scrollbar_for_viewport(
        bounds,
        mission_control_section_scroll_viewport_bounds(bounds),
        content_height,
        scroll_offset,
        paint,
    );
}

fn paint_mission_control_scrollbar_for_viewport(
    bounds: Bounds,
    viewport: Bounds,
    content_height: f32,
    scroll_offset: f32,
    paint: &mut PaintContext,
) {
    if viewport.size.height <= 0.0 || content_height <= viewport.size.height + 0.5 {
        return;
    }
    let max_offset = (content_height - viewport.size.height).max(0.0);
    if max_offset <= 0.5 {
        return;
    }
    let track = Bounds::new(
        bounds.max_x() - 6.0,
        viewport.origin.y,
        2.0,
        viewport.size.height,
    );
    let thumb_height = scrollbar_thumb_height(viewport.size.height, content_height, 16.0);
    let thumb_y = viewport.origin.y
        + ((scroll_offset / max_offset.max(1.0)) * (viewport.size.height - thumb_height));
    paint.scene.draw_quad(
        Quad::new(track)
            .with_background(mission_control_panel_header_color().with_alpha(0.45))
            .with_corner_radius(1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            track.origin.x,
            thumb_y,
            track.size.width,
            thumb_height,
        ))
        .with_background(mission_control_muted_color().with_alpha(0.72))
        .with_corner_radius(1.0),
    );
}

fn scrollbar_thumb_height(viewport_height: f32, content_height: f32, min_thumb_height: f32) -> f32 {
    if !viewport_height.is_finite() || !content_height.is_finite() {
        return 0.0;
    }
    if viewport_height <= 0.0 || content_height <= 0.0 {
        return 0.0;
    }
    let max_thumb_height = viewport_height.max(0.0);
    let min_thumb_height = min_thumb_height.min(max_thumb_height);
    ((viewport_height / content_height) * viewport_height).clamp(min_thumb_height, max_thumb_height)
}

fn paint_mission_control_amount_line(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_color: Hsla,
    value_font_size: f32,
    row_width: f32,
    show_divider: bool,
) -> f32 {
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{label}:"),
        Point::new(x, y),
        12.0,
        mission_control_muted_color(),
    ));
    let value_x = x + mission_control_value_x_offset(label);
    let value_right = x + row_width.max(0.0);
    let value_width = paint
        .text
        .layout_mono(value, Point::ZERO, value_font_size, value_color)
        .bounds()
        .size
        .width;
    let target_x = (value_right - value_width).max(value_x);
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(target_x, y - 1.0),
        value_font_size,
        value_color,
    ));
    let row_bottom = y + 20.0;
    if show_divider {
        paint_mission_control_row_divider(paint, x, row_bottom, row_width)
    } else {
        row_bottom + 20.0
    }
}

fn mission_control_local_model_button_label(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    crate::app_state::mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    )
    .local_model_button_label
}

fn mission_control_local_action_enabled(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> bool {
    crate::app_state::mission_control_local_model_button_enabled(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    )
}

fn mission_control_local_fm_test_button_visible(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> bool {
    mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime)
        == Some(MissionControlLocalRuntimeLane::AppleFoundationModels)
}

fn mission_control_local_fm_test_button_label(
    provider_control: &ProviderControlPaneState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
) -> String {
    if provider_control.local_fm_summary_is_pending() {
        String::from("STREAMING LOCAL FM")
    } else if mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime)
        != Some(MissionControlLocalRuntimeLane::AppleFoundationModels)
    {
        String::from("LOCAL FM UNAVAILABLE")
    } else if provider_runtime.apple_fm.is_ready() {
        String::from("TEST LOCAL FM")
    } else {
        String::from("LOCAL FM NOT READY")
    }
}

fn mission_control_local_fm_test_enabled(
    provider_control: &ProviderControlPaneState,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> bool {
    !provider_control.local_fm_summary_is_pending()
        && mission_control_local_runtime_is_ready(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        )
        && mission_control_local_runtime_lane(desktop_shell_mode, local_inference_runtime)
            == Some(MissionControlLocalRuntimeLane::AppleFoundationModels)
}

fn mission_control_display_model_label(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return "NO SUPPORTED MODEL".to_string();
    }
    trimmed.to_ascii_uppercase()
}

fn mission_control_primary_model_label(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    mission_control_display_model_label(
        crate::app_state::mission_control_local_runtime_view_model(
            desktop_shell_mode,
            provider_runtime,
            local_inference_runtime,
        )
        .model_label
        .as_str(),
    )
}

fn mission_control_backend_label(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    crate::app_state::mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    )
    .backend_label
}

fn mission_control_model_load_status(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    crate::app_state::mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    )
    .load_label
}

fn mission_control_go_online_hint(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    crate::app_state::mission_control_local_runtime_view_model(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    )
    .go_online_hint
}

fn paint_provider_status_pane(
    content_bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    backend_kernel_authority: bool,
    provider_blockers: &[ProviderBlocker],
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let now = std::time::Instant::now();
    let heartbeat_age = provider_runtime
        .heartbeat_age_seconds(now)
        .map_or_else(|| "n/a".to_string(), |age| age.to_string());
    paint.scene.draw_text(paint.text.layout(
        "Launch inventory controls",
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 12.0,
        ),
        11.0,
        theme::text::MUTED,
    ));
    for (row_index, target) in crate::app_state::ProviderInventoryProductToggleTarget::all()
        .iter()
        .enumerate()
    {
        let button_bounds = provider_inventory_toggle_button_bounds(content_bounds, row_index);
        let enabled = provider_runtime.inventory_controls.is_advertised(*target);
        let button_label = if enabled {
            format!("Disable {}", target.display_label())
        } else {
            format!("Enable {}", target.display_label())
        };
        paint_action_button(button_bounds, button_label.as_str(), paint);
    }

    let inventory_heading_y = content_bounds.origin.y + 136.0;
    paint.scene.draw_text(paint.text.layout(
        "Live launch inventory",
        Point::new(content_bounds.origin.x + 12.0, inventory_heading_y),
        11.0,
        theme::text::MUTED,
    ));
    let mut inventory_y = inventory_heading_y + 18.0;
    for row in provider_runtime.inventory_rows.iter().take(3) {
        let line = format!(
            "{} [{}] backend_ready={} lot={} open={} reserved={} available={} delivery={} floor={} terms={} source={}",
            row.target.display_label(),
            if row.enabled { "enabled" } else { "disabled" },
            if row.backend_ready { "yes" } else { "no" },
            row.capacity_lot_id.as_deref().unwrap_or("n/a"),
            row.total_quantity,
            row.reserved_quantity,
            row.available_quantity,
            row.delivery_state,
            format_sats_amount(row.price_floor_sats),
            row.terms_label,
            row.source_badge,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &line,
            Point::new(content_bounds.origin.x + 12.0, inventory_y),
            10.0,
            if row.eligible {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));
        inventory_y += 14.0;
        paint.scene.draw_text(paint.text.layout_mono(
            row.capability_summary.as_str(),
            Point::new(content_bounds.origin.x + 12.0, inventory_y),
            9.0,
            theme::text::MUTED,
        ));
        inventory_y += 18.0;
        if let Some(forward_lot_id) = row.forward_capacity_lot_id.as_deref() {
            let forward_line = format!(
                "forward lot={} open={} reserved={} available={} window={} terms={}",
                forward_lot_id,
                row.forward_total_quantity,
                row.forward_reserved_quantity,
                row.forward_available_quantity,
                row.forward_delivery_window_label
                    .as_deref()
                    .unwrap_or("n/a"),
                row.forward_terms_label.as_deref().unwrap_or("n/a"),
            );
            paint.scene.draw_text(paint.text.layout_mono(
                &forward_line,
                Point::new(content_bounds.origin.x + 12.0, inventory_y),
                9.0,
                theme::text::MUTED,
            ));
            inventory_y += 18.0;
        }
    }
    if let Some(action) = provider_runtime.inventory_last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(content_bounds.origin.x + 12.0, inventory_y),
            10.0,
            theme::text::MUTED,
        ));
        inventory_y += 16.0;
    }
    if let Some(error) = provider_runtime.inventory_last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(content_bounds.origin.x + 12.0, inventory_y),
            10.0,
            theme::status::ERROR,
        ));
        inventory_y += 16.0;
    }

    let mut y = inventory_y + 10.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Lane",
        provider_runtime.execution_lane_label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Execution backend",
        provider_runtime.execution_backend_label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Control authority",
        provider_runtime.control_authority_label(backend_kernel_authority),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Projection stream",
        if earn_job_lifecycle_projection.authority == "non-authoritative" {
            provider_runtime.projection_authority_label()
        } else {
            earn_job_lifecycle_projection.authority.as_str()
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Settlement truth",
        provider_runtime.settlement_truth_label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Mode",
        provider_runtime.mode.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Uptime (s)",
        &provider_runtime.uptime_seconds(now).to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Heartbeat age (s)",
        &heartbeat_age,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Queue depth",
        &provider_runtime.queue_depth.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Active backend",
        provider_runtime.execution_backend_label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Local inference",
        if provider_runtime.gpt_oss.is_ready() {
            "ready"
        } else if provider_runtime.gpt_oss.reachable {
            "degraded"
        } else {
            "offline"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Apple FM",
        if provider_runtime.apple_fm.is_ready() {
            "ready"
        } else if provider_runtime.apple_fm.reachable {
            "degraded"
        } else {
            "offline"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Configured local model",
        provider_runtime
            .gpt_oss
            .configured_model
            .as_deref()
            .unwrap_or("none"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Serving model",
        provider_runtime
            .active_inference_backend()
            .and_then(|backend| match backend {
                crate::state::provider_runtime::LocalInferenceBackend::AppleFoundationModels => {
                    provider_runtime.apple_fm.ready_model.as_deref()
                }
                crate::state::provider_runtime::LocalInferenceBackend::GptOss => provider_runtime
                    .gpt_oss
                    .ready_model
                    .as_deref()
                    .or(provider_runtime.gpt_oss.configured_model.as_deref()),
                crate::state::provider_runtime::LocalInferenceBackend::PsionicTrain => {
                    Some("psionic_train")
                }
            })
            .unwrap_or("none"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Apple model",
        provider_runtime
            .apple_fm
            .ready_model
            .as_deref()
            .unwrap_or("none"),
    );
    if let Some(last_completed) = provider_runtime.last_completed_job_at {
        let seconds = now
            .checked_duration_since(last_completed)
            .map_or(0, |duration| duration.as_secs());
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last completed job (s ago)",
            &seconds.to_string(),
        );
    } else {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last completed job",
            "none",
        );
    }
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last result",
        provider_runtime.last_result.as_deref().unwrap_or("none"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Authority status",
        provider_runtime
            .last_authoritative_status
            .as_deref()
            .unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Authority event",
        provider_runtime
            .last_authoritative_event_id
            .as_deref()
            .unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Authority error class",
        provider_runtime
            .last_authoritative_error_class
            .map(crate::app_state::EarnFailureClass::label)
            .unwrap_or("n/a"),
    );

    paint.scene.draw_text(paint.text.layout(
        "Dependencies",
        Point::new(content_bounds.origin.x + 12.0, y + 4.0),
        11.0,
        theme::text::MUTED,
    ));
    let mut dep_y = y + 20.0;
    let identity_status = if provider_blockers.contains(&ProviderBlocker::IdentityMissing) {
        "degraded"
    } else {
        "ready"
    };
    let wallet_status = if provider_blockers.contains(&ProviderBlocker::WalletError) {
        "degraded"
    } else {
        "ready"
    };
    let gpt_oss_status = if provider_blockers.contains(&ProviderBlocker::GptOssUnavailable)
        || provider_blockers.contains(&ProviderBlocker::GptOssModelUnavailable)
    {
        "degraded"
    } else {
        "ready"
    };
    let apple_fm_status = if provider_blockers
        .contains(&ProviderBlocker::AppleFoundationModelsUnavailable)
        || provider_blockers.contains(&ProviderBlocker::AppleFoundationModelsModelUnavailable)
    {
        "degraded"
    } else {
        "ready"
    };
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("identity: {identity_status}"),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("wallet: {wallet_status}"),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("local_inference: {gpt_oss_status}"),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("apple_fm: {apple_fm_status}"),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "relay: unknown (lane pending)",
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 18.0;

    paint.scene.draw_text(paint.text.layout(
        "Local inference inventory",
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        11.0,
        theme::text::MUTED,
    ));
    dep_y += 16.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "installed: {}",
            if provider_runtime.gpt_oss.available_models.is_empty() {
                "none".to_string()
            } else {
                provider_runtime.gpt_oss.available_models.join(", ")
            }
        ),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "loaded: {}",
            if provider_runtime.gpt_oss.loaded_models.is_empty() {
                "none".to_string()
            } else {
                provider_runtime.gpt_oss.loaded_models.join(", ")
            }
        ),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    if let Some(metrics) = provider_runtime.gpt_oss.last_metrics.as_ref() {
        let total_ms = metrics
            .total_duration_ns
            .map(|ns| ns / 1_000_000)
            .unwrap_or(0);
        let eval_tokens = metrics.eval_count.unwrap_or(0);
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("last gen: total={}ms eval_tokens={}", total_ms, eval_tokens),
            Point::new(content_bounds.origin.x + 12.0, dep_y),
            10.0,
            theme::text::PRIMARY,
        ));
        dep_y += 14.0;
    }

    if let Some(error) = provider_runtime
        .gpt_oss
        .last_error
        .as_deref()
        .or(provider_runtime.last_error_detail.as_deref())
    {
        paint.scene.draw_text(paint.text.layout(
            "Last error",
            Point::new(content_bounds.origin.x + 12.0, dep_y + 18.0),
            11.0,
            theme::status::ERROR,
        ));
        let mut error_y = dep_y + 34.0;
        for line in split_text_for_display(error, 82) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 12.0, error_y),
                11.0,
                theme::status::ERROR,
            ));
            error_y += 14.0;
        }
    }
}

fn paint_earnings_scoreboard_pane(
    content_bounds: Bounds,
    pane_is_active: bool,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    earnings_scoreboard: &EarningsScoreboardState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    job_history: &JobHistoryState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    earnings_jobs_pane::paint_earnings_jobs_pane(
        content_bounds,
        pane_is_active,
        desktop_shell_mode,
        earnings_scoreboard,
        provider_runtime,
        local_inference_runtime,
        job_inbox,
        active_job,
        job_history,
        earn_job_lifecycle_projection,
        spark_wallet,
        paint,
    );
}

pub(crate) fn format_bps_percent(value: Option<u16>) -> String {
    value.map_or_else(
        || "n/a".to_string(),
        |bps| format!("{:.2}%", (bps as f64) / 100.0),
    )
}

pub(crate) fn earnings_scoreboard_amount_display(
    load_state: PaneLoadState,
    amount: String,
) -> String {
    if load_state == PaneLoadState::Loading {
        "LOADING".to_string()
    } else {
        amount
    }
}

fn paint_relay_connections_pane(
    content_bounds: Bounds,
    relay_connections: &RelayConnectionsState,
    relay_connections_inputs: &mut RelayConnectionsPaneInputs,
    paint: &mut PaintContext,
) {
    relay_connections_pane::paint(
        content_bounds,
        relay_connections,
        relay_connections_inputs,
        paint,
    );
}

fn paint_sync_health_pane(
    content_bounds: Bounds,
    sync_health: &SyncHealthState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, sync_health.source_tag.as_str(), paint);

    let rebootstrap_bounds = sync_health_rebootstrap_button_bounds(content_bounds);
    paint_action_button(rebootstrap_bounds, "Rebootstrap sync", paint);

    let state_color = match sync_health.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let mut y = rebootstrap_bounds.max_y() + 12.0;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", sync_health.load_state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;

    if let Some(action) = sync_health.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    if let Some(error) = sync_health.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::status::ERROR,
        ));
        y += 16.0;
    }

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Source",
        &sync_health.source_tag,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Spacetime connection",
        &sync_health.spacetime_connection,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Subscription",
        &sync_health.subscription_state,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Reconnect posture",
        &sync_health.reconnect_posture,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cursor position",
        &sync_health.cursor_position.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cursor target",
        &sync_health.cursor_target_position.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Cursor age (s)",
        &sync_health.cursor_last_advanced_seconds_ago.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Stale threshold (s)",
        &sync_health.cursor_stale_after_seconds.to_string(),
    );

    let stale = if sync_health.cursor_is_stale() {
        "yes"
    } else {
        "no"
    };
    let stale_color = if sync_health.cursor_is_stale() {
        theme::status::ERROR
    } else {
        theme::status::SUCCESS
    };
    paint.scene.draw_text(paint.text.layout(
        &format!("Cursor stale: {stale}"),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        stale_color,
    ));
    y += 16.0;

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Recovery phase",
        sync_health.recovery_phase.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Replay progress",
        &sync_health
            .replay_progress_percent
            .map_or_else(|| "n/a".to_string(), |value| format!("{value}%")),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Replay lag",
        &sync_health
            .replay_lag_seq
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Next retry (ms)",
        &sync_health
            .next_retry_ms
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Token refresh (s)",
        &sync_health
            .token_refresh_after_in_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Disconnect reason",
        sync_health.disconnect_reason.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Stale reason",
        sync_health.stale_cursor_reason.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last applied seq",
        &sync_health.last_applied_event_seq.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Duplicate drops",
        &sync_health.duplicate_drop_count.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Replay count",
        &sync_health.replay_count.to_string(),
    );

    paint.scene.draw_text(paint.text.layout(
        "Legacy websocket compatibility data: intentionally not shown.",
        Point::new(content_bounds.origin.x + 12.0, y),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_network_requests_pane(
    content_bounds: Bounds,
    network_requests: &NetworkRequestsState,
    network_requests_inputs: &mut NetworkRequestsPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "compute.authority", paint);

    let request_type_bounds = network_requests_type_input_bounds(content_bounds);
    let payload_bounds = network_requests_payload_input_bounds(content_bounds);
    let skill_scope_bounds = network_requests_skill_scope_input_bounds(content_bounds);
    let envelope_bounds = network_requests_credit_envelope_input_bounds(content_bounds);
    let start_delay_bounds = network_requests_budget_input_bounds(content_bounds);
    let window_bounds = network_requests_timeout_input_bounds(content_bounds);
    let max_price_bounds = network_requests_max_price_input_bounds(content_bounds);
    let submit_bounds = network_requests_submit_button_bounds(content_bounds);
    let accept_bounds = network_requests_accept_button_bounds(content_bounds);

    network_requests_inputs
        .compute_family
        .set_max_width(request_type_bounds.size.width);
    network_requests_inputs
        .preferred_backend
        .set_max_width(payload_bounds.size.width);
    network_requests_inputs
        .capability_constraints
        .set_max_width(skill_scope_bounds.size.width);
    network_requests_inputs
        .quantity
        .set_max_width(envelope_bounds.size.width);
    network_requests_inputs
        .delivery_start_minutes
        .set_max_width(start_delay_bounds.size.width);
    network_requests_inputs
        .window_minutes
        .set_max_width(window_bounds.size.width);
    network_requests_inputs
        .max_price_sats
        .set_max_width(max_price_bounds.size.width);

    network_requests_inputs
        .compute_family
        .paint(request_type_bounds, paint);
    network_requests_inputs
        .preferred_backend
        .paint(payload_bounds, paint);
    network_requests_inputs
        .capability_constraints
        .paint(skill_scope_bounds, paint);
    network_requests_inputs
        .quantity
        .paint(envelope_bounds, paint);
    network_requests_inputs
        .delivery_start_minutes
        .paint(start_delay_bounds, paint);
    network_requests_inputs
        .window_minutes
        .paint(window_bounds, paint);
    network_requests_inputs
        .max_price_sats
        .paint(max_price_bounds, paint);
    paint_primary_button(submit_bounds, "Request quotes", paint);
    paint_action_button(accept_bounds, "Accept selected quote", paint);

    paint.scene.draw_text(paint.text.layout(
        "Compute family",
        Point::new(
            request_type_bounds.origin.x,
            request_type_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Preferred backend",
        Point::new(payload_bounds.origin.x, payload_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Delivery start in (m)",
        Point::new(
            start_delay_bounds.origin.x,
            start_delay_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Capability envelope constraints",
        Point::new(
            skill_scope_bounds.origin.x,
            skill_scope_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Requested quantity",
        Point::new(envelope_bounds.origin.x, envelope_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Delivery window (m)",
        Point::new(window_bounds.origin.x, window_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Max price (sats)",
        Point::new(max_price_bounds.origin.x, max_price_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        accept_bounds.max_y() + 12.0,
        network_requests.load_state,
        &format!(
            "State: {} / mode: {}",
            network_requests.load_state.label(),
            network_requests.quote_mode.label()
        ),
        network_requests.last_action.as_deref(),
        network_requests.last_error.as_deref(),
    );
    let mut detail_y = y;
    match network_requests.quote_mode {
        crate::app_state::ComputeQuoteMode::Spot => {
            if let Some(rfq) = network_requests.last_spot_rfq.as_ref() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!("RFQ: {}", rfq.summary()),
                    Point::new(content_bounds.origin.x + 12.0, detail_y),
                    9.0,
                    theme::text::MUTED,
                ));
                detail_y += 18.0;
            }

            paint.scene.draw_text(paint.text.layout(
                "Available spot quotes",
                Point::new(content_bounds.origin.x + 12.0, detail_y),
                11.0,
                theme::text::MUTED,
            ));

            if network_requests.spot_quote_candidates.is_empty() {
                paint.scene.draw_text(paint.text.layout(
                    "No compute quotes loaded yet.",
                    Point::new(content_bounds.origin.x + 12.0, detail_y + 18.0),
                    11.0,
                    theme::text::MUTED,
                ));
            } else {
                let visible_rows = network_requests_visible_quote_count(
                    network_requests.spot_quote_candidates.len(),
                );
                for row_index in 0..visible_rows {
                    let quote = &network_requests.spot_quote_candidates[row_index];
                    let row_bounds = network_requests_quote_row_bounds(content_bounds, row_index);
                    let selected = network_requests.selected_spot_quote_id.as_deref()
                        == Some(quote.quote_id.as_str());
                    paint.scene.draw_quad(
                        Quad::new(row_bounds)
                            .with_background(if selected {
                                theme::bg::SURFACE
                            } else {
                                theme::bg::APP.with_alpha(0.78)
                            })
                            .with_border(
                                if selected {
                                    theme::accent::PRIMARY
                                } else {
                                    theme::border::DEFAULT
                                },
                                1.0,
                            )
                            .with_corner_radius(6.0),
                    );
                    let summary = format!(
                        "{} family={} backend={} execution={} topology={} provisioning={} proof={} provider={} lot={} qty={}/{} price={} env={} profile={} terms={} source={}",
                        quote.product_id,
                        quote.compute_family_label(),
                        quote.backend_label(),
                        quote.execution_label(),
                        quote.topology_label(),
                        quote.provisioning_label(),
                        quote.proof_posture_label(),
                        quote.provider_id,
                        quote.capacity_lot_id,
                        quote.requested_quantity,
                        quote.available_quantity,
                        format_sats_amount(quote.price_sats),
                        quote.environment_ref().unwrap_or("-"),
                        quote.sandbox_profile_ref.as_deref().unwrap_or("-"),
                        quote.terms_label,
                        quote.source_badge,
                    );
                    paint.scene.draw_text(paint.text.layout_mono(
                        &summary,
                        Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 7.0),
                        9.0,
                        theme::text::PRIMARY,
                    ));
                    paint.scene.draw_text(paint.text.layout_mono(
                        quote.capability_summary.as_str(),
                        Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 18.0),
                        8.5,
                        theme::text::MUTED,
                    ));
                }
            }

            let accepted_heading_y =
                network_requests_quote_row_bounds(content_bounds, 4).max_y() + 18.0;
            paint.scene.draw_text(paint.text.layout(
                "Accepted spot orders",
                Point::new(content_bounds.origin.x + 12.0, accepted_heading_y),
                11.0,
                theme::text::MUTED,
            ));
            if network_requests.accepted_spot_orders.is_empty() {
                paint.scene.draw_text(paint.text.layout(
                    "No compute orders accepted yet.",
                    Point::new(content_bounds.origin.x + 12.0, accepted_heading_y + 18.0),
                    10.0,
                    theme::text::MUTED,
                ));
            } else {
                for (idx, order) in network_requests
                    .accepted_spot_orders
                    .iter()
                    .take(3)
                    .enumerate()
                {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &format!(
                            "{} -> {} {} backend={} topology={} proof={} env={} profile={} provider={} qty={} price={} at={}",
                            order.quote_id,
                            order.instrument_id,
                            order.product_id,
                            match order.backend_family {
                                Some(openagents_kernel_core::compute::ComputeBackendFamily::GptOss) => "gpt_oss",
                                Some(openagents_kernel_core::compute::ComputeBackendFamily::AppleFoundationModels) => "apple_foundation_models",
                                Some(openagents_kernel_core::compute::ComputeBackendFamily::PsionicTrain) => "psionic_train",
                                None if matches!(order.compute_family, openagents_kernel_core::compute::ComputeFamily::SandboxExecution) => "sandbox",
                                None => "unknown",
                            },
                            order.topology_kind.map(|value| value.label()).unwrap_or("unspecified"),
                            order.proof_posture.map(|value| value.label()).unwrap_or("unspecified"),
                            order
                                .environment_binding
                                .as_ref()
                                .map(|binding| binding.environment_ref.as_str())
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or("-"),
                            order.sandbox_profile_ref.as_deref().unwrap_or("-"),
                            order.provider_id,
                            order.quantity,
                            format_sats_amount(order.price_sats),
                            order.accepted_at_epoch_seconds
                        ),
                        Point::new(
                            content_bounds.origin.x + 12.0,
                            accepted_heading_y + 18.0 + idx as f32 * 14.0,
                        ),
                        9.0,
                        theme::text::PRIMARY,
                    ));
                }
            }
        }
        crate::app_state::ComputeQuoteMode::ForwardPhysical => {
            if let Some(rfq) = network_requests.last_forward_rfq.as_ref() {
                paint.scene.draw_text(paint.text.layout_mono(
                    &format!("RFQ: {}", rfq.summary()),
                    Point::new(content_bounds.origin.x + 12.0, detail_y),
                    9.0,
                    theme::text::MUTED,
                ));
                detail_y += 18.0;
            }

            paint.scene.draw_text(paint.text.layout(
                "Available forward quotes",
                Point::new(content_bounds.origin.x + 12.0, detail_y),
                11.0,
                theme::text::MUTED,
            ));

            if network_requests.forward_quote_candidates.is_empty() {
                paint.scene.draw_text(paint.text.layout(
                    "No forward compute quotes loaded yet.",
                    Point::new(content_bounds.origin.x + 12.0, detail_y + 18.0),
                    11.0,
                    theme::text::MUTED,
                ));
            } else {
                let visible_rows = network_requests_visible_quote_count(
                    network_requests.forward_quote_candidates.len(),
                );
                for row_index in 0..visible_rows {
                    let quote = &network_requests.forward_quote_candidates[row_index];
                    let row_bounds = network_requests_quote_row_bounds(content_bounds, row_index);
                    let selected = network_requests.selected_forward_quote_id.as_deref()
                        == Some(quote.quote_id.as_str());
                    paint.scene.draw_quad(
                        Quad::new(row_bounds)
                            .with_background(if selected {
                                theme::bg::SURFACE
                            } else {
                                theme::bg::APP.with_alpha(0.78)
                            })
                            .with_border(
                                if selected {
                                    theme::accent::PRIMARY
                                } else {
                                    theme::border::DEFAULT
                                },
                                1.0,
                            )
                            .with_corner_radius(6.0),
                    );
                    let summary = format!(
                        "{} family={} backend={} execution={} topology={} provisioning={} proof={} provider={} lot={} qty={}/{} price={} window={} env={} profile={} terms={}",
                        quote.product_id,
                        quote.compute_family_label(),
                        quote.backend_label(),
                        quote.execution_label(),
                        quote.topology_label(),
                        quote.provisioning_label(),
                        quote.proof_posture_label(),
                        quote.provider_id,
                        quote.capacity_lot_id,
                        quote.requested_quantity,
                        quote.available_quantity,
                        format_sats_amount(quote.price_sats),
                        quote.delivery_window_label,
                        quote.environment_ref().unwrap_or("-"),
                        quote.sandbox_profile_ref.as_deref().unwrap_or("-"),
                        quote.terms_label,
                    );
                    paint.scene.draw_text(paint.text.layout_mono(
                        &summary,
                        Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 7.0),
                        9.0,
                        theme::text::PRIMARY,
                    ));
                    paint.scene.draw_text(paint.text.layout_mono(
                        &format!(
                            "{} | collateral={} | remedy={}",
                            quote.capability_summary,
                            quote.collateral_summary,
                            quote.remedy_summary
                        ),
                        Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 18.0),
                        8.5,
                        theme::text::MUTED,
                    ));
                }
            }

            let accepted_heading_y =
                network_requests_quote_row_bounds(content_bounds, 4).max_y() + 18.0;
            paint.scene.draw_text(paint.text.layout(
                "Accepted forward orders",
                Point::new(content_bounds.origin.x + 12.0, accepted_heading_y),
                11.0,
                theme::text::MUTED,
            ));
            if network_requests.accepted_forward_orders.is_empty() {
                paint.scene.draw_text(paint.text.layout(
                    "No forward compute orders accepted yet.",
                    Point::new(content_bounds.origin.x + 12.0, accepted_heading_y + 18.0),
                    10.0,
                    theme::text::MUTED,
                ));
            } else {
                for (idx, order) in network_requests
                    .accepted_forward_orders
                    .iter()
                    .take(3)
                    .enumerate()
                {
                    paint.scene.draw_text(paint.text.layout_mono(
                        &format!(
                            "{} -> {} {} backend={} topology={} proof={} env={} profile={} provider={} qty={} price={} window={} remedy={}",
                            order.quote_id,
                            order.instrument_id,
                            order.product_id,
                            match order.backend_family {
                                Some(openagents_kernel_core::compute::ComputeBackendFamily::GptOss) => "gpt_oss",
                                Some(openagents_kernel_core::compute::ComputeBackendFamily::AppleFoundationModels) => "apple_foundation_models",
                                Some(openagents_kernel_core::compute::ComputeBackendFamily::PsionicTrain) => "psionic_train",
                                None if matches!(order.compute_family, openagents_kernel_core::compute::ComputeFamily::SandboxExecution) => "sandbox",
                                None => "unknown",
                            },
                            order.topology_kind.map(|value| value.label()).unwrap_or("unspecified"),
                            order.proof_posture.map(|value| value.label()).unwrap_or("unspecified"),
                            order
                                .environment_binding
                                .as_ref()
                                .map(|binding| binding.environment_ref.as_str())
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or("-"),
                            order.sandbox_profile_ref.as_deref().unwrap_or("-"),
                            order.provider_id,
                            order.quantity,
                            format_sats_amount(order.price_sats),
                            order.delivery_window_label,
                            order.remedy_summary
                        ),
                        Point::new(
                            content_bounds.origin.x + 12.0,
                            accepted_heading_y + 18.0 + idx as f32 * 14.0,
                        ),
                        9.0,
                        theme::text::PRIMARY,
                    ));
                }
            }
        }
    }
}

fn paint_starter_jobs_pane(
    content_bounds: Bounds,
    starter_jobs: &StarterJobsState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let complete_bounds = starter_jobs_complete_button_bounds(content_bounds);
    let kill_switch_bounds = starter_jobs_kill_switch_button_bounds(content_bounds);
    paint_action_button(complete_bounds, "Complete selected", paint);
    paint_action_button(
        kill_switch_bounds,
        if starter_jobs.kill_switch_enabled {
            "Kill switch: ON"
        } else {
            "Kill switch: OFF"
        },
        paint,
    );

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        complete_bounds.max_y() + 12.0,
        starter_jobs.load_state,
        &format!("State: {}", starter_jobs.load_state.label()),
        starter_jobs.last_action.as_deref(),
        starter_jobs.last_error.as_deref(),
    );
    let controls_summary = format!(
        "Budget {} / {} | Interval {}s | Inflight {}/{}",
        format_sats_amount(starter_jobs.budget_allocated_sats),
        format_sats_amount(starter_jobs.budget_cap_sats),
        starter_jobs.dispatch_interval_seconds,
        starter_jobs.inflight_jobs(),
        starter_jobs.max_inflight_jobs
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &controls_summary,
        Point::new(content_bounds.origin.x + 12.0, y + 10.0),
        10.0,
        theme::text::MUTED,
    ));

    let visible_rows = starter_jobs_visible_row_count(starter_jobs.jobs.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No starter jobs available.",
            Point::new(content_bounds.origin.x + 12.0, y + 28.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let job = &starter_jobs.jobs[row_index];
        let row_bounds = starter_jobs_row_bounds(content_bounds, row_index);
        let selected = starter_jobs.selected_job_id.as_deref() == Some(job.job_id.as_str());
        paint_selectable_row_background(paint, row_bounds, selected);

        let status_color = match job.status {
            StarterJobStatus::Queued => theme::text::MUTED,
            StarterJobStatus::Running => theme::accent::PRIMARY,
            StarterJobStatus::Completed => theme::status::SUCCESS,
        };
        let eligibility = if job.eligible {
            "eligible"
        } else {
            "ineligible"
        };
        let summary = format!(
            "starter-demand {} {} {} {} {}",
            job.job_id,
            job.status.label(),
            format_sats_amount(job.payout_sats),
            eligibility,
            job.summary
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            status_color,
        ));
    }

    if let Some(selected) = starter_jobs.selected() {
        let details_y =
            starter_jobs_row_bounds(content_bounds, visible_rows.saturating_sub(1)).max_y() + 12.0;
        let mut line_y = details_y;
        line_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Selected job",
            &selected.job_id,
        );
        line_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Payout",
            &format_sats_amount(selected.payout_sats),
        );
        let _ = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            line_y,
            "Payout pointer",
            selected.payout_pointer.as_deref().unwrap_or("pending"),
        );
    }
}

fn paint_reciprocal_loop_pane(
    content_bounds: Bounds,
    reciprocal_loop: &ReciprocalLoopState,
    provider_runtime: &ProviderRuntimeState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let start_bounds = reciprocal_loop_start_button_bounds(content_bounds);
    let stop_bounds = reciprocal_loop_stop_button_bounds(content_bounds);
    let reset_bounds = reciprocal_loop_reset_button_bounds(content_bounds);
    if reciprocal_loop.running {
        paint_disabled_button(start_bounds, "Start", paint);
        paint_action_button(stop_bounds, "Stop", paint);
    } else {
        paint_action_button(start_bounds, "Start", paint);
        paint_disabled_button(stop_bounds, "Stop", paint);
    }
    paint_action_button(reset_bounds, "Reset", paint);

    let relay_health = match provider_runtime.mode {
        crate::app_state::ProviderMode::Online => "online",
        crate::app_state::ProviderMode::Connecting => "connecting",
        crate::app_state::ProviderMode::Degraded => "degraded",
        crate::app_state::ProviderMode::Offline => "offline",
    };
    let wallet_health = if spark_wallet.last_error.is_some() {
        "degraded"
    } else {
        spark_wallet.network_status_label()
    };
    let loop_mode = if reciprocal_loop.running {
        "running"
    } else {
        "stopped"
    };
    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        start_bounds.max_y() + 12.0,
        reciprocal_loop.load_state,
        &format!(
            "Loop: {} | Next: {}",
            loop_mode,
            reciprocal_loop.next_direction.label()
        ),
        reciprocal_loop.last_action.as_deref(),
        reciprocal_loop.last_error.as_deref(),
    );

    let health_color = if relay_health == "degraded" || wallet_health == "degraded" {
        theme::status::ERROR
    } else if relay_health == "online" && wallet_health == "connected" {
        theme::status::SUCCESS
    } else {
        theme::text::MUTED
    };
    let compact_value = |raw: Option<&str>| -> String {
        let value = raw.unwrap_or("missing");
        if value.len() > 24 {
            format!("{}..{}", &value[..12], &value[value.len() - 8..])
        } else {
            value.to_string()
        }
    };
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("Health relay={} wallet={}", relay_health, wallet_health),
        Point::new(content_bounds.origin.x + 12.0, y + 10.0),
        10.0,
        health_color,
    ));

    let mut line_y = y + 28.0;
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Local pubkey",
        &compact_value(reciprocal_loop.local_pubkey.as_deref()),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Peer pubkey",
        &compact_value(reciprocal_loop.peer_pubkey.as_deref()),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "In-flight",
        reciprocal_loop
            .in_flight_request_id
            .as_deref()
            .unwrap_or("none"),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Kill switch",
        if reciprocal_loop.kill_switch_active {
            "engaged"
        } else {
            "open"
        },
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Retry attempts",
        &format!(
            "{}/{}",
            reciprocal_loop.retry_attempts, reciprocal_loop.max_retry_attempts
        ),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Backoff until",
        &reciprocal_loop
            .retry_backoff_until_epoch_seconds
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "In-flight limits",
        &format!(
            "local {}/{} peer {}/{}",
            reciprocal_loop.in_flight_local_to_peer(),
            reciprocal_loop.max_in_flight_local_to_peer,
            reciprocal_loop.in_flight_peer_to_local(),
            reciprocal_loop.max_in_flight_peer_to_local
        ),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "A->B dispatched",
        &reciprocal_loop.local_to_peer_dispatched.to_string(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "A->B paid",
        &reciprocal_loop.local_to_peer_paid.to_string(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "B->A paid",
        &reciprocal_loop.peer_to_local_paid.to_string(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "A->B failed",
        &reciprocal_loop.local_to_peer_failed.to_string(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "B->A failed",
        &reciprocal_loop.peer_to_local_failed.to_string(),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Sent/received",
        &format!(
            "{}/{}",
            format_sats_amount(reciprocal_loop.sats_sent),
            format_sats_amount(reciprocal_loop.sats_received)
        ),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Last payment",
        &compact_value(reciprocal_loop.last_payment_pointer.as_deref()),
    );
    line_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Failure class",
        &format!(
            "{} / {}",
            reciprocal_loop
                .last_failure_class
                .map(|value| value.label())
                .unwrap_or("none"),
            reciprocal_loop
                .last_failure_disposition
                .map(|value| value.label())
                .unwrap_or("none")
        ),
    );
    let _ = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        line_y,
        "Failure detail",
        reciprocal_loop
            .last_failure_detail
            .as_deref()
            .unwrap_or("none"),
    );
}

fn paint_activity_feed_pane(
    content_bounds: Bounds,
    activity_feed: &ActivityFeedState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, &activity_feed.projection_stream_id, paint);

    let refresh_bounds = activity_feed_refresh_button_bounds(content_bounds);
    let prev_bounds = activity_feed_prev_page_button_bounds(content_bounds);
    let next_bounds = activity_feed_next_page_button_bounds(content_bounds);
    paint_action_button(refresh_bounds, "Reload stream", paint);
    paint_action_button(prev_bounds, "Prev", paint);
    paint_action_button(next_bounds, "Next", paint);

    let filters = ActivityFeedFilter::all();
    for (index, filter) in filters.into_iter().enumerate() {
        paint_filter_button(
            activity_feed_filter_button_bounds(content_bounds, index),
            filter.label(),
            activity_feed.active_filter == filter,
            paint,
        );
    }

    let page = activity_feed
        .page
        .min(activity_feed.total_pages().saturating_sub(1))
        + 1;
    let filtered_rows = activity_feed.filtered_row_count();
    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        activity_feed_filter_button_bounds(content_bounds, 0).max_y() + 10.0,
        activity_feed.load_state,
        &format!(
            "State: {} | Filter: {} | Page {page}/{}",
            activity_feed.load_state.label(),
            activity_feed.active_filter.label(),
            activity_feed.total_pages()
        ),
        activity_feed.last_action.as_deref(),
        activity_feed.last_error.as_deref(),
    );
    let y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rows",
        &match activity_feed.active_filter {
            ActivityFeedFilter::Nip90 => format!("{filtered_rows} (latest 50)"),
            _ => filtered_rows.to_string(),
        },
    );

    let visible = activity_feed.visible_rows();
    let visible_rows = activity_feed_visible_row_count(visible.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No activity events for this filter.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (row_index, row) in visible.iter().take(visible_rows).enumerate() {
        let row_bounds = activity_feed_row_bounds(content_bounds, row_index);
        let selected = activity_feed.selected_event_id.as_deref() == Some(row.event_id.as_str());
        paint_selectable_row_background(paint, row_bounds, selected);

        let domain_color = match row.domain {
            ActivityEventDomain::Chat => theme::accent::PRIMARY,
            ActivityEventDomain::Cad => theme::accent::PRIMARY,
            ActivityEventDomain::Job => theme::status::SUCCESS,
            ActivityEventDomain::Wallet => theme::status::SUCCESS,
            ActivityEventDomain::Network => theme::text::PRIMARY,
            ActivityEventDomain::Sync => theme::text::MUTED,
            ActivityEventDomain::Sa => theme::accent::PRIMARY,
            ActivityEventDomain::Skl => theme::status::SUCCESS,
            ActivityEventDomain::Ac => theme::status::ERROR,
        };
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "[{}] {} {}",
                row.domain.label(),
                row.occurred_at_epoch_seconds,
                row.summary
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            domain_color,
        ));
    }

    if let Some(selected) = activity_feed.selected()
        && activity_feed.active_filter.matches_row(selected)
    {
        let Some(details_bounds) = activity_feed_details_bounds(content_bounds, visible_rows)
        else {
            return;
        };
        let details_x = details_bounds.origin.x + 12.0;
        let mut details_y = details_bounds.origin.y;
        details_y = paint_label_line(paint, details_x, details_y, "Event ID", &selected.event_id);
        details_y = paint_label_line(paint, details_x, details_y, "Source", &selected.source_tag);
        paint.scene.draw_text(paint.text.layout(
            "Detail:",
            Point::new(details_x, details_y),
            11.0,
            theme::text::MUTED,
        ));
        let Some(detail_viewport) =
            activity_feed_detail_viewport_bounds(content_bounds, visible_rows)
        else {
            return;
        };
        let detail_lines = split_text_for_display(&selected.detail, 72);
        let visible_line_capacity = ((detail_viewport.size.height / 16.0).floor() as usize).max(1);
        let start_line =
            activity_feed.detail_scroll_offset_for(detail_lines.len(), visible_line_capacity);
        let end_line = (start_line + visible_line_capacity).min(detail_lines.len());

        paint.scene.push_clip(detail_viewport);
        let mut line_y = detail_viewport.origin.y;
        for line in detail_lines
            .iter()
            .skip(start_line)
            .take(end_line - start_line)
        {
            paint.scene.draw_text(paint.text.layout_mono(
                line,
                Point::new(detail_viewport.origin.x, line_y),
                11.0,
                theme::text::PRIMARY,
            ));
            line_y += 16.0;
        }
        paint.scene.pop_clip();

        if detail_lines.len() > visible_line_capacity {
            paint.scene.draw_text(paint.text.layout_mono(
                &format!(
                    "Detail lines {}-{} / {} (scroll)",
                    start_line.saturating_add(1),
                    end_line,
                    detail_lines.len()
                ),
                Point::new(details_x, details_bounds.max_y() - 8.0),
                10.0,
                theme::text::MUTED,
            ));
        }
    }
}

fn paint_alerts_recovery_pane(
    content_bounds: Bounds,
    alerts_recovery: &AlertsRecoveryState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let recover_bounds = alerts_recovery_recover_button_bounds(content_bounds);
    let ack_bounds = alerts_recovery_ack_button_bounds(content_bounds);
    let resolve_bounds = alerts_recovery_resolve_button_bounds(content_bounds);
    paint_action_button(recover_bounds, "Run recovery", paint);
    paint_action_button(ack_bounds, "Acknowledge", paint);
    paint_action_button(resolve_bounds, "Resolve", paint);

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        recover_bounds.max_y() + 12.0,
        alerts_recovery.load_state,
        &format!("State: {}", alerts_recovery.load_state.label()),
        alerts_recovery.last_action.as_deref(),
        alerts_recovery.last_error.as_deref(),
    );

    let visible_rows = alerts_recovery_visible_row_count(alerts_recovery.alerts.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No alerts active.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let row = &alerts_recovery.alerts[row_index];
        let row_bounds = alerts_recovery_row_bounds(content_bounds, row_index);
        let selected = alerts_recovery.selected_alert_id.as_deref() == Some(row.alert_id.as_str());
        paint_selectable_row_background(paint, row_bounds, selected);

        let severity_color = match row.severity {
            AlertSeverity::Info => theme::text::MUTED,
            AlertSeverity::Warning => theme::accent::PRIMARY,
            AlertSeverity::Critical => theme::status::ERROR,
        };
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "[{} {} {}] {}",
                row.domain.label(),
                row.severity.label(),
                row.lifecycle.label(),
                row.summary
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            severity_color,
        ));
    }

    if let Some(selected) = alerts_recovery.selected() {
        let details_top =
            alerts_recovery_row_bounds(content_bounds, visible_rows.saturating_sub(1)).max_y()
                + 10.0;
        let mut details_y = details_top;
        details_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Alert ID",
            &selected.alert_id,
        );
        details_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Domain",
            selected.domain.label(),
        );
        details_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Severity",
            selected.severity.label(),
        );
        details_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Lifecycle",
            selected.lifecycle.label(),
        );
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Remediation",
            &selected.remediation,
        );
    }
}

fn paint_settings_pane(
    content_bounds: Bounds,
    settings: &SettingsState,
    settings_inputs: &mut SettingsPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "local", paint);

    let relay_input_bounds = settings_relay_input_bounds(content_bounds);
    let wallet_input_bounds = settings_wallet_default_input_bounds(content_bounds);
    let provider_input_bounds = settings_provider_queue_input_bounds(content_bounds);
    let save_bounds = settings_save_button_bounds(content_bounds);
    let reset_bounds = settings_reset_button_bounds(content_bounds);

    paint.scene.draw_text(paint.text.layout(
        "Relay URL",
        Point::new(
            relay_input_bounds.origin.x,
            relay_input_bounds.origin.y - 8.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Wallet Default Send (₿)",
        Point::new(
            wallet_input_bounds.origin.x,
            wallet_input_bounds.origin.y - 8.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Provider Max Queue Depth",
        Point::new(
            provider_input_bounds.origin.x,
            provider_input_bounds.origin.y - 8.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    settings_inputs
        .relay_url
        .set_max_width(relay_input_bounds.size.width);
    settings_inputs
        .wallet_default_send_sats
        .set_max_width(wallet_input_bounds.size.width);
    settings_inputs
        .provider_max_queue_depth
        .set_max_width(provider_input_bounds.size.width);
    settings_inputs.relay_url.paint(relay_input_bounds, paint);
    settings_inputs
        .wallet_default_send_sats
        .paint(wallet_input_bounds, paint);
    settings_inputs
        .provider_max_queue_depth
        .paint(provider_input_bounds, paint);

    paint_primary_button(save_bounds, "Save settings", paint);
    paint_action_button(reset_bounds, "Reset defaults", paint);

    let state_color = match settings.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let mut y = save_bounds.max_y() + 12.0;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", settings.load_state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Identity path",
        &settings.document.identity_path,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Schema version",
        &settings.document.schema_version.to_string(),
    );

    let reconnect_note = if settings.document.reconnect_required {
        "Reconnect required for relay/provider changes."
    } else {
        "No reconnect required."
    };
    paint.scene.draw_text(paint.text.layout(
        reconnect_note,
        Point::new(content_bounds.origin.x + 12.0, y),
        10.0,
        if settings.document.reconnect_required {
            theme::accent::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
    y += 16.0;

    if let Some(action) = settings.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    if let Some(error) = settings.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::status::ERROR,
        ));
    }
}

fn paint_credentials_pane(
    content_bounds: Bounds,
    credentials: &CredentialsState,
    credentials_inputs: &mut CredentialsPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "local", paint);

    let name_input_bounds = credentials_name_input_bounds(content_bounds);
    let value_input_bounds = credentials_value_input_bounds(content_bounds);
    let add_custom_bounds = credentials_add_custom_button_bounds(content_bounds);
    let save_value_bounds = credentials_save_value_button_bounds(content_bounds);
    let delete_bounds = credentials_delete_button_bounds(content_bounds);
    let toggle_enabled_bounds = credentials_toggle_enabled_button_bounds(content_bounds);
    let import_bounds = credentials_import_button_bounds(content_bounds);
    let reload_bounds = credentials_reload_button_bounds(content_bounds);
    let scope_codex_bounds = credentials_scope_codex_button_bounds(content_bounds);
    let scope_spark_bounds = credentials_scope_spark_button_bounds(content_bounds);
    let scope_skills_bounds = credentials_scope_skills_button_bounds(content_bounds);
    let scope_global_bounds = credentials_scope_global_button_bounds(content_bounds);

    paint.scene.draw_text(paint.text.layout(
        "Variable name",
        Point::new(name_input_bounds.origin.x, name_input_bounds.origin.y - 8.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Value (secure storage)",
        Point::new(
            value_input_bounds.origin.x,
            value_input_bounds.origin.y - 8.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    credentials_inputs
        .variable_name
        .set_max_width(name_input_bounds.size.width);
    credentials_inputs
        .variable_value
        .set_max_width(value_input_bounds.size.width);
    credentials_inputs
        .variable_name
        .paint(name_input_bounds, paint);
    credentials_inputs
        .variable_value
        .paint(value_input_bounds, paint);

    let selected = credentials.selected_entry();
    let selected_template = selected.is_some_and(|entry| entry.template);
    let selected_enabled = selected.is_some_and(|entry| entry.enabled);
    let selected_scopes = selected.map_or(0, |entry| entry.scopes);

    paint_action_button(add_custom_bounds, "Add custom", paint);
    paint_action_button(save_value_bounds, "Save value", paint);
    paint_action_button(
        delete_bounds,
        if selected_template {
            "Clear value"
        } else {
            "Delete slot"
        },
        paint,
    );
    paint_action_button(
        toggle_enabled_bounds,
        if selected_enabled {
            "Disable slot"
        } else {
            "Enable slot"
        },
        paint,
    );
    paint_action_button(import_bounds, "Import env", paint);
    paint_action_button(reload_bounds, "Reload", paint);
    paint_action_button(
        scope_codex_bounds,
        &format!(
            "Codex:{}",
            if (selected_scopes & crate::credentials::CREDENTIAL_SCOPE_CODEX) != 0 {
                "on"
            } else {
                "off"
            }
        ),
        paint,
    );
    paint_action_button(
        scope_spark_bounds,
        &format!(
            "Spark:{}",
            if (selected_scopes & crate::credentials::CREDENTIAL_SCOPE_SPARK) != 0 {
                "on"
            } else {
                "off"
            }
        ),
        paint,
    );
    paint_action_button(
        scope_skills_bounds,
        &format!(
            "Skills:{}",
            if (selected_scopes & crate::credentials::CREDENTIAL_SCOPE_SKILLS) != 0 {
                "on"
            } else {
                "off"
            }
        ),
        paint,
    );
    paint_action_button(
        scope_global_bounds,
        &format!(
            "Global:{}",
            if (selected_scopes & crate::credentials::CREDENTIAL_SCOPE_GLOBAL) != 0 {
                "on"
            } else {
                "off"
            }
        ),
        paint,
    );

    let summary_y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        scope_global_bounds.max_y() + 10.0,
        credentials.load_state,
        &format!("State: {}", credentials.load_state.label()),
        credentials.last_action.as_deref(),
        credentials.last_error.as_deref(),
    );

    let visible_rows = credentials_visible_row_count(credentials.entries.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No credential slots available.",
            Point::new(content_bounds.origin.x + 12.0, summary_y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let entry = &credentials.entries[row_index];
        let row_bounds = credentials_row_bounds(content_bounds, row_index);
        let selected_row = credentials.selected_name.as_deref() == Some(entry.name.as_str());
        paint_selectable_row_background(paint, row_bounds, selected_row);

        let entry_type = if entry.template { "template" } else { "custom" };
        let summary = format!(
            "{} {} value:{} enabled:{} type:{} scopes:{}{}{}{}",
            if entry.secret { "[secret]" } else { "[text]" },
            entry.name,
            if entry.has_value { "set" } else { "missing" },
            if entry.enabled { "yes" } else { "no" },
            entry_type,
            if (entry.scopes & crate::credentials::CREDENTIAL_SCOPE_CODEX) != 0 {
                "C"
            } else {
                "-"
            },
            if (entry.scopes & crate::credentials::CREDENTIAL_SCOPE_SPARK) != 0 {
                "S"
            } else {
                "-"
            },
            if (entry.scopes & crate::credentials::CREDENTIAL_SCOPE_SKILLS) != 0 {
                "K"
            } else {
                "-"
            },
            if (entry.scopes & crate::credentials::CREDENTIAL_SCOPE_GLOBAL) != 0 {
                "G"
            } else {
                "-"
            },
        );
        paint.scene.draw_text(paint.text.layout_mono(
            summary.as_str(),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected_row {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));
    }
}

fn paint_nostr_identity_pane(
    content_bounds: Bounds,
    nostr_identity: Option<&nostr::NostrIdentity>,
    nostr_identity_error: Option<&str>,
    nostr_secret_state: &NostrSecretState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "local", paint);

    let now = std::time::Instant::now();
    let secrets_revealed = nostr_secret_state.is_revealed(now);
    let identity_state = nostr_identity_view_state(nostr_identity, nostr_identity_error);
    let identity_state_color = match identity_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };

    let regenerate_bounds = nostr_regenerate_button_bounds(content_bounds);
    let reveal_bounds = nostr_reveal_button_bounds(content_bounds);
    let copy_secret_bounds = nostr_copy_secret_button_bounds(content_bounds);
    paint_action_button(regenerate_bounds, "Regenerate keys", paint);
    paint_action_button(
        reveal_bounds,
        if secrets_revealed {
            "Hide secrets"
        } else {
            "Reveal 12s"
        },
        paint,
    );
    paint_action_button(copy_secret_bounds, "Copy nsec", paint);

    let mut y = regenerate_bounds.origin.y + regenerate_bounds.size.height + 14.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Identity path",
        &nostr_identity.map_or_else(
            || "Unavailable".to_string(),
            |identity| identity.identity_path.display().to_string(),
        ),
    );
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", identity_state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        identity_state_color,
    ));
    y += 16.0;

    if let Some(remaining) = nostr_secret_state
        .revealed_until
        .and_then(|until| until.checked_duration_since(now))
    {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Security",
            &format!(
                "Secrets visible for {:.0}s more. Values auto-hide for safety.",
                remaining.as_secs_f32().ceil()
            ),
        );
    }

    if let Some(copy_notice) = nostr_secret_state.copy_notice.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Clipboard",
            copy_notice,
        );
    }

    if let Some(identity) = nostr_identity {
        let nsec_display = if secrets_revealed {
            identity.nsec.clone()
        } else {
            mask_secret(&identity.nsec)
        };
        let private_hex_display = if secrets_revealed {
            identity.private_key_hex.clone()
        } else {
            mask_secret(&identity.private_key_hex)
        };
        let mnemonic_display = if secrets_revealed {
            identity.mnemonic.clone()
        } else {
            mask_mnemonic(&identity.mnemonic)
        };

        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "npub",
            &identity.npub,
        );
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "nsec",
            &nsec_display,
        );
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Public key (hex)",
            &identity.public_key_hex,
        );
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Private key (hex)",
            &private_hex_display,
        );
        let agent_preview = match nostr::derive_agent_keypair(&identity.mnemonic, 0)
            .and_then(|keypair| keypair.npub())
        {
            Ok(value) => value,
            Err(error) => format!("error:{error}"),
        };
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Agent account[0] npub",
            &agent_preview,
        );
        let skill_preview = match nostr::derive_skill_keypair(&identity.mnemonic, 0, 1, 0)
            .and_then(|keypair| keypair.npub())
        {
            Ok(value) => value,
            Err(error) => format!("error:{error}"),
        };
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Skill[agent0:1:0] npub",
            &skill_preview,
        );
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Mnemonic",
            &mnemonic_display,
        );
    } else if let Some(error) = nostr_identity_error {
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Identity error",
            error,
        );
    } else {
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Identity",
            "No identity loaded yet. Regenerate keys to initialize custody material.",
        );
    }
}

fn nostr_identity_view_state(
    nostr_identity: Option<&nostr::NostrIdentity>,
    nostr_identity_error: Option<&str>,
) -> PaneLoadState {
    if nostr_identity_error.is_some() {
        return PaneLoadState::Error;
    }
    if nostr_identity.is_some() {
        return PaneLoadState::Ready;
    }
    PaneLoadState::Loading
}

fn paint_job_inbox_pane(
    content_bounds: Bounds,
    job_inbox: &JobInboxState,
    provider_runtime: &ProviderRuntimeState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let accept_bounds = job_inbox_accept_button_bounds(content_bounds);
    let reject_bounds = job_inbox_reject_button_bounds(content_bounds);
    let preview_only = provider_runtime.mode == crate::app_state::ProviderMode::Offline;
    paint_primary_button(
        accept_bounds,
        if preview_only {
            "Go Online to claim"
        } else {
            "Accept selected"
        },
        paint,
    );
    paint_action_button(
        reject_bounds,
        if preview_only {
            "Preview only"
        } else {
            "Reject selected"
        },
        paint,
    );

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        accept_bounds.max_y() + 12.0,
        job_inbox.load_state,
        &format!("State: {}", job_inbox.load_state.label()),
        job_inbox.last_action.as_deref(),
        job_inbox.last_error.as_deref(),
    );

    if let Some(reason) = job_inbox.preview_block_reason(provider_runtime.mode) {
        paint.scene.draw_text(paint.text.layout(
            reason,
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::accent::PRIMARY,
        ));
        y += 16.0;
    }

    match job_inbox.load_state {
        PaneLoadState::Loading => {
            paint.scene.draw_text(paint.text.layout(
                "Waiting for deterministic replay cursor...",
                Point::new(content_bounds.origin.x + 12.0, y),
                11.0,
                theme::text::MUTED,
            ));
            return;
        }
        PaneLoadState::Error | PaneLoadState::Ready => {}
    }

    let visible_rows = job_inbox_visible_row_count(job_inbox.requests.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No requests in inbox.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let now_epoch_seconds = mission_control_now_epoch_seconds();
    for row_index in 0..visible_rows {
        let request = &job_inbox.requests[row_index];
        let demand_risk = request.demand_risk_assessment_at(now_epoch_seconds);
        let row_bounds = job_inbox_row_bounds(content_bounds, row_index);
        let selected =
            job_inbox.selected_request_id.as_deref() == Some(request.request_id.as_str());
        paint_selectable_row_background(paint, row_bounds, selected);

        let status_color = match request.validation {
            crate::app_state::JobInboxValidation::Valid => theme::status::SUCCESS,
            crate::app_state::JobInboxValidation::Pending => theme::accent::PRIMARY,
            crate::app_state::JobInboxValidation::Invalid(_) => theme::status::ERROR,
        };
        let summary = format!(
            "#{} {} {} src:{} risk:{}/{} scope:{} env:{} {} ttl:{}s fresh:{} {} {} eligibility:{}",
            request.arrival_seq,
            request.request_id,
            request.capability,
            request.demand_source.label(),
            demand_risk.class.label(),
            demand_risk.disposition.label(),
            request.skill_scope_id.as_deref().unwrap_or("none"),
            request.ac_envelope_event_id.as_deref().unwrap_or("none"),
            format_sats_amount(request.price_sats),
            request.ttl_seconds,
            request_freshness_summary(request, now_epoch_seconds),
            request.validation.label(),
            request.decision.label(),
            request.eligibility_label(provider_runtime.mode)
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected {
                theme::text::PRIMARY
            } else {
                status_color
            },
        ));
    }

    if let Some(selected) = job_inbox.selected_request() {
        let selected_demand_risk = selected.demand_risk_assessment_at(now_epoch_seconds);
        let details_y =
            job_inbox_row_bounds(content_bounds, visible_rows.saturating_sub(1)).max_y() + 12.0;
        let x = content_bounds.origin.x + 12.0;
        let mut line_y = details_y;
        line_y = paint_label_line(paint, x, line_y, "Selected requester", &selected.requester);
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Selected request id",
            &selected.request_id,
        );
        line_y = paint_label_line(paint, x, line_y, "Decision", &selected.decision.label());
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Eligibility",
            selected.eligibility_label(provider_runtime.mode),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Demand source",
            selected.demand_source.label(),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Request freshness",
            request_freshness_summary(selected, now_epoch_seconds).as_str(),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Request created",
            &format_epoch_seconds_option(selected.created_at_epoch_seconds),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Request expires",
            &format_epoch_seconds_option(selected.expires_at_epoch_seconds),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Demand risk",
            selected_demand_risk.class.label(),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Risk policy",
            selected_demand_risk.disposition.label(),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Risk note",
            selected_demand_risk.note.as_str(),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Skill scope",
            selected.skill_scope_id.as_deref().unwrap_or("none"),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "SKL manifest a",
            selected.skl_manifest_a.as_deref().unwrap_or("none"),
        );
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "SA tick request",
            selected
                .sa_tick_request_event_id
                .as_deref()
                .unwrap_or("none"),
        );
        let _ = paint_label_line(
            paint,
            x,
            line_y,
            "AC envelope",
            selected.ac_envelope_event_id.as_deref().unwrap_or("none"),
        );
    }
}

fn format_epoch_seconds_option(epoch_seconds: Option<u64>) -> String {
    epoch_seconds
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn request_freshness_summary(request: &JobInboxRequest, now_epoch_seconds: u64) -> String {
    match (
        request.created_at_epoch_seconds,
        request.expires_at_epoch_seconds,
    ) {
        (Some(created_at), Some(expires_at)) if now_epoch_seconds >= expires_at => format!(
            "expired {}s ago (created={} expires={})",
            now_epoch_seconds.saturating_sub(expires_at),
            created_at,
            expires_at
        ),
        (Some(created_at), Some(expires_at)) => format!(
            "fresh // {}s left (created={} expires={})",
            expires_at.saturating_sub(now_epoch_seconds),
            created_at,
            expires_at
        ),
        (_, Some(expires_at)) => format!(
            "expires {}",
            if now_epoch_seconds >= expires_at {
                format!("{}s ago", now_epoch_seconds.saturating_sub(expires_at))
            } else {
                format!("in {}s", expires_at.saturating_sub(now_epoch_seconds))
            }
        ),
        _ => "unknown".to_string(),
    }
}

fn paint_active_job_pane(
    content_bounds: Bounds,
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(
        content_bounds,
        &earn_job_lifecycle_projection.stream_id,
        paint,
    );

    let advance_bounds = active_job_advance_button_bounds(content_bounds);
    let abort_bounds = active_job_abort_button_bounds(content_bounds);
    let copy_bounds = active_job_copy_button_bounds(content_bounds);
    paint_disabled_button(advance_bounds, "Execution auto", paint);
    if active_job.runtime_supports_abort {
        paint_action_button(abort_bounds, "Abort job", paint);
    } else {
        paint_disabled_button(abort_bounds, "Abort unsupported", paint);
    }
    paint_action_button(copy_bounds, "Copy all", paint);
    let viewport =
        active_job_scroll_viewport_bounds(content_bounds, active_job.runtime_supports_abort);
    paint.scene.push_clip(viewport);
    let chunk_len = (((viewport.size.width - 8.0).max(48.0) / 6.2).floor() as usize).max(12);
    let lines = build_active_job_scroll_lines(
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
        chunk_len,
    );
    let line_height = 14.0;
    let content_height = (lines.len() as f32 * line_height).max(viewport.size.height);
    let max_offset = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = active_job.scroll_offset_px.clamp(0.0, max_offset);
    let mut line_y = viewport.origin.y - scroll_offset;
    for line in lines.iter() {
        if line_y + line_height < viewport.origin.y {
            line_y += line_height;
            continue;
        }
        if line_y > viewport.max_y() {
            break;
        }
        paint.scene.draw_text(paint.text.layout_mono(
            &line.text,
            Point::new(viewport.origin.x, line_y),
            10.0,
            line.color,
        ));
        line_y += line_height;
    }
    paint.scene.pop_clip();

    if max_offset > 0.0 {
        let track = Bounds::new(
            content_bounds.max_x() - 8.0,
            viewport.origin.y,
            4.0,
            viewport.size.height,
        );
        let thumb_height = scrollbar_thumb_height(viewport.size.height, content_height, 18.0);
        let thumb_y = viewport.origin.y
            + ((scroll_offset / max_offset.max(1.0)) * (viewport.size.height - thumb_height));
        paint
            .scene
            .draw_quad(Quad::new(track).with_background(theme::bg::APP.with_alpha(0.45)));
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                track.origin.x,
                thumb_y,
                track.size.width,
                thumb_height,
            ))
            .with_background(theme::accent::PRIMARY.with_alpha(0.75)),
        );
    }
}

pub(crate) fn active_job_clipboard_text(
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
) -> String {
    let mut lines = vec!["Active Job".to_string(), String::new()];
    lines.extend(
        build_active_job_scroll_lines(active_job, earn_job_lifecycle_projection, spark_wallet, 120)
            .into_iter()
            .map(|line| line.text),
    );
    lines.join("\n")
}

struct ActiveJobRenderLine {
    text: String,
    color: Hsla,
}

fn push_active_job_wrapped_line(
    lines: &mut Vec<ActiveJobRenderLine>,
    prefix: &str,
    value: &str,
    chunk_len: usize,
    color: Hsla,
) {
    let prefix = prefix.to_string();
    let indent = " ".repeat(prefix.chars().count());
    let available = chunk_len.saturating_sub(prefix.chars().count()).max(8);
    let wrapped = split_text_for_display(value, available);
    for (index, chunk) in wrapped.into_iter().enumerate() {
        let text = if index == 0 {
            format!("{prefix}{chunk}")
        } else {
            format!("{indent}{chunk}")
        };
        lines.push(ActiveJobRenderLine { text, color });
    }
}

fn active_job_timeline_stage_reached(
    active_job: &ActiveJobState,
    stage: JobLifecycleStage,
) -> bool {
    let Some(job) = active_job.job.as_ref() else {
        return false;
    };

    if job.stage != JobLifecycleStage::Failed {
        return match stage {
            JobLifecycleStage::Received => true,
            JobLifecycleStage::Accepted => true,
            JobLifecycleStage::Running => matches!(
                job.stage,
                JobLifecycleStage::Running | JobLifecycleStage::Delivered | JobLifecycleStage::Paid
            ),
            JobLifecycleStage::Delivered => {
                matches!(
                    job.stage,
                    JobLifecycleStage::Delivered | JobLifecycleStage::Paid
                )
            }
            JobLifecycleStage::Paid => job.stage == JobLifecycleStage::Paid,
            JobLifecycleStage::Failed => false,
        };
    }

    match stage {
        JobLifecycleStage::Received => true,
        JobLifecycleStage::Accepted => true,
        JobLifecycleStage::Running => {
            active_job.execution_turn_completed
                || active_job.execution_output.as_deref().is_some()
                || active_job.result_publish_in_flight
                || job.sa_tick_result_event_id.is_some()
                || active_job_has_authoritative_payment_pointer(job.payment_id.as_deref())
        }
        JobLifecycleStage::Delivered => {
            job.sa_tick_result_event_id.is_some()
                || active_job_has_authoritative_payment_pointer(job.payment_id.as_deref())
        }
        JobLifecycleStage::Paid => {
            active_job_has_authoritative_payment_pointer(job.payment_id.as_deref())
        }
        JobLifecycleStage::Failed => true,
    }
}

fn active_job_has_authoritative_payment_pointer(pointer: Option<&str>) -> bool {
    let Some(pointer) = pointer else {
        return false;
    };
    let pointer = pointer.trim();
    !pointer.is_empty()
        && !pointer.starts_with("pending:")
        && !pointer.starts_with("pay:")
        && !pointer.starts_with("inv-")
        && !pointer.starts_with("pay-req-")
}

fn active_job_result_publish_status(active_job: &ActiveJobState) -> String {
    let Some(job) = active_job.job.as_ref() else {
        return "n/a".to_string();
    };
    if job.sa_tick_result_event_id.is_some() {
        return "confirmed on relays".to_string();
    }
    let age_suffix = active_job
        .result_publish_last_queued_epoch_seconds
        .map(|queued_at| {
            let now_epoch_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            format!(
                " queued {}s ago",
                now_epoch_seconds.saturating_sub(queued_at)
            )
        })
        .unwrap_or_default();
    if active_job.result_publish_in_flight {
        return format!(
            "awaiting relay confirmation attempt #{}{}",
            active_job.result_publish_attempt_count.max(1),
            age_suffix
        );
    }
    if active_job.pending_result_publish_event_id.is_some() {
        return format!(
            "retry pending attempt #{}{}",
            active_job.result_publish_attempt_count.max(1),
            age_suffix
        );
    }
    "not queued".to_string()
}

fn active_job_request_freshness_summary(job: &ActiveJobRecord, now_epoch_seconds: u64) -> String {
    match (
        job.accepted_at_epoch_seconds,
        job.request_created_at_epoch_seconds,
        job.request_expires_at_epoch_seconds,
    ) {
        (Some(accepted_at), Some(created_at), Some(expires_at)) if accepted_at <= expires_at => {
            format!(
                "accepted fresh // {}s remaining at accept (created={} accepted={} expires={})",
                expires_at.saturating_sub(accepted_at),
                created_at,
                accepted_at,
                expires_at
            )
        }
        (Some(accepted_at), Some(created_at), Some(expires_at)) => format!(
            "accepted stale // {}s after expiry (created={} accepted={} expires={})",
            accepted_at.saturating_sub(expires_at),
            created_at,
            accepted_at,
            expires_at
        ),
        (_, Some(created_at), Some(expires_at)) if now_epoch_seconds >= expires_at => format!(
            "expired {}s ago (created={} expires={})",
            now_epoch_seconds.saturating_sub(expires_at),
            created_at,
            expires_at
        ),
        (_, Some(created_at), Some(expires_at)) => format!(
            "fresh // {}s left (created={} expires={})",
            expires_at.saturating_sub(now_epoch_seconds),
            created_at,
            expires_at
        ),
        _ => "unknown".to_string(),
    }
}

fn build_active_job_scroll_lines(
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
    chunk_len: usize,
) -> Vec<ActiveJobRenderLine> {
    let mut lines = Vec::new();
    let state_color = match active_job.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    lines.push(ActiveJobRenderLine {
        text: format!("State: {}", active_job.load_state.label()),
        color: state_color,
    });
    if !active_job.runtime_supports_abort {
        lines.push(ActiveJobRenderLine {
            text: "Abort disabled: runtime lane does not support cancel.".to_string(),
            color: theme::text::MUTED,
        });
    }
    if let Some(action) = active_job.last_action.as_deref() {
        push_active_job_wrapped_line(
            &mut lines,
            "Action: ",
            action,
            chunk_len,
            theme::text::MUTED,
        );
    }
    if let Some(error) = active_job.last_error.as_deref() {
        push_active_job_wrapped_line(
            &mut lines,
            "Error: ",
            error,
            chunk_len,
            theme::status::ERROR,
        );
    }

    if active_job.load_state == PaneLoadState::Loading {
        lines.push(ActiveJobRenderLine {
            text: "Waiting for active-job replay frame...".to_string(),
            color: theme::text::MUTED,
        });
        return lines;
    }

    let Some(job) = active_job.job.as_ref() else {
        lines.push(ActiveJobRenderLine {
            text: "No active job selected.".to_string(),
            color: theme::text::MUTED,
        });
        return lines;
    };
    let flow_snapshot = crate::nip90_compute_flow::build_active_job_flow_snapshot(
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
    )
    .expect("active job snapshot should exist when job exists");
    let now_epoch_seconds = mission_control_now_epoch_seconds();

    let pending_result_event_id = active_job
        .pending_result_publish_event_id
        .as_deref()
        .unwrap_or("n/a");
    let stage_display = active_job_stage_display(job.stage, flow_snapshot.phase);
    let metadata_rows = vec![
        ("Job ID", job.job_id.clone()),
        ("Requester", job.requester.clone()),
        ("Capability", job.capability.clone()),
        ("Demand source", job.demand_source.label().to_string()),
        ("Demand risk", job.demand_risk_class.label().to_string()),
        (
            "Risk policy",
            job.demand_risk_disposition.label().to_string(),
        ),
        ("Stage", stage_display),
        (
            "Flow authority",
            flow_snapshot.authority.as_str().to_string(),
        ),
        ("Flow phase", flow_snapshot.phase.as_str().to_string()),
        ("Next event", flow_snapshot.next_expected_event.clone()),
        (
            "Request freshness",
            active_job_request_freshness_summary(job, now_epoch_seconds),
        ),
        (
            "Request created",
            format_epoch_seconds_option(job.request_created_at_epoch_seconds),
        ),
        (
            "Request expires",
            format_epoch_seconds_option(job.request_expires_at_epoch_seconds),
        ),
        (
            "Projection authority",
            flow_snapshot.projection_authority.as_str().to_string(),
        ),
        (
            "Skill scope",
            job.skill_scope_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "SKL manifest",
            job.skl_manifest_a
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "SA tick request",
            job.sa_tick_request_event_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "SA tick result",
            job.sa_tick_result_event_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "Trajectory session",
            job.sa_trajectory_session_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "AC envelope",
            job.ac_envelope_event_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "AC settlement",
            job.ac_settlement_event_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "AC default",
            job.ac_default_event_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        (
            "Invoice ID",
            job.invoice_id.clone().unwrap_or_else(|| "n/a".to_string()),
        ),
        (
            "Payment ID",
            job.payment_id.clone().unwrap_or_else(|| "n/a".to_string()),
        ),
        ("Result event", pending_result_event_id.to_string()),
        (
            "Result publish",
            flow_snapshot.result_publish_status.clone(),
        ),
    ];
    for (label, value) in metadata_rows {
        push_active_job_wrapped_line(
            &mut lines,
            &format!("{label}: "),
            value.as_str(),
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    push_active_job_wrapped_line(
        &mut lines,
        "Risk note: ",
        job.demand_risk_note.as_str(),
        chunk_len,
        theme::text::PRIMARY,
    );
    if let Some(window_seconds) = flow_snapshot.continuity_window_seconds {
        push_active_job_wrapped_line(
            &mut lines,
            "Continuity window: ",
            &format!("{window_seconds}s"),
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if let Some(bolt11) = flow_snapshot.pending_bolt11.as_deref() {
        let compact_invoice = crate::nip90_compute_flow::compact_payment_invoice(bolt11);
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement invoice: ",
            compact_invoice.as_str(),
            chunk_len,
            theme::text::PRIMARY,
        );
    } else if matches!(
        flow_snapshot.phase,
        crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment
            | crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet
            | crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment
            | crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid
    ) {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement invoice: ",
            "none",
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement outcome: ",
            "compute completed and the result was delivered; preparing a Lightning invoice for buyer settlement",
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement outcome: ",
            "compute completed and the result was delivered; awaiting buyer Lightning payment",
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet
    {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement outcome: ",
            "seller settlement appears confirmed, but local buyer wallet confirmation is still pending",
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement outcome: ",
            "compute completed and the result was delivered, but buyer settlement never arrived",
            chunk_len,
            theme::status::ERROR,
        );
    }
    if let Some(status) = flow_snapshot.settlement_status.as_deref() {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement status: ",
            status,
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if let Some(method) = flow_snapshot.settlement_method.as_deref() {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement method: ",
            method,
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if let Some(amount) = flow_snapshot.settlement_amount_sats {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement amount: ",
            &format!("{amount} sats"),
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if let Some(fees) = flow_snapshot.settlement_fees_sats {
        push_active_job_wrapped_line(
            &mut lines,
            "Settlement fees: ",
            &format!("{fees} sats"),
            chunk_len,
            theme::text::PRIMARY,
        );
    }
    if let Some(delta) = flow_snapshot.settlement_net_wallet_delta_sats {
        let delta_label = crate::spark_wallet::format_wallet_delta_sats(delta);
        push_active_job_wrapped_line(
            &mut lines,
            "Wallet delta: ",
            delta_label.as_str(),
            chunk_len,
            theme::text::PRIMARY,
        );
    }

    lines.push(ActiveJobRenderLine {
        text: "Timeline".to_string(),
        color: theme::text::MUTED,
    });
    let stage_flow = [
        JobLifecycleStage::Received,
        JobLifecycleStage::Accepted,
        JobLifecycleStage::Running,
        JobLifecycleStage::Delivered,
        JobLifecycleStage::Paid,
    ];
    for stage in stage_flow {
        let reached = active_job_timeline_stage_reached(active_job, stage);
        lines.push(ActiveJobRenderLine {
            text: format!("[{}] {}", if reached { "x" } else { " " }, stage.label()),
            color: if reached {
                theme::status::SUCCESS
            } else {
                theme::text::MUTED
            },
        });
    }

    if let Some(reason) = job.failure_reason.as_deref() {
        push_active_job_wrapped_line(
            &mut lines,
            "Failure reason: ",
            reason,
            chunk_len,
            theme::status::ERROR,
        );
    }

    lines.push(ActiveJobRenderLine {
        text: "Execution log".to_string(),
        color: theme::text::MUTED,
    });
    for event in job.events.iter() {
        push_active_job_wrapped_line(
            &mut lines,
            &format!("[#{:03}] ", event.seq),
            &event.message,
            chunk_len,
            theme::text::PRIMARY,
        );
    }

    lines
}

fn paint_job_history_pane(
    content_bounds: Bounds,
    job_history: &JobHistoryState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    job_history_inputs: &mut JobHistoryPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(
        content_bounds,
        &earn_job_lifecycle_projection.stream_id,
        paint,
    );

    let search_bounds = job_history_search_input_bounds(content_bounds);
    let status_bounds = job_history_status_button_bounds(content_bounds);
    let time_bounds = job_history_time_button_bounds(content_bounds);
    let prev_bounds = job_history_prev_page_button_bounds(content_bounds);
    let next_bounds = job_history_next_page_button_bounds(content_bounds);

    job_history_inputs
        .search_job_id
        .set_max_width(search_bounds.size.width);
    job_history_inputs.search_job_id.paint(search_bounds, paint);
    paint_tertiary_button(
        status_bounds,
        &format!("Status: {}", job_history.status_filter.label()),
        paint,
    );
    paint_tertiary_button(
        time_bounds,
        &format!("Range: {}", job_history.time_range.label()),
        paint,
    );
    paint_tertiary_button(prev_bounds, "Prev", paint);
    paint_tertiary_button(next_bounds, "Next", paint);

    paint.scene.draw_text(paint.text.layout(
        "Search job id",
        Point::new(search_bounds.origin.x, search_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let state_color = match job_history.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let mut y = search_bounds.max_y() + 12.0;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", job_history.load_state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;

    let page = job_history
        .page
        .min(job_history.total_pages().saturating_sub(1))
        + 1;
    paint.scene.draw_text(paint.text.layout(
        &format!("Page {page}/{}", job_history.total_pages()),
        Point::new(content_bounds.origin.x + 12.0, y),
        10.0,
        theme::text::MUTED,
    ));
    y += 16.0;
    paint.scene.draw_text(paint.text.layout(
        &format!(
            "Projection: {} (rows={})",
            earn_job_lifecycle_projection.authority,
            earn_job_lifecycle_projection.rows.len()
        ),
        Point::new(content_bounds.origin.x + 12.0, y),
        10.0,
        theme::text::MUTED,
    ));
    y += 16.0;

    if let Some(action) = job_history.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    if let Some(error) = job_history.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::status::ERROR,
        ));
        y += 16.0;
    }

    if job_history.load_state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Loading deterministic history receipts...",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let rows = job_history.paged_rows();
    if rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No rows for the current filters.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (idx, row) in rows.iter().enumerate() {
        let row_top = y + idx as f32 * 34.0;
        let row_bounds = Bounds::new(
            content_bounds.origin.x + 12.0,
            row_top,
            (content_bounds.size.width - 24.0).max(200.0),
            30.0,
        );
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(theme::bg::APP.with_alpha(0.78))
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(6.0),
        );
        let row_line = format!(
            "{} {} src:{} payer_nostr:{} payee_nostr:{} ts:{} scope:{} tick:{} set:{} def:{} proof:{} qty:{}/{} var:{} rej:{} {} {}",
            row.job_id,
            row.status.label(),
            row.demand_source.label(),
            row.requester_nostr_pubkey.as_deref().unwrap_or("unknown"),
            row.provider_nostr_pubkey.as_deref().unwrap_or("unknown"),
            row.completed_at_epoch_seconds,
            row.skill_scope_id.as_deref().unwrap_or("none"),
            row.sa_tick_result_event_id.as_deref().unwrap_or("none"),
            row.ac_settlement_event_id.as_deref().unwrap_or("none"),
            row.ac_default_event_id.as_deref().unwrap_or("none"),
            row.delivery_proof_status_label.as_deref().unwrap_or("none"),
            row.delivery_accepted_quantity.unwrap_or(0),
            row.delivery_metered_quantity.unwrap_or(0),
            row.delivery_variance_reason_label
                .as_deref()
                .unwrap_or("none"),
            row.delivery_rejection_reason_label
                .as_deref()
                .unwrap_or("none"),
            row.result_hash,
            row.payment_pointer
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &row_line,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 8.0),
            10.0,
            if row.status == crate::app_state::JobHistoryStatus::Succeeded {
                theme::status::SUCCESS
            } else {
                theme::status::ERROR
            },
        ));
    }
}

fn paint_spark_wallet_pane(
    content_bounds: Bounds,
    spark_wallet: &SparkPaneState,
    spark_inputs: &mut SparkPaneInputs,
    paint: &mut PaintContext,
) {
    wallet_pane::paint_wallet_pane(content_bounds, spark_wallet, spark_inputs, paint);
}

#[cfg(test)]
fn spark_wallet_view_state(spark_wallet: &SparkPaneState) -> PaneLoadState {
    wallet_pane::spark_wallet_view_state(spark_wallet)
}

fn paint_create_invoice_pane(
    content_bounds: Bounds,
    spark_wallet: &SparkPaneState,
    create_invoice_inputs: &mut CreateInvoicePaneInputs,
    paint: &mut PaintContext,
) {
    wallet_pane::paint_create_invoice_pane(
        content_bounds,
        spark_wallet,
        create_invoice_inputs,
        paint,
    );
}

#[cfg(test)]
fn create_invoice_view_state(spark_wallet: &SparkPaneState) -> PaneLoadState {
    wallet_pane::create_invoice_view_state(spark_wallet)
}

fn paint_pay_invoice_pane(
    content_bounds: Bounds,
    spark_wallet: &SparkPaneState,
    pay_invoice_inputs: &mut PayInvoicePaneInputs,
    paint: &mut PaintContext,
) {
    wallet_pane::paint_pay_invoice_pane(content_bounds, spark_wallet, pay_invoice_inputs, paint);
}

#[cfg(test)]
fn pay_invoice_view_state(spark_wallet: &SparkPaneState) -> PaneLoadState {
    wallet_pane::pay_invoice_view_state(spark_wallet)
}

#[cfg(test)]
fn payment_terminal_status(spark_wallet: &SparkPaneState) -> &str {
    wallet_pane::payment_terminal_status(spark_wallet)
}

pub(crate) fn paint_state_summary(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    load_state: PaneLoadState,
    summary: &str,
    last_action: Option<&str>,
    last_error: Option<&str>,
) -> f32 {
    let state_color = match load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };

    let mut line_y = y;
    paint.scene.draw_text(
        paint
            .text
            .layout(summary, Point::new(x, line_y), 11.0, state_color),
    );
    line_y += 16.0;

    if let Some(action) = last_action {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(x, line_y),
            10.0,
            theme::text::MUTED,
        ));
        line_y += 16.0;
    }
    if let Some(error) = last_error {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(x, line_y),
            10.0,
            theme::status::ERROR,
        ));
        line_y += 16.0;
    }

    line_y
}

pub(crate) fn paint_selectable_row_background(
    paint: &mut PaintContext,
    row_bounds: Bounds,
    selected: bool,
) {
    paint.scene.draw_quad(
        Quad::new(row_bounds)
            .with_background(if selected {
                theme::accent::PRIMARY.with_alpha(0.18)
            } else {
                theme::bg::APP.with_alpha(0.78)
            })
            .with_border(
                if selected {
                    theme::accent::PRIMARY
                } else {
                    theme::border::DEFAULT
                },
                1.0,
            )
            .with_corner_radius(6.0),
    );
}

pub(crate) fn paint_source_badge(content_bounds: Bounds, source: &str, paint: &mut PaintContext) {
    let label = format!("source: {source}");
    let max_width = (content_bounds.size.width - 20.0).max(84.0);
    let badge_width = (label.chars().count() as f32 * 6.4 + 12.0).min(max_width);
    let badge_bounds = Bounds::new(
        content_bounds.max_x() - badge_width - 10.0,
        content_bounds.origin.y + 8.0,
        badge_width,
        18.0,
    );

    paint.scene.draw_quad(
        Quad::new(badge_bounds)
            .with_background(theme::bg::APP.with_alpha(0.88))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(3.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &label,
        Point::new(badge_bounds.origin.x + 6.0, badge_bounds.origin.y + 6.0),
        9.0,
        theme::text::MUTED,
    ));
}

pub(crate) fn paint_action_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint_button(bounds, label, ButtonStyle::Secondary, paint);
}

pub(crate) fn paint_primary_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint_button(bounds, label, ButtonStyle::Primary, paint);
}

pub(crate) fn paint_secondary_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint_button(bounds, label, ButtonStyle::Secondary, paint);
}

pub(crate) fn paint_tertiary_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint_button(bounds, label, ButtonStyle::Tertiary, paint);
}

fn paint_filter_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    if active {
        paint_secondary_button(bounds, label, paint);
        return;
    }
    paint_tertiary_button(bounds, label, paint);
}

pub(crate) fn paint_disabled_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint_button(bounds, label, ButtonStyle::Disabled, paint);
}

fn paint_mission_control_go_online_button(
    bounds: Bounds,
    label: &str,
    enabled: bool,
    _accent: Hsla,
    paint: &mut PaintContext,
) {
    let is_go_online = label == "GO ONLINE";
    let border = if enabled {
        if is_go_online {
            mission_control_green_color()
        } else {
            mission_control_orange_color()
        }
    } else {
        mission_control_panel_border_color()
    };
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    let pulse = ((now_secs * 2.4).sin() * 0.5) + 0.5;
    let glow_alpha = if enabled && is_go_online {
        0.14 + pulse * 0.08
    } else if enabled {
        0.10
    } else {
        0.0
    };
    if glow_alpha > 0.0 {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x - 6.0,
                bounds.origin.y - 6.0,
                bounds.size.width + 12.0,
                bounds.size.height + 12.0,
            ))
            .with_background(border.with_alpha(glow_alpha * 0.38))
            .with_border(border.with_alpha(glow_alpha), 1.0)
            .with_corner_radius(12.0),
        );
    }

    if is_go_online && enabled {
        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(Hsla::from_hex(0x142019))
                .with_border(border, 1.0)
                .with_corner_radius(4.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + 1.0,
                bounds.origin.y + 1.0,
                (bounds.size.width - 2.0).max(0.0),
                (bounds.size.height * 0.56).max(0.0),
            ))
            .with_background(border.with_alpha(0.24))
            .with_corner_radius(3.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + 1.0,
                bounds.origin.y + bounds.size.height * 0.42,
                (bounds.size.width - 2.0).max(0.0),
                (bounds.size.height * 0.58 - 1.0).max(0.0),
            ))
            .with_background(Hsla::from_hex(0x111A15).with_alpha(0.92))
            .with_corner_radius(3.0),
        );
    } else if !is_go_online && enabled {
        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(Hsla::from_hex(0x251A17))
                .with_border(border, 1.0)
                .with_corner_radius(4.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + 1.0,
                bounds.origin.y + 1.0,
                (bounds.size.width - 2.0).max(0.0),
                (bounds.size.height * 0.56).max(0.0),
            ))
            .with_background(border.with_alpha(0.20))
            .with_corner_radius(3.0),
        );
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + 1.0,
                bounds.origin.y + bounds.size.height * 0.42,
                (bounds.size.width - 2.0).max(0.0),
                (bounds.size.height * 0.58 - 1.0).max(0.0),
            ))
            .with_background(Hsla::from_hex(0x2D1B15).with_alpha(0.92))
            .with_corner_radius(3.0),
        );
    } else {
        paint.scene.draw_quad(
            Quad::new(bounds)
                .with_background(if enabled {
                    mission_control_panel_header_color()
                } else {
                    mission_control_panel_color()
                })
                .with_border(border, 1.0)
                .with_corner_radius(4.0),
        );
    }
    paint_button_label_mono(
        bounds,
        label,
        if is_go_online { 24.0 } else { 18.0 },
        if enabled {
            Hsla::from_hex(0xFFFFFF)
        } else {
            mission_control_muted_color()
        },
        paint,
    );
}

fn paint_mission_control_command_button(
    bounds: Bounds,
    label: &str,
    accent: Hsla,
    enabled: bool,
    paint: &mut PaintContext,
) {
    let border = if enabled {
        accent
    } else {
        mission_control_panel_border_color()
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(if enabled {
                mission_control_panel_header_color().with_alpha(0.55)
            } else {
                mission_control_panel_color()
            })
            .with_border(border.with_alpha(if enabled { 0.85 } else { 0.5 }), 1.0)
            .with_corner_radius(3.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            1.0,
        ))
        .with_background(border.with_alpha(if enabled { 0.2 } else { 0.08 })),
    );
    paint_button_label_mono(
        bounds,
        label,
        12.0,
        if enabled {
            mission_control_text_color()
        } else {
            mission_control_muted_color()
        },
        paint,
    );
}

fn paint_mission_control_wallet_refresh_icon_button(
    bounds: Bounds,
    color: Hsla,
    hovered: bool,
    click_feedback: f32,
    paint: &mut PaintContext,
) {
    let feedback = click_feedback.clamp(0.0, 1.0);
    if hovered || feedback > 0.0 {
        let glow_alpha = (if hovered { 0.16 } else { 0.0 }) + feedback * 0.34;
        let border_alpha = (if hovered { 0.38 } else { 0.0 }) + feedback * 0.52;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x - 3.0,
                bounds.origin.y - 3.0,
                bounds.size.width + 6.0,
                bounds.size.height + 6.0,
            ))
            .with_background(color.with_alpha(glow_alpha.clamp(0.0, 0.62)))
            .with_border(color.with_alpha(border_alpha.clamp(0.0, 0.86)), 1.0)
            .with_corner_radius(6.0),
        );
    }
    let icon_size = 14.0f32
        .min(bounds.size.width.max(0.0))
        .min(bounds.size.height.max(0.0));
    if icon_size <= 0.0 {
        return;
    }
    let icon_bounds = Bounds::new(
        bounds.origin.x + (bounds.size.width - icon_size) * 0.5,
        bounds.origin.y + (bounds.size.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    paint.scene.draw_svg(
        SvgQuad::new(
            icon_bounds,
            Arc::<[u8]>::from(MISSION_CONTROL_REFRESH_ICON_SVG_RAW.as_bytes()),
        )
        .with_tint(color.with_alpha((0.82 + feedback * 0.18).clamp(0.0, 1.0))),
    );
}

fn paint_mission_control_log_copy_icon_button(
    bounds: Bounds,
    color: Hsla,
    hovered: bool,
    click_feedback: f32,
    paint: &mut PaintContext,
) {
    let feedback = click_feedback.clamp(0.0, 1.0);
    if hovered || feedback > 0.0 {
        let glow_alpha = (if hovered { 0.16 } else { 0.0 }) + feedback * 0.34;
        let border_alpha = (if hovered { 0.38 } else { 0.0 }) + feedback * 0.52;
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x - 3.0,
                bounds.origin.y - 3.0,
                bounds.size.width + 6.0,
                bounds.size.height + 6.0,
            ))
            .with_background(color.with_alpha(glow_alpha.clamp(0.0, 0.62)))
            .with_border(color.with_alpha(border_alpha.clamp(0.0, 0.86)), 1.0)
            .with_corner_radius(6.0),
        );
    }
    let icon_size = 14.0f32
        .min(bounds.size.width.max(0.0))
        .min(bounds.size.height.max(0.0));
    if icon_size <= 0.0 {
        return;
    }
    let icon_bounds = Bounds::new(
        bounds.origin.x + (bounds.size.width - icon_size) * 0.5,
        bounds.origin.y + (bounds.size.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    paint.scene.draw_svg(
        SvgQuad::new(
            icon_bounds,
            Arc::<[u8]>::from(MISSION_CONTROL_COPY_ICON_SVG_RAW.as_bytes()),
        )
        .with_tint(color.with_alpha((0.82 + feedback * 0.18).clamp(0.0, 1.0))),
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonStyle {
    Primary,
    Secondary,
    Tertiary,
    Disabled,
}

fn paint_button(bounds: Bounds, label: &str, style: ButtonStyle, paint: &mut PaintContext) {
    match style {
        ButtonStyle::Primary => {
            let glow = Hsla::from_hex(0x0891B2);
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x - 4.0,
                    bounds.origin.y - 4.0,
                    bounds.size.width + 8.0,
                    bounds.size.height + 8.0,
                ))
                .with_background(glow.with_alpha(0.08))
                .with_border(theme::border::DEFAULT.with_alpha(0.0), 1.0)
                .with_corner_radius(14.0),
            );
            paint.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(Hsla::from_hex(0x121419).with_alpha(0.85))
                    .with_border(Hsla::from_hex(0x0891B2), 1.0)
                    .with_corner_radius(10.0),
            );
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + 1.0,
                    bounds.origin.y + 1.0,
                    (bounds.size.width - 2.0).max(0.0),
                    (bounds.size.height * 0.62).max(0.0),
                ))
                .with_background(Hsla::from_hex(0x03857F).with_alpha(0.34))
                .with_corner_radius(9.0),
            );
            paint_button_label_mono(bounds, label, 18.0, Hsla::from_hex(0xFFFFFF), paint);
        }
        ButtonStyle::Secondary => {
            paint.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(theme::bg::HOVER)
                    .with_border(theme::border::DEFAULT, 1.0)
                    .with_corner_radius(6.0),
            );
            paint_button_label(
                bounds,
                label,
                theme::font_size::SM,
                theme::text::PRIMARY,
                paint,
            );
        }
        ButtonStyle::Tertiary => {
            paint.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(theme::bg::APP.with_alpha(0.0))
                    .with_border(theme::border::DEFAULT.with_alpha(0.0), 1.0)
                    .with_corner_radius(6.0),
            );
            paint_button_label(
                bounds,
                label,
                theme::font_size::SM,
                theme::text::SECONDARY,
                paint,
            );
        }
        ButtonStyle::Disabled => {
            paint.scene.draw_quad(
                Quad::new(bounds)
                    .with_background(theme::bg::SURFACE.with_alpha(0.72))
                    .with_border(theme::border::DEFAULT, 1.0)
                    .with_corner_radius(6.0),
            );
            paint_button_label(
                bounds,
                label,
                theme::font_size::SM,
                theme::text::MUTED,
                paint,
            );
        }
    }
}

fn paint_button_label(
    bounds: Bounds,
    label: &str,
    font_size: f32,
    color: Hsla,
    paint: &mut PaintContext,
) {
    let mut run = paint.text.layout(label, Point::ZERO, font_size, color);
    let run_bounds = run.bounds();
    let origin = Point::new(
        bounds.origin.x + ((bounds.size.width - run_bounds.size.width).max(0.0) * 0.5)
            - run_bounds.origin.x,
        bounds.origin.y + ((bounds.size.height - run_bounds.size.height).max(0.0) * 0.5)
            - run_bounds.origin.y,
    );
    run.origin = origin;
    paint.scene.draw_text(run);
}

fn paint_button_label_mono(
    bounds: Bounds,
    label: &str,
    font_size: f32,
    color: Hsla,
    paint: &mut PaintContext,
) {
    let mut run = paint.text.layout_mono(label, Point::ZERO, font_size, color);
    let run_bounds = run.bounds();
    let origin = Point::new(
        bounds.origin.x + ((bounds.size.width - run_bounds.size.width).max(0.0) * 0.5)
            - run_bounds.origin.x,
        bounds.origin.y + ((bounds.size.height - run_bounds.size.height).max(0.0) * 0.5)
            - run_bounds.origin.y,
    );
    run.origin = origin;
    paint.scene.draw_text(run);
}

pub(crate) fn paint_label_line(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
) -> f32 {
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(x + 122.0, y),
        theme::font_size::SM,
        theme::text::PRIMARY,
    ));
    y + 18.0
}

pub(crate) fn paint_wrapped_label_line(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_chunk_len: usize,
) -> f32 {
    paint_wrapped_label_line_with_style(
        paint,
        x,
        y,
        label,
        value,
        value_chunk_len,
        theme::text::MUTED,
        false,
    )
}

fn paint_wrapped_label_line_mission_control_label(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_chunk_len: usize,
    row_width: f32,
    show_divider: bool,
) -> f32 {
    let value_x = x + mission_control_value_x_offset(label);
    let value_right = x + row_width.max(0.0);
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{label}:"),
        Point::new(x, y),
        12.0,
        mission_control_muted_color(),
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, value_chunk_len.max(1)) {
        let value_width = paint
            .text
            .layout_mono(&chunk, Point::ZERO, 12.0, mission_control_text_color())
            .bounds()
            .size
            .width;
        let target_x = (value_right - value_width).max(value_x);
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(target_x, line_y),
            12.0,
            mission_control_text_color(),
        ));
        line_y += MISSION_CONTROL_LABEL_ROW_LINE_HEIGHT;
    }
    let row_bottom = line_y.max(y + MISSION_CONTROL_LABEL_ROW_LINE_HEIGHT);
    if show_divider {
        paint_mission_control_row_divider(paint, x, row_bottom, row_width)
    } else {
        row_bottom + MISSION_CONTROL_LABEL_ROW_TRAILING_GAP
    }
}

fn mission_control_wrapped_label_line_height(
    value: &str,
    value_chunk_len: usize,
    show_divider: bool,
) -> f32 {
    let line_count = split_text_for_display(value, value_chunk_len.max(1))
        .len()
        .max(1) as f32;
    line_count * MISSION_CONTROL_LABEL_ROW_LINE_HEIGHT
        + if show_divider {
            MISSION_CONTROL_ROW_DIVIDER_TOP_GAP
                + MISSION_CONTROL_ROW_DIVIDER_HEIGHT
                + MISSION_CONTROL_ROW_DIVIDER_BOTTOM_GAP
        } else {
            MISSION_CONTROL_LABEL_ROW_TRAILING_GAP
        }
}

fn paint_mission_control_body_block(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_chunk_len: usize,
    row_width: f32,
    show_divider: bool,
) -> f32 {
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(x, y),
        12.0,
        mission_control_muted_color(),
    ));

    let mut line_y = y + MISSION_CONTROL_BODY_BLOCK_LABEL_GAP;
    for chunk in split_text_for_display(value, value_chunk_len.max(1)) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(x, line_y),
            11.0,
            mission_control_text_color(),
        ));
        line_y += MISSION_CONTROL_BODY_BLOCK_LINE_HEIGHT;
    }
    let row_bottom = line_y + MISSION_CONTROL_BODY_BLOCK_BOTTOM_GAP;
    if show_divider {
        paint_mission_control_row_divider(paint, x, row_bottom, row_width)
    } else {
        row_bottom + MISSION_CONTROL_BODY_BLOCK_TRAILING_GAP
    }
}

fn mission_control_body_block_height(
    value: &str,
    value_chunk_len: usize,
    show_divider: bool,
) -> f32 {
    let line_count = split_text_for_display(value, value_chunk_len.max(1))
        .len()
        .max(1) as f32;
    MISSION_CONTROL_BODY_BLOCK_LABEL_GAP
        + line_count * MISSION_CONTROL_BODY_BLOCK_LINE_HEIGHT
        + MISSION_CONTROL_BODY_BLOCK_BOTTOM_GAP
        + if show_divider {
            MISSION_CONTROL_ROW_DIVIDER_TOP_GAP
                + MISSION_CONTROL_ROW_DIVIDER_HEIGHT
                + MISSION_CONTROL_ROW_DIVIDER_BOTTOM_GAP
        } else {
            MISSION_CONTROL_BODY_BLOCK_TRAILING_GAP
        }
}

fn mission_control_load_funds_content_height(
    layout: &crate::pane_system::MissionControlLoadFundsLayout,
    wallet_network: &str,
    wallet_status: &str,
    lightning_state_label: &str,
    lightning_target_text: &str,
    recent_receive_history: &str,
) -> f32 {
    let value_chunk_len = mission_control_value_chunk_len(layout.details_column);
    let body_chunk_len = mission_control_body_chunk_len(layout.details_column);
    let detail_height =
        mission_control_wrapped_label_line_height(wallet_network, value_chunk_len, true)
            + mission_control_wrapped_label_line_height(wallet_status, value_chunk_len, true)
            + mission_control_wrapped_label_line_height(
                lightning_state_label,
                value_chunk_len,
                true,
            )
            + mission_control_body_block_height(lightning_target_text, body_chunk_len, true)
            + mission_control_body_block_height(recent_receive_history, body_chunk_len, false);
    let controls_height =
        (layout.copy_seed_button.max_y() - layout.controls_column.origin.y).max(0.0);

    controls_height.max(detail_height) + MISSION_CONTROL_LOAD_FUNDS_CONTENT_BOTTOM_PADDING
}

fn paint_mission_control_row_divider(
    paint: &mut PaintContext,
    x: f32,
    row_bottom: f32,
    row_width: f32,
) -> f32 {
    let divider_y = row_bottom + MISSION_CONTROL_ROW_DIVIDER_TOP_GAP;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(x, divider_y, row_width.max(0.0), 1.0))
            .with_background(mission_control_panel_border_color().with_alpha(0.36)),
    );
    divider_y + MISSION_CONTROL_ROW_DIVIDER_HEIGHT + MISSION_CONTROL_ROW_DIVIDER_BOTTOM_GAP
}

fn paint_wrapped_label_line_with_style(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_chunk_len: usize,
    label_color: Hsla,
    label_mono: bool,
) -> f32 {
    let value_x = x + mission_control_value_x_offset(label);
    if label_mono {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{label}:"),
            Point::new(x, y),
            theme::font_size::SM,
            label_color,
        ));
    } else {
        paint.scene.draw_text(paint.text.layout(
            &format!("{label}:"),
            Point::new(x, y),
            theme::font_size::SM,
            label_color,
        ));
    }

    let mut line_y = y;
    for chunk in split_text_for_display(value, value_chunk_len.max(1)) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(value_x, line_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        ));
        line_y += 18.0;
    }
    line_y.max(y + 18.0)
}

fn mission_control_wrapped_row_height(
    value: &str,
    value_chunk_len: usize,
    show_divider: bool,
) -> f32 {
    let lines = split_text_for_display(value, value_chunk_len.max(1))
        .len()
        .max(1) as f32;
    if show_divider {
        lines * 18.0 + 21.0
    } else {
        lines * 18.0 + 20.0
    }
}

fn mission_control_value_x_offset(label: &str) -> f32 {
    (label.chars().count() as f32 * 6.4 + 18.0).clamp(82.0, 118.0)
}

pub(crate) fn paint_multiline_phrase(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
) -> f32 {
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, 72) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(x + 122.0, line_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        ));
        line_y += 18.0;
    }
    line_y
}

fn mission_control_value_chunk_len(card: Bounds) -> usize {
    let value_width = (card.size.width - 146.0).max(48.0);
    ((value_width / 6.2).floor() as usize).max(8)
}

fn mission_control_body_chunk_len(card: Bounds) -> usize {
    let body_width = (card.size.width - 24.0).max(48.0);
    ((body_width / 6.2).floor() as usize).max(12)
}

fn mission_control_now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn mission_control_now_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn mission_control_now_epoch_seconds_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn mission_control_anim_seconds_f64() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    std::time::Instant::now()
        .duration_since(*start)
        .as_secs_f64()
}

fn mission_control_lightning_receive_state_label(state: SparkInvoiceState) -> &'static str {
    match state {
        SparkInvoiceState::Empty => "EMPTY",
        SparkInvoiceState::Ready => "READY",
        SparkInvoiceState::Expired => "REFRESH",
    }
}

fn mission_control_recent_receive_history(spark_wallet: &SparkPaneState) -> String {
    let recent_receives: Vec<String> = spark_wallet
        .recent_payments
        .iter()
        .filter(|payment| payment.direction.eq_ignore_ascii_case("receive"))
        .take(3)
        .map(|payment| {
            format!(
                "{}  {}",
                payment.status.to_ascii_uppercase(),
                format_sats_amount(payment.amount_sats)
            )
        })
        .collect();
    if recent_receives.is_empty() {
        "No receives recorded yet.".to_string()
    } else {
        recent_receives.join("\n")
    }
}

pub(crate) fn mission_control_blocker_detail(
    blocker: ProviderBlocker,
    spark_wallet: &SparkPaneState,
    provider_runtime: &ProviderRuntimeState,
) -> String {
    match blocker {
        ProviderBlocker::WalletError => spark_wallet
            .last_error
            .as_deref()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| blocker.detail().to_string()),
        ProviderBlocker::GptOssUnavailable | ProviderBlocker::GptOssModelUnavailable => {
            provider_runtime
                .gpt_oss
                .last_error
                .as_deref()
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| match blocker {
                    ProviderBlocker::GptOssUnavailable => {
                        "Local inference backend is unavailable".to_string()
                    }
                    ProviderBlocker::GptOssModelUnavailable => {
                        "No local inference model is ready".to_string()
                    }
                    _ => blocker.detail().to_string(),
                })
        }
        ProviderBlocker::AppleFoundationModelsUnavailable
        | ProviderBlocker::AppleFoundationModelsModelUnavailable => provider_runtime
            .apple_fm
            .availability_error_message()
            .as_deref()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| blocker.detail().to_string()),
        _ => blocker.detail().to_string(),
    }
}

fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "<hidden>".to_string();
    }
    if trimmed.len() <= 8 {
        return "••••••••".to_string();
    }

    format!("{}••••••••{}", &trimmed[..4], &trimmed[trimmed.len() - 4..])
}

fn mask_mnemonic(phrase: &str) -> String {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if words.is_empty() {
        return "<hidden>".to_string();
    }
    words.iter().map(|_| "••••").collect::<Vec<_>>().join(" ")
}

pub(crate) fn split_text_for_display(text: &str, chunk_len: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let chunk_len = chunk_len.max(1);
    for line in text.lines() {
        let line_chars = line.chars().collect::<Vec<_>>();
        if line_chars.is_empty() {
            chunks.push(String::new());
            continue;
        }
        chunks.extend(
            line_chars
                .chunks(chunk_len)
                .map(|chunk| chunk.iter().collect::<String>()),
        );
    }
    if text.ends_with('\n') {
        chunks.push(String::new());
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::{
        InactivePaneRenderPolicy, active_job_clipboard_text, build_active_job_scroll_lines,
        buy_mode_payments_inactive_preview_state, create_invoice_view_state,
        earnings_scoreboard_amount_display, inactive_pane_render_policy,
        mission_control_active_jobs_panel_state, mission_control_backend_label,
        mission_control_body_chunk_len, mission_control_buy_mode_panel_state,
        mission_control_buy_mode_payment_label, mission_control_go_online_hint,
        mission_control_lightning_receive_state_label, mission_control_load_funds_content_height,
        mission_control_local_action_enabled, mission_control_local_fm_test_button_label,
        mission_control_local_fm_test_enabled, mission_control_local_model_button_label,
        mission_control_model_load_status, mission_control_primary_model_label,
        mission_control_value_chunk_len, mission_control_value_x_offset, nostr_identity_view_state,
        pay_invoice_view_state, payment_terminal_status, provider_control_inactive_preview_state,
        request_freshness_summary, scrollbar_thumb_height, spark_wallet_view_state,
        split_text_for_display,
    };
    use crate::app_state::{
        ActiveJobState, AutopilotChatState, BuyModePaymentsPaneState,
        EarnJobLifecycleProjectionState, JobDemandSource, JobInboxDecision, JobInboxRequest,
        JobInboxState, JobInboxValidation, JobLifecycleStage, PaneKind, PaneLoadState,
        ProviderControlPaneState,
    };
    use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
    use crate::pane_system::mission_control_load_funds_layout_with_scroll;
    use crate::spark_wallet::{SparkInvoiceState, SparkPaneState};
    use crate::state::operations::{
        BuyerResolutionMode, NetworkRequestSubmission, NetworkRequestsState,
    };
    use crate::state::provider_runtime::ProviderRuntimeState;
    use wgpui::Bounds;

    fn queue_buy_mode_request_for_tests() -> NetworkRequestsState {
        let mut requests = NetworkRequestsState::default();
        requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-render-001".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "Reply with the exact text BUY MODE OK.".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq: 21,
            })
            .expect("buy mode request should queue");
        requests
    }

    fn fixture_buy_mode() -> BuyModePaymentsPaneState {
        BuyModePaymentsPaneState::default()
    }

    fn fixture_autopilot_chat() -> AutopilotChatState {
        AutopilotChatState::default()
    }

    #[test]
    fn inactive_pane_render_policy_marks_heavy_panes_as_summary() {
        assert_eq!(
            inactive_pane_render_policy(PaneKind::ProviderControl),
            InactivePaneRenderPolicy::Summary
        );
        assert_eq!(
            inactive_pane_render_policy(PaneKind::AutopilotChat),
            InactivePaneRenderPolicy::Summary
        );
        assert_eq!(
            inactive_pane_render_policy(PaneKind::LogStream),
            InactivePaneRenderPolicy::Summary
        );
        assert_eq!(
            inactive_pane_render_policy(PaneKind::BuyerRaceMatrix),
            InactivePaneRenderPolicy::Summary
        );
        assert_eq!(
            inactive_pane_render_policy(PaneKind::JobInbox),
            InactivePaneRenderPolicy::Full
        );
    }

    #[test]
    fn provider_control_inactive_preview_surfaces_runtime_truth() {
        let mut provider_runtime = ProviderRuntimeState::default();
        provider_runtime.mode = crate::app_state::ProviderMode::Connecting;
        provider_runtime.last_result = Some("provider warming local runtime".to_string());

        let preview = provider_control_inactive_preview_state(
            crate::desktop_shell::DesktopShellMode::Production,
            false,
            &provider_runtime,
            &LocalInferenceExecutionSnapshot::default(),
            &[],
            &ProviderControlPaneState::default(),
            &SparkPaneState::default(),
        );

        assert_eq!(preview.load_state, PaneLoadState::Loading);
        assert!(preview.summary.contains("connecting"));
        assert!(
            preview
                .detail_lines
                .iter()
                .any(|line| line.contains("preflight"))
        );
        assert!(
            preview
                .detail_lines
                .iter()
                .any(|line| line.contains("control"))
        );
    }

    #[test]
    fn buy_mode_inactive_preview_surfaces_panel_summary_and_fact_count() {
        let mut buy_mode = fixture_buy_mode();
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let preview = buy_mode_payments_inactive_preview_state(
            true,
            &fixture_autopilot_chat(),
            &buy_mode,
            &queue_buy_mode_request_for_tests(),
            &crate::state::nip90_payment_facts::Nip90PaymentFactLedgerState::default(),
            &SparkPaneState::default(),
        );

        assert!(preview.summary.contains("req"));
        assert!(
            preview
                .detail_lines
                .iter()
                .any(|line| line.contains("fact rows"))
        );
    }

    #[test]
    fn scrollbar_thumb_height_caps_minimum_to_small_viewport() {
        assert_eq!(scrollbar_thumb_height(6.0, 48.0, 16.0), 6.0);
        assert_eq!(scrollbar_thumb_height(6.0, 48.0, 18.0), 6.0);
    }

    #[test]
    fn scrollbar_thumb_height_keeps_requested_floor_for_normal_viewport() {
        assert_eq!(scrollbar_thumb_height(80.0, 800.0, 16.0), 16.0);
        assert_eq!(scrollbar_thumb_height(80.0, 800.0, 18.0), 18.0);
    }

    fn fixture_autopilot_chat_with_buy_mode_peer(
        peer_pubkey: &str,
        online_for_compute: bool,
    ) -> AutopilotChatState {
        let config = crate::app_state::DefaultNip28ChannelConfig::from_env_or_default();
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection.relay_events.clear();
        chat.managed_chat_projection.outbound_messages.clear();
        chat.managed_chat_projection.local_state =
            crate::app_state::ManagedChatLocalState::default();
        chat.managed_chat_projection.snapshot =
            crate::app_state::ManagedChatProjectionSnapshot::default();
        chat.managed_chat_projection.projection_revision = chat
            .managed_chat_projection
            .projection_revision
            .saturating_add(1);
        let group_id = "oa-main".to_string();
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let presence_mode = if online_for_compute {
            crate::autopilot_peer_roster::AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE
        } else {
            crate::autopilot_peer_roster::AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE
        };
        let presence_payload = serde_json::json!({
            "type": crate::autopilot_peer_roster::AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": presence_mode,
            "pubkey": peer_pubkey,
            "capabilities": ["5050"],
            "ready_model": "apple-foundation-model",
            "started_at": now_epoch_seconds.saturating_sub(20),
            "expires_at": now_epoch_seconds.saturating_add(120)
        })
        .to_string();
        let chat_message = crate::app_state::ManagedChatMessageProjection {
            event_id: "chat-target-001".to_string(),
            group_id: group_id.clone(),
            channel_id: config.channel_id.clone(),
            author_pubkey: peer_pubkey.to_string(),
            content: "online".to_string(),
            created_at: now_epoch_seconds.saturating_sub(30),
            reply_to_event_id: None,
            mention_pubkeys: Vec::new(),
            reaction_summaries: Vec::new(),
            reply_child_ids: Vec::new(),
            delivery_state: crate::app_state::ManagedChatDeliveryState::Confirmed,
            delivery_error: None,
            attempt_count: 0,
        };
        let presence_message = crate::app_state::ManagedChatMessageProjection {
            event_id: "presence-target-001".to_string(),
            group_id: group_id.clone(),
            channel_id: config.channel_id.clone(),
            author_pubkey: peer_pubkey.to_string(),
            content: presence_payload,
            created_at: now_epoch_seconds.saturating_sub(10),
            reply_to_event_id: None,
            mention_pubkeys: Vec::new(),
            reaction_summaries: Vec::new(),
            reply_child_ids: Vec::new(),
            delivery_state: crate::app_state::ManagedChatDeliveryState::Confirmed,
            delivery_error: None,
            attempt_count: 0,
        };
        chat.managed_chat_projection.snapshot.groups.push(
            crate::app_state::ManagedChatGroupProjection {
                group_id: group_id.clone(),
                metadata: nostr::GroupMetadata::new().with_name("OpenAgents Main"),
                roles: Vec::new(),
                members: Vec::new(),
                channel_ids: vec![config.channel_id.clone()],
                unread_count: 0,
                mention_count: 0,
            },
        );
        chat.managed_chat_projection.snapshot.channels.push(
            crate::app_state::ManagedChatChannelProjection {
                channel_id: config.channel_id.clone(),
                group_id,
                room_mode: nostr::ManagedRoomMode::ManagedChannel,
                metadata: nostr::ChannelMetadata::new("main", "", ""),
                hints: nostr::ManagedChannelHints::default(),
                relay_url: Some(config.relay_url.clone()),
                message_ids: vec![
                    chat_message.event_id.clone(),
                    presence_message.event_id.clone(),
                ],
                root_message_ids: vec![
                    chat_message.event_id.clone(),
                    presence_message.event_id.clone(),
                ],
                unread_count: 0,
                mention_count: 0,
                latest_message_id: Some(presence_message.event_id.clone()),
            },
        );
        chat.managed_chat_projection
            .snapshot
            .messages
            .insert(chat_message.event_id.clone(), chat_message);
        chat.managed_chat_projection
            .snapshot
            .messages
            .insert(presence_message.event_id.clone(), presence_message);
        chat
    }

    #[test]
    fn earnings_scoreboard_amount_display_shows_loading_until_ready() {
        assert_eq!(
            earnings_scoreboard_amount_display(PaneLoadState::Loading, "\u{20BF} 0".to_string()),
            "LOADING"
        );
        assert_eq!(
            earnings_scoreboard_amount_display(PaneLoadState::Ready, "\u{20BF} 2".to_string()),
            "\u{20BF} 2"
        );
        assert_eq!(
            earnings_scoreboard_amount_display(PaneLoadState::Error, "\u{20BF} 2".to_string()),
            "\u{20BF} 2"
        );
    }

    fn fixture_active_job_request(request_id: &str) -> JobInboxRequest {
        JobInboxRequest {
            request_id: request_id.to_string(),
            requester: "buyer".to_string(),
            source_relay_url: None,
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: Some("Reply with OK".to_string()),
            execution_prompt: Some("Reply with OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some(request_id.to_string()),
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
            validation: JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: JobInboxDecision::Accepted {
                reason: "accepted".to_string(),
            },
        }
    }

    #[test]
    fn spark_wallet_view_state_prioritizes_error_then_loading_then_ready() {
        let mut state = SparkPaneState::default();
        state.last_error = Some("wallet lane failed".to_string());
        assert_eq!(spark_wallet_view_state(&state), PaneLoadState::Error);

        state.last_error = None;
        assert_eq!(spark_wallet_view_state(&state), PaneLoadState::Loading);

        state.network_status = Some(openagents_spark::NetworkStatusReport {
            status: openagents_spark::NetworkStatus::Connected,
            detail: None,
        });
        state.balance = Some(openagents_spark::Balance {
            spark_sats: 1,
            lightning_sats: 2,
            onchain_sats: 3,
        });
        assert_eq!(spark_wallet_view_state(&state), PaneLoadState::Ready);
    }

    #[test]
    fn pay_invoice_view_state_and_terminal_status_are_deterministic() {
        let mut state = SparkPaneState::default();
        assert_eq!(pay_invoice_view_state(&state), PaneLoadState::Loading);
        assert_eq!(payment_terminal_status(&state), "idle");

        state.last_action = Some("Payment sent (pay-123)".to_string());
        assert_eq!(payment_terminal_status(&state), "sent");

        state.last_error = Some("send failed".to_string());
        assert_eq!(pay_invoice_view_state(&state), PaneLoadState::Error);
        assert_eq!(payment_terminal_status(&state), "failed");
    }

    #[test]
    fn create_invoice_view_state_transitions_loading_to_ready() {
        let mut state = SparkPaneState::default();
        assert_eq!(create_invoice_view_state(&state), PaneLoadState::Loading);

        state.last_invoice = Some("lnbc1example".to_string());
        assert_eq!(create_invoice_view_state(&state), PaneLoadState::Ready);

        state.last_error = Some("invoice failed".to_string());
        assert_eq!(create_invoice_view_state(&state), PaneLoadState::Error);
    }

    #[test]
    fn nostr_identity_view_state_reports_loading_ready_error() {
        assert_eq!(
            nostr_identity_view_state(None, None),
            PaneLoadState::Loading
        );

        let identity = nostr::NostrIdentity {
            identity_path: std::path::PathBuf::from("/tmp/identity"),
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            npub: "npub1example".to_string(),
            nsec: "nsec1example".to_string(),
            public_key_hex: "11".to_string(),
            private_key_hex: "22".to_string(),
        };
        assert_eq!(
            nostr_identity_view_state(Some(&identity), None),
            PaneLoadState::Ready
        );
        assert_eq!(
            nostr_identity_view_state(Some(&identity), Some("corrupt mnemonic")),
            PaneLoadState::Error
        );
    }

    #[test]
    fn split_text_for_display_preserves_newline_boundaries() {
        let chunks = split_text_for_display("shape:\nline-1\n\nraw:{\"x\":1}", 12);
        assert_eq!(
            chunks,
            vec![
                "shape:".to_string(),
                "line-1".to_string(),
                "".to_string(),
                "raw:{\"x\":1}".to_string(),
            ]
        );
    }

    #[test]
    fn request_freshness_summary_reports_fresh_and_expired_states() {
        let request = fixture_active_job_request("req-active-job-freshness");
        assert!(request_freshness_summary(&request, 1_760_000_010).contains("fresh // 65s left"));
        assert!(request_freshness_summary(&request, 1_760_000_080).contains("expired 5s ago"));
    }

    #[test]
    fn failed_active_job_timeline_does_not_mark_paid_without_payment() {
        let request = fixture_active_job_request("req-active-job-failed");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.execution_turn_completed = true;
        active_job.execution_output = Some("BUY MODE OK".to_string());
        let job = active_job.job.as_mut().expect("active job");
        job.stage = JobLifecycleStage::Failed;
        job.failure_reason =
            Some(
                "job result publish continuity timed out after 195s while awaiting relay delivery confirmation"
                    .to_string(),
            );

        let lines = build_active_job_scroll_lines(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            120,
        );
        let texts = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();

        assert!(texts.contains(&"[x] received"));
        assert!(texts.contains(&"[x] accepted"));
        assert!(texts.contains(&"[x] running"));
        assert!(texts.contains(&"[ ] delivered"));
        assert!(texts.contains(&"[ ] paid"));
    }

    #[test]
    fn delivered_unpaid_active_job_shows_nonpayment_state() {
        let request = fixture_active_job_request("req-active-job-unpaid");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.pending_bolt11 = Some("lnbc20n1activejobunpaid".to_string());
        let job = active_job.job.as_mut().expect("active job");
        job.stage = JobLifecycleStage::Failed;
        job.sa_tick_result_event_id = Some("result-active-job-unpaid-001".to_string());
        job.failure_reason = Some(
            "job delivered but unpaid timed out after 195s while awaiting buyer settlement"
                .to_string(),
        );

        let lines = build_active_job_scroll_lines(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            120,
        );
        let texts = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();

        assert!(
            texts
                .iter()
                .any(|text| { text.contains("Flow phase: delivered-unpaid") })
        );
        assert!(
            texts
                .iter()
                .any(|text| { text.contains("Next event: buyer settlement timed out") })
        );
        assert!(
            texts
                .iter()
                .any(|text| { text.contains("Settlement invoice: lnbc20n1activejobunpaid") })
        );
        assert!(texts.iter().any(|text| {
            text.contains(
                "Settlement outcome: compute completed and the result was delivered, but buyer settlement never arrived",
            )
        }));
        assert!(texts.contains(&"[x] delivered"));
        assert!(texts.contains(&"[ ] paid"));
    }

    #[test]
    fn delivered_active_job_says_awaiting_buyer_payment() {
        let request = fixture_active_job_request("req-active-job-awaiting-payment");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.pending_bolt11 = Some("lnbc20n1awaitingbuyer".to_string());
        active_job.job.as_mut().expect("active job").stage = JobLifecycleStage::Delivered;

        let lines = build_active_job_scroll_lines(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
            120,
        );
        let texts = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();

        assert!(
            texts
                .iter()
                .any(|text| { text.contains("Stage: delivered (awaiting buyer payment)") })
        );
        assert!(
            texts
                .iter()
                .any(|text| { text.contains("Next event: buyer Lightning payment") })
        );
        assert!(texts.iter().any(|text| {
            text.contains(
                "Settlement outcome: compute completed and the result was delivered; awaiting buyer Lightning payment",
            )
        }));
    }

    #[test]
    fn active_job_clipboard_text_includes_header_and_request_id() {
        let request = fixture_active_job_request("req-active-job-copy");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);

        let output = active_job_clipboard_text(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert!(output.starts_with("Active Job"));
        assert!(output.contains("Job ID: job-req-active-job-copy"));
        assert!(output.contains("[x] received"));
    }

    #[test]
    fn active_job_clipboard_text_includes_demand_risk_truth() {
        let request = fixture_active_job_request("req-active-job-risk");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);

        let output = active_job_clipboard_text(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert!(output.contains("Demand risk: speculative-open-network"));
        assert!(output.contains("Risk policy: manual-only"));
    }

    #[test]
    fn active_job_clipboard_text_includes_request_freshness_truth() {
        let request = fixture_active_job_request("req-active-job-freshness-copy");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.job.as_mut().unwrap().accepted_at_epoch_seconds = Some(1_760_000_010);

        let output = active_job_clipboard_text(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert!(output.contains("Request freshness: accepted fresh"));
        assert!(output.contains("Request created: 1760000000"));
        assert!(output.contains("Request expires: 1760000075"));
    }

    #[test]
    fn active_job_clipboard_text_includes_settlement_fee_truth() {
        let request = fixture_active_job_request("req-active-job-settlement-copy");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("active job exists");
        job.stage = JobLifecycleStage::Paid;
        job.payment_id = Some("wallet-active-job-settlement-001".to_string());

        let mut spark_wallet = SparkPaneState::default();
        spark_wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-active-job-settlement-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 1,
                method: "lightning".to_string(),
                timestamp: 1_762_700_778,
                ..Default::default()
            });

        let output = active_job_clipboard_text(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &spark_wallet,
        );

        assert!(output.contains("Settlement amount: 2 sats"));
        assert!(output.contains("Settlement fees: 1 sats"));
        assert!(output.contains("Wallet delta: +2 sats"));
    }

    #[test]
    fn active_job_timeline_ignores_nonwallet_settlement_feedback_for_paid_state() {
        let request = fixture_active_job_request("req-active-job-feedback-only");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("active job exists");
        job.stage = JobLifecycleStage::Failed;
        job.sa_tick_result_event_id = Some("result-feedback-only-001".to_string());
        job.ac_settlement_event_id = Some("feedback-only-001".to_string());
        job.failure_reason = Some(
            "job delivered but unpaid timed out after 195s while awaiting buyer settlement"
                .to_string(),
        );

        let output = active_job_clipboard_text(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert!(output.contains("[x] delivered"));
        assert!(output.contains("[ ] paid"));
        assert!(output.contains("AC settlement: feedback-only-001"));
        assert!(output.contains("Payment ID: n/a"));
    }

    #[test]
    fn mission_control_chunk_lengths_respect_minimums() {
        let narrow = Bounds::new(0.0, 0.0, 180.0, 100.0);
        let wide = Bounds::new(0.0, 0.0, 360.0, 100.0);

        assert!(mission_control_value_chunk_len(narrow) >= 8);
        assert!(mission_control_body_chunk_len(narrow) >= 12);
        assert!(mission_control_value_chunk_len(wide) > mission_control_value_chunk_len(narrow));
        assert!(mission_control_body_chunk_len(wide) > mission_control_body_chunk_len(narrow));
    }

    #[test]
    fn load_funds_content_height_grows_when_wrapped_invoice_needs_more_lines() {
        let invoice = "lnbc1".repeat(64);
        let recent_receives = "2026-03-17 09:31 // +100 sats // ready";
        let narrow_layout = mission_control_load_funds_layout_with_scroll(
            Bounds::new(0.0, 0.0, 720.0, 680.0),
            false,
            0.0,
        );
        let wide_layout = mission_control_load_funds_layout_with_scroll(
            Bounds::new(0.0, 0.0, 1180.0, 680.0),
            false,
            0.0,
        );

        let narrow_height = mission_control_load_funds_content_height(
            &narrow_layout,
            "Spark // mainnet",
            "connected",
            "invoice ready",
            &invoice,
            recent_receives,
        );
        let wide_height = mission_control_load_funds_content_height(
            &wide_layout,
            "Spark // mainnet",
            "connected",
            "invoice ready",
            &invoice,
            recent_receives,
        );

        assert!(narrow_height > wide_height);
    }

    #[test]
    fn mission_control_value_offset_clamps_for_short_and_long_labels() {
        assert_eq!(mission_control_value_x_offset("Mode"), 82.0);
        assert!(mission_control_value_x_offset("Wallet status") > 82.0);
        assert_eq!(
            mission_control_value_x_offset("Projection authority status"),
            118.0
        );
    }

    #[test]
    fn mission_control_lightning_receive_state_label_tracks_expiry() {
        assert_eq!(
            mission_control_lightning_receive_state_label(SparkInvoiceState::Empty),
            "EMPTY"
        );
        assert_eq!(
            mission_control_lightning_receive_state_label(SparkInvoiceState::Ready),
            "READY"
        );
        assert_eq!(
            mission_control_lightning_receive_state_label(SparkInvoiceState::Expired),
            "REFRESH"
        );
    }

    #[test]
    fn mission_control_button_prefers_supported_runtime_lane() {
        let mut provider = ProviderRuntimeState::default();
        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.ready_model = Some("apple-foundation-model".to_string());
        provider.apple_fm.bridge_status = Some("running".to_string());

        let local = LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            ..LocalInferenceExecutionSnapshot::default()
        };

        let button_label = mission_control_local_model_button_label(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        );
        if crate::app_state::mission_control_uses_apple_fm() {
            assert_eq!(button_label, "REFRESH APPLE FM");
        } else {
            assert_eq!(button_label, "UNLOAD GPT-OSS");
        }
        assert_eq!(
            mission_control_go_online_hint(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local
            ),
            ""
        );
    }

    #[test]
    fn mission_control_primary_model_label_preserves_full_runtime_name_for_wrapping() {
        let mut provider = ProviderRuntimeState::default();
        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.ready_model = Some("apple-foundation-model-preview".to_string());
        provider.apple_fm.bridge_status = Some("running".to_string());

        let local = LocalInferenceExecutionSnapshot::default();

        assert_eq!(
            mission_control_primary_model_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "APPLE-FOUNDATION-MODEL-PREVIEW"
        );
    }

    #[test]
    fn mission_control_apple_fm_ready_lane_renders_expected_status() {
        if !crate::app_state::mission_control_uses_apple_fm() {
            return;
        }

        let mut provider = ProviderRuntimeState::default();
        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.ready_model = Some("apple-fm".to_string());
        provider.apple_fm.bridge_status = Some("running".to_string());

        let local = LocalInferenceExecutionSnapshot::default();

        assert_eq!(
            mission_control_local_model_button_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "REFRESH APPLE FM"
        );
        assert!(mission_control_local_action_enabled(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        ));
        assert_eq!(
            mission_control_primary_model_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "APPLE-FM"
        );
        assert_eq!(
            mission_control_backend_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "Apple FM bridge (running)"
        );
        assert_eq!(
            mission_control_model_load_status(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "ready"
        );
        assert_eq!(
            mission_control_go_online_hint(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            ""
        );
    }

    #[test]
    fn mission_control_reports_missing_supported_runtime_lane() {
        let provider = ProviderRuntimeState::default();
        let local = LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "metal".to_string(),
            ..LocalInferenceExecutionSnapshot::default()
        };

        if crate::app_state::mission_control_uses_apple_fm() {
            assert_eq!(
                mission_control_local_model_button_label(
                    crate::desktop_shell::DesktopShellMode::Production,
                    &provider,
                    &local
                ),
                "START APPLE FM"
            );
            assert_eq!(
                mission_control_go_online_hint(
                    crate::desktop_shell::DesktopShellMode::Production,
                    &provider,
                    &local
                ),
                "Start Apple FM before you go online."
            );
        } else {
            assert_eq!(
                mission_control_local_model_button_label(
                    crate::desktop_shell::DesktopShellMode::Production,
                    &provider,
                    &local
                ),
                "OPEN GPT-OSS WORKBENCH"
            );
            assert_eq!(
                mission_control_go_online_hint(
                    crate::desktop_shell::DesktopShellMode::Production,
                    &provider,
                    &local
                ),
                "GPT-OSS backend is METAL. Go Online currently requires CUDA for the compute lane."
            );
        }
    }

    #[test]
    fn mission_control_gpt_oss_ready_lane_renders_expected_status() {
        if crate::app_state::mission_control_uses_apple_fm() {
            return;
        }

        let provider = ProviderRuntimeState::default();
        let local = LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            artifact_present: true,
            backend_label: "cuda".to_string(),
            ..LocalInferenceExecutionSnapshot::default()
        };

        assert_eq!(
            mission_control_local_model_button_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "UNLOAD GPT-OSS"
        );
        assert!(mission_control_local_action_enabled(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        ));
        assert_eq!(
            mission_control_primary_model_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "GPT-OSS-20B"
        );
        assert_eq!(
            mission_control_backend_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "GPT-OSS / CUDA"
        );
        assert_eq!(
            mission_control_model_load_status(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            "loaded / artifact present"
        );
        assert_eq!(
            mission_control_go_online_hint(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local,
            ),
            ""
        );
    }

    #[test]
    fn mission_control_gpt_oss_busy_button_disables_inline_action() {
        if crate::app_state::mission_control_uses_apple_fm() {
            return;
        }

        let provider = ProviderRuntimeState::default();
        let local = LocalInferenceExecutionSnapshot {
            reachable: true,
            busy: true,
            backend_label: "cuda".to_string(),
            artifact_present: true,
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            ..LocalInferenceExecutionSnapshot::default()
        };

        assert_eq!(
            mission_control_local_model_button_label(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local
            ),
            "GPT-OSS BUSY"
        );
        assert!(!mission_control_local_action_enabled(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        ));
        assert_eq!(
            mission_control_go_online_hint(
                crate::desktop_shell::DesktopShellMode::Production,
                &provider,
                &local
            ),
            "GPT-OSS is loading. Go Online unlocks when the configured model is ready."
        );
    }

    #[test]
    fn mission_control_local_fm_test_button_tracks_ready_and_streaming_states() {
        if !crate::app_state::mission_control_uses_apple_fm() {
            return;
        }
        let mut provider = ProviderRuntimeState::default();
        let local = LocalInferenceExecutionSnapshot::default();
        let mut provider_control = ProviderControlPaneState::default();

        assert_eq!(
            mission_control_local_fm_test_button_label(
                &provider_control,
                &provider,
                &local,
                crate::desktop_shell::DesktopShellMode::Production,
            ),
            "LOCAL FM NOT READY"
        );
        assert!(!mission_control_local_fm_test_enabled(
            &provider_control,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        ));

        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.ready_model = Some("apple-foundation-model".to_string());
        assert_eq!(
            mission_control_local_fm_test_button_label(
                &provider_control,
                &provider,
                &local,
                crate::desktop_shell::DesktopShellMode::Production,
            ),
            "TEST LOCAL FM"
        );
        assert!(mission_control_local_fm_test_enabled(
            &provider_control,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        ));

        provider_control.local_fm_summary_pending_request_id = Some("mission-control-fm-1".into());
        assert_eq!(
            mission_control_local_fm_test_button_label(
                &provider_control,
                &provider,
                &local,
                crate::desktop_shell::DesktopShellMode::Production,
            ),
            "STREAMING LOCAL FM"
        );
        assert!(!mission_control_local_fm_test_enabled(
            &provider_control,
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &local,
        ));
    }

    #[test]
    fn mission_control_go_online_hint_ignores_positive_apple_fm_health_message() {
        let mut provider = ProviderRuntimeState::default();
        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.availability_message = Some("Foundation Models is available".to_string());

        let local = LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "metal".to_string(),
            ..LocalInferenceExecutionSnapshot::default()
        };

        if crate::app_state::mission_control_uses_apple_fm() {
            assert_eq!(
                mission_control_go_online_hint(
                    crate::desktop_shell::DesktopShellMode::Production,
                    &provider,
                    &local
                ),
                "Refresh Apple FM health before you go online."
            );
        } else {
            assert_eq!(
                mission_control_go_online_hint(
                    crate::desktop_shell::DesktopShellMode::Production,
                    &provider,
                    &local
                ),
                "GPT-OSS backend is METAL. Go Online currently requires CUDA for the compute lane."
            );
        }
    }

    #[test]
    fn mission_control_buy_mode_panel_state_respects_feature_gate() {
        let requests = NetworkRequestsState::default();
        let wallet = SparkPaneState::default();
        let autopilot_chat = fixture_autopilot_chat();
        let buy_mode = fixture_buy_mode();
        let now = std::time::Instant::now();

        assert_eq!(
            mission_control_buy_mode_panel_state(
                false,
                &autopilot_chat,
                &buy_mode,
                &requests,
                &wallet,
                now,
            ),
            None
        );

        let panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &wallet,
            now,
        )
        .expect("enabled buy mode should expose panel state");
        assert!(panel.summary.contains("Buy Mode blocked"));
        assert_eq!(panel.mode, "off");
        assert_eq!(panel.next, "off");
        assert_eq!(panel.payment, "idle");
        assert_eq!(panel.button_label, "START BUY MODE");
        assert!(!panel.button_enabled);

        let mut funded_wallet = SparkPaneState::default();
        funded_wallet.balance = Some(openagents_spark::Balance {
            spark_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        let funded_panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &funded_wallet,
            now,
        )
        .expect("funded buy mode should expose panel state");
        assert!(funded_panel.summary.contains("Buy Mode off"));
        assert_eq!(funded_panel.mode, "off");
        assert_eq!(funded_panel.payment, "idle");
        assert!(funded_panel.button_enabled);

        let mut armed = fixture_buy_mode();
        armed.toggle_buy_mode_loop(now);
        let armed_panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &armed,
            &requests,
            &wallet,
            now,
        )
        .expect("armed buy mode should expose panel state");
        assert_eq!(armed_panel.mode, "on");
        assert_eq!(armed_panel.next, "blocked");
        assert_eq!(armed_panel.button_label, "STOP BUY MODE");
        assert!(armed_panel.button_enabled);
    }

    #[test]
    fn mission_control_buy_mode_panel_state_requires_payment_settlement() {
        let mut buy_mode = fixture_buy_mode();
        let autopilot_chat = fixture_autopilot_chat();
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let mut requests = queue_buy_mode_request_for_tests();
        let provider_pubkey = "44".repeat(32);
        requests.apply_nip90_request_publish_outcome(
            "req-buy-render-001",
            "event-buy-render-001",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            "req-buy-render-001",
            provider_pubkey.as_str(),
            "feedback-buy-render-001",
            Some("payment-required"),
            Some("pay invoice"),
            Some(2_000),
            Some("lnbc1buyrender"),
        );
        requests
            .prepare_auto_payment_attempt(
                "req-buy-render-001",
                "lnbc1buyrender",
                Some(2_000),
                1_762_700_040,
            )
            .expect("payment-required invoice should prepare");

        let paying = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &SparkPaneState::default(),
            now,
        )
        .expect("enabled buy mode should expose panel state");
        assert_eq!(paying.mode, "on");
        assert_eq!(paying.next, "wallet payment");
        assert_eq!(paying.provider, "444444..4444");
        assert_eq!(paying.work, "invoice");
        assert_eq!(paying.payment, "queued");
        assert!(paying.summary.contains("provider 444444..4444"));
        assert!(paying.summary.contains("invoice received"));
        assert!(paying.summary.contains("payment queued"));
        assert_eq!(paying.button_label, "STOP BUY MODE");
        assert!(paying.button_enabled);

        requests.apply_nip90_buyer_result_event(
            "req-buy-render-001",
            provider_pubkey.as_str(),
            "result-buy-render-001",
            Some("success"),
        );
        let before_payment = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &SparkPaneState::default(),
            now,
        )
        .expect("enabled buy mode should expose panel state");
        assert_eq!(before_payment.work, "done");
        assert_eq!(before_payment.payment, "queued");
        assert!(before_payment.summary.contains("result received"));
        assert!(before_payment.summary.contains("payment queued"));
        assert_eq!(
            mission_control_buy_mode_payment_label(
                requests.latest_request_by_type(
                    crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE
                ),
                &SparkPaneState::default()
            ),
            "queued".to_string()
        );

        requests.mark_auto_payment_sent(
            "req-buy-render-001",
            "wallet-payment-buy-render-001",
            1_762_700_041,
        );
        let settled = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &SparkPaneState::default(),
            now,
        )
        .expect("enabled buy mode should expose panel state");
        assert_eq!(settled.work, "done");
        assert_eq!(settled.payment, "sent");
        assert!(settled.summary.contains("payment sent"));
        assert!(settled.button_enabled);
    }

    #[test]
    fn mission_control_buy_mode_panel_state_surfaces_selected_target_when_idle() {
        let mut buy_mode = fixture_buy_mode();
        let autopilot_chat = fixture_autopilot_chat_with_buy_mode_peer(&"66".repeat(32), true);
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(openagents_spark::Balance {
            spark_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        let panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &NetworkRequestsState::default(),
            &wallet,
            now,
        )
        .expect("buy mode panel should render selected idle target");

        assert_eq!(panel.next, "dispatch-ready");
        assert_eq!(panel.provider, "666666..6666");
        assert_eq!(panel.work, "1/1");
        assert!(panel.summary.contains("target 666666..6666"));
        assert!(panel.summary.contains("roster 1/1"));
        assert!(panel.summary.contains("model apple-foundation-model"));
    }

    #[test]
    fn mission_control_buy_mode_panel_state_surfaces_target_blocker_when_no_peer_is_eligible() {
        let mut buy_mode = fixture_buy_mode();
        let autopilot_chat = fixture_autopilot_chat_with_buy_mode_peer(&"77".repeat(32), false);
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(openagents_spark::Balance {
            spark_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        let panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &NetworkRequestsState::default(),
            &wallet,
            now,
        )
        .expect("buy mode panel should render target blocker");

        assert_eq!(panel.next, "waiting-peer");
        assert_eq!(panel.provider, "none");
        assert_eq!(panel.work, "blocked");
        assert!(
            panel
                .summary
                .contains("no eligible Autopilot peers are online for compute")
        );
        assert!(panel.summary.contains("provider-offline"));
    }

    #[test]
    fn mission_control_buy_mode_panel_state_surfaces_payable_winner_and_loser_summary() {
        let mut buy_mode = fixture_buy_mode();
        let autopilot_chat = fixture_autopilot_chat();
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let mut requests = queue_buy_mode_request_for_tests();
        let payable_provider = "31".repeat(32);
        let losing_provider = "41".repeat(32);

        requests.apply_nip90_request_publish_outcome(
            "req-buy-render-001",
            "event-buy-render-001",
            3,
            1,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            "req-buy-render-001",
            payable_provider.as_str(),
            "feedback-buy-render-3388",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc1buyrender3388"),
        );
        requests.apply_nip90_buyer_result_event(
            "req-buy-render-001",
            payable_provider.as_str(),
            "result-buy-render-3388",
            Some("success"),
        );
        requests.apply_nip90_buyer_result_event(
            "req-buy-render-001",
            losing_provider.as_str(),
            "result-buy-render-loser-3388",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            "req-buy-render-001",
            losing_provider.as_str(),
            "feedback-buy-render-loser-3388",
            Some("processing"),
            Some("still working"),
            None,
            None,
        );

        let panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &SparkPaneState::default(),
            now,
        )
        .expect("enabled buy mode should expose panel state");

        assert_eq!(panel.provider, "313131..3131");
        assert!(panel.summary.contains("payable 313131..3131"));
        assert!(!panel.summary.contains("blocker "));
        assert!(panel.summary.contains("1 losers ignored"));
        assert!(panel.summary.contains("late result"));
        assert!(panel.summary.contains("non-winning provider noise ignored"));
    }

    #[test]
    fn mission_control_active_jobs_panel_state_surfaces_publish_continuity() {
        let mut provider = ProviderRuntimeState::default();
        provider.mode = crate::app_state::ProviderMode::Online;
        let request = fixture_active_job_request("req-active-panel-3389");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.job.as_mut().expect("job should exist").stage = JobLifecycleStage::Running;
        active_job.execution_turn_completed = true;
        active_job.result_publish_in_flight = true;
        active_job.pending_result_publish_event_id = Some("result-active-panel-3389".to_string());
        active_job.result_publish_attempt_count = 4;
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        active_job.result_publish_first_queued_epoch_seconds =
            Some(now_epoch_seconds.saturating_sub(40));
        active_job.result_publish_last_queued_epoch_seconds =
            Some(now_epoch_seconds.saturating_sub(5));

        let panel = mission_control_active_jobs_panel_state(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &LocalInferenceExecutionSnapshot::default(),
            &JobInboxState::default(),
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert_eq!(panel.headline, "ACTIVE");
        assert!(
            panel
                .lines
                .iter()
                .any(|line| { line.contains("FLOW // RELAY / PUBLISHING-RESULT") })
        );
        assert!(
            panel
                .lines
                .iter()
                .any(|line| { line.contains("NEXT // RELAY CONFIRMATION") })
        );
        assert!(panel.lines.iter().any(|line| {
            line.contains("CONT // RESULT SIGNED")
                && line.contains("RELAY ATTEMPT 4")
                && line.contains("WINDOW 195S")
        }));
    }

    #[test]
    fn mission_control_active_jobs_panel_state_distinguishes_wallet_settlement() {
        let mut provider = ProviderRuntimeState::default();
        provider.mode = crate::app_state::ProviderMode::Online;
        let request = fixture_active_job_request("req-active-panel-settle-3389");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.job.as_mut().expect("job should exist").stage = JobLifecycleStage::Delivered;
        active_job.pending_bolt11 = Some("lnbc1settle3389".to_string());

        let panel = mission_control_active_jobs_panel_state(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &LocalInferenceExecutionSnapshot::default(),
            &JobInboxState::default(),
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert_eq!(panel.headline, "AWAITING PAYMENT");
        assert!(
            panel
                .lines
                .iter()
                .any(|line| { line.contains("FLOW // RESULT DELIVERED / AWAITING BUYER PAYMENT") })
        );
        assert!(
            panel
                .lines
                .iter()
                .any(|line| { line.contains("NEXT // BUYER LIGHTNING PAYMENT") })
        );
        assert!(panel.lines.iter().any(|line| {
            line.contains("CONT // AWAITING BUYER PAYMENT") && line.contains("WINDOW 195S")
        }));
        assert!(panel.lines.iter().any(|line| {
            line.contains("SETTLE // AWAITING BUYER PAYMENT") && line.contains("WINDOW 195S")
        }));
    }

    #[test]
    fn mission_control_active_jobs_panel_state_distinguishes_delivered_unpaid_timeout() {
        let mut provider = ProviderRuntimeState::default();
        provider.mode = crate::app_state::ProviderMode::Online;
        let request = fixture_active_job_request("req-active-panel-unpaid-3403");
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.pending_bolt11 = Some("lnbc20n1panelunpaid".to_string());
        let job = active_job.job.as_mut().expect("job should exist");
        job.stage = JobLifecycleStage::Failed;
        job.sa_tick_result_event_id = Some("result-active-panel-unpaid-3403".to_string());
        job.failure_reason = Some(
            "job delivered but unpaid timed out after 195s while awaiting buyer settlement"
                .to_string(),
        );

        let panel = mission_control_active_jobs_panel_state(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &LocalInferenceExecutionSnapshot::default(),
            &JobInboxState::default(),
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        );

        assert_eq!(panel.headline, "UNPAID");
        assert!(
            panel
                .lines
                .iter()
                .any(|line| { line.contains("FLOW // RESULT DELIVERED / BUYER NEVER PAID") })
        );
        assert!(
            panel
                .lines
                .iter()
                .any(|line| { line.contains("NEXT // BUYER SETTLEMENT TIMED OUT") })
        );
        assert!(panel.lines.iter().any(|line| {
            line.contains("CONT // BUYER NEVER SETTLED")
                && line.contains("RESULT RESULT")
                && line.contains("WINDOW 195S")
        }));
        assert!(panel.lines.iter().any(|line| {
            line.contains("SETTLE // BUYER NEVER PAID // INVOICE READY // WINDOW 195S")
        }));
    }

    #[test]
    fn mission_control_buy_mode_panel_state_shows_pending_and_failed_wallet_payment_states() {
        let mut buy_mode = fixture_buy_mode();
        let autopilot_chat = fixture_autopilot_chat();
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let mut requests = queue_buy_mode_request_for_tests();
        let provider_pubkey = "55".repeat(32);
        requests.apply_nip90_buyer_feedback_event(
            "req-buy-render-001",
            provider_pubkey.as_str(),
            "feedback-buy-render-002",
            Some("payment-required"),
            Some("pay invoice"),
            Some(2_000),
            Some("lnbc1buyrenderpending"),
        );
        requests
            .prepare_auto_payment_attempt(
                "req-buy-render-001",
                "lnbc1buyrenderpending",
                Some(2_000),
                1_762_700_050,
            )
            .expect("payment-required invoice should prepare");
        requests.record_auto_payment_pointer("req-buy-render-001", "wallet-buy-render-001");

        let pending = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &SparkPaneState::default(),
            now,
        )
        .expect("buy mode panel should render pending wallet confirmation");
        assert_eq!(pending.payment, "pending");
        assert!(
            pending
                .summary
                .contains("payment pending Spark confirmation")
        );

        let mut wallet = SparkPaneState::default();
        wallet.recent_payments.push(openagents_spark::PaymentSummary {
            id: "wallet-buy-render-001".to_string(),
            direction: "send".to_string(),
            status: "failed".to_string(),
            amount_sats: 2,
            fees_sats: 3,
            method: "lightning".to_string(),
            status_detail: Some(
                "lightning send failed before preimage settlement; see Mission Control log for Breez terminal detail"
                    .to_string(),
            ),
            timestamp: 1_762_700_051,
            ..Default::default()
        });

        let failed = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &wallet,
            now,
        )
        .expect("buy mode panel should render failed wallet state");
        assert_eq!(failed.payment, "failed");
        assert!(
            failed
                .summary
                .contains("lightning send failed before preimage settlement")
        );
        assert!(failed.summary.contains("2 sats invoice"));
        assert!(failed.summary.contains("3 sats fee"));
        assert!(failed.summary.contains("5 sats total debit"));
        assert!(failed.summary.contains("wallet delta -5 sats"));
    }

    #[test]
    fn mission_control_buy_mode_panel_state_distinguishes_seller_settled_local_pending() {
        let mut buy_mode = fixture_buy_mode();
        let autopilot_chat = fixture_autopilot_chat();
        let now = std::time::Instant::now();
        buy_mode.toggle_buy_mode_loop(now);
        let mut requests = queue_buy_mode_request_for_tests();
        let provider_pubkey = "88".repeat(32);
        requests.apply_nip90_request_publish_outcome(
            "req-buy-render-001",
            "event-buy-render-settled",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_result_event(
            "req-buy-render-001",
            provider_pubkey.as_str(),
            "result-buy-render-settled",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            "req-buy-render-001",
            provider_pubkey.as_str(),
            "feedback-buy-render-settled-invoice",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc1buyrendersettled"),
        );
        requests
            .prepare_auto_payment_attempt(
                "req-buy-render-001",
                "lnbc1buyrendersettled",
                Some(2_000),
                1_762_700_060,
            )
            .expect("payment-required invoice should prepare");
        requests.record_auto_payment_pointer("req-buy-render-001", "wallet-buy-settled-001");
        requests.apply_nip90_buyer_feedback_event(
            "req-buy-render-001",
            provider_pubkey.as_str(),
            "feedback-buy-render-settled-success",
            Some("success"),
            Some("wallet-confirmed settlement recorded"),
            Some(2_000),
            None,
        );

        let panel = mission_control_buy_mode_panel_state(
            true,
            &autopilot_chat,
            &buy_mode,
            &requests,
            &SparkPaneState::default(),
            now,
        )
        .expect("buy mode panel should render seller-settled pending local wallet state");

        assert_eq!(panel.next, "buyer local wallet confirmation");
        assert_eq!(panel.provider, "888888..8888");
        assert_eq!(panel.work, "settled");
        assert_eq!(panel.payment, "pending");
        assert!(panel.summary.contains("seller settlement confirmed"));
        assert!(
            panel
                .summary
                .contains("seller settled; awaiting local wallet confirmation")
        );
        assert!(
            panel
                .summary
                .contains("phase seller-settled-pending-wallet")
        );
    }
}
