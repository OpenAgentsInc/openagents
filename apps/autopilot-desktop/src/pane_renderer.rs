use crate::app_state::{
    DesktopPane, NostrSecretState, PaneKind, PayInvoicePaneInputs, SparkPaneInputs,
};
use crate::pane_system::{
    PANE_TITLE_HEIGHT, nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds,
    nostr_reveal_button_bounds, pane_content_bounds,
};
use crate::spark_pane;
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
        spark_wallet: &SparkPaneState,
        spark_inputs: &mut SparkPaneInputs,
        pay_invoice_inputs: &mut PayInvoicePaneInputs,
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

fn paint_nostr_identity_pane(
    content_bounds: Bounds,
    nostr_identity: Option<&nostr::NostrIdentity>,
    nostr_identity_error: Option<&str>,
    nostr_secret_state: &NostrSecretState,
    paint: &mut PaintContext,
) {
    let now = std::time::Instant::now();
    let secrets_revealed = nostr_secret_state.is_revealed(now);

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
    }
}

fn paint_spark_wallet_pane(
    content_bounds: Bounds,
    spark_wallet: &SparkPaneState,
    spark_inputs: &mut SparkPaneInputs,
    paint: &mut PaintContext,
) {
    let layout = spark_pane::layout(content_bounds);

    paint_action_button(layout.refresh_button, "Refresh wallet", paint);
    paint_action_button(layout.spark_address_button, "Spark receive", paint);
    paint_action_button(layout.bitcoin_address_button, "Bitcoin receive", paint);
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

fn paint_pay_invoice_pane(
    content_bounds: Bounds,
    spark_wallet: &SparkPaneState,
    pay_invoice_inputs: &mut PayInvoicePaneInputs,
    paint: &mut PaintContext,
) {
    let layout = spark_pane::pay_invoice_layout(content_bounds);
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

fn paint_action_button(bounds: Bounds, label: &str, paint: &mut PaintContext) {
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

fn paint_label_line(paint: &mut PaintContext, x: f32, y: f32, label: &str, value: &str) -> f32 {
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

fn paint_multiline_phrase(
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

fn split_text_for_display(text: &str, chunk_len: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![String::new()];
    }

    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(chunk_len.max(1))
        .map(|chunk| chunk.iter().collect())
        .collect()
}
