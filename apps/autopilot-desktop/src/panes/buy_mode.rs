use wgpui::components::hud::{DotShape, DotsGrid};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, BuyModePaymentsPaneState, MISSION_CONTROL_BUY_MODE_REQUEST_TYPE,
    NetworkRequestsState, buy_mode_payments_status_lines,
};
use crate::nip90_compute_flow::build_buyer_request_flow_snapshot;
use crate::pane_renderer::{
    MissionControlBuyModePanelState, mission_control_buy_mode_panel_state, paint_action_button,
    paint_source_badge,
};
use crate::pane_system::{
    buy_mode_payments_copy_button_bounds, buy_mode_payments_ledger_bounds,
    buy_mode_payments_toggle_button_bounds,
};
use crate::spark_wallet::SparkPaneState;
use crate::state::nip90_payment_facts::{
    Nip90PaymentFact, Nip90PaymentFactLedgerState, Nip90PaymentFactSourceQuality,
    Nip90PaymentFactStatus,
};

const LEDGER_HEADER_HEIGHT: f32 = 26.0;
const LEDGER_ROW_HEIGHT: f32 = 62.0;
const LEDGER_ROW_GAP: f32 = 8.0;
const DETAIL_PANEL_MIN_WIDTH: f32 = 260.0;

pub fn paint(
    content_bounds: Bounds,
    buy_mode_enabled: bool,
    autopilot_chat: &AutopilotChatState,
    pane_state: &mut BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    payment_facts: &Nip90PaymentFactLedgerState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    let now = std::time::Instant::now();
    let view_state = mission_control_buy_mode_panel_state(
        buy_mode_enabled,
        autopilot_chat,
        pane_state,
        network_requests,
        spark_wallet,
        now,
    );
    let ledger_view = build_visual_ledger_view(payment_facts, network_requests, spark_wallet);

    paint_source_badge(content_bounds, "buy+facts", paint);
    pane_state.sync_rows(network_requests, spark_wallet);

    paint_buy_mode_button(
        buy_mode_payments_toggle_button_bounds(content_bounds),
        view_state.as_ref(),
        paint,
    );
    paint_action_button(
        buy_mode_payments_copy_button_bounds(content_bounds),
        "Copy ledger",
        paint,
    );

    let summary = view_state
        .as_ref()
        .map(|state| state.summary.as_str())
        .unwrap_or("Buy Mode is disabled for this session.");
    paint.scene.draw_text(paint.text.layout_mono(
        summary,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 40.0,
        ),
        11.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Visualization-first buyer ledger: amount bars, fee overlays, request evidence, and explicit Nostr provider vs Lightning destination identity labels.",
        Point::new(content_bounds.origin.x + 12.0, content_bounds.origin.y + 58.0),
        10.0,
        theme::text::MUTED,
    ));

    if let Some(view_state) = view_state.as_ref() {
        paint_status_row(content_bounds, view_state, paint);
    }

    let status_lines =
        buy_mode_payments_status_lines(pane_state, network_requests, spark_wallet, now);
    for (index, line) in status_lines.iter().take(2).enumerate() {
        paint.scene.draw_text(paint.text.layout_mono(
            line,
            Point::new(
                content_bounds.origin.x + 12.0,
                content_bounds.origin.y + 132.0 + (index as f32 * 14.0),
            ),
            9.5,
            theme::text::SECONDARY,
        ));
    }

    paint_visual_ledger(
        buy_mode_payments_ledger_bounds(content_bounds),
        &ledger_view,
        paint,
    );
}

#[derive(Clone, Debug)]
struct BuyModeVisualLedgerView {
    rows: Vec<BuyModeVisualLedgerRow>,
    max_total_debit_sats: u64,
    total_fee_sats: u64,
    total_debit_sats: u64,
    request_projection_rows: usize,
    wallet_rows: usize,
}

#[derive(Clone, Debug)]
struct BuyModeVisualLedgerRow {
    request_id: String,
    status_label: String,
    source_label: String,
    source_note: String,
    source_quality: Option<Nip90PaymentFactSourceQuality>,
    amount_sats: u64,
    fees_sats: u64,
    total_debit_sats: u64,
    provider_nostr_pubkey: String,
    lightning_destination_pubkey: String,
    payment_pointer: String,
    payment_hash: String,
    settlement_authority: String,
    request_event_id: String,
    result_event_id: String,
    invoice_event_id: String,
    sort_epoch_seconds: u64,
    degraded: bool,
    status: Option<Nip90PaymentFactStatus>,
}

fn build_visual_ledger_view(
    payment_facts: &Nip90PaymentFactLedgerState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> BuyModeVisualLedgerView {
    let mut rows = payment_facts
        .facts
        .iter()
        .filter(|fact| fact.request_type == MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
        .map(row_from_fact)
        .collect::<Vec<_>>();

    if rows.is_empty()
        && let Some(request) =
            network_requests.latest_request_by_type(MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
    {
        rows.push(row_from_live_snapshot(&build_buyer_request_flow_snapshot(
            request,
            spark_wallet,
        )));
    }

    rows.sort_by(|left, right| {
        right
            .sort_epoch_seconds
            .cmp(&left.sort_epoch_seconds)
            .then_with(|| left.request_id.cmp(&right.request_id))
    });

    let max_total_debit_sats = rows
        .iter()
        .map(|row| row.total_debit_sats.max(row.amount_sats))
        .max()
        .unwrap_or(1)
        .max(1);
    let total_fee_sats = rows.iter().map(|row| row.fees_sats).sum();
    let total_debit_sats = rows.iter().map(|row| row.total_debit_sats).sum();
    let request_projection_rows = rows.iter().filter(|row| row.degraded).count();
    let wallet_rows = rows.len().saturating_sub(request_projection_rows);

    BuyModeVisualLedgerView {
        rows,
        max_total_debit_sats,
        total_fee_sats,
        total_debit_sats,
        request_projection_rows,
        wallet_rows,
    }
}

fn row_from_fact(fact: &Nip90PaymentFact) -> BuyModeVisualLedgerRow {
    let source_label = match fact.source_quality {
        Nip90PaymentFactSourceQuality::BuyerWalletReconciled => "wallet",
        Nip90PaymentFactSourceQuality::RequestProjection => "request",
        Nip90PaymentFactSourceQuality::LogBackfill => "backfill",
        Nip90PaymentFactSourceQuality::SellerReceiptProjection => "receipt",
        Nip90PaymentFactSourceQuality::SellerWalletReconciled => "seller-wallet",
    };
    let source_note = match fact.source_quality {
        Nip90PaymentFactSourceQuality::BuyerWalletReconciled => {
            "wallet-reconciled buyer send".to_string()
        }
        Nip90PaymentFactSourceQuality::RequestProjection => {
            "request-derived until wallet confirmation".to_string()
        }
        Nip90PaymentFactSourceQuality::LogBackfill => "session-log backfill".to_string(),
        Nip90PaymentFactSourceQuality::SellerReceiptProjection => {
            "seller receipt projection".to_string()
        }
        Nip90PaymentFactSourceQuality::SellerWalletReconciled => {
            "seller wallet reconciliation".to_string()
        }
    };
    let status_label = fact.status.label().to_string();
    let amount_sats = fact.amount_sats.unwrap_or_default();
    let fees_sats = fact.fees_sats.unwrap_or_default();
    let total_debit_sats = fact
        .total_debit_sats
        .unwrap_or_else(|| amount_sats.saturating_add(fees_sats))
        .max(amount_sats);

    BuyModeVisualLedgerRow {
        request_id: fact.request_id.clone(),
        status_label,
        source_label: source_label.to_string(),
        source_note,
        source_quality: Some(fact.source_quality),
        amount_sats,
        fees_sats,
        total_debit_sats,
        provider_nostr_pubkey: fact
            .provider_nostr_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        lightning_destination_pubkey: fact
            .lightning_destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payment_pointer: fact
            .buyer_payment_pointer
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payment_hash: fact
            .buyer_payment_hash
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        settlement_authority: fact.settlement_authority.clone(),
        request_event_id: fact
            .request_event_id
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        result_event_id: fact
            .result_event_id
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        invoice_event_id: fact
            .invoice_event_id
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        sort_epoch_seconds: fact.latest_event_epoch_seconds().unwrap_or_default(),
        degraded: !matches!(
            fact.source_quality,
            Nip90PaymentFactSourceQuality::BuyerWalletReconciled
        ),
        status: Some(fact.status),
    }
}

fn row_from_live_snapshot(
    snapshot: &crate::nip90_compute_flow::BuyerRequestFlowSnapshot,
) -> BuyModeVisualLedgerRow {
    let amount_sats = snapshot.invoice_amount_sats.unwrap_or(snapshot.budget_sats);
    let fees_sats = snapshot.fees_sats.unwrap_or_default();
    let total_debit_sats = snapshot
        .total_debit_sats
        .unwrap_or_else(|| amount_sats.saturating_add(fees_sats))
        .max(amount_sats);

    BuyModeVisualLedgerRow {
        request_id: snapshot.request_id.clone(),
        status_label: snapshot.status.label().to_string(),
        source_label: "live-request".to_string(),
        source_note: "live request projection fallback".to_string(),
        source_quality: None,
        amount_sats,
        fees_sats,
        total_debit_sats,
        provider_nostr_pubkey: snapshot.provider_pubkey().unwrap_or("-").to_string(),
        lightning_destination_pubkey: snapshot
            .destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payment_pointer: snapshot
            .payment_pointer
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payment_hash: snapshot
            .payment_hash
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        settlement_authority: snapshot.authority.as_str().to_string(),
        request_event_id: snapshot
            .published_request_event_id
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        result_event_id: snapshot
            .winning_result_event_id
            .clone()
            .or(snapshot.last_result_event_id.clone())
            .unwrap_or_else(|| "-".to_string()),
        invoice_event_id: snapshot
            .last_feedback_event_id
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        sort_epoch_seconds: snapshot
            .timestamp
            .or(snapshot.request_published_at_epoch_seconds)
            .unwrap_or_default(),
        degraded: true,
        status: None,
    }
}

fn paint_visual_ledger(
    ledger_bounds: Bounds,
    ledger_view: &BuyModeVisualLedgerView,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(ledger_bounds)
            .with_background(Hsla::from_hex(0x07121a).with_alpha(0.95))
            .with_border(Hsla::from_hex(0x2aa7e0).with_alpha(0.24), 1.0)
            .with_corner_radius(10.0),
    );

    let mut grid = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(22.0)
        .size(1.0)
        .color(Hsla::from_hex(0x2aa7e0).with_alpha(0.08))
        .animation_progress(1.0);
    grid.paint(
        Bounds::new(
            ledger_bounds.origin.x + 8.0,
            ledger_bounds.origin.y + 8.0,
            ledger_bounds.size.width - 16.0,
            ledger_bounds.size.height - 16.0,
        ),
        paint,
    );

    paint.scene.draw_text(paint.text.layout_mono(
        format!(
            "{} rows  //  {} wallet  //  {} degraded  //  total debit={} sats  //  fees={} sats",
            ledger_view.rows.len(),
            ledger_view.wallet_rows,
            ledger_view.request_projection_rows,
            ledger_view.total_debit_sats,
            ledger_view.total_fee_sats,
        )
        .as_str(),
        Point::new(ledger_bounds.origin.x + 12.0, ledger_bounds.origin.y + 16.0),
        9.5,
        theme::text::SECONDARY,
    ));

    let detail_width = (ledger_bounds.size.width * 0.34).max(DETAIL_PANEL_MIN_WIDTH);
    let rows_width = (ledger_bounds.size.width - detail_width - 20.0).max(180.0);
    let rows_bounds = Bounds::new(
        ledger_bounds.origin.x + 12.0,
        ledger_bounds.origin.y + LEDGER_HEADER_HEIGHT + 10.0,
        rows_width,
        ledger_bounds.size.height - LEDGER_HEADER_HEIGHT - 22.0,
    );
    let detail_bounds = Bounds::new(
        rows_bounds.max_x() + 12.0,
        rows_bounds.origin.y,
        ledger_bounds.max_x() - rows_bounds.max_x() - 24.0,
        rows_bounds.size.height,
    );

    paint_ledger_rows(rows_bounds, ledger_view, paint);
    paint_ledger_detail(detail_bounds, ledger_view.rows.first(), paint);
}

fn paint_ledger_rows(
    bounds: Bounds,
    ledger_view: &BuyModeVisualLedgerView,
    paint: &mut PaintContext,
) {
    let visible_rows = ((bounds.size.height + LEDGER_ROW_GAP)
        / (LEDGER_ROW_HEIGHT + LEDGER_ROW_GAP))
        .floor()
        .max(1.0) as usize;

    if ledger_view.rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No Buy Mode payment facts yet. The pane will promote rows here as soon as request or wallet evidence lands.",
            Point::new(bounds.origin.x, bounds.origin.y + 12.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (index, row) in ledger_view.rows.iter().take(visible_rows).enumerate() {
        let row_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + index as f32 * (LEDGER_ROW_HEIGHT + LEDGER_ROW_GAP),
            bounds.size.width,
            LEDGER_ROW_HEIGHT,
        );
        paint_ledger_row(
            row_bounds,
            row,
            ledger_view.max_total_debit_sats,
            index == 0,
            paint,
        );
    }
}

fn paint_ledger_row(
    bounds: Bounds,
    row: &BuyModeVisualLedgerRow,
    max_total_debit_sats: u64,
    focused: bool,
    paint: &mut PaintContext,
) {
    let accent = row_status_color(row);
    let background = if focused {
        Hsla::from_hex(0x0d1e29).with_alpha(0.98)
    } else {
        Hsla::from_hex(0x0a161f).with_alpha(0.94)
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(accent.with_alpha(if focused { 0.34 } else { 0.2 }), 1.0)
            .with_corner_radius(8.0),
    );

    paint.scene.draw_text(paint.text.layout_mono(
        short_id(row.request_id.as_str()).as_str(),
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 13.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint_status_chip(
        Bounds::new(bounds.max_x() - 166.0, bounds.origin.y + 8.0, 74.0, 18.0),
        row.status_label.as_str(),
        accent,
        paint,
    );
    paint_status_chip(
        Bounds::new(bounds.max_x() - 84.0, bounds.origin.y + 8.0, 72.0, 18.0),
        row.source_label.as_str(),
        if row.degraded {
            Hsla::from_hex(0xffbf69)
        } else {
            Hsla::from_hex(0x77dd77)
        },
        paint,
    );

    let bar_bounds = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 30.0,
        bounds.size.width - 20.0,
        10.0,
    );
    paint.scene.draw_quad(
        Quad::new(bar_bounds)
            .with_background(theme::bg::ELEVATED.with_alpha(0.5))
            .with_corner_radius(5.0),
    );
    let fill_width = if max_total_debit_sats == 0 {
        0.0
    } else {
        bar_bounds.size.width * (row.amount_sats as f32 / max_total_debit_sats as f32)
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bar_bounds.origin.x,
            bar_bounds.origin.y,
            fill_width.max(4.0),
            bar_bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.92))
        .with_corner_radius(5.0),
    );
    if row.fees_sats > 0 {
        let fee_width =
            bar_bounds.size.width * (row.fees_sats as f32 / max_total_debit_sats as f32);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                (bar_bounds.origin.x + fill_width - fee_width).max(bar_bounds.origin.x),
                bar_bounds.origin.y,
                fee_width.max(2.0),
                bar_bounds.size.height,
            ))
            .with_background(Hsla::from_hex(0xff7f50).with_alpha(0.92))
            .with_corner_radius(5.0),
        );
    }

    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "amount={} sats  fee={} sats  debit={} sats",
                row.amount_sats, row.fees_sats, row.total_debit_sats
            )
            .as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.origin.y + 50.0),
            8.8,
            theme::text::SECONDARY,
        ),
    );
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "nostr={}  lightning={}",
                short_id(row.provider_nostr_pubkey.as_str()),
                short_id(row.lightning_destination_pubkey.as_str()),
            )
            .as_str(),
            Point::new(bounds.origin.x + 210.0, bounds.origin.y + 50.0),
            8.8,
            theme::text::MUTED,
        ),
    );
}

fn paint_ledger_detail(
    bounds: Bounds,
    row: Option<&BuyModeVisualLedgerRow>,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x091118).with_alpha(0.96))
            .with_border(Hsla::from_hex(0x2aa7e0).with_alpha(0.18), 1.0)
            .with_corner_radius(8.0),
    );

    paint.scene.draw_text(paint.text.layout_mono(
        "DETAIL",
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 16.0),
        10.0,
        theme::text::PRIMARY,
    ));

    let Some(row) = row else {
        paint.scene.draw_text(paint.text.layout(
            "No detail row available yet.",
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    };

    paint.scene.draw_text(paint.text.layout(
        row.source_note.as_str(),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 34.0),
        10.0,
        if row.degraded {
            Hsla::from_hex(0xffbf69)
        } else {
            theme::text::MUTED
        },
    ));

    let mut y = bounds.origin.y + 58.0;
    for (label, value) in [
        ("Request", row.request_id.as_str()),
        ("Payment ptr", row.payment_pointer.as_str()),
        ("Payment hash", row.payment_hash.as_str()),
        ("Provider nostr", row.provider_nostr_pubkey.as_str()),
        ("Lightning dst", row.lightning_destination_pubkey.as_str()),
        ("Request evt", row.request_event_id.as_str()),
        ("Result evt", row.result_event_id.as_str()),
        ("Invoice evt", row.invoice_event_id.as_str()),
        ("Authority", row.settlement_authority.as_str()),
    ] {
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(bounds.origin.x + 12.0, y),
            8.8,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            value,
            Point::new(bounds.origin.x + 100.0, y),
            8.8,
            theme::text::PRIMARY,
        ));
        y += 18.0;
    }

    if let Some(source_quality) = row.source_quality {
        paint.scene.draw_text(paint.text.layout_mono(
            format!("source_quality={}", source_quality.label()).as_str(),
            Point::new(bounds.origin.x + 12.0, bounds.max_y() - 18.0),
            8.8,
            theme::text::SECONDARY,
        ));
    }
}

fn paint_status_chip(bounds: Bounds, label: &str, color: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(color.with_alpha(0.12))
            .with_border(color.with_alpha(0.4), 1.0)
            .with_corner_radius(7.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 11.0),
        8.5,
        color.with_alpha(0.96),
    ));
}

fn row_status_color(row: &BuyModeVisualLedgerRow) -> Hsla {
    match row.status {
        Some(Nip90PaymentFactStatus::BuyerWalletSettled) => Hsla::from_hex(0x77dd77),
        Some(Nip90PaymentFactStatus::BuyerPaymentPending) => Hsla::from_hex(0x63c7ff),
        Some(Nip90PaymentFactStatus::InvoiceObserved) => Hsla::from_hex(0xffbf69),
        Some(Nip90PaymentFactStatus::ResultObserved) => Hsla::from_hex(0x75b7ff),
        Some(Nip90PaymentFactStatus::Failed) => Hsla::from_hex(0xf25f5c),
        _ => Hsla::from_hex(0x8da9c4),
    }
}

fn short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 16 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..8], &trimmed[trimmed.len() - 6..])
    }
}

fn paint_buy_mode_button(
    bounds: Bounds,
    view_state: Option<&MissionControlBuyModePanelState>,
    paint: &mut PaintContext,
) {
    let (label, enabled, accent) = if let Some(state) = view_state {
        let accent = if !state.button_enabled {
            theme::text::MUTED
        } else if state.button_active {
            Hsla::from_hex(0x2A9D5B)
        } else {
            Hsla::from_hex(0x2AA7E0)
        };
        (state.button_label.as_str(), state.button_enabled, accent)
    } else {
        ("BUY MODE DISABLED", false, theme::text::MUTED)
    };

    let background = if enabled {
        theme::bg::HOVER.with_alpha(0.85)
    } else {
        theme::bg::SURFACE
    };
    let border = accent.with_alpha(if enabled { 0.55 } else { 0.2 });
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(border, 1.0)
            .with_corner_radius(6.0),
    );
    let label_layout = paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 6.0),
        10.0,
        accent,
    );
    paint.scene.draw_text(label_layout);
}

fn paint_status_row(
    content_bounds: Bounds,
    view_state: &MissionControlBuyModePanelState,
    paint: &mut PaintContext,
) {
    let labels = [
        ("MODE", view_state.mode.as_str()),
        ("NEXT", view_state.next.as_str()),
        ("PROV", view_state.provider.as_str()),
        ("WORK", view_state.work.as_str()),
        ("PAY", view_state.payment.as_str()),
    ];
    let row_x = content_bounds.origin.x + 12.0;
    let row_y = content_bounds.origin.y + 84.0;
    let gap = 8.0;
    let cell_width = ((content_bounds.size.width - 24.0 - gap * 4.0) / 5.0).max(0.0);
    for (index, (label, value)) in labels.iter().enumerate() {
        let cell_bounds = Bounds::new(
            row_x + index as f32 * (cell_width + gap),
            row_y,
            cell_width,
            34.0,
        );
        paint.scene.draw_quad(
            Quad::new(cell_bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(Hsla::from_hex(0x2AA7E0).with_alpha(0.3), 1.0)
                .with_corner_radius(6.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(cell_bounds.origin.x + 8.0, cell_bounds.origin.y + 6.0),
            9.0,
            theme::text::MUTED,
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            value,
            Point::new(cell_bounds.origin.x + 8.0, cell_bounds.origin.y + 18.0),
            10.0,
            theme::text::PRIMARY,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::build_visual_ledger_view;
    use crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE;
    use crate::spark_wallet::SparkPaneState;
    use crate::state::nip90_payment_facts::Nip90PaymentFactLedgerState;
    use crate::state::operations::{
        BuyerResolutionMode, NetworkRequestSubmission, NetworkRequestsState,
    };
    use std::path::PathBuf;

    fn temp_path(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should advance")
            .as_nanos();
        std::env::temp_dir().join(format!("openagents-{label}-{nonce}.json"))
    }

    #[test]
    fn visual_ledger_rows_split_provider_nostr_from_lightning_destination() {
        let path = temp_path("buy-mode-ledger-viz");
        let mut ledger = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-viz-001".to_string()),
                request_type: MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "viz".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["providernostr001".to_string()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 60,
                authority_command_seq: 1,
            })
            .expect("request should queue");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-viz-request-001",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "providernostr001",
            "event-viz-feedback-001",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc2n1viz"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            "providernostr001",
            "event-viz-result-001",
            Some("success"),
        );
        requests.mark_auto_payment_sent(request_id.as_str(), "wallet-viz-001", 1_762_700_333);

        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-viz-001".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 1,
                timestamp: 1_762_700_333,
                method: "lightning".to_string(),
                destination_pubkey: Some("03lightningdest001".to_string()),
                payment_hash: Some("hash-viz-001".to_string()),
                ..Default::default()
            });

        ledger.sync_from_current_truth(
            &requests,
            &crate::app_state::JobHistoryState::default(),
            &wallet,
            Some("localbuyer001"),
        );

        let view = build_visual_ledger_view(&ledger, &requests, &wallet);
        let row = view.rows.first().expect("row should exist");
        assert_eq!(row.provider_nostr_pubkey, "providernostr001");
        assert_eq!(row.lightning_destination_pubkey, "03lightningdest001");
        assert_eq!(row.payment_pointer, "wallet-viz-001");
        assert_eq!(row.payment_hash, "hash-viz-001");
        assert_eq!(row.source_label, "wallet");
        assert!(!row.degraded);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn visual_ledger_falls_back_to_live_request_when_fact_ledger_is_empty() {
        let ledger =
            Nip90PaymentFactLedgerState::from_path_for_tests(temp_path("buy-mode-live-fallback"));
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-live-fallback-001".to_string()),
                request_type: MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "fallback".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["providerfallback001".to_string()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 60,
                authority_command_seq: 1,
            })
            .expect("request should queue");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-live-fallback-001",
            1,
            0,
            None,
        );

        let view = build_visual_ledger_view(&ledger, &requests, &SparkPaneState::default());
        let row = view.rows.first().expect("fallback row should exist");
        assert_eq!(row.request_id, "req-buy-live-fallback-001");
        assert_eq!(row.source_label, "live-request");
        assert!(row.degraded);
        assert_eq!(row.request_event_id, "event-live-fallback-001");
    }
}
