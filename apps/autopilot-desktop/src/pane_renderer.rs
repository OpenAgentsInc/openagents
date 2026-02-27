use crate::app_state::{
    ActiveJobState, ActivityEventDomain, ActivityFeedFilter, ActivityFeedState,
    AgentNetworkSimulationPaneState, AgentProfileStatePaneState, AgentScheduleTickPaneState,
    AlertSeverity, AlertsRecoveryState, AutopilotChatState, ChatPaneInputs, CodexAccountPaneState,
    CodexAppsPaneState, CodexConfigPaneState, CodexMcpPaneState, CodexModelsPaneState,
    CodexRemoteSkillsPaneState, CreateInvoicePaneInputs, CreditDeskPaneState,
    CreditSettlementLedgerPaneState, DesktopPane, EarningsScoreboardState, JobHistoryPaneInputs,
    JobHistoryState, JobInboxState, JobLifecycleStage, NetworkRequestStatus,
    NetworkRequestsPaneInputs, NetworkRequestsState, NostrSecretState, PaneKind, PaneLoadState,
    PayInvoicePaneInputs, ProviderBlocker, ProviderRuntimeState, RelayConnectionsPaneInputs,
    RelayConnectionsState, RelaySecuritySimulationPaneState, SettingsPaneInputs, SettingsState,
    SkillRegistryPaneState, SkillTrustRevocationPaneState, SparkPaneInputs, StarterJobStatus,
    StarterJobsState, SyncHealthState, TrajectoryAuditPaneState,
    TreasuryExchangeSimulationPaneState,
};
use crate::pane_system::{
    PANE_TITLE_HEIGHT, active_job_abort_button_bounds, active_job_advance_button_bounds,
    activity_feed_filter_button_bounds, activity_feed_refresh_button_bounds,
    activity_feed_row_bounds, activity_feed_visible_row_count, alerts_recovery_ack_button_bounds,
    alerts_recovery_recover_button_bounds, alerts_recovery_resolve_button_bounds,
    alerts_recovery_row_bounds, alerts_recovery_visible_row_count,
    earnings_scoreboard_refresh_button_bounds, go_online_toggle_button_bounds,
    job_history_next_page_button_bounds, job_history_prev_page_button_bounds,
    job_history_search_input_bounds, job_history_status_button_bounds,
    job_history_time_button_bounds, job_inbox_accept_button_bounds, job_inbox_reject_button_bounds,
    job_inbox_row_bounds, job_inbox_visible_row_count, network_requests_budget_input_bounds,
    network_requests_credit_envelope_input_bounds, network_requests_payload_input_bounds,
    network_requests_skill_scope_input_bounds, network_requests_submit_button_bounds,
    network_requests_timeout_input_bounds, network_requests_type_input_bounds,
    nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds, nostr_reveal_button_bounds,
    pane_content_bounds, settings_provider_queue_input_bounds, settings_relay_input_bounds,
    settings_reset_button_bounds, settings_save_button_bounds,
    settings_wallet_default_input_bounds, starter_jobs_complete_button_bounds,
    starter_jobs_row_bounds, starter_jobs_visible_row_count, sync_health_rebootstrap_button_bounds,
};
use crate::panes::{
    agent as agent_pane, chat as chat_pane, codex as codex_pane, credit as credit_pane,
    relay_connections as relay_connections_pane, simulation as simulation_pane,
    skill as skill_pane, wallet as wallet_pane,
};
use crate::spark_wallet::SparkPaneState;
use wgpui::{Bounds, Component, PaintContext, Point, Quad, theme};

pub struct PaneRenderer;

impl PaneRenderer {
    #[expect(
        clippy::too_many_arguments,
        reason = "Pane rendering orchestrates all per-pane state until pane modules are split."
    )]
    pub fn paint(
        panes: &mut [DesktopPane],
        active_id: Option<u64>,
        nostr_identity: Option<&nostr::NostrIdentity>,
        nostr_identity_error: Option<&str>,
        nostr_secret_state: &NostrSecretState,
        autopilot_chat: &AutopilotChatState,
        codex_account: &CodexAccountPaneState,
        codex_models: &CodexModelsPaneState,
        codex_config: &CodexConfigPaneState,
        codex_mcp: &CodexMcpPaneState,
        codex_apps: &CodexAppsPaneState,
        codex_remote_skills: &CodexRemoteSkillsPaneState,
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
        activity_feed: &ActivityFeedState,
        alerts_recovery: &AlertsRecoveryState,
        settings: &SettingsState,
        job_inbox: &JobInboxState,
        active_job: &ActiveJobState,
        job_history: &JobHistoryState,
        agent_profile_state: &AgentProfileStatePaneState,
        agent_schedule_tick: &AgentScheduleTickPaneState,
        trajectory_audit: &TrajectoryAuditPaneState,
        skill_registry: &SkillRegistryPaneState,
        skill_trust_revocation: &SkillTrustRevocationPaneState,
        credit_desk: &CreditDeskPaneState,
        credit_settlement_ledger: &CreditSettlementLedgerPaneState,
        agent_network_simulation: &AgentNetworkSimulationPaneState,
        treasury_exchange_simulation: &TreasuryExchangeSimulationPaneState,
        relay_security_simulation: &RelaySecuritySimulationPaneState,
        spark_wallet: &SparkPaneState,
        spark_inputs: &mut SparkPaneInputs,
        pay_invoice_inputs: &mut PayInvoicePaneInputs,
        create_invoice_inputs: &mut CreateInvoicePaneInputs,
        relay_connections_inputs: &mut RelayConnectionsPaneInputs,
        network_requests_inputs: &mut NetworkRequestsPaneInputs,
        settings_inputs: &mut SettingsPaneInputs,
        job_history_inputs: &mut JobHistoryPaneInputs,
        chat_inputs: &mut ChatPaneInputs,
        paint: &mut PaintContext,
    ) -> u32 {
        let mut indices: Vec<usize> = (0..panes.len()).collect();
        indices.sort_by_key(|idx| panes[*idx].z_index);

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
                    .with_corner_radius(0.0),
            );

            match pane.kind {
                PaneKind::Empty => paint_empty_pane(content_bounds, paint),
                PaneKind::AutopilotChat => {
                    paint_autopilot_chat_pane(content_bounds, autopilot_chat, chat_inputs, paint);
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
                PaneKind::CodexRemoteSkills => {
                    codex_pane::paint_remote_skills_pane(
                        content_bounds,
                        codex_remote_skills,
                        paint,
                    );
                }
                PaneKind::GoOnline => {
                    paint_go_online_pane(
                        content_bounds,
                        provider_runtime,
                        sa_lane,
                        skl_lane,
                        ac_lane,
                        provider_blockers,
                        paint,
                    );
                }
                PaneKind::ProviderStatus => {
                    paint_provider_status_pane(
                        content_bounds,
                        provider_runtime,
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
                PaneKind::ActivityFeed => {
                    paint_activity_feed_pane(content_bounds, activity_feed, paint);
                }
                PaneKind::AlertsRecovery => {
                    paint_alerts_recovery_pane(content_bounds, alerts_recovery, paint);
                }
                PaneKind::Settings => {
                    paint_settings_pane(content_bounds, settings, settings_inputs, paint);
                }
                PaneKind::JobInbox => {
                    paint_job_inbox_pane(content_bounds, job_inbox, paint);
                }
                PaneKind::ActiveJob => {
                    paint_active_job_pane(content_bounds, active_job, paint);
                }
                PaneKind::JobHistory => {
                    paint_job_history_pane(content_bounds, job_history, job_history_inputs, paint);
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
                PaneKind::AgentNetworkSimulation => {
                    simulation_pane::paint_agent_network_simulation_pane(
                        content_bounds,
                        agent_network_simulation,
                        paint,
                    );
                }
                PaneKind::TreasuryExchangeSimulation => {
                    simulation_pane::paint_treasury_exchange_simulation_pane(
                        content_bounds,
                        treasury_exchange_simulation,
                        paint,
                    );
                }
                PaneKind::RelaySecuritySimulation => {
                    simulation_pane::paint_relay_security_simulation_pane(
                        content_bounds,
                        relay_security_simulation,
                        paint,
                    );
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
    paint_source_badge(content_bounds, "local", paint);
    chat_pane::paint(content_bounds, autopilot_chat, chat_inputs, paint);
}

fn paint_go_online_pane(
    content_bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
    sa_lane: &crate::runtime_lanes::SaLaneSnapshot,
    skl_lane: &crate::runtime_lanes::SklLaneSnapshot,
    ac_lane: &crate::runtime_lanes::AcLaneSnapshot,
    provider_blockers: &[ProviderBlocker],
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let toggle_bounds = go_online_toggle_button_bounds(content_bounds);
    let toggle_label = if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
        "Go Online"
    } else {
        "Go Offline"
    };
    paint_action_button(toggle_bounds, toggle_label, paint);

    let now = std::time::Instant::now();
    let mut y = toggle_bounds.max_y() + 14.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Provider mode",
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
        "SA runner mode",
        sa_lane.mode.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "SA profile event",
        sa_lane.profile_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "SKL trust tier",
        skl_lane.trust_tier.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "SKL manifest",
        skl_lane.manifest_a.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "AC credit lane",
        if ac_lane.credit_available {
            "available"
        } else {
            "unavailable"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "AC envelope",
        ac_lane.envelope_event_id.as_deref().unwrap_or("n/a"),
    );

    if provider_blockers.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Preflight: clear",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::SUCCESS,
        ));
    } else {
        paint.scene.draw_text(paint.text.layout(
            "Preflight blockers:",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for blocker in provider_blockers {
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("{} - {}", blocker.code(), blocker.detail()),
                Point::new(content_bounds.origin.x + 12.0, y),
                10.0,
                theme::status::ERROR,
            ));
            y += 14.0;
        }
    }

    if let Some(code) = provider_runtime.degraded_reason_code.as_deref() {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("Last reason code: {code}"),
            Point::new(content_bounds.origin.x + 12.0, y + 12.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_provider_status_pane(
    content_bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
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
            .as_deref()
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
        "relay: unknown (lane pending)",
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));

    if let Some(error) = provider_runtime.last_error_detail.as_deref() {
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
        "Sats today",
        &earnings_scoreboard.sats_today.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Lifetime sats",
        &earnings_scoreboard.lifetime_sats.to_string(),
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
    paint_source_badge(content_bounds, "runtime", paint);

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
        "Cursor position",
        &sync_health.cursor_position.to_string(),
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
    paint_action_button(submit_bounds, "Submit request", paint);

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
        "Budget sats",
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
                .with_corner_radius(4.0),
        );
        let status_color = match request.status {
            NetworkRequestStatus::Submitted => theme::accent::PRIMARY,
            NetworkRequestStatus::Streaming => theme::status::SUCCESS,
            NetworkRequestStatus::Completed => theme::status::SUCCESS,
            NetworkRequestStatus::Failed => theme::status::ERROR,
        };
        let summary = format!(
            "{} {} scope:{} env:{} budget:{} timeout:{}s stream:{} [{}|{}|{}|{}]",
            request.request_id,
            request.request_type,
            request.skill_scope_id.as_deref().unwrap_or("none"),
            request.credit_envelope_ref.as_deref().unwrap_or("none"),
            request.budget_sats,
            request.timeout_seconds,
            request.response_stream_id,
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
    paint_action_button(complete_bounds, "Complete selected", paint);

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        complete_bounds.max_y() + 12.0,
        starter_jobs.load_state,
        &format!("State: {}", starter_jobs.load_state.label()),
        starter_jobs.last_action.as_deref(),
        starter_jobs.last_error.as_deref(),
    );

    let visible_rows = starter_jobs_visible_row_count(starter_jobs.jobs.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No starter jobs available.",
            Point::new(content_bounds.origin.x + 12.0, y),
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
            "starter {} {} {} sats {} {}",
            job.job_id,
            job.status.label(),
            job.payout_sats,
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
            "Payout sats",
            &selected.payout_sats.to_string(),
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

fn paint_activity_feed_pane(
    content_bounds: Bounds,
    activity_feed: &ActivityFeedState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime+wallet+local", paint);

    let refresh_bounds = activity_feed_refresh_button_bounds(content_bounds);
    paint_action_button(refresh_bounds, "Refresh feed", paint);

    let filters = ActivityFeedFilter::all();
    for (index, filter) in filters.into_iter().enumerate() {
        paint_filter_button(
            activity_feed_filter_button_bounds(content_bounds, index),
            filter.label(),
            activity_feed.active_filter == filter,
            paint,
        );
    }

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        activity_feed_filter_button_bounds(content_bounds, 0).max_y() + 10.0,
        activity_feed.load_state,
        &format!(
            "State: {} | Filter: {}",
            activity_feed.load_state.label(),
            activity_feed.active_filter.label()
        ),
        activity_feed.last_action.as_deref(),
        activity_feed.last_error.as_deref(),
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
        && activity_feed.active_filter.matches(selected.domain)
    {
        let details_top =
            activity_feed_row_bounds(content_bounds, visible_rows.saturating_sub(1)).max_y() + 10.0;
        let mut details_y = details_top;
        details_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Event ID",
            &selected.event_id,
        );
        details_y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Source",
            &selected.source_tag,
        );
        let _ = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            details_y,
            "Detail",
            &selected.detail,
        );
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
        "Wallet Default Send (sats)",
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

    paint_action_button(save_bounds, "Save settings", paint);
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
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let accept_bounds = job_inbox_accept_button_bounds(content_bounds);
    let reject_bounds = job_inbox_reject_button_bounds(content_bounds);
    paint_action_button(accept_bounds, "Accept selected", paint);
    paint_action_button(reject_bounds, "Reject selected", paint);

    let y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        accept_bounds.max_y() + 12.0,
        job_inbox.load_state,
        &format!("State: {}", job_inbox.load_state.label()),
        job_inbox.last_action.as_deref(),
        job_inbox.last_error.as_deref(),
    );

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
            "#{} {} {} scope:{} env:{} {} sats ttl:{}s {} {}",
            request.arrival_seq,
            request.request_id,
            request.capability,
            request.skill_scope_id.as_deref().unwrap_or("none"),
            request.ac_envelope_event_id.as_deref().unwrap_or("none"),
            request.price_sats,
            request.ttl_seconds,
            request.validation.label(),
            request.decision.label()
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
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let advance_bounds = active_job_advance_button_bounds(content_bounds);
    let abort_bounds = active_job_abort_button_bounds(content_bounds);
    paint_action_button(advance_bounds, "Advance stage", paint);
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
        "Stage",
        job.stage.label(),
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
    job_history_inputs: &mut JobHistoryPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let search_bounds = job_history_search_input_bounds(content_bounds);
    let status_bounds = job_history_status_button_bounds(content_bounds);
    let time_bounds = job_history_time_button_bounds(content_bounds);
    let prev_bounds = job_history_prev_page_button_bounds(content_bounds);
    let next_bounds = job_history_next_page_button_bounds(content_bounds);

    job_history_inputs
        .search_job_id
        .set_max_width(search_bounds.size.width);
    job_history_inputs.search_job_id.paint(search_bounds, paint);
    paint_action_button(
        status_bounds,
        &format!("Status: {}", job_history.status_filter.label()),
        paint,
    );
    paint_action_button(
        time_bounds,
        &format!("Range: {}", job_history.time_range.label()),
        paint,
    );
    paint_action_button(prev_bounds, "Prev", paint);
    paint_action_button(next_bounds, "Next", paint);

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
                .with_corner_radius(4.0),
        );
        let row_line = format!(
            "{} {} ts:{} scope:{} tick:{} set:{} def:{} {} {}",
            row.job_id,
            row.status.label(),
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
            .with_corner_radius(4.0),
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
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::accent::PRIMARY.with_alpha(0.15))
            .with_border(theme::accent::PRIMARY, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        11.0,
        theme::text::PRIMARY,
    ));
}

fn paint_filter_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    let background = if active {
        theme::accent::PRIMARY.with_alpha(0.22)
    } else {
        theme::bg::APP.with_alpha(0.68)
    };
    let border = if active {
        theme::accent::PRIMARY
    } else {
        theme::border::DEFAULT
    };
    let text = if active {
        theme::text::PRIMARY
    } else {
        theme::text::MUTED
    };

    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 9.0),
        10.0,
        text,
    ));
}

fn paint_disabled_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.72))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        11.0,
        theme::text::MUTED,
    ));
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
        11.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(x + 122.0, y),
        11.0,
        theme::text::PRIMARY,
    ));
    y + 16.0
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
        11.0,
        theme::text::MUTED,
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, 72) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(x + 122.0, line_y),
            11.0,
            theme::text::PRIMARY,
        ));
        line_y += 16.0;
    }
    line_y
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

    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(chunk_len.max(1))
        .map(|chunk| chunk.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        create_invoice_view_state, nostr_identity_view_state, pay_invoice_view_state,
        payment_terminal_status, spark_wallet_view_state,
    };
    use crate::app_state::PaneLoadState;
    use crate::spark_wallet::SparkPaneState;

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
}
