use crate::app_state::{
    ActiveJobState, EarnJobLifecycleProjectionState, EarningsScoreboardState, JobHistoryState,
    JobHistoryStatus, JobInboxRequest, JobInboxState, JobLifecycleStage, PaneLoadState,
    ProviderRuntimeState,
};
use crate::bitcoin_display::{format_mission_control_amount, format_sats_amount};
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::pane_renderer::{
    app_text_style, earnings_scoreboard_amount_display, format_bps_percent,
    mission_control_panel_border_color, mission_control_panel_color,
    mission_control_panel_header_color, paint_secondary_button, paint_source_badge,
    paint_state_summary, paint_tertiary_button, split_text_for_display,
};
use crate::pane_system::{
    earnings_scoreboard_active_job_button_bounds, earnings_scoreboard_history_button_bounds,
    earnings_scoreboard_job_inbox_button_bounds, earnings_scoreboard_refresh_button_bounds,
};
use crate::spark_wallet::SparkPaneState;
use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, theme};

const SECTION_GAP: f32 = 16.0;
const SECTION_PADDING: f32 = 16.0;
const SECTION_HEADER_HEIGHT: f32 = 26.0;
const PANEL_RADIUS: f32 = 3.0;
const METRIC_CARD_HEIGHT: f32 = 68.0;
const PREVIEW_ROW_HEIGHT: f32 = 40.0;
const DETAIL_ROW_LABEL_WIDTH: f32 = 156.0;
const SECTION_SCROLLBAR_GUTTER: f32 = 10.0;
const SECTION_SCROLLBAR_WIDTH: f32 = 3.0;

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
    pane_is_active: bool,
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    earnings_scoreboard: &mut EarningsScoreboardState,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    job_history: &JobHistoryState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    if !pane_is_active {
        paint_inactive_preview(
            content_bounds,
            earnings_scoreboard,
            provider_runtime,
            job_inbox,
            active_job,
            job_history,
            spark_wallet,
            paint,
        );
        return;
    }

    paint_source_badge(content_bounds, "runtime+wallet+receipts", paint);

    paint_tertiary_button(
        earnings_scoreboard_refresh_button_bounds(content_bounds),
        "Refresh metrics",
        paint,
    );
    paint_secondary_button(
        earnings_scoreboard_job_inbox_button_bounds(content_bounds),
        "Open Job Inbox",
        paint,
    );
    paint_secondary_button(
        earnings_scoreboard_active_job_button_bounds(content_bounds),
        "Open Active Job",
        paint,
    );
    paint_secondary_button(
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

    let grid_top = summary_bottom + 18.0;
    let inner_width = (content_bounds.size.width - SECTION_PADDING * 2.0).max(0.0);
    let column_width = ((inner_width - SECTION_GAP) / 2.0).max(0.0);
    let remaining_height = (content_bounds.max_y() - grid_top - SECTION_PADDING).max(0.0);
    let top_row_height = (remaining_height * 0.62).clamp(236.0, 340.0);
    let bottom_row_height = (remaining_height - top_row_height - SECTION_GAP).max(104.0);
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

fn scoreboard_summary_bottom(
    content_bounds: Bounds,
    earnings_scoreboard: &EarningsScoreboardState,
) -> f32 {
    let stale = earnings_scoreboard.is_stale(std::time::Instant::now());
    let stale_suffix = if stale { " (stale)" } else { "" };
    let button_bottom = earnings_scoreboard_history_button_bounds(content_bounds).max_y();
    button_bottom
        + 12.0
        + 16.0
        + if earnings_scoreboard.last_action.is_some() {
            16.0
        } else {
            0.0
        }
        + if earnings_scoreboard.last_error.is_some() {
            16.0
        } else {
            0.0
        }
        + if stale_suffix.is_empty() { 0.0 } else { 0.0 }
}

pub fn earnings_section_panel_bounds(
    content_bounds: Bounds,
    earnings_scoreboard: &EarningsScoreboardState,
) -> Bounds {
    let summary_bottom = scoreboard_summary_bottom(content_bounds, earnings_scoreboard);
    let grid_top = summary_bottom + 18.0;
    let inner_width = (content_bounds.size.width - SECTION_PADDING * 2.0).max(0.0);
    let column_width = ((inner_width - SECTION_GAP) / 2.0).max(0.0);
    let remaining_height = (content_bounds.max_y() - grid_top - SECTION_PADDING).max(0.0);
    let top_row_height = (remaining_height * 0.62).clamp(236.0, 340.0);
    let left_x = content_bounds.origin.x + SECTION_PADDING;
    Bounds::new(left_x, grid_top, column_width, top_row_height)
}

fn paint_inactive_preview(
    bounds: Bounds,
    earnings_scoreboard: &EarningsScoreboardState,
    provider_runtime: &ProviderRuntimeState,
    job_inbox: &JobInboxState,
    active_job: &ActiveJobState,
    job_history: &JobHistoryState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(bounds, "runtime+wallet+receipts", paint);

    let summary_bottom = paint_state_summary(
        paint,
        bounds.origin.x + SECTION_PADDING,
        bounds.origin.y + 12.0,
        earnings_scoreboard.load_state,
        &format!("State: {}", earnings_scoreboard.load_state.label()),
        earnings_scoreboard.last_action.as_deref(),
        earnings_scoreboard.last_error.as_deref(),
    );
    let x = bounds.origin.x + SECTION_PADDING;
    let mut y = summary_bottom + 12.0;

    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "TODAY {}   MONTH {}   ALL {}",
            format_sats_amount(earnings_scoreboard.sats_today),
            format_sats_amount(earnings_scoreboard.sats_this_month),
            format_sats_amount(earnings_scoreboard.lifetime_sats),
        ),
        Point::new(x, y),
        12.0,
        theme::status::SUCCESS,
    ));
    y += 22.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "PROVIDER {}   INBOX {}   HISTORY {}",
            provider_runtime.mode.label().to_ascii_uppercase(),
            job_inbox.requests.len(),
            job_history.rows.len(),
        ),
        Point::new(x, y),
        10.0,
        theme::text::PRIMARY,
    ));
    y += 18.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "ACTIVE {}",
            active_job
                .job
                .as_ref()
                .map(|job| format!("{} {}", job.job_id, job.stage.label()))
                .unwrap_or_else(|| "none".to_string())
                .to_ascii_uppercase()
        ),
        Point::new(x, y),
        10.0,
        theme::accent::PRIMARY,
    ));
    y += 18.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!(
            "WALLET {}",
            spark_wallet
                .total_balance_sats()
                .map(format_sats_amount)
                .unwrap_or_else(|| "loading".to_string())
                .to_ascii_uppercase()
        ),
        Point::new(x, y),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Activate pane for full metrics, inbox preview, and receipt history.",
        Point::new(x, bounds.max_y() - 22.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_section_panel(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(mission_control_panel_color())
            .with_border(mission_control_panel_border_color().with_alpha(0.72), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_border(accent.with_alpha(0.06), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            4.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.9))
        .with_corner_radius(PANEL_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 4.0,
            bounds.origin.y,
            (bounds.size.width - 4.0).max(0.0),
            SECTION_HEADER_HEIGHT,
        ))
        .with_background(mission_control_panel_header_color().with_alpha(0.88)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 4.0,
            bounds.origin.y,
            (bounds.size.width - 4.0).max(0.0),
            1.0,
        ))
        .with_background(accent.with_alpha(0.22)),
    );
    let heading_style = app_text_style(crate::ui_style::AppTextRole::SectionHeading);
    let marker_origin = Point::new(bounds.origin.x + 14.0, bounds.origin.y + 8.0);
    let marker = paint
        .text
        .layout_mono("\\\\", marker_origin, heading_style.font_size, accent);
    let marker_width = marker.bounds().size.width;
    paint.scene.draw_text(marker);
    paint.scene.draw_text(paint.text.layout_mono(
        &title.to_ascii_uppercase(),
        Point::new(marker_origin.x + marker_width + 6.0, marker_origin.y),
        heading_style.font_size,
        heading_style.color,
    ));
}

fn section_content_clip_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + SECTION_HEADER_HEIGHT + 4.0,
        (bounds.size.width - 16.0).max(0.0),
        (bounds.size.height - SECTION_HEADER_HEIGHT - 12.0).max(0.0),
    )
}

pub fn earnings_scroll_viewport_bounds(
    content_bounds: Bounds,
    earnings_scoreboard: &EarningsScoreboardState,
) -> Bounds {
    let bounds = earnings_section_panel_bounds(content_bounds, earnings_scoreboard);
    let clip = section_content_clip_bounds(bounds);
    Bounds::new(
        clip.origin.x,
        clip.origin.y,
        (clip.size.width - SECTION_SCROLLBAR_GUTTER).max(0.0),
        clip.size.height,
    )
}

fn paint_earnings_section(
    bounds: Bounds,
    earnings_scoreboard: &mut EarningsScoreboardState,
    paint: &mut PaintContext,
) {
    let viewport = Bounds::new(
        section_content_clip_bounds(bounds).origin.x,
        section_content_clip_bounds(bounds).origin.y,
        (section_content_clip_bounds(bounds).size.width - SECTION_SCROLLBAR_GUTTER).max(0.0),
        section_content_clip_bounds(bounds).size.height,
    );
    paint.scene.push_clip(viewport);
    let inner_x = bounds.origin.x + SECTION_PADDING;
    let row_width = viewport.size.width - SECTION_PADDING * 2.0;
    let content_height = earnings_section_content_height(earnings_scoreboard, row_width);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = earnings_scoreboard.clamp_scroll_offset(max_scroll);
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 16.0 - scroll_offset;
    let card_gap = 10.0;
    let card_width = ((viewport.size.width - SECTION_PADDING * 2.0 - card_gap * 2.0) / 3.0).max(0.0);
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
    y += METRIC_CARD_HEIGHT + 16.0;
    y = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "Jobs today",
        &earnings_value_or_loading(
            earnings_scoreboard.load_state,
            earnings_scoreboard.jobs_today.to_string(),
        ),
    );
    y = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "Last job result",
        &earnings_value_or_loading(
            earnings_scoreboard.load_state,
            earnings_scoreboard.last_job_result.clone(),
        ),
    );
    y = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "Completion ratio",
        &format_bps_percent(earnings_scoreboard.completion_ratio_bps),
    );
    y = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "Payout success",
        &format_bps_percent(earnings_scoreboard.payout_success_ratio_bps),
    );
    y = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "First job latency (s)",
        &earnings_scoreboard
            .first_job_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    y = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "Wallet confirm avg (s)",
        &earnings_scoreboard
            .avg_wallet_confirmation_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
    );
    let _ = paint_earnings_detail_row(
        paint,
        inner_x,
        y,
        row_width,
        "Online uptime (s)",
        &earnings_scoreboard.online_uptime_seconds.to_string(),
    );
    paint.scene.pop_clip();
    paint_earnings_scrollbar(bounds, viewport, content_height, scroll_offset, paint);
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
    paint.scene.push_clip(section_content_clip_bounds(bounds));
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
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 20.0;
    paint.scene.draw_text(paint.text.layout_mono(
        panel.headline.as_str(),
        Point::new(x, y),
        20.0,
        headline_color,
    ));
    y += 34.0;
    let chunk_len =
        (((bounds.size.width - SECTION_PADDING * 2.0).max(96.0) / 6.4).floor() as usize).max(18);
    let max_lines = if active_job.job.is_some() { 9 } else { 6 };
    for line in panel.lines.iter().take(max_lines) {
        for chunk in split_text_for_display(line, chunk_len) {
            paint.scene.draw_text(paint.text.layout_mono(
                &chunk,
                Point::new(x, y),
                10.5,
                theme::text::PRIMARY,
            ));
            y += 18.0;
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
    paint.scene.pop_clip();
}

fn paint_inbox_section(
    bounds: Bounds,
    job_inbox: &JobInboxState,
    provider_runtime: &ProviderRuntimeState,
    paint: &mut PaintContext,
) {
    paint.scene.push_clip(section_content_clip_bounds(bounds));
    let x = bounds.origin.x + SECTION_PADDING;
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 20.0;
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
    y += 22.0;

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
            y + index as f32 * (PREVIEW_ROW_HEIGHT + 10.0),
            bounds.size.width - SECTION_PADDING * 2.0,
            PREVIEW_ROW_HEIGHT,
        );
        let selected =
            job_inbox.selected_request_id.as_deref() == Some(request.request_id.as_str());
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if selected {
                    theme::accent::PRIMARY.with_alpha(0.14)
                } else {
                    theme::bg::APP.with_alpha(0.74)
                })
                .with_border(
                    if selected {
                        theme::accent::PRIMARY.with_alpha(0.78)
                    } else {
                        mission_control_panel_border_color().with_alpha(0.46)
                    },
                    1.0,
                )
                .with_corner_radius(PANEL_RADIUS),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "{} {} {} {}",
                request.request_id,
                request.capability,
                format_sats_amount(request.price_sats),
                request.validation.label()
            ),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 10.0),
            10.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "{} | {} | {}",
                request_freshness_summary(request, now_epoch_seconds),
                request.demand_source.label(),
                request.decision.label()
            ),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 25.0),
            8.8,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout(
        "Open Job Inbox to accept, reject, or inspect full request metadata.",
        Point::new(x, bounds.max_y() - 22.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.pop_clip();
}

fn paint_history_section(bounds: Bounds, job_history: &JobHistoryState, paint: &mut PaintContext) {
    paint.scene.push_clip(section_content_clip_bounds(bounds));
    let x = bounds.origin.x + SECTION_PADDING;
    let mut y = bounds.origin.y + SECTION_HEADER_HEIGHT + 20.0;
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
    y += 22.0;

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
            y + index as f32 * (PREVIEW_ROW_HEIGHT + 10.0),
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
                .with_background(theme::bg::APP.with_alpha(0.72))
                .with_border(accent.with_alpha(0.54), 1.0)
                .with_corner_radius(PANEL_RADIUS),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &format!(
                "{} {} {}",
                row.job_id,
                row.status.label(),
                format_sats_amount(row.payout_sats)
            ),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 10.0),
            10.0,
            accent,
        ));
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "completed={} payer_nostr={} payee_nostr={}",
                row.completed_at_epoch_seconds,
                compact_identity(row.requester_nostr_pubkey.as_deref()),
                compact_identity(row.provider_nostr_pubkey.as_deref())
            ),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 25.0),
            8.8,
            theme::text::MUTED,
        ));
    }

    paint.scene.draw_text(paint.text.layout(
        "Open Job History for search, paging, and receipt details.",
        Point::new(x, bounds.max_y() - 22.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.pop_clip();
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
            .with_background(theme::bg::APP.with_alpha(0.74))
            .with_border(mission_control_panel_border_color().with_alpha(0.52), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 36.0),
        15.0,
        value_color,
    ));
}

fn earnings_detail_row_height(value: &str, row_width: f32) -> f32 {
    let value_width = (row_width - DETAIL_ROW_LABEL_WIDTH).max(40.0);
    let wrap_chars = ((value_width / 8.0).floor() as usize).max(10);
    let lines = split_text_for_display(value, wrap_chars).len().clamp(1, 2) as f32;
    let line_y = 16.0 * lines;
    let divider_y = line_y.max(16.0) + 8.0;
    divider_y + 10.0
}

fn earnings_section_content_height(
    earnings_scoreboard: &EarningsScoreboardState,
    row_width: f32,
) -> f32 {
    let mut height = 16.0 + METRIC_CARD_HEIGHT + 16.0;
    height += earnings_detail_row_height(
        &earnings_value_or_loading(
            earnings_scoreboard.load_state,
            earnings_scoreboard.jobs_today.to_string(),
        ),
        row_width,
    );
    height += earnings_detail_row_height(
        &earnings_value_or_loading(
            earnings_scoreboard.load_state,
            earnings_scoreboard.last_job_result.clone(),
        ),
        row_width,
    );
    height += earnings_detail_row_height(
        &format_bps_percent(earnings_scoreboard.completion_ratio_bps),
        row_width,
    );
    height += earnings_detail_row_height(
        &format_bps_percent(earnings_scoreboard.payout_success_ratio_bps),
        row_width,
    );
    height += earnings_detail_row_height(
        &earnings_scoreboard
            .first_job_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
        row_width,
    );
    height += earnings_detail_row_height(
        &earnings_scoreboard
            .avg_wallet_confirmation_latency_seconds
            .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
        row_width,
    );
    height += earnings_detail_row_height(
        &earnings_scoreboard.online_uptime_seconds.to_string(),
        row_width,
    );
    height + 12.0
}

fn paint_earnings_scrollbar(
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
    let track_bounds = Bounds::new(
        bounds.max_x() - 8.0,
        viewport.origin.y,
        SECTION_SCROLLBAR_WIDTH,
        viewport.size.height,
    );
    let thumb_height = ((viewport.size.height / content_height) * viewport.size.height)
        .clamp(18.0, viewport.size.height.max(0.0));
    let thumb_y = viewport.origin.y
        + ((scroll_offset / max_offset.max(1.0)) * (viewport.size.height - thumb_height));
    paint.scene.draw_quad(
        Quad::new(track_bounds)
            .with_background(theme::bg::APP.with_alpha(0.42))
            .with_corner_radius(1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            track_bounds.origin.x,
            thumb_y,
            track_bounds.size.width,
            thumb_height,
        ))
        .with_background(theme::text::MUTED.with_alpha(0.82))
        .with_corner_radius(1.0),
    );
}

fn paint_earnings_detail_row(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    row_width: f32,
    label: &str,
    value: &str,
) -> f32 {
    let label_style = app_text_style(crate::ui_style::AppTextRole::FormLabel);
    let value_style = app_text_style(crate::ui_style::AppTextRole::FormValue);
    let value_x = x + DETAIL_ROW_LABEL_WIDTH;
    let value_width = (row_width - DETAIL_ROW_LABEL_WIDTH).max(40.0);
    let wrap_chars = ((value_width / 8.0).floor() as usize).max(10);

    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(x, y),
        label_style.font_size,
        label_style.color,
    ));

    let mut line_y = y;
    for chunk in split_text_for_display(value, wrap_chars).into_iter().take(2) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(value_x, line_y),
            value_style.font_size,
            value_style.color,
        ));
        line_y += 16.0;
    }

    let divider_y = line_y.max(y + 16.0) + 8.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(x, divider_y, row_width.max(0.0), 1.0))
            .with_background(mission_control_panel_border_color().with_alpha(0.32)),
    );

    divider_y + 10.0
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
