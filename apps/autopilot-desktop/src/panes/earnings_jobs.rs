use crate::app_state::{
    ActiveJobState, EarnJobLifecycleProjectionState, EarningsScoreboardState, JobHistoryState,
    JobHistoryStatus, JobInboxRequest, JobInboxState, JobLifecycleStage, PaneLoadState,
    ProviderRuntimeState,
};
use crate::bitcoin_display::{format_mission_control_amount, format_sats_amount};
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::pane_renderer::{
    earnings_scoreboard_amount_display, format_bps_percent, paint_action_button, paint_label_line,
    paint_source_badge, paint_state_summary, paint_wrapped_label_line, split_text_for_display,
};
use crate::pane_system::{
    earnings_scoreboard_active_job_button_bounds, earnings_scoreboard_history_button_bounds,
    earnings_scoreboard_job_inbox_button_bounds, earnings_scoreboard_refresh_button_bounds,
};
use crate::spark_wallet::SparkPaneState;
use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, theme};

const SECTION_GAP: f32 = 12.0;
const SECTION_PADDING: f32 = 12.0;
const SECTION_HEADER_HEIGHT: f32 = 24.0;
const METRIC_CARD_HEIGHT: f32 = 54.0;
const PREVIEW_ROW_HEIGHT: f32 = 32.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissionControlActiveJobsPanelState {
    pub headline: String,
    pub lines: Vec<String>,
}

pub(crate) fn active_jobs_panel_state(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
) -> MissionControlActiveJobsPanelState {
    if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
        return MissionControlActiveJobsPanelState {
            headline: "STANDBY".to_string(),
            lines: vec![
                if crate::app_state::mission_control_sell_compute_supported(
                    desktop_shell_mode,
                    local_inference_runtime,
                ) {
                    "GO ONLINE TO ACCEPT PAID JOBS.".to_string()
                } else {
                    "PLATFORM NOT SUPPORTED FOR SELLING COMPUTE.".to_string()
                },
                "NEXT // GO ONLINE".to_string(),
                format!("OBSERVED REQUESTS // {}", job_inbox.requests.len()),
            ],
        };
    }

    let Some(job) = active_job.job.as_ref() else {
        return MissionControlActiveJobsPanelState {
            headline: "SCANNING".to_string(),
            lines: vec![
                "WATCHING RELAYS FOR MATCHES.".to_string(),
                "NEXT // MATCHING REQUEST".to_string(),
                format!("OBSERVED REQUESTS // {}", job_inbox.requests.len()),
            ],
        };
    };

    let flow_snapshot = crate::nip90_compute_flow::build_active_job_flow_snapshot(
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
    )
    .expect("active job snapshot should exist when job exists");

    let flow_line = match flow_snapshot.phase {
        crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment => {
            "FLOW // RESULT DELIVERED / AWAITING BUYER PAYMENT".to_string()
        }
        crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet => {
            "FLOW // SELLER SETTLED / BUYER LOCAL WALLET PENDING".to_string()
        }
        crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment => {
            "FLOW // RESULT DELIVERED / PREPARING BUYER INVOICE".to_string()
        }
        crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid => {
            "FLOW // RESULT DELIVERED / BUYER NEVER PAID".to_string()
        }
        _ => format!(
            "FLOW // {} / {}",
            flow_snapshot.authority.as_str().to_ascii_uppercase(),
            flow_snapshot.phase.as_str().to_ascii_uppercase()
        ),
    };

    let mut lines = vec![
        format!("CAPABILITY // {}", job.capability.to_ascii_uppercase()),
        flow_line,
        format!(
            "NEXT // {}",
            flow_snapshot.next_expected_event.to_ascii_uppercase()
        ),
    ];
    if let Some(continuity_summary) = flow_snapshot.mission_control_continuity_summary() {
        lines.push(format!(
            "CONT // {}",
            continuity_summary.to_ascii_uppercase()
        ));
    } else {
        lines.push("CONT // NONE".to_string());
    }
    lines.push(format!(
        "PAYOUT // {}",
        format_mission_control_amount(job.quoted_price_sats)
    ));
    if let Some(amount) = flow_snapshot.settlement_amount_sats {
        let mut settlement = format!(
            "SETTLE // RECEIVED {}",
            format_mission_control_amount(amount)
        );
        if let Some(fees) = flow_snapshot.settlement_fees_sats {
            settlement.push_str(" // FEE ");
            settlement.push_str(format_mission_control_amount(fees).as_str());
        }
        if let Some(delta) = flow_snapshot.settlement_net_wallet_delta_sats {
            settlement.push_str(" // DELTA ");
            settlement.push_str(
                crate::spark_wallet::format_wallet_delta_sats(delta)
                    .to_ascii_uppercase()
                    .as_str(),
            );
        }
        lines.push(settlement);
    } else if flow_snapshot.phase
        == crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet
    {
        let mut settlement = "SETTLE // SELLER SETTLED // BUYER LOCAL WALLET PENDING".to_string();
        if flow_snapshot.payment_pointer.is_some() {
            settlement.push_str(" // POINTER READY");
        }
        if let Some(window) = flow_snapshot.continuity_window_seconds {
            settlement.push_str(" // WINDOW ");
            settlement.push_str(format!("{window}S").as_str());
        }
        lines.push(settlement);
    } else if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment {
        let mut settlement = "SETTLE // AWAITING BUYER PAYMENT".to_string();
        if flow_snapshot.payment_pointer.is_some() {
            settlement.push_str(" // POINTER READY");
        }
        if let Some(window) = flow_snapshot.continuity_window_seconds {
            settlement.push_str(" // WINDOW ");
            settlement.push_str(format!("{window}S").as_str());
        }
        lines.push(settlement);
    } else if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment {
        let mut settlement = "SETTLE // PREPARING BUYER INVOICE".to_string();
        if let Some(window) = flow_snapshot.continuity_window_seconds {
            settlement.push_str(" // WINDOW ");
            settlement.push_str(format!("{window}S").as_str());
        }
        lines.push(settlement);
    } else if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid {
        let mut settlement = "SETTLE // BUYER NEVER PAID".to_string();
        if flow_snapshot.pending_bolt11.is_some() {
            settlement.push_str(" // INVOICE READY");
        } else {
            settlement.push_str(" // INVOICE MISSING");
        }
        if let Some(window) = flow_snapshot.continuity_window_seconds {
            settlement.push_str(" // WINDOW ");
            settlement.push_str(format!("{window}S").as_str());
        }
        lines.push(settlement);
    }

    MissionControlActiveJobsPanelState {
        headline: if flow_snapshot.phase
            == crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment
        {
            "AWAITING PAYMENT".to_string()
        } else if flow_snapshot.phase
            == crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet
        {
            "LOCAL CONFIRM".to_string()
        } else if flow_snapshot.phase
            == crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment
        {
            "INVOICING".to_string()
        } else if flow_snapshot.phase == crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid
        {
            "UNPAID".to_string()
        } else if job.stage == JobLifecycleStage::Failed {
            "FAULT".to_string()
        } else {
            "ACTIVE".to_string()
        },
        lines,
    }
}

pub fn paint_earnings_jobs_pane(
    content_bounds: Bounds,
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
    paint_source_badge(content_bounds, "runtime+wallet+receipts", paint);

    paint_action_button(
        earnings_scoreboard_refresh_button_bounds(content_bounds),
        "Refresh metrics",
        paint,
    );
    paint_action_button(
        earnings_scoreboard_job_inbox_button_bounds(content_bounds),
        "Open Job Inbox",
        paint,
    );
    paint_action_button(
        earnings_scoreboard_active_job_button_bounds(content_bounds),
        "Open Active Job",
        paint,
    );
    paint_action_button(
        earnings_scoreboard_history_button_bounds(content_bounds),
        "Open Job History",
        paint,
    );

    let stale = earnings_scoreboard.is_stale(std::time::Instant::now());
    let stale_suffix = if stale { " (stale)" } else { "" };
    let button_bottom = earnings_scoreboard_history_button_bounds(content_bounds).max_y();
    let summary_bottom = paint_state_summary(
        paint,
        content_bounds.origin.x + SECTION_PADDING,
        button_bottom + 12.0,
        earnings_scoreboard.load_state,
        &format!(
            "State: {}{stale_suffix}",
            earnings_scoreboard.load_state.label()
        ),
        earnings_scoreboard.last_action.as_deref(),
        earnings_scoreboard.last_error.as_deref(),
    );

    let grid_top = summary_bottom + 8.0;
    let inner_width = (content_bounds.size.width - SECTION_PADDING * 2.0).max(0.0);
    let column_width = ((inner_width - SECTION_GAP) / 2.0).max(0.0);
    let remaining_height = (content_bounds.max_y() - grid_top - SECTION_PADDING).max(0.0);
    let top_row_height = (remaining_height * 0.52).clamp(188.0, 248.0);
    let bottom_row_height = (remaining_height - top_row_height - SECTION_GAP).max(120.0);
    let left_x = content_bounds.origin.x + SECTION_PADDING;
    let right_x = left_x + column_width + SECTION_GAP;

    let earnings_bounds = Bounds::new(left_x, grid_top, column_width, top_row_height);
    let active_bounds = Bounds::new(right_x, grid_top, column_width, top_row_height);
    let inbox_bounds = Bounds::new(
        left_x,
        earnings_bounds.max_y() + SECTION_GAP,
        column_width,
        bottom_row_height,
    );
    let history_bounds = Bounds::new(
        right_x,
        active_bounds.max_y() + SECTION_GAP,
        column_width,
        bottom_row_height,
    );

    paint_section_panel(earnings_bounds, "Earnings", theme::status::SUCCESS, paint);
    paint_earnings_section(earnings_bounds, earnings_scoreboard, paint);

    paint_section_panel(active_bounds, "Active Job", theme::accent::PRIMARY, paint);
    paint_active_section(
        active_bounds,
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
        job_inbox,
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
        paint,
    );

    paint_section_panel(inbox_bounds, "Inbox Preview", theme::accent::PRIMARY, paint);
    paint_inbox_section(inbox_bounds, job_inbox, provider_runtime, paint);

    paint_section_panel(
        history_bounds,
        "Recent History",
        theme::status::SUCCESS,
        paint,
    );
    paint_history_section(history_bounds, job_history, paint);
}

fn paint_section_panel(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(accent.with_alpha(0.45), 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            SECTION_HEADER_HEIGHT,
        ))
        .with_background(theme::bg::APP.with_alpha(0.72))
        .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            4.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.9))
        .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 6.0),
        10.0,
        theme::text::PRIMARY,
    ));
}

fn paint_earnings_section(
    bounds: Bounds,
    earnings_scoreboard: &EarningsScoreboardState,
    paint: &mut PaintContext,
) {
    let inner_x = bounds.origin.x + SECTION_PADDING;
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 10.0;
    let card_gap = 8.0;
    let card_width = ((bounds.size.width - SECTION_PADDING * 2.0 - card_gap * 2.0) / 3.0).max(0.0);
    let today_display = earnings_scoreboard_amount_display(
        earnings_scoreboard.load_state,
        format_sats_amount(earnings_scoreboard.sats_today),
    );
    let month_display = earnings_scoreboard_amount_display(
        earnings_scoreboard.load_state,
        format_sats_amount(earnings_scoreboard.sats_this_month),
    );
    let lifetime_display = earnings_scoreboard_amount_display(
        earnings_scoreboard.load_state,
        format_sats_amount(earnings_scoreboard.lifetime_sats),
    );
    for (index, (label, value, color)) in [
        ("Today", today_display.as_str(), theme::status::SUCCESS),
        ("This Month", month_display.as_str(), theme::accent::PRIMARY),
        ("All Time", lifetime_display.as_str(), theme::text::PRIMARY),
    ]
    .into_iter()
    .enumerate()
    {
        let x = inner_x + index as f32 * (card_width + card_gap);
        paint_metric_card(
            Bounds::new(x, y, card_width, METRIC_CARD_HEIGHT),
            label,
            value,
            color,
            paint,
        );
    }
    y += METRIC_CARD_HEIGHT + 12.0;
    y = paint_label_line(
        paint,
        inner_x,
        y,
        "Jobs today",
        &earnings_value_or_loading(
            earnings_scoreboard.load_state,
            earnings_scoreboard.jobs_today.to_string(),
        ),
    );
    y = paint_wrapped_label_line(
        paint,
        inner_x,
        y,
        "Last job result",
        &earnings_value_or_loading(
            earnings_scoreboard.load_state,
            earnings_scoreboard.last_job_result.clone(),
        ),
        44,
    );
    y = paint_label_line(
        paint,
        inner_x,
        y,
        "Completion ratio",
        &format_bps_percent(earnings_scoreboard.completion_ratio_bps),
    );
    y = paint_label_line(
        paint,
        inner_x,
        y,
        "Payout success",
        &format_bps_percent(earnings_scoreboard.payout_success_ratio_bps),
    );
    y = paint_label_line(
        paint,
        inner_x,
        y,
        "First job latency (s)",
        &earnings_scoreboard
            .first_job_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    y = paint_label_line(
        paint,
        inner_x,
        y,
        "Wallet confirm avg (s)",
        &earnings_scoreboard
            .avg_wallet_confirmation_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    let _ = paint_label_line(
        paint,
        inner_x,
        y,
        "Online uptime (s)",
        &earnings_scoreboard.online_uptime_seconds.to_string(),
    );
}

fn paint_active_section(
    bounds: Bounds,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    let panel = active_jobs_panel_state(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
        job_inbox,
        active_job,
        earn_job_lifecycle_projection,
        spark_wallet,
    );
    let headline_color = match panel.headline.as_str() {
        "STANDBY" => theme::accent::PRIMARY,
        "FAULT" | "UNPAID" => theme::status::ERROR,
        "AWAITING PAYMENT" | "LOCAL CONFIRM" | "INVOICING" => theme::accent::PRIMARY,
        "ACTIVE" => theme::status::SUCCESS,
        _ => theme::text::PRIMARY,
    };
    let x = bounds.origin.x + SECTION_PADDING;
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 8.0;
    paint.scene.draw_text(paint.text.layout_mono(
        panel.headline.as_str(),
        Point::new(x, y),
        18.0,
        headline_color,
    ));
    y += 28.0;
    let chunk_len =
        (((bounds.size.width - SECTION_PADDING * 2.0).max(96.0) / 6.4).floor() as usize).max(18);
    let max_lines = if active_job.job.is_some() { 9 } else { 6 };
    for line in panel.lines.iter().take(max_lines) {
        for chunk in split_text_for_display(line, chunk_len) {
            paint.scene.draw_text(paint.text.layout_mono(
                &chunk,
                Point::new(x, y),
                9.5,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
    }
    if panel.lines.len() > max_lines {
        paint.scene.draw_text(paint.text.layout(
            "Open Active Job for the full execution log.",
            Point::new(x, bounds.max_y() - 22.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_inbox_section(
    bounds: Bounds,
    job_inbox: &JobInboxState,
    provider_runtime: &ProviderRuntimeState,
    paint: &mut PaintContext,
) {
    let x = bounds.origin.x + SECTION_PADDING;
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 8.0;
    let summary = format!(
        "Observed {} request(s){}",
        job_inbox.requests.len(),
        if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
            " while offline"
        } else {
            ""
        }
    );
    paint.scene.draw_text(
        paint
            .text
            .layout(&summary, Point::new(x, y), 10.0, theme::text::MUTED),
    );
    y += 18.0;

    if job_inbox.load_state == PaneLoadState::Loading && job_inbox.requests.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for inbox replay frame...",
            Point::new(x, y),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    if job_inbox.requests.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No requests visible yet.",
            Point::new(x, y),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    let now_epoch_seconds = current_epoch_seconds();
    for (index, request) in job_inbox.requests.iter().take(3).enumerate() {
        let row_bounds = Bounds::new(
            x,
            y + index as f32 * (PREVIEW_ROW_HEIGHT + 6.0),
            bounds.size.width - SECTION_PADDING * 2.0,
            PREVIEW_ROW_HEIGHT,
        );
        let selected =
            job_inbox.selected_request_id.as_deref() == Some(request.request_id.as_str());
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if selected {
                    theme::bg::APP.with_alpha(0.84)
                } else {
                    theme::bg::APP.with_alpha(0.65)
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
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "{} {} {} {}",
                request.request_id,
                request.capability,
                format_sats_amount(request.price_sats),
                request.validation.label()
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 7.0),
            9.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "{} | {} | {}",
                request_freshness_summary(request, now_epoch_seconds),
                request.demand_source.label(),
                request.decision.label()
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 18.0),
            8.5,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout(
        "Open Job Inbox to accept, reject, or inspect full request metadata.",
        Point::new(x, bounds.max_y() - 22.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_history_section(bounds: Bounds, job_history: &JobHistoryState, paint: &mut PaintContext) {
    let x = bounds.origin.x + SECTION_PADDING;
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 8.0;
    paint.scene.draw_text(paint.text.layout(
        &format!(
            "Rows: {} | filter={} | range={}",
            job_history.rows.len(),
            job_history.status_filter.label(),
            job_history.time_range.label()
        ),
        Point::new(x, y),
        10.0,
        theme::text::MUTED,
    ));
    y += 18.0;

    if job_history.load_state == PaneLoadState::Loading && job_history.rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for persisted receipts...",
            Point::new(x, y),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    if job_history.rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No settled or failed jobs recorded yet.",
            Point::new(x, y),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (index, row) in job_history.rows.iter().take(3).enumerate() {
        let row_bounds = Bounds::new(
            x,
            y + index as f32 * (PREVIEW_ROW_HEIGHT + 6.0),
            bounds.size.width - SECTION_PADDING * 2.0,
            PREVIEW_ROW_HEIGHT,
        );
        let accent = if row.status == JobHistoryStatus::Succeeded {
            theme::status::SUCCESS
        } else {
            theme::status::ERROR
        };
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(theme::bg::APP.with_alpha(0.7))
                .with_border(accent.with_alpha(0.6), 1.0)
                .with_corner_radius(6.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "{} {} {}",
                row.job_id,
                row.status.label(),
                format_sats_amount(row.payout_sats)
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 7.0),
            9.0,
            accent,
        ));
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "completed={} payer={} payee={}",
                row.completed_at_epoch_seconds,
                compact_identity(row.requester_nostr_pubkey.as_deref()),
                compact_identity(row.provider_nostr_pubkey.as_deref())
            ),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 18.0),
            8.5,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout(
        "Open Job History for search, paging, and receipt details.",
        Point::new(x, bounds.max_y() - 22.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_metric_card(
    bounds: Bounds,
    label: &str,
    value: &str,
    value_color: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.72))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(6.0),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 8.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 26.0),
        13.0,
        value_color,
    ));
}

fn earnings_value_or_loading(load_state: PaneLoadState, value: String) -> String {
    if load_state == PaneLoadState::Loading {
        "LOADING".to_string()
    } else {
        value
    }
}

fn request_freshness_summary(request: &JobInboxRequest, now_epoch_seconds: u64) -> String {
    match request.expires_in_seconds(now_epoch_seconds) {
        Some(0) => "expired".to_string(),
        Some(seconds) => format!("{seconds}s left"),
        None => "freshness n/a".to_string(),
    }
}

fn compact_pointer(pointer: &str) -> String {
    let pointer = pointer.trim();
    if pointer.len() <= 18 {
        return pointer.to_string();
    }
    format!("{}..{}", &pointer[..8], &pointer[pointer.len() - 6..])
}

fn compact_identity(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "unknown".to_string();
    };
    if value.len() <= 18 {
        value.to_string()
    } else {
        format!("{}..{}", &value[..10], &value[value.len() - 6..])
    }
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
