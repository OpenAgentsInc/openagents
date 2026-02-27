use wgpui::PaintContext;

use crate::app_state::{CreditDeskPaneState, CreditSettlementLedgerPaneState};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    credit_desk_envelope_button_bounds, credit_desk_intent_button_bounds,
    credit_desk_offer_button_bounds, credit_desk_spend_button_bounds,
    credit_settlement_default_button_bounds, credit_settlement_reputation_button_bounds,
    credit_settlement_verify_button_bounds,
};

pub fn paint_credit_desk_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CreditDeskPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let intent = credit_desk_intent_button_bounds(content_bounds);
    let offer = credit_desk_offer_button_bounds(content_bounds);
    let envelope = credit_desk_envelope_button_bounds(content_bounds);
    let spend = credit_desk_spend_button_bounds(content_bounds);

    paint_action_button(intent, "Intent", paint);
    paint_action_button(offer, "Offer", paint);
    paint_action_button(envelope, "Envelope", paint);
    paint_action_button(spend, "Authorize Spend", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        intent.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Scope",
        &pane_state.scope,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Requested sats",
        &pane_state.requested_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Offered sats",
        &pane_state.offered_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Envelope cap",
        &pane_state.envelope_cap_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Spend sats",
        &pane_state.spend_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Spend job id",
        &pane_state.spend_job_id,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39240 intent",
        pane_state.intent_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39241 offer",
        pane_state.offer_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39242 envelope",
        pane_state.envelope_event_id.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39243 spend",
        pane_state.spend_event_id.as_deref().unwrap_or("n/a"),
    );
}

pub fn paint_credit_settlement_ledger_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CreditSettlementLedgerPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let verify = credit_settlement_verify_button_bounds(content_bounds);
    let default_notice = credit_settlement_default_button_bounds(content_bounds);
    let reputation = credit_settlement_reputation_button_bounds(content_bounds);

    paint_action_button(verify, "Verify Settlement", paint);
    paint_action_button(default_notice, "Emit Default", paint);
    paint_action_button(reputation, "Emit Reputation", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        verify.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Result event",
        &pane_state.result_event_id,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Payment pointer",
        &pane_state.payment_pointer,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Default reason",
        &pane_state.default_reason,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39244 settlement",
        pane_state.settlement_event_id.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "39245 default",
        pane_state.default_event_id.as_deref().unwrap_or("n/a"),
    );
}
