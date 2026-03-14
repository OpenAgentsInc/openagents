use chrono::{Local, TimeZone, Utc};
use serde_json::{Value, json};
use wgpui::{Bounds, PaintContext, Point, Quad, theme};

use crate::app_state::{Nip90SentPaymentsPaneState, Nip90SentPaymentsWindowPreset};
use crate::bitcoin_display::format_sats_amount;
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_secondary_button, paint_source_badge,
    paint_tertiary_button,
};
use crate::pane_system::{
    nip90_sent_payments_copy_button_bounds, nip90_sent_payments_window_button_bounds,
};
use crate::state::nip90_buyer_payment_attempts::{
    Nip90BuyerPaymentAttempt, Nip90BuyerPaymentAttemptLedgerState,
};
use crate::state::operations::{RelayConnectionStatus, RelayConnectionsState};

const PADDING: f32 = 12.0;
const BUTTON_HEIGHT: f32 = 22.0;
const HEADER_TOP: f32 = 42.0;
const METRIC_TOP: f32 = 92.0;
const METRIC_HEIGHT: f32 = 78.0;
const PANEL_TOP: f32 = 186.0;
const PANEL_GAP: f32 = 12.0;
const PANEL_HEIGHT: f32 = 118.0;
const RECENT_TOP: f32 = PANEL_TOP + PANEL_HEIGHT + PANEL_GAP;
const RECENT_ROW_HEIGHT: f32 = 26.0;
const MAX_RECENT_ROWS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip90SentPaymentsPaneView {
    pub window_preset: Nip90SentPaymentsWindowPreset,
    pub window_label: String,
    pub window_range_label: String,
    pub report: crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentWindowReport,
    pub connected_relay_count: usize,
    pub relay_urls_considered: Vec<String>,
    pub latest_counted_payment_epoch_seconds: Option<u64>,
    pub latest_counted_payment_label: String,
    pub custom_window_label: String,
    pub authoritative_note: String,
    pub degraded_note: String,
    pub recent_rows: Vec<Nip90SentPaymentsRecentRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip90SentPaymentsRecentRow {
    pub request_id: String,
    pub provider_label: String,
    pub amount_sats: u64,
    pub timestamp_epoch_seconds: u64,
    pub timestamp_label: String,
    pub relay_count: usize,
}

pub fn paint(
    content_bounds: Bounds,
    pane_state: &Nip90SentPaymentsPaneState,
    buyer_payment_attempts: &Nip90BuyerPaymentAttemptLedgerState,
    relay_connections: &RelayConnectionsState,
    paint: &mut PaintContext,
) {
    paint_source_badge(
        content_bounds,
        "stream.nip90_buyer_payment_attempts.v1",
        paint,
    );
    paint_window_buttons(content_bounds, pane_state, paint);

    let view = match build_view(
        pane_state,
        buyer_payment_attempts,
        relay_connections,
        current_epoch_seconds(),
    ) {
        Ok(view) => view,
        Err(error) => {
            paint_error_state(content_bounds, pane_state, error.as_str(), paint);
            return;
        }
    };

    paint_header(content_bounds, &view, paint);
    paint_metric_row(content_bounds, &view, paint);
    paint_summary_panels(content_bounds, &view, paint);
    paint_recent_rows(content_bounds, &view, paint);
}

pub fn build_view(
    pane_state: &Nip90SentPaymentsPaneState,
    buyer_payment_attempts: &Nip90BuyerPaymentAttemptLedgerState,
    relay_connections: &RelayConnectionsState,
    now_epoch_seconds: u64,
) -> Result<Nip90SentPaymentsPaneView, String> {
    let resolved_window = pane_state.resolve_window(now_epoch_seconds)?;
    let report = buyer_payment_attempts.window_report(
        resolved_window.start_epoch_seconds,
        resolved_window.end_epoch_seconds,
    );
    let relay_urls_considered = connected_relay_urls(relay_connections);
    let mut counted_attempts = buyer_payment_attempts
        .attempts
        .iter()
        .filter(|attempt| {
            attempt.counts_in_definitive_totals()
                && attempt
                    .effective_timestamp_epoch_seconds()
                    .is_some_and(|timestamp| {
                        timestamp >= resolved_window.start_epoch_seconds
                            && timestamp < resolved_window.end_epoch_seconds
                    })
        })
        .cloned()
        .collect::<Vec<_>>();
    counted_attempts.sort_by(|left, right| {
        right
            .effective_timestamp_epoch_seconds()
            .unwrap_or(0)
            .cmp(&left.effective_timestamp_epoch_seconds().unwrap_or(0))
            .then_with(|| {
                right
                    .amount_sats
                    .unwrap_or(0)
                    .cmp(&left.amount_sats.unwrap_or(0))
            })
            .then_with(|| left.request_id.cmp(&right.request_id))
    });

    let latest_counted_payment_epoch_seconds = counted_attempts
        .first()
        .and_then(Nip90BuyerPaymentAttempt::effective_timestamp_epoch_seconds);
    let latest_counted_payment_label = latest_counted_payment_epoch_seconds
        .map(local_timestamp_label)
        .unwrap_or_else(|| "none in window".to_string());
    let recent_rows = counted_attempts
        .iter()
        .take(MAX_RECENT_ROWS)
        .filter_map(recent_row_from_attempt)
        .collect::<Vec<_>>();
    let degraded_binding_count = report.degraded_binding_count;

    Ok(Nip90SentPaymentsPaneView {
        window_preset: resolved_window.preset,
        window_label: window_label_for_preset(resolved_window.preset),
        window_range_label: format_window_range(
            resolved_window.start_epoch_seconds,
            resolved_window.end_epoch_seconds,
        ),
        report,
        connected_relay_count: relay_urls_considered.len(),
        relay_urls_considered,
        latest_counted_payment_epoch_seconds,
        latest_counted_payment_label,
        custom_window_label: custom_window_label(pane_state),
        authoritative_note:
            "Wallet-settled buyer sends count once per payment pointer after relay fan-in dedupe."
                .to_string(),
        degraded_note: if degraded_binding_count == 0 {
            "No degraded bindings were observed inside this window.".to_string()
        } else {
            format!(
                "{} degraded bindings were observed in-window but excluded from definitive totals.",
                degraded_binding_count
            )
        },
        recent_rows,
    })
}

pub fn clipboard_text(
    pane_state: &Nip90SentPaymentsPaneState,
    buyer_payment_attempts: &Nip90BuyerPaymentAttemptLedgerState,
    relay_connections: &RelayConnectionsState,
    now_epoch_seconds: u64,
) -> Result<String, String> {
    let view = build_view(
        pane_state,
        buyer_payment_attempts,
        relay_connections,
        now_epoch_seconds,
    )?;
    let mut lines = vec![
        "NIP-90 sent payments".to_string(),
        format!("window={} {}", view.window_label, view.window_range_label),
        format!(
            "totals payment_count={} total_sats_sent={} total_fee_sats={} total_wallet_debit_sats={}",
            view.report.payment_count,
            view.report.total_sats_sent,
            view.report.total_fee_sats,
            view.report.total_wallet_debit_sats
        ),
        format!(
            "detail deduped_request_count={} degraded_binding_count={} connected_relay_count={}",
            view.report.deduped_request_count,
            view.report.degraded_binding_count,
            view.connected_relay_count
        ),
        format!(
            "latest_counted_payment={}",
            view.latest_counted_payment_label
        ),
    ];
    if !view.relay_urls_considered.is_empty() {
        lines.push(format!(
            "connected_relays={}",
            view.relay_urls_considered.join(", ")
        ));
    }
    for row in &view.recent_rows {
        lines.push(format!(
            "row {} {} sats req={} provider={} relays={}",
            row.timestamp_label,
            row.amount_sats,
            row.request_id,
            row.provider_label,
            row.relay_count
        ));
    }
    Ok(lines.join("\n"))
}

pub fn snapshot_payload(
    pane_state: &Nip90SentPaymentsPaneState,
    buyer_payment_attempts: &Nip90BuyerPaymentAttemptLedgerState,
    relay_connections: &RelayConnectionsState,
    now_epoch_seconds: u64,
) -> Value {
    match build_view(
        pane_state,
        buyer_payment_attempts,
        relay_connections,
        now_epoch_seconds,
    ) {
        Ok(view) => json!({
            "selected_window": view.window_preset.key(),
            "window_label": view.window_label,
            "window_range_label": view.window_range_label,
            "window_start_epoch_seconds": view.report.start_epoch_seconds,
            "window_end_epoch_seconds": view.report.end_epoch_seconds,
            "window_start_rfc3339": rfc3339_timestamp(view.report.start_epoch_seconds),
            "window_end_rfc3339": rfc3339_timestamp(view.report.end_epoch_seconds),
            "payment_count": view.report.payment_count,
            "total_sats_sent": view.report.total_sats_sent,
            "total_fee_sats": view.report.total_fee_sats,
            "total_wallet_debit_sats": view.report.total_wallet_debit_sats,
            "deduped_request_count": view.report.deduped_request_count,
            "degraded_binding_count": view.report.degraded_binding_count,
            "connected_relay_count": view.connected_relay_count,
            "relay_urls_considered": view.relay_urls_considered,
            "latest_counted_payment_epoch_seconds": view.latest_counted_payment_epoch_seconds,
            "latest_counted_payment_rfc3339": view
                .latest_counted_payment_epoch_seconds
                .map(rfc3339_timestamp),
            "custom_start_epoch_seconds": pane_state.custom_start_epoch_seconds,
            "custom_end_epoch_seconds": pane_state.custom_end_epoch_seconds,
            "custom_window_label": view.custom_window_label,
            "authoritative_note": view.authoritative_note,
            "degraded_note": view.degraded_note,
            "last_action": pane_state.last_action,
            "last_error": pane_state.last_error,
        }),
        Err(error) => json!({
            "selected_window": pane_state.selected_window.key(),
            "custom_start_epoch_seconds": pane_state.custom_start_epoch_seconds,
            "custom_end_epoch_seconds": pane_state.custom_end_epoch_seconds,
            "last_action": pane_state.last_action,
            "last_error": pane_state.last_error,
            "window_error": error,
        }),
    }
}

fn paint_window_buttons(
    content_bounds: Bounds,
    pane_state: &Nip90SentPaymentsPaneState,
    paint: &mut PaintContext,
) {
    for (index, preset) in [
        Nip90SentPaymentsWindowPreset::Daily,
        Nip90SentPaymentsWindowPreset::Rolling24h,
        Nip90SentPaymentsWindowPreset::Rolling7d,
        Nip90SentPaymentsWindowPreset::Rolling30d,
        Nip90SentPaymentsWindowPreset::Custom,
    ]
    .iter()
    .enumerate()
    {
        let bounds = nip90_sent_payments_window_button_bounds(content_bounds, index);
        if pane_state.selected_window == *preset {
            paint_secondary_button(bounds, preset.label(), paint);
        } else {
            paint_tertiary_button(bounds, preset.label(), paint);
        }
    }
    paint_action_button(
        nip90_sent_payments_copy_button_bounds(content_bounds),
        "Copy",
        paint,
    );
}

fn paint_error_state(
    content_bounds: Bounds,
    pane_state: &Nip90SentPaymentsPaneState,
    error: &str,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "NIP-90 SENT PAYMENTS  //  WINDOW ERROR",
        Point::new(
            content_bounds.origin.x + PADDING,
            content_bounds.origin.y + HEADER_TOP,
        ),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        error,
        Point::new(
            content_bounds.origin.x + PADDING,
            content_bounds.origin.y + HEADER_TOP + 24.0,
        ),
        11.0,
        theme::status::ERROR,
    ));
    let mut y = content_bounds.origin.y + HEADER_TOP + 54.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        y,
        "Selected window",
        pane_state.selected_window.label(),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + PADDING,
        y,
        "Stored custom",
        custom_window_label(pane_state).as_str(),
    );
}

fn paint_header(
    content_bounds: Bounds,
    view: &Nip90SentPaymentsPaneView,
    paint: &mut PaintContext,
) {
    paint.scene.draw_text(paint.text.layout_mono(
        "NIP-90 SENT PAYMENTS  //  DEFINITIVE BUYER SEND REPORT",
        Point::new(
            content_bounds.origin.x + PADDING,
            content_bounds.origin.y + HEADER_TOP,
        ),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Every relay we are currently connected to informs request/result/invoice evidence, but only wallet-authoritative buyer sends count in the totals below.",
        Point::new(
            content_bounds.origin.x + PADDING,
            content_bounds.origin.y + HEADER_TOP + 20.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "window={}  range={}  connected_relays={}  latest={}",
                view.window_label,
                view.window_range_label,
                view.connected_relay_count,
                view.latest_counted_payment_label,
            )
            .as_str(),
            Point::new(
                content_bounds.origin.x + PADDING,
                content_bounds.origin.y + HEADER_TOP + 38.0,
            ),
            10.0,
            theme::text::SECONDARY,
        ),
    );
}

fn paint_metric_row(
    content_bounds: Bounds,
    view: &Nip90SentPaymentsPaneView,
    paint: &mut PaintContext,
) {
    let metric_width =
        ((content_bounds.size.width - PADDING * 2.0 - PANEL_GAP * 3.0) / 4.0).max(120.0);
    let entries = [
        ("Payments", view.report.payment_count.to_string()),
        (
            "Total sent",
            format_sats_amount(view.report.total_sats_sent),
        ),
        ("Fees", format_sats_amount(view.report.total_fee_sats)),
        (
            "Wallet debit",
            format_sats_amount(view.report.total_wallet_debit_sats),
        ),
    ];
    for (index, (label, value)) in entries.iter().enumerate() {
        let bounds = Bounds::new(
            content_bounds.origin.x + PADDING + index as f32 * (metric_width + PANEL_GAP),
            content_bounds.origin.y + METRIC_TOP,
            metric_width,
            METRIC_HEIGHT,
        );
        paint_metric_card(bounds, label, value.as_str(), paint);
    }
}

fn paint_metric_card(bounds: Bounds, label: &str, value: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::ELEVATED)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 12.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 36.0),
        18.0,
        theme::text::PRIMARY,
    ));
}

fn paint_summary_panels(
    content_bounds: Bounds,
    view: &Nip90SentPaymentsPaneView,
    paint: &mut PaintContext,
) {
    let panel_width = ((content_bounds.size.width - PADDING * 2.0 - PANEL_GAP) * 0.5).max(220.0);
    let left_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + PANEL_TOP,
        panel_width,
        PANEL_HEIGHT,
    );
    let right_bounds = Bounds::new(
        left_bounds.max_x() + PANEL_GAP,
        left_bounds.origin.y,
        (content_bounds.max_x() - left_bounds.max_x() - PADDING - PANEL_GAP).max(220.0),
        PANEL_HEIGHT,
    );

    paint_panel_shell(left_bounds, "Window Detail", paint);
    let mut left_y = left_bounds.origin.y + 28.0;
    left_y = paint_label_line(
        paint,
        left_bounds.origin.x + 10.0,
        left_y,
        "Requests",
        view.report.deduped_request_count.to_string().as_str(),
    );
    left_y = paint_label_line(
        paint,
        left_bounds.origin.x + 10.0,
        left_y,
        "Degraded",
        view.report.degraded_binding_count.to_string().as_str(),
    );
    left_y = paint_label_line(
        paint,
        left_bounds.origin.x + 10.0,
        left_y,
        "Latest counted",
        view.latest_counted_payment_label.as_str(),
    );
    let _ = paint_label_line(
        paint,
        left_bounds.origin.x + 10.0,
        left_y,
        "Custom",
        view.custom_window_label.as_str(),
    );

    paint_panel_shell(right_bounds, "Relay Scope", paint);
    paint.scene.draw_text(paint.text.layout(
        view.authoritative_note.as_str(),
        Point::new(right_bounds.origin.x + 10.0, right_bounds.origin.y + 28.0),
        10.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        view.degraded_note.as_str(),
        Point::new(right_bounds.origin.x + 10.0, right_bounds.origin.y + 48.0),
        10.0,
        theme::text::MUTED,
    ));
    let relay_line = if view.relay_urls_considered.is_empty() {
        "No relays are currently connected.".to_string()
    } else {
        format!(
            "Connected relays: {}",
            view.relay_urls_considered.join(", ")
        )
    };
    paint.scene.draw_text(paint.text.layout(
        relay_line.as_str(),
        Point::new(right_bounds.origin.x + 10.0, right_bounds.origin.y + 74.0),
        10.0,
        theme::text::PRIMARY,
    ));
}

fn paint_recent_rows(
    content_bounds: Bounds,
    view: &Nip90SentPaymentsPaneView,
    paint: &mut PaintContext,
) {
    let panel_bounds = Bounds::new(
        content_bounds.origin.x + PADDING,
        content_bounds.origin.y + RECENT_TOP,
        content_bounds.size.width - PADDING * 2.0,
        (content_bounds.max_y() - content_bounds.origin.y - RECENT_TOP - PADDING).max(96.0),
    );
    paint_panel_shell(panel_bounds, "Recent Counted Sends", paint);

    if view.recent_rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No wallet-settled buyer sends landed inside the selected window.",
            Point::new(panel_bounds.origin.x + 10.0, panel_bounds.origin.y + 30.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (index, row) in view.recent_rows.iter().enumerate() {
        let row_bounds = Bounds::new(
            panel_bounds.origin.x + 8.0,
            panel_bounds.origin.y + 24.0 + index as f32 * RECENT_ROW_HEIGHT,
            panel_bounds.size.width - 16.0,
            RECENT_ROW_HEIGHT - 2.0,
        );
        if row_bounds.max_y() > panel_bounds.max_y() - 6.0 {
            break;
        }
        let fill = if index % 2 == 0 {
            theme::bg::SURFACE
        } else {
            theme::bg::APP
        };
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(fill)
                .with_border(theme::border::DEFAULT.with_alpha(0.25), 1.0)
                .with_corner_radius(4.0),
        );
        let summary = format!(
            "{}  {}  req {}  provider {}  relays {}",
            row.timestamp_label,
            format_sats_amount(row.amount_sats),
            row.request_id,
            row.provider_label,
            row.relay_count,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            summary.as_str(),
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 8.0),
            10.0,
            theme::text::PRIMARY,
        ));
    }
}

fn paint_panel_shell(bounds: Bounds, title: &str, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::ELEVATED)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 10.0),
        10.0,
        theme::text::MUTED,
    ));
}

fn recent_row_from_attempt(
    attempt: &Nip90BuyerPaymentAttempt,
) -> Option<Nip90SentPaymentsRecentRow> {
    let timestamp_epoch_seconds = attempt.effective_timestamp_epoch_seconds()?;
    Some(Nip90SentPaymentsRecentRow {
        request_id: compact_id(attempt.request_id.as_str()),
        provider_label: compact_identity(
            attempt
                .provider_nostr_pubkey
                .as_deref()
                .unwrap_or("unknown-provider"),
        ),
        amount_sats: attempt.amount_sats.unwrap_or(0),
        timestamp_epoch_seconds,
        timestamp_label: local_timestamp_label(timestamp_epoch_seconds),
        relay_count: attempt.relay_evidence.deduped_relay_count(),
    })
}

fn connected_relay_urls(relay_connections: &RelayConnectionsState) -> Vec<String> {
    relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == RelayConnectionStatus::Connected)
        .map(|relay| relay.url.clone())
        .collect::<Vec<_>>()
}

fn compact_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        return trimmed.to_string();
    }
    format!("{}..{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
}

fn compact_identity(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 16 {
        return trimmed.to_string();
    }
    format!("{}..{}", &trimmed[..8], &trimmed[trimmed.len() - 6..])
}

fn window_label_for_preset(preset: Nip90SentPaymentsWindowPreset) -> String {
    match preset {
        Nip90SentPaymentsWindowPreset::Daily => "Daily (local day)".to_string(),
        Nip90SentPaymentsWindowPreset::Rolling24h => "Last 24h".to_string(),
        Nip90SentPaymentsWindowPreset::Rolling7d => "Last 7d".to_string(),
        Nip90SentPaymentsWindowPreset::Rolling30d => "Last 30d".to_string(),
        Nip90SentPaymentsWindowPreset::Custom => "Custom".to_string(),
    }
}

fn custom_window_label(pane_state: &Nip90SentPaymentsPaneState) -> String {
    match (
        pane_state.custom_start_epoch_seconds,
        pane_state.custom_end_epoch_seconds,
    ) {
        (Some(start), Some(end)) if start < end => format_window_range(start, end),
        (Some(start), Some(end)) => format!("invalid {}..{}", start, end),
        (Some(start), None) => format!("start {} / end unset", start),
        (None, Some(end)) => format!("start unset / end {}", end),
        (None, None) => "not set".to_string(),
    }
}

fn format_window_range(start_epoch_seconds: u64, end_epoch_seconds: u64) -> String {
    format!(
        "{} -> {}",
        local_timestamp_label(start_epoch_seconds),
        local_timestamp_label(end_epoch_seconds)
    )
}

fn local_timestamp_label(epoch_seconds: u64) -> String {
    Local
        .timestamp_opt(epoch_seconds as i64, 0)
        .single()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| format!("epoch:{epoch_seconds}"))
}

fn rfc3339_timestamp(epoch_seconds: u64) -> String {
    Utc.timestamp_opt(epoch_seconds as i64, 0)
        .single()
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| format!("epoch:{epoch_seconds}"))
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{build_view, snapshot_payload};
    use crate::app_state::{Nip90SentPaymentsPaneState, Nip90SentPaymentsWindowPreset};
    use crate::state::nip90_buyer_payment_attempts::{
        Nip90BuyerPaymentAttempt, Nip90BuyerPaymentAttemptLedgerState,
        Nip90BuyerPaymentBindingQuality, Nip90BuyerPaymentRelayEvidence,
        Nip90BuyerPaymentSourceQuality,
    };
    use crate::state::operations::{
        RelayConnectionRow, RelayConnectionStatus, RelayConnectionsState,
    };
    use serde_json::Value;
    use std::path::PathBuf;

    fn unique_temp_path(label: &str) -> PathBuf {
        let unique = format!(
            "openagents-nip90-sent-pane-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch time available")
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }

    fn authoritative_attempt(
        pointer: &str,
        request_id: &str,
        timestamp: u64,
        amount_sats: u64,
    ) -> Nip90BuyerPaymentAttempt {
        Nip90BuyerPaymentAttempt {
            payment_pointer: pointer.to_string(),
            request_id: request_id.to_string(),
            request_type: "nip90.textgen".to_string(),
            wallet_direction: "send".to_string(),
            wallet_status: "settled".to_string(),
            wallet_confirmed_at: Some(timestamp),
            wallet_first_seen_at: Some(timestamp.saturating_sub(2)),
            amount_sats: Some(amount_sats),
            fees_sats: Some(1),
            total_debit_sats: Some(amount_sats + 1),
            payment_hash: Some(format!("hash:{pointer}")),
            destination_pubkey: Some("ln-destination".to_string()),
            buyer_nostr_pubkey: Some("buyer-001".to_string()),
            provider_nostr_pubkey: Some("provider-001-abcdef".to_string()),
            binding_quality: Nip90BuyerPaymentBindingQuality::AppObserved,
            source_quality: Nip90BuyerPaymentSourceQuality::WalletAuthoritative,
            relay_evidence: Nip90BuyerPaymentRelayEvidence {
                request_publish_selected_relays: vec!["wss://relay.one".to_string()],
                result_observed_relays: vec!["wss://relay.two".to_string()],
                ..Nip90BuyerPaymentRelayEvidence::default()
            },
        }
    }

    fn degraded_attempt(
        pointer: &str,
        request_id: &str,
        timestamp: u64,
    ) -> Nip90BuyerPaymentAttempt {
        Nip90BuyerPaymentAttempt {
            payment_pointer: pointer.to_string(),
            request_id: request_id.to_string(),
            request_type: "nip90.textgen".to_string(),
            wallet_direction: "send".to_string(),
            wallet_status: "settled".to_string(),
            wallet_confirmed_at: Some(timestamp),
            wallet_first_seen_at: Some(timestamp),
            amount_sats: Some(3),
            fees_sats: Some(0),
            total_debit_sats: Some(3),
            payment_hash: None,
            destination_pubkey: None,
            buyer_nostr_pubkey: Some("buyer-001".to_string()),
            provider_nostr_pubkey: Some("provider-002".to_string()),
            binding_quality: Nip90BuyerPaymentBindingQuality::RequestFactBackfill,
            source_quality: Nip90BuyerPaymentSourceQuality::DegradedRecovery,
            relay_evidence: Nip90BuyerPaymentRelayEvidence::default(),
        }
    }

    #[test]
    fn custom_window_view_reports_authoritative_totals() {
        let mut ledger =
            Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(unique_temp_path("view"));
        ledger.attempts = vec![
            authoritative_attempt("wallet-001", "req-001", 1_762_700_120, 21),
            authoritative_attempt("wallet-002", "req-002", 1_762_700_180, 34),
            degraded_attempt("wallet-003", "req-003", 1_762_700_190),
        ];
        let relay_connections = RelayConnectionsState {
            relays: vec![
                RelayConnectionRow {
                    url: "wss://relay.one".to_string(),
                    status: RelayConnectionStatus::Connected,
                    latency_ms: Some(12),
                    last_seen_seconds_ago: Some(1),
                    last_error: None,
                },
                RelayConnectionRow {
                    url: "wss://relay.offline".to_string(),
                    status: RelayConnectionStatus::Disconnected,
                    latency_ms: None,
                    last_seen_seconds_ago: None,
                    last_error: None,
                },
            ],
            ..RelayConnectionsState::default()
        };
        let mut pane_state = Nip90SentPaymentsPaneState::default();
        pane_state.custom_start_epoch_seconds = Some(1_762_700_100);
        pane_state.custom_end_epoch_seconds = Some(1_762_700_200);
        pane_state.selected_window = Nip90SentPaymentsWindowPreset::Custom;

        let view = build_view(&pane_state, &ledger, &relay_connections, 1_762_700_199)
            .expect("custom view should build");
        assert_eq!(view.report.payment_count, 2);
        assert_eq!(view.report.total_sats_sent, 55);
        assert_eq!(view.report.total_fee_sats, 2);
        assert_eq!(view.report.total_wallet_debit_sats, 57);
        assert_eq!(view.report.deduped_request_count, 2);
        assert_eq!(view.report.degraded_binding_count, 1);
        assert_eq!(view.connected_relay_count, 1);
        assert_eq!(
            view.latest_counted_payment_epoch_seconds,
            Some(1_762_700_180)
        );
    }

    #[test]
    fn snapshot_payload_exposes_selected_window_and_totals() {
        let mut ledger =
            Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(unique_temp_path("snapshot"));
        ledger.attempts = vec![authoritative_attempt(
            "wallet-001",
            "req-001",
            1_762_700_120,
            21,
        )];
        let relay_connections = RelayConnectionsState {
            relays: vec![RelayConnectionRow {
                url: "wss://relay.one".to_string(),
                status: RelayConnectionStatus::Connected,
                latency_ms: Some(12),
                last_seen_seconds_ago: Some(1),
                last_error: None,
            }],
            ..RelayConnectionsState::default()
        };
        let mut pane_state = Nip90SentPaymentsPaneState::default();
        pane_state.custom_start_epoch_seconds = Some(1_762_700_100);
        pane_state.custom_end_epoch_seconds = Some(1_762_700_200);
        pane_state.selected_window = Nip90SentPaymentsWindowPreset::Custom;

        let payload = snapshot_payload(&pane_state, &ledger, &relay_connections, 1_762_700_199);
        assert_eq!(
            payload.get("selected_window").and_then(Value::as_str),
            Some("custom")
        );
        assert_eq!(
            payload.get("payment_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            payload.get("total_sats_sent").and_then(Value::as_u64),
            Some(21)
        );
        assert_eq!(
            payload.get("connected_relay_count").and_then(Value::as_u64),
            Some(1)
        );
    }
}
