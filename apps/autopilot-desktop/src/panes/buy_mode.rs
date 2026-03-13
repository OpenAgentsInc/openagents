use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AutopilotChatState, BuyModePaymentsPaneState, NetworkRequestsState,
    buy_mode_payments_status_lines,
};
use crate::pane_renderer::{
    MissionControlBuyModePanelState, mission_control_buy_mode_panel_state, paint_action_button,
    paint_source_badge,
};
use crate::pane_system::{
    buy_mode_payments_copy_button_bounds, buy_mode_payments_ledger_bounds,
    buy_mode_payments_toggle_button_bounds,
};
use crate::spark_wallet::SparkPaneState;

pub fn paint(
    content_bounds: Bounds,
    buy_mode_enabled: bool,
    autopilot_chat: &AutopilotChatState,
    pane_state: &mut BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
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
    paint_source_badge(content_bounds, "buy", paint);
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
        Point::new(content_bounds.origin.x + 12.0, content_bounds.origin.y + 40.0),
        11.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Fixed smoke-test lane: kind 5050, 2 sats budget, one in-flight request, app-owned targeting state.",
        Point::new(content_bounds.origin.x + 12.0, content_bounds.origin.y + 58.0),
        10.0,
        theme::text::MUTED,
    ));

    if let Some(view_state) = view_state.as_ref() {
        paint_status_row(content_bounds, view_state, paint);
    }

    let status_lines =
        buy_mode_payments_status_lines(pane_state, network_requests, spark_wallet, now);
    for (index, line) in status_lines.iter().enumerate() {
        paint.scene.draw_text(paint.text.layout_mono(
            line,
            Point::new(
                content_bounds.origin.x + 12.0,
                content_bounds.origin.y + 132.0 + (index as f32 * 14.0),
            ),
            10.0,
            theme::text::SECONDARY,
        ));
    }

    pane_state.ledger.set_title("");
    pane_state
        .ledger
        .paint(buy_mode_payments_ledger_bounds(content_bounds), paint);
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
