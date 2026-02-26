use wgpui::{Component, InputEvent, PaintContext, Point, theme};

use crate::app_state::{
    CreateInvoicePaneInputs, PaneKind, PaneLoadState, PayInvoicePaneInputs, RenderState,
    SparkPaneInputs,
};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    split_text_for_display,
};
use crate::pane_system::pane_content_bounds;
use crate::spark_pane::{self, CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkPaneState;

pub fn paint_wallet_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    spark_inputs: &mut SparkPaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "wallet", paint);

    let layout = spark_pane::layout(content_bounds);
    let state = spark_wallet_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };

    paint_action_button(layout.refresh_button, "Refresh wallet", paint);
    paint_action_button(layout.spark_address_button, "Spark receive", paint);
    paint_action_button(layout.bitcoin_address_button, "Bitcoin receive", paint);
    paint_action_button(layout.copy_spark_address_button, "Copy Spark", paint);
    paint_action_button(layout.create_invoice_button, "Create invoice", paint);
    paint_action_button(layout.send_payment_button, "Send payment", paint);

    spark_inputs
        .invoice_amount
        .set_max_width(layout.invoice_amount_input.size.width);
    spark_inputs
        .send_request
        .set_max_width(layout.send_request_input.size.width);
    spark_inputs
        .send_amount
        .set_max_width(layout.send_amount_input.size.width);

    spark_inputs
        .invoice_amount
        .paint(layout.invoice_amount_input, paint);
    spark_inputs
        .send_request
        .paint(layout.send_request_input, paint);
    spark_inputs
        .send_amount
        .paint(layout.send_amount_input, paint);

    paint.scene.draw_text(paint.text.layout(
        "Invoice sats",
        Point::new(
            layout.invoice_amount_input.origin.x,
            layout.invoice_amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Send request / invoice",
        Point::new(
            layout.send_request_input.origin.x,
            layout.send_request_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Send sats (optional)",
        Point::new(
            layout.send_amount_input.origin.x,
            layout.send_amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let mut y = layout.details_origin.y;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for first refresh to hydrate balance and payment history.",
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Network",
        spark_wallet.network_name(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Connection",
        spark_wallet.network_status_label(),
    );

    let (spark_sats, lightning_sats, onchain_sats, total_sats) =
        if let Some(balance) = spark_wallet.balance.as_ref() {
            (
                balance.spark_sats,
                balance.lightning_sats,
                balance.onchain_sats,
                balance.total_sats(),
            )
        } else {
            (0, 0, 0, 0)
        };
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Spark sats",
        &spark_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Lightning sats",
        &lightning_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Onchain sats",
        &onchain_sats.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Total sats",
        &total_sats.to_string(),
    );

    if let Some(path) = spark_wallet.identity_path.as_ref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Identity path",
            &path.display().to_string(),
        );
    }
    if let Some(address) = spark_wallet.spark_address.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Spark address",
            address,
        );
    }
    if let Some(address) = spark_wallet.bitcoin_address.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Bitcoin address",
            address,
        );
    }
    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last invoice",
            invoice,
        );
    }
    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last payment id",
            payment_id,
        );
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last action",
            last_action,
        );
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 12.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }

    if !spark_wallet.recent_payments.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Recent payments",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        y += 16.0;

        for payment in spark_wallet.recent_payments.iter().take(6) {
            let line = format!(
                "{} {} {} sats [{}]",
                payment.direction, payment.status, payment.amount_sats, payment.id
            );
            paint.scene.draw_text(paint.text.layout_mono(
                &line,
                Point::new(content_bounds.origin.x + 12.0, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
    }
}

pub fn paint_create_invoice_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    create_invoice_inputs: &mut CreateInvoicePaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "wallet", paint);

    let layout = spark_pane::create_invoice_layout(content_bounds);
    let state = create_invoice_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };

    paint_action_button(layout.create_invoice_button, "Create invoice", paint);
    paint_action_button(layout.copy_invoice_button, "Copy invoice", paint);

    create_invoice_inputs
        .amount_sats
        .set_max_width(layout.amount_input.size.width);
    create_invoice_inputs
        .description
        .set_max_width(layout.description_input.size.width);
    create_invoice_inputs
        .expiry_seconds
        .set_max_width(layout.expiry_input.size.width);

    create_invoice_inputs
        .amount_sats
        .paint(layout.amount_input, paint);
    create_invoice_inputs
        .description
        .paint(layout.description_input, paint);
    create_invoice_inputs
        .expiry_seconds
        .paint(layout.expiry_input, paint);

    paint.scene.draw_text(paint.text.layout(
        "Invoice sats",
        Point::new(
            layout.amount_input.origin.x,
            layout.amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Expiry (seconds)",
        Point::new(
            layout.expiry_input.origin.x,
            layout.expiry_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Description (optional)",
        Point::new(
            layout.description_input.origin.x,
            layout.description_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let mut y = layout.details_origin.y;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "No invoice generated yet. Submit amount/description/expiry to create one.",
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }

    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Generated invoice",
            invoice,
        );
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "QR payload",
            invoice,
        );
    }

    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last action",
            last_action,
        );
    }

    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 12.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }
}

pub fn paint_pay_invoice_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    pay_invoice_inputs: &mut PayInvoicePaneInputs,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "wallet", paint);

    let layout = spark_pane::pay_invoice_layout(content_bounds);
    let state = pay_invoice_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    paint_action_button(layout.send_payment_button, "Pay invoice", paint);

    pay_invoice_inputs
        .payment_request
        .set_max_width(layout.payment_request_input.size.width);
    pay_invoice_inputs
        .amount_sats
        .set_max_width(layout.amount_input.size.width);

    pay_invoice_inputs
        .payment_request
        .paint(layout.payment_request_input, paint);
    pay_invoice_inputs
        .amount_sats
        .paint(layout.amount_input, paint);

    paint.scene.draw_text(paint.text.layout(
        "Lightning invoice / payment request",
        Point::new(
            layout.payment_request_input.origin.x,
            layout.payment_request_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Send sats (optional)",
        Point::new(
            layout.amount_input.origin.x,
            layout.amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));

    let mut y = layout.details_origin.y;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for wallet connection and first payment lifecycle update.",
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Network",
        spark_wallet.network_name(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Connection",
        spark_wallet.network_status_label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Payment status",
        payment_terminal_status(spark_wallet),
    );

    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last payment id",
            payment_id,
        );
    }

    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last action",
            last_action,
        );
    }

    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 12.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }
}

pub fn topmost_spark_action_hit_in_order(
    state: &RenderState,
    point: Point,
    pane_order: &[usize],
) -> Option<(u64, SparkPaneAction)> {
    for pane_idx in pane_order {
        let pane_idx = *pane_idx;
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkWallet {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::layout(content_bounds);
        if let Some(action) = spark_pane::hit_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn topmost_create_invoice_action_hit_in_order(
    state: &RenderState,
    point: Point,
    pane_order: &[usize],
) -> Option<(u64, CreateInvoicePaneAction)> {
    for pane_idx in pane_order {
        let pane_idx = *pane_idx;
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkCreateInvoice {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::create_invoice_layout(content_bounds);
        if let Some(action) = spark_pane::hit_create_invoice_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn topmost_pay_invoice_action_hit_in_order(
    state: &RenderState,
    point: Point,
    pane_order: &[usize],
) -> Option<(u64, PayInvoicePaneAction)> {
    for pane_idx in pane_order {
        let pane_idx = *pane_idx;
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkPayInvoice {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::pay_invoice_layout(content_bounds);
        if let Some(action) = spark_pane::hit_pay_invoice_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn dispatch_spark_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_spark = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkWallet)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_spark else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::layout(content_bounds);
    let mut handled = false;

    handled |= state
        .spark_inputs
        .invoice_amount
        .event(event, layout.invoice_amount_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .send_request
        .event(event, layout.send_request_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .send_amount
        .event(event, layout.send_amount_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn dispatch_create_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_create_invoice = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkCreateInvoice)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_create_invoice else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::create_invoice_layout(content_bounds);
    let mut handled = false;

    handled |= state
        .create_invoice_inputs
        .amount_sats
        .event(event, layout.amount_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .create_invoice_inputs
        .description
        .event(event, layout.description_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .create_invoice_inputs
        .expiry_seconds
        .event(event, layout.expiry_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn dispatch_pay_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pay_invoice = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkPayInvoice)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pay_invoice else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::pay_invoice_layout(content_bounds);
    let mut handled = false;

    handled |= state
        .pay_invoice_inputs
        .payment_request
        .event(
            event,
            layout.payment_request_input,
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .pay_invoice_inputs
        .amount_sats
        .event(event, layout.amount_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn spark_wallet_view_state(spark_wallet: &SparkPaneState) -> PaneLoadState {
    if spark_wallet.last_error.is_some() {
        return PaneLoadState::Error;
    }

    if spark_wallet.network_status.is_none() || spark_wallet.balance.is_none() {
        return PaneLoadState::Loading;
    }

    PaneLoadState::Ready
}

pub fn create_invoice_view_state(spark_wallet: &SparkPaneState) -> PaneLoadState {
    if spark_wallet.last_error.is_some() {
        return PaneLoadState::Error;
    }
    if spark_wallet.last_invoice.is_none() {
        return PaneLoadState::Loading;
    }
    PaneLoadState::Ready
}

pub fn pay_invoice_view_state(spark_wallet: &SparkPaneState) -> PaneLoadState {
    if spark_wallet.last_error.is_some() {
        return PaneLoadState::Error;
    }

    if spark_wallet.network_status.is_none() {
        return PaneLoadState::Loading;
    }

    PaneLoadState::Ready
}

pub fn payment_terminal_status(spark_wallet: &SparkPaneState) -> &str {
    if spark_wallet.last_error.is_some() {
        return "failed";
    }
    if spark_wallet
        .last_action
        .as_deref()
        .is_some_and(|action| action.starts_with("Payment sent"))
    {
        return "sent";
    }
    "idle"
}
