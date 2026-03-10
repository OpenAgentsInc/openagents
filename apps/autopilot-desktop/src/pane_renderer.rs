use crate::app_state::{
    ActiveJobState, ActivityEventDomain, ActivityFeedFilter, ActivityFeedState,
    AgentProfileStatePaneState, AgentScheduleTickPaneState, AlertSeverity, AlertsRecoveryState,
    AutopilotChatState, CadDemoPaneState, CalculatorPaneInputs, CastControlPaneState,
    ChatPaneInputs, CodexAccountPaneState, CodexAppsPaneState, CodexConfigPaneState,
    CodexDiagnosticsPaneState, CodexLabsPaneState, CodexMcpPaneState, CodexModelsPaneState,
    CreateInvoicePaneInputs, CredentialsPaneInputs, CredentialsState, CreditDeskPaneState,
    CreditSettlementLedgerPaneState, DesktopPane, EarnJobLifecycleProjectionState,
    EarningsScoreboardState, JobHistoryPaneInputs, JobHistoryState, JobInboxState,
    JobLifecycleStage, LocalInferencePaneInputs, LocalInferencePaneState, MissionControlPaneState,
    NetworkRequestsPaneInputs, NetworkRequestsState, NostrSecretState, PaneKind, PaneLoadState,
    PayInvoicePaneInputs, ProjectOpsPaneState, ProviderBlocker, ProviderRuntimeState,
    ReciprocalLoopState, RelayConnectionsPaneInputs, RelayConnectionsState, SettingsPaneInputs,
    SettingsState, SkillRegistryPaneState, SkillTrustRevocationPaneState, SparkPaneInputs,
    StarterJobStatus, StarterJobsState, SyncHealthState, TrajectoryAuditPaneState,
};
use crate::bitcoin_display::{
    BitcoinAmountDisplayMode, format_mission_control_amount, format_sats_amount,
};
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::pane_system::{
    PANE_TITLE_HEIGHT, active_job_abort_button_bounds, active_job_advance_button_bounds,
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
    credentials_visible_row_count, earnings_scoreboard_refresh_button_bounds,
    go_online_toggle_button_bounds, job_history_next_page_button_bounds,
    job_history_prev_page_button_bounds, job_history_search_input_bounds,
    job_history_status_button_bounds, job_history_time_button_bounds,
    job_inbox_accept_button_bounds, job_inbox_reject_button_bounds, job_inbox_row_bounds,
    job_inbox_visible_row_count, mission_control_amount_toggle_button_bounds,
    mission_control_documentation_button_bounds, mission_control_download_model_button_bounds,
    mission_control_layout, network_requests_accept_button_bounds,
    network_requests_budget_input_bounds, network_requests_credit_envelope_input_bounds,
    network_requests_max_price_input_bounds, network_requests_payload_input_bounds,
    network_requests_quote_row_bounds, network_requests_skill_scope_input_bounds,
    network_requests_submit_button_bounds, network_requests_timeout_input_bounds,
    network_requests_type_input_bounds, network_requests_visible_quote_count,
    nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds, nostr_reveal_button_bounds,
    pane_content_bounds, provider_inventory_toggle_button_bounds,
    reciprocal_loop_reset_button_bounds, reciprocal_loop_start_button_bounds,
    reciprocal_loop_stop_button_bounds, settings_provider_queue_input_bounds,
    settings_relay_input_bounds, settings_reset_button_bounds, settings_save_button_bounds,
    settings_wallet_default_input_bounds, starter_jobs_complete_button_bounds,
    starter_jobs_kill_switch_button_bounds, starter_jobs_row_bounds,
    starter_jobs_visible_row_count, sync_health_rebootstrap_button_bounds,
};
use crate::panes::{
    agent as agent_pane, cad as cad_pane, calculator as calculator_pane, cast as cast_pane,
    chat as chat_pane, codex as codex_pane, credit as credit_pane,
    local_inference as local_inference_pane, project_ops as project_ops_pane,
    relay_connections as relay_connections_pane, skill as skill_pane, wallet as wallet_pane,
};
use crate::spark_wallet::SparkPaneState;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, SvgQuad, theme};

pub struct PaneRenderer;

const INACTIVE_PANE_OVERLAY_ALPHA: f32 = 0.2;
const ACTIVE_PANE_FOCUS_CLEARANCE: f32 = 8.0;

impl PaneRenderer {
    #[expect(
        clippy::too_many_arguments,
        reason = "Pane rendering orchestrates all per-pane state until pane modules are split."
    )]
    pub fn paint(
        panes: &mut [DesktopPane],
        canvas_bounds: Bounds,
        active_id: Option<u64>,
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
        codex_diagnostics: &CodexDiagnosticsPaneState,
        sa_lane: &crate::runtime_lanes::SaLaneSnapshot,
        skl_lane: &crate::runtime_lanes::SklLaneSnapshot,
        ac_lane: &crate::runtime_lanes::AcLaneSnapshot,
        provider_runtime: &ProviderRuntimeState,
        local_inference_runtime: &LocalInferenceExecutionSnapshot,
        local_inference: &LocalInferencePaneState,
        provider_blockers: &[ProviderBlocker],
        earnings_scoreboard: &EarningsScoreboardState,
        relay_connections: &RelayConnectionsState,
        sync_health: &SyncHealthState,
        network_requests: &NetworkRequestsState,
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
        spark_inputs: &mut SparkPaneInputs,
        pay_invoice_inputs: &mut PayInvoicePaneInputs,
        create_invoice_inputs: &mut CreateInvoicePaneInputs,
        relay_connections_inputs: &mut RelayConnectionsPaneInputs,
        network_requests_inputs: &mut NetworkRequestsPaneInputs,
        local_inference_inputs: &mut LocalInferencePaneInputs,
        settings_inputs: &mut SettingsPaneInputs,
        credentials_inputs: &mut CredentialsPaneInputs,
        job_history_inputs: &mut JobHistoryPaneInputs,
        chat_inputs: &mut ChatPaneInputs,
        calculator_inputs: &mut CalculatorPaneInputs,
        mission_control: &mut MissionControlPaneState,
        paint: &mut PaintContext,
    ) -> u32 {
        let mut indices: Vec<usize> = (0..panes.len()).collect();
        indices.sort_by_key(|idx| panes[*idx].z_index);
        let dim_inactive_panes = panes.len() > 1 && active_id.is_some();

        let mut next_layer: u32 = 1;
        for idx in indices {
            paint.scene.set_layer(next_layer);
            next_layer = next_layer.saturating_add(1);

            let pane = &mut panes[idx];

            paint
                .scene
                .draw_quad(Quad::new(pane.bounds).with_background(theme::bg::APP));

            pane.frame.set_title(&pane.title);
            pane.frame.set_active(active_id == Some(pane.id));
            pane.frame.set_title_height(PANE_TITLE_HEIGHT);
            pane.frame.paint(pane.bounds, paint);

            let content_bounds = pane_content_bounds(pane.bounds);
            paint.scene.draw_quad(
                Quad::new(content_bounds)
                    .with_background(theme::bg::SURFACE)
                    .with_corner_radius(6.0),
            );

            match pane.kind {
                PaneKind::Empty => paint_empty_pane(content_bounds, paint),
                PaneKind::AutopilotChat => {
                    paint_autopilot_chat_pane(
                        content_bounds,
                        autopilot_chat,
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
                    codex_pane::paint_labs_pane(content_bounds, codex_labs, paint);
                }
                PaneKind::CodexDiagnostics => {
                    codex_pane::paint_diagnostics_pane(content_bounds, codex_diagnostics, paint);
                }
                PaneKind::GoOnline => {
                    paint_go_online_pane(
                        content_bounds,
                        mission_control,
                        provider_runtime,
                        local_inference_runtime,
                        earn_job_lifecycle_projection,
                        backend_kernel_authority,
                        sa_lane,
                        skl_lane,
                        ac_lane,
                        provider_blockers,
                        earnings_scoreboard,
                        spark_wallet,
                        job_inbox,
                        active_job,
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
                    local_inference_pane::paint(
                        content_bounds,
                        local_inference,
                        local_inference_runtime,
                        local_inference_inputs,
                        paint,
                    );
                }
                PaneKind::EarningsScoreboard => {
                    paint_earnings_scoreboard_pane(
                        content_bounds,
                        earnings_scoreboard,
                        provider_runtime,
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
        }

        if dim_inactive_panes {
            let Some(active_bounds) = panes
                .iter()
                .find(|pane| Some(pane.id) == active_id)
                .map(|pane| pane.bounds)
            else {
                return next_layer;
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
                return next_layer;
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

        next_layer
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

fn paint_autopilot_chat_pane(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    spacetime_presence: &crate::spacetime_presence::SpacetimePresenceSnapshot,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    chat_pane::paint(
        content_bounds,
        autopilot_chat,
        spacetime_presence,
        chat_inputs,
        paint,
    );
}

fn paint_go_online_pane(
    content_bounds: Bounds,
    mission_control: &mut MissionControlPaneState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    backend_kernel_authority: bool,
    sa_lane: &crate::runtime_lanes::SaLaneSnapshot,
    skl_lane: &crate::runtime_lanes::SklLaneSnapshot,
    ac_lane: &crate::runtime_lanes::AcLaneSnapshot,
    provider_blockers: &[ProviderBlocker],
    earnings_scoreboard: &EarningsScoreboardState,
    spark_wallet: &SparkPaneState,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    paint: &mut PaintContext,
) {
    mission_control.sync_log_stream(
        provider_runtime,
        local_inference_runtime,
        provider_blockers,
        earn_job_lifecycle_projection,
        spark_wallet,
        job_inbox,
        active_job,
    );

    let layout = mission_control_layout(content_bounds);
    let now = std::time::Instant::now();
    let status_label = provider_runtime.mode.label().to_ascii_uppercase();
    let status_color = match provider_runtime.mode {
        crate::app_state::ProviderMode::Offline => theme::status::WARNING,
        crate::app_state::ProviderMode::Connecting => theme::accent::PRIMARY,
        crate::app_state::ProviderMode::Online => theme::status::SUCCESS,
        crate::app_state::ProviderMode::Degraded => theme::status::ERROR,
    };

    let status_y = layout.status_row.origin.y + 4.0;
    let status_font_size = 12.0;
    let status_right = layout.status_row.max_x() - 12.0;
    let status_gap = 8.0;
    let status_value_width = paint
        .text
        .layout_mono(&status_label, Point::ZERO, status_font_size, status_color)
        .bounds()
        .size
        .width;
    let status_label_width = paint
        .text
        .layout_mono("STATUS:", Point::ZERO, status_font_size, theme::text::MUTED)
        .bounds()
        .size
        .width;
    let status_value_x = status_right - status_value_width;
    let status_label_x =
        (status_value_x - status_gap - status_label_width).max(layout.status_row.origin.x);
    paint.scene.draw_text(paint.text.layout_mono(
        "STATUS:",
        Point::new(status_label_x, status_y),
        status_font_size,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &status_label,
        Point::new(status_value_x, status_y),
        status_font_size,
        status_color,
    ));

    paint_mission_control_section_panel(
        layout.sell_panel,
        "SELL COMPUTE",
        theme::accent::PRIMARY,
        paint,
    );
    paint_mission_control_section_panel(
        layout.earnings_panel,
        "EARNINGS",
        theme::status::SUCCESS,
        paint,
    );
    paint_mission_control_section_panel(
        layout.wallet_panel,
        "WALLET",
        theme::accent::PRIMARY,
        paint,
    );
    paint_mission_control_section_panel(layout.actions_panel, "", theme::border::DEFAULT, paint);
    paint_mission_control_section_panel(
        layout.active_jobs_panel,
        "ACTIVE JOBS",
        theme::accent::PRIMARY,
        paint,
    );

    let toggle_bounds = go_online_toggle_button_bounds(content_bounds);
    let wants_online = matches!(
        provider_runtime.mode,
        crate::app_state::ProviderMode::Offline | crate::app_state::ProviderMode::Degraded
    );
    let toggle_label = if wants_online {
        "GO ONLINE"
    } else {
        "GO OFFLINE"
    };
    paint_mission_control_go_online_button(toggle_bounds, toggle_label, paint);

    let sell_clip = mission_control_section_clip_bounds(layout.sell_panel);
    let sell_value_chunk_len = mission_control_value_chunk_len(layout.sell_panel);
    paint.scene.push_clip(sell_clip);
    let mut sell_y = toggle_bounds.max_y() + 19.0;
    sell_y = paint_wrapped_label_line(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Mode",
        provider_runtime.mode.label(),
        sell_value_chunk_len,
    );
    sell_y = paint_wrapped_label_line(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Model",
        &mission_control_primary_model_label(local_inference_runtime),
        sell_value_chunk_len,
    );
    sell_y = paint_wrapped_label_line(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Backend",
        &mission_control_backend_label(provider_runtime, local_inference_runtime),
        sell_value_chunk_len,
    );
    sell_y = paint_wrapped_label_line(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Load",
        &mission_control_model_load_status(local_inference_runtime),
        sell_value_chunk_len,
    );
    sell_y = paint_wrapped_label_line(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Control",
        provider_runtime.control_authority_label(backend_kernel_authority),
        sell_value_chunk_len,
    );
    let preflight_value = if provider_blockers.is_empty() {
        "clear".to_string()
    } else {
        format!("{} blocker(s)", provider_blockers.len())
    };
    sell_y = paint_wrapped_label_line(
        paint,
        layout.sell_panel.origin.x + 12.0,
        sell_y,
        "Preflight",
        &preflight_value,
        sell_value_chunk_len,
    );
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        sell_y = paint_wrapped_label_line(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "Uptime",
            &provider_runtime.uptime_seconds(now).to_string(),
            sell_value_chunk_len,
        );
    }
    if sa_lane.mode != crate::runtime_lanes::SaRunnerMode::Offline {
        sell_y = paint_wrapped_label_line(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "Runner",
            sa_lane.mode.label(),
            sell_value_chunk_len,
        );
    }
    if skl_lane.trust_tier != crate::runtime_lanes::SkillTrustTier::Unknown {
        sell_y = paint_wrapped_label_line(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "SKL Trust",
            skl_lane.trust_tier.label(),
            sell_value_chunk_len,
        );
    }
    if ac_lane.credit_available {
        sell_y = paint_wrapped_label_line(
            paint,
            layout.sell_panel.origin.x + 12.0,
            sell_y,
            "Credit",
            "available",
            sell_value_chunk_len,
        );
    }
    let load_hint = mission_control_go_online_hint(provider_runtime, local_inference_runtime);
    if !load_hint.is_empty() {
        for (index, line) in split_text_for_display(&load_hint, sell_value_chunk_len)
            .into_iter()
            .enumerate()
        {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(
                    layout.sell_panel.origin.x + 12.0,
                    sell_y + index as f32 * 14.0,
                ),
                10.0,
                theme::text::MUTED,
            ));
        }
    }
    paint.scene.pop_clip();

    let toggle_display_bounds = mission_control_amount_toggle_button_bounds(content_bounds);
    paint_mission_control_amount_toggle(
        toggle_display_bounds,
        mission_control.amount_display_mode,
        paint,
    );
    let earnings_clip = mission_control_section_clip_bounds(layout.earnings_panel);
    paint.scene.push_clip(earnings_clip);
    let mut earnings_y = layout.earnings_panel.origin.y + 53.0;
    earnings_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.earnings_panel.origin.x + 12.0,
        earnings_y,
        "Today",
        &format_mission_control_amount(
            earnings_scoreboard.sats_today,
            mission_control.amount_display_mode,
        ),
        mission_control_value_chunk_len(layout.earnings_panel),
        layout.earnings_panel.size.width - 24.0,
        true,
    );
    earnings_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.earnings_panel.origin.x + 12.0,
        earnings_y,
        "This Month",
        &format_mission_control_amount(
            earnings_scoreboard.sats_this_month,
            mission_control.amount_display_mode,
        ),
        mission_control_value_chunk_len(layout.earnings_panel),
        layout.earnings_panel.size.width - 24.0,
        true,
    );
    earnings_y = paint_mission_control_amount_line(
        paint,
        layout.earnings_panel.origin.x + 12.0,
        earnings_y,
        "All Time",
        &format_mission_control_amount(
            earnings_scoreboard.lifetime_sats,
            mission_control.amount_display_mode,
        ),
        Hsla::from_hex(0x6FD7D2),
        14.0,
        layout.earnings_panel.size.width - 24.0,
        false,
    );
    paint_first_sats_progress(
        Bounds::new(
            layout.earnings_panel.origin.x + 12.0,
            earnings_y + 6.0,
            layout.earnings_panel.size.width - 24.0,
            48.0,
        ),
        earnings_scoreboard.lifetime_sats,
        mission_control.amount_display_mode,
        paint,
    );
    paint.scene.pop_clip();

    let wallet_clip = mission_control_section_clip_bounds(layout.wallet_panel);
    paint.scene.push_clip(wallet_clip);
    let wallet_balance = spark_wallet
        .balance
        .as_ref()
        .map(|balance| {
            format_mission_control_amount(balance.total_sats(), mission_control.amount_display_mode)
        })
        .unwrap_or_else(|| "loading".to_string());
    let wallet_status = match spark_wallet.network_status_label() {
        "connected" => "Connected",
        "disconnected" => "Disconnected",
        _ => "Unknown",
    };
    let wallet_address = spark_wallet
        .spark_address
        .as_deref()
        .or(spark_wallet.bitcoin_address.as_deref())
        .map(mask_secret)
        .unwrap_or_else(|| "not generated".to_string());
    let wallet_value_chunk_len = mission_control_value_chunk_len(layout.wallet_panel);
    let mut wallet_y = layout.wallet_panel.origin.y + 41.0;
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
    wallet_y = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Address",
        &wallet_address,
        wallet_value_chunk_len,
        layout.wallet_panel.size.width - 24.0,
        false,
    );
    let _ = paint_wrapped_label_line_mission_control_label(
        paint,
        layout.wallet_panel.origin.x + 12.0,
        wallet_y,
        "Balance",
        &wallet_balance,
        wallet_value_chunk_len,
        layout.wallet_panel.size.width - 24.0,
        true,
    );
    paint.scene.pop_clip();

    let download_bounds = mission_control_download_model_button_bounds(content_bounds);
    let download_label = mission_control_download_button_label(local_inference_runtime);
    if local_inference_runtime.busy || local_inference_runtime.is_ready() {
        paint_disabled_button(download_bounds, &download_label, paint);
    } else {
        paint_secondary_button(download_bounds, &download_label, paint);
    }
    paint_action_button(
        mission_control_documentation_button_bounds(content_bounds),
        "DOCUMENTATION",
        paint,
    );

    let active_clip = mission_control_section_clip_bounds(layout.active_jobs_panel);
    paint.scene.push_clip(active_clip);
    let active_summary = if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
        "Go Online to Start Jobs.".to_string()
    } else if let Some(job) = active_job.job.as_ref() {
        format!(
            "{} [{}] {}",
            job.capability,
            job.stage.label(),
            format_mission_control_amount(
                job.quoted_price_sats,
                mission_control.amount_display_mode
            )
        )
    } else {
        "Watching relays for matching jobs.".to_string()
    };
    for (index, line) in split_text_for_display(
        &active_summary,
        mission_control_body_chunk_len(layout.active_jobs_panel),
    )
    .into_iter()
    .enumerate()
    {
        paint.scene.draw_text(paint.text.layout(
            &line,
            Point::new(
                layout.active_jobs_panel.origin.x + 12.0,
                layout.active_jobs_panel.origin.y + 43.0 + index as f32 * 16.0,
            ),
            11.0,
            if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
                theme::text::MUTED
            } else {
                theme::text::PRIMARY
            },
        ));
    }
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("Observed requests: {}", job_inbox.requests.len()),
            Point::new(
                layout.active_jobs_panel.origin.x + 12.0,
                layout.active_jobs_panel.max_y() - 18.0,
            ),
            10.0,
            theme::text::MUTED,
        ));
    }
    paint.scene.pop_clip();

    paint_mission_control_section_panel(layout.log_stream, "LOG STREAM", theme::border::DEFAULT, paint);
    let log_body_bounds = Bounds::new(
        layout.log_stream.origin.x,
        layout.log_stream.origin.y + 24.0,
        layout.log_stream.size.width,
        (layout.log_stream.size.height - 24.0).max(0.0),
    );
    mission_control.log_stream.set_title("");
    mission_control.log_stream.paint(log_body_bounds, paint);
}

fn paint_mission_control_section_panel(
    bounds: Bounds,
    title: &str,
    _accent: Hsla,
    paint: &mut PaintContext,
) {
    let width = bounds.size.width.max(0.0);
    let height = bounds.size.height.max(0.0);
    if width <= 1.0 || height <= 1.0 {
        return;
    }

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<defs>
<linearGradient id="sectionBorderGrad" x1="0%" y1="0%" x2="100%" y2="100%">
<stop offset="0%" stop-color="#21242C"/>
<stop offset="100%" stop-color="#6FD7D2" stop-opacity="0.25"/>
</linearGradient>
</defs>
<rect x="0.5" y="0.5" width="{rw}" height="{rh}" rx="10" ry="10" fill="#1A1E27" fill-opacity="0.70" stroke="url(#sectionBorderGrad)" stroke-width="1"/>
</svg>"##,
        w = width,
        h = height,
        rw = (width - 1.0).max(0.0),
        rh = (height - 1.0).max(0.0),
    );
    paint.scene.draw_svg(SvgQuad::new(
        bounds,
        std::sync::Arc::<[u8]>::from(svg.into_bytes()),
    ));

    if !title.is_empty() {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("// {title}"),
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
            12.0,
            Hsla::from_hex(0xD8DFF0),
        ));
    }
}

fn mission_control_section_clip_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + 8.0,
        (bounds.size.width - 16.0).max(0.0),
        (bounds.size.height - 16.0).max(0.0),
    )
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
        Hsla::from_hex(0x8A909E),
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
    paint.scene.draw_text(
        paint.text
            .layout_mono(
                value,
                Point::new(target_x, y - 1.0),
                value_font_size,
                value_color,
            ),
    );
    let row_bottom = y + 20.0;
    if show_divider {
        paint_mission_control_row_divider(paint, x, row_bottom, row_width)
    } else {
        row_bottom + 20.0
    }
}

fn mission_control_download_button_label(
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    if local_inference_runtime.busy {
        return String::from("LOADING GPT-OSS 20B");
    }
    if local_inference_runtime.is_ready() {
        return String::from("GPT-OSS 20B READY");
    }
    let model = local_inference_runtime
        .configured_model
        .as_deref()
        .unwrap_or("gpt-oss-20b");
    format!("LOAD {}", mission_control_short_model_label(model))
}

fn mission_control_short_model_label(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.len() <= 18 {
        return trimmed.to_ascii_uppercase();
    }
    let prefix: String = trimmed.chars().take(15).collect();
    format!("{}...", prefix.to_ascii_uppercase())
}

fn mission_control_primary_model_label(
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    local_inference_runtime
        .ready_model
        .as_deref()
        .or(local_inference_runtime.configured_model.as_deref())
        .map(mission_control_short_model_label)
        .unwrap_or_else(|| String::from("GPT-OSS 20B"))
}

fn mission_control_backend_label(
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    if local_inference_runtime.is_ready() || local_inference_runtime.configured_model.is_some() {
        let backend = if local_inference_runtime.backend_label.trim().is_empty() {
            "psionic"
        } else {
            local_inference_runtime.backend_label.as_str()
        };
        return format!("Psionic GPT-OSS ({backend})");
    }
    provider_runtime.execution_backend_label().to_string()
}

fn mission_control_model_load_status(
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    if local_inference_runtime.busy {
        String::from("loading")
    } else if local_inference_runtime.is_ready() {
        String::from("loaded")
    } else if !local_inference_runtime.artifact_present {
        String::from("artifact missing")
    } else {
        String::from("not loaded")
    }
}

fn mission_control_go_online_hint(
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> String {
    if !matches!(
        provider_runtime.mode,
        crate::app_state::ProviderMode::Offline | crate::app_state::ProviderMode::Degraded
    ) {
        return String::new();
    }
    if local_inference_runtime.busy {
        return String::from(
            "GPT-OSS 20B is loading. Go Online unlocks when the model is resident.",
        );
    }
    if !local_inference_runtime.artifact_present {
        return local_inference_runtime
            .configured_model_path
            .as_deref()
            .map(|path| {
                format!("Mission Control needs GPT-OSS 20B at {path} before you can go online.")
            })
            .unwrap_or_else(|| {
                String::from("Mission Control needs GPT-OSS 20B before you can go online.")
            });
    }
    if !local_inference_runtime.is_ready() {
        return String::from("Load GPT-OSS 20B before you go online.");
    }
    String::new()
}

fn paint_first_sats_progress(
    bounds: Bounds,
    lifetime_sats: u64,
    amount_display_mode: BitcoinAmountDisplayMode,
    paint: &mut PaintContext,
) {
    const FIRST_SATS_MILESTONES: [u64; 4] = [10, 25, 50, 100];
    let next_target = FIRST_SATS_MILESTONES
        .into_iter()
        .find(|target| lifetime_sats < *target);
    let progress_label = match next_target {
        Some(target) => format!(
            "Next milestone: {} / {} ({} to go)",
            format_mission_control_amount(lifetime_sats, amount_display_mode),
            format_mission_control_amount(target, amount_display_mode),
            format_mission_control_amount(
                target.saturating_sub(lifetime_sats),
                amount_display_mode
            )
        ),
        None => format!(
            "{} earned. First ladder cleared.",
            format_mission_control_amount(lifetime_sats, amount_display_mode)
        ),
    };
    let progress_ratio = next_target.map_or(1.0, |target| {
        if target == 0 {
            1.0
        } else {
            (lifetime_sats as f32 / target as f32).clamp(0.0, 1.0)
        }
    });

    paint.scene.draw_text(paint.text.layout(
        "First earnings progression",
        Point::new(bounds.origin.x, bounds.origin.y),
        11.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &progress_label,
        Point::new(bounds.origin.x, bounds.origin.y + 16.0),
        10.0,
        theme::text::PRIMARY,
    ));

    let track_bounds = Bounds::new(
        bounds.origin.x,
        bounds.origin.y + 30.0,
        bounds.size.width,
        10.0,
    );
    let fill_bounds = Bounds::new(
        track_bounds.origin.x,
        track_bounds.origin.y,
        track_bounds.size.width * progress_ratio,
        track_bounds.size.height,
    );
    paint.scene.draw_quad(
        Quad::new(track_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(5.0),
    );
    paint.scene.draw_quad(
        Quad::new(fill_bounds)
            .with_background(theme::status::SUCCESS.with_alpha(0.72))
            .with_corner_radius(5.0),
    );
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
        if provider_runtime.ollama.is_ready() {
            "ready"
        } else if provider_runtime.ollama.reachable {
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
            .ollama
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
                crate::state::provider_runtime::LocalInferenceBackend::Ollama => provider_runtime
                    .ollama
                    .ready_model
                    .as_deref()
                    .or(provider_runtime.ollama.configured_model.as_deref()),
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
    let ollama_status = if provider_blockers.contains(&ProviderBlocker::OllamaUnavailable)
        || provider_blockers.contains(&ProviderBlocker::OllamaModelUnavailable)
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
        &format!("local_inference: {ollama_status}"),
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
            if provider_runtime.ollama.available_models.is_empty() {
                "none".to_string()
            } else {
                provider_runtime.ollama.available_models.join(", ")
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
            if provider_runtime.ollama.loaded_models.is_empty() {
                "none".to_string()
            } else {
                provider_runtime.ollama.loaded_models.join(", ")
            }
        ),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    if let Some(metrics) = provider_runtime.ollama.last_metrics.as_ref() {
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
        .ollama
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
    earnings_scoreboard: &EarningsScoreboardState,
    provider_runtime: &ProviderRuntimeState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime+wallet", paint);

    let refresh_bounds = earnings_scoreboard_refresh_button_bounds(content_bounds);
    paint_action_button(refresh_bounds, "Refresh metrics", paint);

    let state_color = match earnings_scoreboard.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let stale = earnings_scoreboard.is_stale(std::time::Instant::now());
    let stale_suffix = if stale { " (stale)" } else { "" };
    let mut y = refresh_bounds.max_y() + 14.0;
    paint.scene.draw_text(paint.text.layout(
        &format!(
            "State: {}{stale_suffix}",
            earnings_scoreboard.load_state.label()
        ),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;

    if let Some(action) = earnings_scoreboard.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            action,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    if let Some(error) = earnings_scoreboard.last_error.as_deref() {
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
        "Today",
        &format_sats_amount(earnings_scoreboard.sats_today),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "This month",
        &format_sats_amount(earnings_scoreboard.sats_this_month),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Lifetime",
        &format_sats_amount(earnings_scoreboard.lifetime_sats),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Jobs today",
        &earnings_scoreboard.jobs_today.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last job result",
        &earnings_scoreboard.last_job_result,
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Online uptime (s)",
        &provider_runtime
            .uptime_seconds(std::time::Instant::now())
            .to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "First job latency (s)",
        &earnings_scoreboard
            .first_job_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Completion ratio",
        &format_bps_percent(earnings_scoreboard.completion_ratio_bps),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Payout success ratio",
        &format_bps_percent(earnings_scoreboard.payout_success_ratio_bps),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Wallet confirm latency (avg s)",
        &earnings_scoreboard
            .avg_wallet_confirmation_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
}

fn format_bps_percent(value: Option<u16>) -> String {
    value.map_or_else(
        || "n/a".to_string(),
        |bps| format!("{:.2}%", (bps as f64) / 100.0),
    )
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
                        "{} family={} backend={} provider={} lot={} qty={}/{} price={} terms={} source={}",
                        quote.product_id,
                        quote.compute_family_label(),
                        quote.backend_label(),
                        quote.provider_id,
                        quote.capacity_lot_id,
                        quote.requested_quantity,
                        quote.available_quantity,
                        format_sats_amount(quote.price_sats),
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
                            "{} -> {} {} provider={} qty={} price={} at={}",
                            order.quote_id,
                            order.instrument_id,
                            order.product_id,
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
                        "{} family={} backend={} provider={} lot={} qty={}/{} price={} window={} terms={}",
                        quote.product_id,
                        quote.compute_family_label(),
                        quote.backend_label(),
                        quote.provider_id,
                        quote.capacity_lot_id,
                        quote.requested_quantity,
                        quote.available_quantity,
                        format_sats_amount(quote.price_sats),
                        quote.delivery_window_label,
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
                            "{} -> {} {} provider={} qty={} price={} window={} remedy={}",
                            order.quote_id,
                            order.instrument_id,
                            order.product_id,
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

    for row_index in 0..visible_rows {
        let request = &job_inbox.requests[row_index];
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
            "#{} {} {} src:{} scope:{} env:{} {} ttl:{}s {} {} eligibility:{}",
            request.arrival_seq,
            request.request_id,
            request.capability,
            request.demand_source.label(),
            request.skill_scope_id.as_deref().unwrap_or("none"),
            request.ac_envelope_event_id.as_deref().unwrap_or("none"),
            format_sats_amount(request.price_sats),
            request.ttl_seconds,
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

fn paint_active_job_pane(
    content_bounds: Bounds,
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    paint: &mut PaintContext,
) {
    paint_source_badge(
        content_bounds,
        &earn_job_lifecycle_projection.stream_id,
        paint,
    );

    let advance_bounds = active_job_advance_button_bounds(content_bounds);
    let abort_bounds = active_job_abort_button_bounds(content_bounds);
    paint_disabled_button(advance_bounds, "Execution auto", paint);
    if active_job.runtime_supports_abort {
        paint_action_button(abort_bounds, "Abort job", paint);
    } else {
        paint_disabled_button(abort_bounds, "Abort unsupported", paint);
        paint.scene.draw_text(paint.text.layout(
            "Abort disabled: runtime lane does not support cancel.",
            Point::new(content_bounds.origin.x + 12.0, abort_bounds.max_y() + 8.0),
            10.0,
            theme::text::MUTED,
        ));
    }

    let mut y = advance_bounds.max_y()
        + if active_job.runtime_supports_abort {
            12.0
        } else {
            24.0
        };
    y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        active_job.load_state,
        &format!("State: {}", active_job.load_state.label()),
        active_job.last_action.as_deref(),
        active_job.last_error.as_deref(),
    );

    if active_job.load_state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for active-job replay frame...",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let Some(job) = active_job.job.as_ref() else {
        paint.scene.draw_text(paint.text.layout(
            "No active job selected.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    };

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Job ID",
        &job.job_id,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Requester",
        &job.requester,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Capability",
        &job.capability,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Demand source",
        job.demand_source.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Stage",
        job.stage.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Projection authority",
        &earn_job_lifecycle_projection.authority,
    );

    let stage_flow = [
        JobLifecycleStage::Received,
        JobLifecycleStage::Accepted,
        JobLifecycleStage::Running,
        JobLifecycleStage::Delivered,
        JobLifecycleStage::Paid,
    ];
    paint.scene.draw_text(paint.text.layout(
        "Timeline",
        Point::new(content_bounds.origin.x + 12.0, y + 4.0),
        11.0,
        theme::text::MUTED,
    ));
    let mut timeline_y = y + 20.0;
    let current_idx = stage_flow
        .iter()
        .position(|stage| *stage == job.stage)
        .unwrap_or(stage_flow.len().saturating_sub(1));
    for (idx, stage) in stage_flow.iter().enumerate() {
        let marker = if idx <= current_idx { "x" } else { " " };
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("[{marker}] {}", stage.label()),
            Point::new(content_bounds.origin.x + 12.0, timeline_y),
            10.0,
            if idx <= current_idx {
                theme::status::SUCCESS
            } else {
                theme::text::MUTED
            },
        ));
        timeline_y += 14.0;
    }

    let mut metadata_y = timeline_y + 6.0;
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "Skill scope",
        job.skill_scope_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "SKL manifest",
        job.skl_manifest_a.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "SA tick request",
        job.sa_tick_request_event_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "SA tick result",
        job.sa_tick_result_event_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "Trajectory session",
        job.sa_trajectory_session_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "AC envelope",
        job.ac_envelope_event_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "AC settlement",
        job.ac_settlement_event_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "AC default",
        job.ac_default_event_id.as_deref().unwrap_or("none"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "Invoice ID",
        job.invoice_id.as_deref().unwrap_or("n/a"),
    );
    metadata_y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        metadata_y,
        "Payment ID",
        job.payment_id.as_deref().unwrap_or("n/a"),
    );
    if let Some(reason) = job.failure_reason.as_deref() {
        metadata_y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            metadata_y,
            "Failure reason",
            reason,
        );
    }

    paint.scene.draw_text(paint.text.layout(
        "Execution log",
        Point::new(content_bounds.origin.x + 12.0, metadata_y + 4.0),
        11.0,
        theme::text::MUTED,
    ));
    let mut log_y = metadata_y + 20.0;
    for event in job.events.iter().take(10) {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("[#{:03}] {}", event.seq, event.message),
            Point::new(content_bounds.origin.x + 12.0, log_y),
            10.0,
            theme::text::PRIMARY,
        ));
        log_y += 14.0;
    }
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
            "{} {} src:{} ts:{} scope:{} tick:{} set:{} def:{} proof:{} qty:{}/{} var:{} rej:{} {} {}",
            row.job_id,
            row.status.label(),
            row.demand_source.label(),
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

fn paint_disabled_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint_button(bounds, label, ButtonStyle::Disabled, paint);
}

fn paint_mission_control_go_online_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    let base_layer = paint.scene.layer();
    let button_layer = base_layer.saturating_add(1);
    paint.scene.set_layer(button_layer);

    let glow = Hsla::from_hex(0x0891B2);
    let mut glow_outer_spread = 6.0;
    let mut glow_inner_spread = 3.0;
    if label == "GO ONLINE" {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f32())
            .unwrap_or(0.0);
        let pulse = ((now_secs * 2.4).sin() * 0.5) + 0.5;
        glow_outer_spread = 6.0 + pulse * 2.0;
        glow_inner_spread = 3.0 + pulse * 1.0;
    }
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x - glow_outer_spread,
            bounds.origin.y - glow_outer_spread,
            bounds.size.width + glow_outer_spread * 2.0,
            bounds.size.height + glow_outer_spread * 2.0,
        ))
        .with_border(glow.with_alpha(0.16), 1.0)
        .with_corner_radius(10.0 + glow_outer_spread),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x - glow_inner_spread,
            bounds.origin.y - glow_inner_spread,
            bounds.size.width + glow_inner_spread * 2.0,
            bounds.size.height + glow_inner_spread * 2.0,
        ))
        .with_border(glow.with_alpha(0.30), 1.0)
        .with_corner_radius(10.0 + glow_inner_spread),
    );
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x0A5F78))
            .with_border(Hsla::from_hex(0x0891B2), 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 1.0,
            bounds.origin.y + 1.0,
            (bounds.size.width - 2.0).max(0.0),
            (bounds.size.height * 0.5 - 1.0).max(0.0),
        ))
        .with_background(Hsla::from_hex(0x0891B2))
        .with_corner_radius(9.0),
    );
    paint_button_label_mono(bounds, label, 24.0, Hsla::from_hex(0xFFFFFF), paint);

    paint.scene.set_layer(base_layer);
}

fn paint_mission_control_amount_toggle(
    bounds: Bounds,
    mode: BitcoinAmountDisplayMode,
    paint: &mut PaintContext,
) {
    let integer_active = matches!(mode, BitcoinAmountDisplayMode::Integer);
    let outer = bounds;
    let segment_width = (outer.size.width / 2.0).max(0.0);
    let active_segment = if integer_active {
        Bounds::new(outer.origin.x + 1.0, outer.origin.y + 1.0, (segment_width - 2.0).max(0.0), (outer.size.height - 2.0).max(0.0))
    } else {
        Bounds::new(outer.origin.x + segment_width + 1.0, outer.origin.y + 1.0, (segment_width - 2.0).max(0.0), (outer.size.height - 2.0).max(0.0))
    };

    paint.scene.draw_quad(
        Quad::new(outer)
            .with_background(Hsla::from_hex(0x121419))
            .with_border(Hsla::from_hex(0x8A909E), 1.0)
            .with_corner_radius(10.0),
    );
    paint.scene.draw_quad(
        Quad::new(active_segment)
            .with_background(Hsla::from_hex(0x0891B2))
            .with_corner_radius(9.0),
    );

    let left_label = "INTEGER";
    let right_label = "LEGACY";
    let left_color = if integer_active {
        Hsla::from_hex(0xFFFFFF)
    } else {
        Hsla::from_hex(0x8A909E)
    };
    let right_color = if integer_active {
        Hsla::from_hex(0x8A909E)
    } else {
        Hsla::from_hex(0xFFFFFF)
    };
    paint_button_label_mono(
        Bounds::new(outer.origin.x, outer.origin.y, segment_width, outer.size.height),
        left_label,
        10.0,
        left_color,
        paint,
    );
    paint_button_label_mono(
        Bounds::new(
            outer.origin.x + segment_width,
            outer.origin.y,
            segment_width,
            outer.size.height,
        ),
        right_label,
        10.0,
        right_color,
        paint,
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
            let w = bounds.size.width.max(0.0);
            let h = bounds.size.height.max(0.0);
            if w > 1.0 && h > 1.0 {
                let svg = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<defs>
<radialGradient id="primaryButtonFill" cx="50%" cy="50%" r="50%">
<stop offset="2.4%" stop-color="#03857F" stop-opacity="0.60"/>
<stop offset="100%" stop-color="#121419" stop-opacity="0.20"/>
</radialGradient>
</defs>
<rect x="0.5" y="0.5" width="{rw}" height="{rh}" rx="10" ry="10" fill="url(#primaryButtonFill)" stroke="#0891B2" stroke-width="1"/>
</svg>"##,
                    w = w,
                    h = h,
                    rw = (w - 1.0).max(0.0),
                    rh = (h - 1.0).max(0.0),
                );
                paint.scene.draw_svg(SvgQuad::new(
                    bounds,
                    std::sync::Arc::<[u8]>::from(svg.into_bytes()),
                ));
            }
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
        Hsla::from_hex(0x8A909E),
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, value_chunk_len.max(1)) {
        let value_width = paint
            .text
            .layout_mono(&chunk, Point::ZERO, 12.0, theme::text::PRIMARY)
            .bounds()
            .size
            .width;
        let target_x = (value_right - value_width).max(value_x);
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(target_x, line_y),
            12.0,
            theme::text::PRIMARY,
        ));
        line_y += 18.0;
    }
    let row_bottom = line_y.max(y + 18.0);
    if show_divider {
        paint_mission_control_row_divider(paint, x, row_bottom, row_width)
    } else {
        row_bottom + 20.0
    }
}

fn paint_mission_control_row_divider(
    paint: &mut PaintContext,
    x: f32,
    row_bottom: f32,
    row_width: f32,
) -> f32 {
    let divider_y = row_bottom + 10.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(x, divider_y, row_width.max(0.0), 1.0))
            .with_background(Hsla::from_hex(0x0E0E0F)),
    );
    divider_y + 1.0 + 10.0
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

fn mission_control_blocker_detail(
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
        ProviderBlocker::OllamaUnavailable | ProviderBlocker::OllamaModelUnavailable => {
            provider_runtime
                .ollama
                .last_error
                .as_deref()
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| match blocker {
                    ProviderBlocker::OllamaUnavailable => {
                        "Local inference backend is unavailable".to_string()
                    }
                    ProviderBlocker::OllamaModelUnavailable => {
                        "No local inference model is ready".to_string()
                    }
                    _ => blocker.detail().to_string(),
                })
        }
        ProviderBlocker::AppleFoundationModelsUnavailable
        | ProviderBlocker::AppleFoundationModelsModelUnavailable => provider_runtime
            .apple_fm
            .last_error
            .as_deref()
            .or(provider_runtime.apple_fm.availability_message.as_deref())
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
        create_invoice_view_state, mission_control_body_chunk_len, mission_control_value_chunk_len,
        mission_control_value_x_offset, nostr_identity_view_state, pay_invoice_view_state,
        payment_terminal_status, spark_wallet_view_state, split_text_for_display,
    };
    use crate::app_state::PaneLoadState;
    use crate::spark_wallet::SparkPaneState;
    use wgpui::Bounds;

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
    fn mission_control_chunk_lengths_respect_minimums() {
        let narrow = Bounds::new(0.0, 0.0, 180.0, 100.0);
        let wide = Bounds::new(0.0, 0.0, 360.0, 100.0);

        assert!(mission_control_value_chunk_len(narrow) >= 8);
        assert!(mission_control_body_chunk_len(narrow) >= 12);
        assert!(mission_control_value_chunk_len(wide) > mission_control_value_chunk_len(narrow));
        assert!(mission_control_body_chunk_len(wide) > mission_control_body_chunk_len(narrow));
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
}
