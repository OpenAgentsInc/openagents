use wgpui::{Bounds, Component, PaintContext, Point, theme};

use crate::app_state::{BuyModePaymentsPaneState, NetworkRequestsState};
use crate::pane_renderer::paint_source_badge;
use crate::pane_system::buy_mode_payments_ledger_bounds;
use crate::spark_wallet::SparkPaneState;
use crate::state::operations::NetworkRequestStatus;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut BuyModePaymentsPaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "buy", paint);
    pane_state.sync_rows(network_requests, spark_wallet);

    let requests = network_requests
        .submitted
        .iter()
        .filter(|request| {
            request.request_type == crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE
        })
        .filter(|request| {
            request.payment_required_at_epoch_seconds.is_some()
                || request.payment_sent_at_epoch_seconds.is_some()
                || request.payment_failed_at_epoch_seconds.is_some()
                || request.last_payment_pointer.is_some()
                || request.pending_bolt11.is_some()
                || request
                    .payment_error
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
        })
        .collect::<Vec<_>>();

    let mut paid = 0usize;
    let mut pending = 0usize;
    let mut failed = 0usize;
    let mut sats_sent = 0u64;
    for request in &requests {
        let wallet_amount = request
            .last_payment_pointer
            .as_deref()
            .and_then(|payment_id| {
                spark_wallet
                    .recent_payments
                    .iter()
                    .find(|payment| payment.id == payment_id)
                    .map(|payment| payment.amount_sats)
            });
        if request.last_payment_pointer.is_some() || request.payment_sent_at_epoch_seconds.is_some()
        {
            paid = paid.saturating_add(1);
            sats_sent = sats_sent.saturating_add(wallet_amount.unwrap_or(request.budget_sats));
        } else if request.status == NetworkRequestStatus::Failed
            || request.payment_failed_at_epoch_seconds.is_some()
            || request.payment_error.is_some()
        {
            failed = failed.saturating_add(1);
        } else {
            pending = pending.saturating_add(1);
        }
    }

    let summary = format!(
        "{} rows  //  {} sent  //  {} pending  //  {} failed  //  {} sats",
        requests.len(),
        paid,
        pending,
        failed,
        sats_sent
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &summary,
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 24.0,
        ),
        11.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Rows are sourced from buy-mode requests and matched to Spark payments when a wallet pointer is available.",
        Point::new(content_bounds.origin.x + 12.0, content_bounds.origin.y + 40.0),
        10.0,
        theme::text::MUTED,
    ));

    pane_state.ledger.set_title("");
    pane_state
        .ledger
        .paint(buy_mode_payments_ledger_bounds(content_bounds), paint);
}
