use crate::app_state::{
    ActiveJobState, ActivityEventDomain, ActivityFeedFilter, ActivityFeedState,
    AgentProfileStatePaneState, AgentScheduleTickPaneState, AlertSeverity, AlertsRecoveryState,
    AutopilotChatState, CadDemoPaneState, CalculatorPaneInputs, CastControlPaneState,
    ChatPaneInputs, CodexAccountPaneState, CodexAppsPaneState, CodexConfigPaneState,
    CodexDiagnosticsPaneState, CodexLabsPaneState, CodexMcpPaneState, CodexModelsPaneState,
    CreateInvoicePaneInputs, CredentialsPaneInputs, CredentialsState, CreditDeskPaneState,
    CreditSettlementLedgerPaneState, DesktopPane, EarnJobLifecycleProjectionState,
    EarningsScoreboardState, JobHistoryPaneInputs, JobHistoryState, JobInboxState,
    JobLifecycleStage, NetworkRequestStatus, NetworkRequestsPaneInputs, NetworkRequestsState,
    NostrSecretState, PaneKind, PaneLoadState, PayInvoicePaneInputs, ProviderBlocker,
    ProviderRuntimeState, ReciprocalLoopState, RelayConnectionsPaneInputs, RelayConnectionsState,
    SettingsPaneInputs, SettingsState, SkillRegistryPaneState, SkillTrustRevocationPaneState,
    SparkPaneInputs, StarterJobStatus, StarterJobsState, SyncHealthState, TrajectoryAuditPaneState,
};
use crate::bitcoin_display::format_sats_amount;
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
    job_inbox_visible_row_count, network_requests_budget_input_bounds,
    network_requests_credit_envelope_input_bounds, network_requests_payload_input_bounds,
    network_requests_skill_scope_input_bounds, network_requests_submit_button_bounds,
    network_requests_timeout_input_bounds, network_requests_type_input_bounds,
    nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds, nostr_reveal_button_bounds,
    pane_content_bounds, reciprocal_loop_reset_button_bounds, reciprocal_loop_start_button_bounds,
    reciprocal_loop_stop_button_bounds, settings_provider_queue_input_bounds,
    settings_relay_input_bounds, settings_reset_button_bounds, settings_save_button_bounds,
    settings_wallet_default_input_bounds, starter_jobs_complete_button_bounds,
    starter_jobs_kill_switch_button_bounds, starter_jobs_row_bounds,
    starter_jobs_visible_row_count, sync_health_rebootstrap_button_bounds,
};
use crate::panes::{
    agent as agent_pane, cad as cad_pane, calculator as calculator_pane, cast as cast_pane,
    chat as chat_pane, codex as codex_pane, credit as credit_pane,
    relay_connections as relay_connections_pane, skill as skill_pane, wallet as wallet_pane,
};
use crate::spark_wallet::SparkPaneState;
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

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
        settings_inputs: &mut SettingsPaneInputs,
        credentials_inputs: &mut CredentialsPaneInputs,
        job_history_inputs: &mut JobHistoryPaneInputs,
        chat_inputs: &mut ChatPaneInputs,
        calculator_inputs: &mut CalculatorPaneInputs,
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
                    paint_autopilot_chat_pane(content_bounds, autopilot_chat, chat_inputs, paint);
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
                        provider_runtime,
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
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    chat_pane::paint(content_bounds, autopilot_chat, chat_inputs, paint);
}

fn paint_go_online_pane(
    content_bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
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
    let toggle_bounds = go_online_toggle_button_bounds(content_bounds);
    let toggle_label = if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
        "Go Online"
    } else {
        "Go Offline"
    };
    paint_action_button(toggle_bounds, toggle_label, paint);

    let title_x = toggle_bounds.max_x() + 18.0;
    paint.scene.draw_text(paint.text.layout(
        "Mission Control",
        Point::new(title_x, content_bounds.origin.y + 16.0),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Earn-first shell for provider state, wallet truth, and job flow.",
        Point::new(title_x, content_bounds.origin.y + 36.0),
        11.0,
        theme::text::MUTED,
    ));

    let header_bottom_y = (content_bounds.origin.y + 50.0).max(toggle_bounds.max_y());
    let card_y = header_bottom_y + 16.0;
    let card_gap = 12.0;
    let top_card_height = 240.0;
    let card_width = ((content_bounds.size.width - 24.0 - card_gap) * 0.5).max(240.0);
    let left_card = Bounds::new(
        content_bounds.origin.x + 12.0,
        card_y,
        card_width,
        top_card_height,
    );
    let right_card = Bounds::new(
        left_card.max_x() + card_gap,
        card_y,
        card_width,
        top_card_height,
    );
    let bottom_card = Bounds::new(
        content_bounds.origin.x + 12.0,
        left_card.max_y() + card_gap,
        content_bounds.size.width - 24.0,
        (content_bounds.max_y() - left_card.max_y() - card_gap - 12.0).max(180.0),
    );

    paint_mission_control_card(left_card, "Provider Rig", theme::accent::PRIMARY, paint);
    paint_mission_control_card(
        right_card,
        "Wallet + First Earnings",
        theme::status::SUCCESS,
        paint,
    );
    paint_mission_control_card(bottom_card, "Job Flow", theme::border::DEFAULT, paint);

    let left_clip = Bounds::new(
        left_card.origin.x + 8.0,
        left_card.origin.y + 8.0,
        left_card.size.width - 16.0,
        left_card.size.height - 16.0,
    );
    let right_clip = Bounds::new(
        right_card.origin.x + 8.0,
        right_card.origin.y + 8.0,
        right_card.size.width - 16.0,
        right_card.size.height - 16.0,
    );
    let left_value_chunk_len = mission_control_value_chunk_len(left_card);
    let right_value_chunk_len = mission_control_value_chunk_len(right_card);
    let left_body_chunk_len = mission_control_body_chunk_len(left_card);

    let now = std::time::Instant::now();
    paint.scene.push_clip(left_clip);
    let mut left_y = left_card.origin.y + 32.0;
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Lane",
        provider_runtime.execution_lane_label(),
        left_value_chunk_len,
    );
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Backend",
        provider_runtime.execution_backend_label(),
        left_value_chunk_len,
    );
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Control",
        provider_runtime.control_authority_label(backend_kernel_authority),
        left_value_chunk_len,
    );
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Projection",
        if earn_job_lifecycle_projection.authority == "non-authoritative" {
            provider_runtime.projection_authority_label()
        } else {
            earn_job_lifecycle_projection.authority.as_str()
        },
        left_value_chunk_len,
    );
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Settlement",
        provider_runtime.settlement_truth_label(),
        left_value_chunk_len,
    );
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Mode",
        provider_runtime.mode.label(),
        left_value_chunk_len,
    );
    if provider_runtime.mode != crate::app_state::ProviderMode::Offline {
        left_y = paint_wrapped_label_line(
            paint,
            left_card.origin.x + 12.0,
            left_y,
            "Uptime",
            &provider_runtime.uptime_seconds(now).to_string(),
            left_value_chunk_len,
        );
    }
    if sa_lane.mode != crate::runtime_lanes::SaRunnerMode::Offline {
        left_y = paint_wrapped_label_line(
            paint,
            left_card.origin.x + 12.0,
            left_y,
            "Runner",
            sa_lane.mode.label(),
            left_value_chunk_len,
        );
    }
    if skl_lane.trust_tier != crate::runtime_lanes::SkillTrustTier::Unknown {
        left_y = paint_wrapped_label_line(
            paint,
            left_card.origin.x + 12.0,
            left_y,
            "SKL trust",
            skl_lane.trust_tier.label(),
            left_value_chunk_len,
        );
    }
    if ac_lane.credit_available {
        left_y = paint_wrapped_label_line(
            paint,
            left_card.origin.x + 12.0,
            left_y,
            "Credit",
            "available",
            left_value_chunk_len,
        );
    }
    left_y = paint_wrapped_label_line(
        paint,
        left_card.origin.x + 12.0,
        left_y,
        "Ollama",
        if provider_runtime.ollama.is_ready() {
            "ready"
        } else if provider_runtime.ollama.reachable {
            "degraded"
        } else {
            "offline"
        },
        left_value_chunk_len,
    );
    if let Some(model) = provider_runtime
        .ollama
        .ready_model
        .as_deref()
        .or(provider_runtime.ollama.configured_model.as_deref())
        .filter(|model| !model.trim().is_empty())
    {
        left_y = paint_wrapped_label_line(
            paint,
            left_card.origin.x + 12.0,
            left_y,
            "Model",
            model,
            left_value_chunk_len,
        );
    }
    if provider_blockers.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Preflight clear",
            Point::new(left_card.origin.x + 12.0, left_y + 2.0),
            11.0,
            theme::status::SUCCESS,
        ));
    } else {
        paint.scene.draw_text(paint.text.layout(
            "Preflight blockers",
            Point::new(left_card.origin.x + 12.0, left_y + 2.0),
            11.0,
            theme::status::ERROR,
        ));
        let mut blocker_y = left_y + 18.0;
        for blocker in provider_blockers.iter().take(3) {
            for line in split_text_for_display(
                &format!(
                    "{} - {}",
                    blocker.code(),
                    mission_control_blocker_detail(*blocker, spark_wallet, provider_runtime)
                ),
                left_body_chunk_len,
            ) {
                paint.scene.draw_text(paint.text.layout_mono(
                    &line,
                    Point::new(left_card.origin.x + 12.0, blocker_y),
                    10.0,
                    theme::status::ERROR,
                ));
                blocker_y += 14.0;
            }
        }
        let hidden_blockers = provider_blockers.len().saturating_sub(3);
        if hidden_blockers > 0 {
            paint.scene.draw_text(paint.text.layout(
                &format!("+{hidden_blockers} more blockers"),
                Point::new(left_card.origin.x + 12.0, blocker_y),
                10.0,
                theme::status::ERROR,
            ));
        }
    }
    paint.scene.pop_clip();

    paint.scene.push_clip(right_clip);
    let mut right_y = right_card.origin.y + 32.0;
    let wallet_balance = spark_wallet
        .balance
        .as_ref()
        .map(|balance| format_sats_amount(balance.total_sats()))
        .unwrap_or_else(|| "loading".to_string());
    right_y = paint_wrapped_label_line(
        paint,
        right_card.origin.x + 12.0,
        right_y,
        "Wallet",
        &wallet_balance,
        right_value_chunk_len,
    );
    right_y = paint_wrapped_label_line(
        paint,
        right_card.origin.x + 12.0,
        right_y,
        "Wallet status",
        spark_wallet.network_status_label(),
        right_value_chunk_len,
    );
    right_y = paint_wrapped_label_line(
        paint,
        right_card.origin.x + 12.0,
        right_y,
        "Today",
        &format_sats_amount(earnings_scoreboard.sats_today),
        right_value_chunk_len,
    );
    right_y = paint_wrapped_label_line(
        paint,
        right_card.origin.x + 12.0,
        right_y,
        "Lifetime",
        &format_sats_amount(earnings_scoreboard.lifetime_sats),
        right_value_chunk_len,
    );
    right_y = paint_wrapped_label_line(
        paint,
        right_card.origin.x + 12.0,
        right_y,
        "Jobs today",
        &earnings_scoreboard.jobs_today.to_string(),
        right_value_chunk_len,
    );
    paint_first_sats_progress(
        Bounds::new(
            right_card.origin.x + 12.0,
            right_y + 4.0,
            right_card.size.width - 24.0,
            48.0,
        ),
        earnings_scoreboard.lifetime_sats,
        paint,
    );
    paint.scene.pop_clip();

    let active_summary = active_job.job.as_ref().map_or_else(
        || "No active job yet".to_string(),
        |job| {
            format!(
                "{} [{}] {}",
                job.capability,
                job.stage.label(),
                format_sats_amount(job.quoted_price_sats)
            )
        },
    );
    let jobs_title_y = bottom_card.origin.y + 32.0;
    paint.scene.draw_text(paint.text.layout(
        "Active job",
        Point::new(bottom_card.origin.x + 12.0, jobs_title_y),
        11.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        &active_summary,
        Point::new(bottom_card.origin.x + 126.0, jobs_title_y),
        11.0,
        theme::text::PRIMARY,
    ));

    let list_origin_y = jobs_title_y + 24.0;
    let recent_requests = job_inbox.requests.iter().rev().take(5).collect::<Vec<_>>();
    if recent_requests.is_empty() {
        let empty_label = if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
            "No jobs visible yet. Relay preview is warming up; observed market activity will appear here before you go online."
        } else {
            "No jobs visible yet. Stay online and Mission Control will fill as demand arrives."
        };
        paint.scene.draw_text(paint.text.layout(
            empty_label,
            Point::new(bottom_card.origin.x + 12.0, list_origin_y + 4.0),
            11.0,
            theme::text::MUTED,
        ));
    } else {
        let row_height = 28.0;
        for (index, request) in recent_requests.iter().enumerate() {
            let row_bounds = Bounds::new(
                bottom_card.origin.x + 12.0,
                list_origin_y + index as f32 * row_height,
                bottom_card.size.width - 24.0,
                22.0,
            );
            let accent = match request.demand_source {
                crate::app_state::JobDemandSource::StarterDemand => theme::status::SUCCESS,
                crate::app_state::JobDemandSource::OpenNetwork => theme::accent::PRIMARY,
            };
            paint.scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(theme::bg::APP.with_alpha(0.55))
                    .with_border(theme::border::DEFAULT, 1.0)
                    .with_corner_radius(4.0),
            );
            let source_label = match request.demand_source {
                crate::app_state::JobDemandSource::StarterDemand => "STARTER",
                crate::app_state::JobDemandSource::OpenNetwork => "OPEN",
            };
            paint.scene.draw_text(paint.text.layout_mono(
                source_label,
                Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 6.0),
                9.0,
                accent,
            ));
            paint.scene.draw_text(paint.text.layout_mono(
                &format_sats_amount(request.price_sats),
                Point::new(row_bounds.origin.x + 74.0, row_bounds.origin.y + 6.0),
                10.0,
                theme::text::PRIMARY,
            ));
            let preview_suffix = if provider_runtime.mode == crate::app_state::ProviderMode::Offline
            {
                "preview".to_string()
            } else {
                request.decision.label()
            };
            paint.scene.draw_text(paint.text.layout(
                &format!(
                    "{}  {}  {}",
                    request.capability, request.requester, preview_suffix
                ),
                Point::new(row_bounds.origin.x + 146.0, row_bounds.origin.y + 5.0),
                10.0,
                theme::text::PRIMARY,
            ));
        }
    }

    if let Some(code) = provider_runtime.degraded_reason_code.as_deref() {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("Last reason code: {code}"),
            Point::new(bottom_card.origin.x + 12.0, bottom_card.max_y() - 18.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_mission_control_card(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.52))
            .with_border(accent.with_alpha(0.72), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout(
        title,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 12.0),
        12.0,
        theme::text::PRIMARY,
    ));
}

fn paint_first_sats_progress(bounds: Bounds, lifetime_sats: u64, paint: &mut PaintContext) {
    const FIRST_SATS_MILESTONES: [u64; 4] = [10, 25, 50, 100];
    let next_target = FIRST_SATS_MILESTONES
        .into_iter()
        .find(|target| lifetime_sats < *target);
    let progress_label = match next_target {
        Some(target) => format!(
            "Next milestone: {} / {} ({} to go)",
            format_sats_amount(lifetime_sats),
            format_sats_amount(target),
            format_sats_amount(target.saturating_sub(lifetime_sats))
        ),
        None => format!(
            "{} earned. First ladder cleared.",
            format_sats_amount(lifetime_sats)
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
    let mut y = content_bounds.origin.y + 12.0;
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
        "Ollama",
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
        "Configured model",
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
            .ollama
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
        &format!("ollama: {ollama_status}"),
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
        "Ollama inventory",
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
    paint_source_badge(content_bounds, "runtime", paint);

    let request_type_bounds = network_requests_type_input_bounds(content_bounds);
    let payload_bounds = network_requests_payload_input_bounds(content_bounds);
    let skill_scope_bounds = network_requests_skill_scope_input_bounds(content_bounds);
    let envelope_bounds = network_requests_credit_envelope_input_bounds(content_bounds);
    let budget_bounds = network_requests_budget_input_bounds(content_bounds);
    let timeout_bounds = network_requests_timeout_input_bounds(content_bounds);
    let submit_bounds = network_requests_submit_button_bounds(content_bounds);

    network_requests_inputs
        .request_type
        .set_max_width(request_type_bounds.size.width);
    network_requests_inputs
        .payload
        .set_max_width(payload_bounds.size.width);
    network_requests_inputs
        .skill_scope_id
        .set_max_width(skill_scope_bounds.size.width);
    network_requests_inputs
        .credit_envelope_ref
        .set_max_width(envelope_bounds.size.width);
    network_requests_inputs
        .budget_sats
        .set_max_width(budget_bounds.size.width);
    network_requests_inputs
        .timeout_seconds
        .set_max_width(timeout_bounds.size.width);

    network_requests_inputs
        .request_type
        .paint(request_type_bounds, paint);
    network_requests_inputs.payload.paint(payload_bounds, paint);
    network_requests_inputs
        .skill_scope_id
        .paint(skill_scope_bounds, paint);
    network_requests_inputs
        .credit_envelope_ref
        .paint(envelope_bounds, paint);
    network_requests_inputs
        .budget_sats
        .paint(budget_bounds, paint);
    network_requests_inputs
        .timeout_seconds
        .paint(timeout_bounds, paint);
    paint_primary_button(submit_bounds, "Submit request", paint);

    paint.scene.draw_text(paint.text.layout(
        "Request type",
        Point::new(
            request_type_bounds.origin.x,
            request_type_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Payload",
        Point::new(payload_bounds.origin.x, payload_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Budget (₿)",
        Point::new(budget_bounds.origin.x, budget_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Skill scope id",
        Point::new(
            skill_scope_bounds.origin.x,
            skill_scope_bounds.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Credit envelope ref",
        Point::new(envelope_bounds.origin.x, envelope_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Timeout (s)",
        Point::new(timeout_bounds.origin.x, timeout_bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        submit_bounds.max_y() + 12.0,
        network_requests.load_state,
        &format!("State: {}", network_requests.load_state.label()),
        network_requests.last_action.as_deref(),
        network_requests.last_error.as_deref(),
    );

    if network_requests.submitted.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No network requests submitted yet.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (idx, request) in network_requests.submitted.iter().take(6).enumerate() {
        let row_bounds = Bounds::new(
            content_bounds.origin.x + 12.0,
            y + idx as f32 * 34.0,
            (content_bounds.size.width - 24.0).max(220.0),
            30.0,
        );
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(theme::bg::APP.with_alpha(0.78))
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(6.0),
        );
        let status_color = match request.status {
            NetworkRequestStatus::Submitted => theme::accent::PRIMARY,
            NetworkRequestStatus::Streaming => theme::status::SUCCESS,
            NetworkRequestStatus::Processing => theme::accent::SECONDARY,
            NetworkRequestStatus::PaymentRequired => theme::status::WARNING,
            NetworkRequestStatus::ResultReceived => theme::status::SUCCESS,
            NetworkRequestStatus::Paid => theme::status::SUCCESS,
            NetworkRequestStatus::Completed => theme::status::SUCCESS,
            NetworkRequestStatus::Failed => theme::status::ERROR,
        };
        let summary = format!(
            "{} {} resolution:{} targets:{} scope:{} env:{} budget:{} timeout:{}s stream:{} published:{} feedback:{} result:{} provider:{} winner:{} winner_event:{} duplicates:{} loser_feedback:{} payment:{} required_at:{} sent_at:{} failed_at:{} pay_error:{} [{}|{}|{}|{}]",
            request.request_id,
            request.request_type,
            request.resolution_mode.label(),
            if request.target_provider_pubkeys.is_empty() {
                "any".to_string()
            } else {
                request.target_provider_pubkeys.join(",")
            },
            request.skill_scope_id.as_deref().unwrap_or("none"),
            request.credit_envelope_ref.as_deref().unwrap_or("none"),
            format_sats_amount(request.budget_sats),
            request.timeout_seconds,
            request.response_stream_id,
            request
                .published_request_event_id
                .as_deref()
                .unwrap_or("n/a"),
            request.last_feedback_status.as_deref().unwrap_or("none"),
            request.last_result_event_id.as_deref().unwrap_or("none"),
            request.last_provider_pubkey.as_deref().unwrap_or("none"),
            request.winning_provider_pubkey.as_deref().unwrap_or("none"),
            request.winning_result_event_id.as_deref().unwrap_or("none"),
            request.duplicate_outcomes.len(),
            request.resolution_feedbacks.len(),
            request.last_payment_pointer.as_deref().unwrap_or("none"),
            request
                .payment_required_at_epoch_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            request
                .payment_sent_at_epoch_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            request
                .payment_failed_at_epoch_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            request.payment_error.as_deref().unwrap_or("none"),
            request.status.label(),
            request.authority_status.as_deref().unwrap_or("pending"),
            request.authority_event_id.as_deref().unwrap_or("event:n/a"),
            request
                .authority_error_class
                .as_deref()
                .unwrap_or("error:n/a")
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            status_color,
        ));
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
            "{} {} src:{} ts:{} scope:{} tick:{} set:{} def:{} {} {}",
            row.job_id,
            row.status.label(),
            row.demand_source.label(),
            row.completed_at_epoch_seconds,
            row.skill_scope_id.as_deref().unwrap_or("none"),
            row.sa_tick_result_event_id.as_deref().unwrap_or("none"),
            row.ac_settlement_event_id.as_deref().unwrap_or("none"),
            row.ac_default_event_id.as_deref().unwrap_or("none"),
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
    paint_secondary_button(bounds, label, paint);
}

pub(crate) fn paint_primary_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::accent::PRIMARY)
            .with_border(theme::accent::PRIMARY, 1.0)
            .with_corner_radius(6.0),
    );
    paint_button_label(bounds, label, theme::font_size::SM, theme::bg::APP, paint);
}

pub(crate) fn paint_secondary_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
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

pub(crate) fn paint_tertiary_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
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

fn paint_filter_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    if active {
        paint_secondary_button(bounds, label, paint);
        return;
    }
    paint_tertiary_button(bounds, label, paint);
}

fn paint_disabled_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
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
    let value_x = x + mission_control_value_x_offset(label);
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));

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
                .unwrap_or_else(|| blocker.detail().to_string())
        }
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
