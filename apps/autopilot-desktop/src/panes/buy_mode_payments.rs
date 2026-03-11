use wgpui::{Bounds, Component, PaintContext, Point, theme};

use crate::app_state::{
    BuyModePaymentsPaneState, NetworkRequestsState, buy_mode_payments_summary_text,
};
use crate::pane_renderer::{paint_action_button, paint_source_badge};
use crate::pane_system::{buy_mode_payments_copy_button_bounds, buy_mode_payments_ledger_bounds};
use crate::spark_wallet::SparkPaneState;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "buy", paint);
    pane_state.sync_rows(network_requests, spark_wallet);
    paint_action_button(
        buy_mode_payments_copy_button_bounds(content_bounds),
        "Copy all",
        paint,
    );

    let summary = buy_mode_payments_summary_text(network_requests, spark_wallet);
    paint.scene.draw_text(paint.text.layout_mono(
        &summary,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 32.0,
        ),
        11.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Rows are sourced from buy-mode requests. When wallet evidence exists, rows are matched to Spark payments by wallet pointer, including returned HTLC detail when Spark exposes it.",
        Point::new(content_bounds.origin.x + 12.0, content_bounds.origin.y + 48.0),
        10.0,
        theme::text::MUTED,
    ));

    pane_state.ledger.set_title("");
    pane_state
        .ledger
        .paint(buy_mode_payments_ledger_bounds(content_bounds), paint);
}
