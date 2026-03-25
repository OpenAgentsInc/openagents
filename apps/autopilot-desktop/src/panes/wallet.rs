use std::sync::Arc;

use wgpui::clipboard::copy_to_clipboard;
use wgpui::{Bounds, Component, Hsla, InputEvent, MouseButton, PaintContext, Point, Quad, SvgQuad, theme};

use crate::app_state::{
    CreateInvoicePaneInputs, PaneKind, PaneLoadState, PayInvoicePaneInputs, RenderState,
    SparkPaneInputs,
};
use crate::pane_renderer::{
    paint_action_button, paint_source_badge, split_text_for_display,
};
use crate::pane_system::pane_content_bounds;
use crate::spark_pane;
use crate::spark_wallet::SparkPaneState;

const WALLET_COPY_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M480 400L288 400C279.2 400 272 392.8 272 384L272 128C272 119.2 279.2 112 288 112L421.5 112C425.7 112 429.8 113.7 432.8 116.7L491.3 175.2C494.3 178.2 496 182.3 496 186.5L496 384C496 392.8 488.8 400 480 400zM288 448L480 448C515.3 448 544 419.3 544 384L544 186.5C544 169.5 537.3 153.2 525.3 141.2L466.7 82.7C454.7 70.7 438.5 64 421.5 64L288 64C252.7 64 224 92.7 224 128L224 384C224 419.3 252.7 448 288 448zM160 192C124.7 192 96 220.7 96 256L96 512C96 547.3 124.7 576 160 576L352 576C387.3 576 416 547.3 416 512L416 496L368 496L368 512C368 520.8 360.8 528 352 528L160 528C151.2 528 144 520.8 144 512L144 256C144 247.2 151.2 240 160 240L176 240L176 192L160 192z"/></svg>"##;

#[derive(Clone, Default)]
struct WalletCopyButtons {
    send_request: Bounds,
    spark_address: Option<Bounds>,
    bitcoin_address: Option<Bounds>,
    last_invoice: Option<Bounds>,
    last_action: Option<Bounds>,
}

pub fn paint_wallet_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    spark_inputs: &mut SparkPaneInputs,
    details_scroll_offset: f32,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "wallet", paint);
    paint
        .scene
        .draw_quad(Quad::new(content_bounds).with_background(wallet_panel_bg()));

    let layout = spark_pane::layout(content_bounds);
    let controls_panel = Bounds::new(
        content_bounds.origin.x + 6.0,
        content_bounds.origin.y + 6.0,
        (content_bounds.size.width - 12.0).max(0.0),
        (layout.details_origin.y - content_bounds.origin.y - 12.0).max(0.0),
    );
    paint_wallet_section_panel(controls_panel, "CONTROL", wallet_cyan(), paint);

    let details_clip = wallet_details_scroll_bounds(content_bounds);
    paint_wallet_section_panel(details_clip, "DETAILS", wallet_orange(), paint);

    let state = spark_wallet_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => wallet_cyan(),
        PaneLoadState::Error => theme::status::ERROR,
    };

    paint_wallet_button(layout.refresh_button, "Refresh", wallet_cyan(), paint);
    paint_wallet_button(layout.spark_address_button, "Spark receive", wallet_green(), paint);
    paint_wallet_button(layout.bitcoin_address_button, "BTC receive", wallet_green(), paint);
    paint_wallet_button_disabled(layout.create_wallet_button, "Create New Wallet", paint);
    paint_wallet_button(
        layout.use_identity_path_button,
        "Use wallet file",
        wallet_orange(),
        paint,
    );
    paint_wallet_button(
        layout.use_default_identity_button,
        "Use default",
        wallet_orange(),
        paint,
    );
    paint_wallet_button(
        layout.use_mnemonic_phrase_button,
        "Use mnemonic",
        wallet_orange(),
        paint,
    );
    paint_wallet_button(
        layout.create_invoice_button,
        "Create Lightning",
        wallet_cyan(),
        paint,
    );
    paint_wallet_button(layout.send_payment_button, "Send payment", wallet_cyan(), paint);

    if spark_inputs.identity_path.get_value().trim().is_empty()
        && let Some(path) = spark_wallet.identity_path.as_ref()
    {
        spark_inputs
            .identity_path
            .set_value(path.display().to_string());
    }

    spark_inputs
        .identity_path
        .set_max_width(layout.identity_path_input.size.width);
    spark_inputs
        .identity_path
        .paint(layout.identity_path_input, paint);
    spark_inputs
        .mnemonic_phrase
        .set_max_width(layout.mnemonic_phrase_input.size.width);
    spark_inputs
        .mnemonic_phrase
        .paint(layout.mnemonic_phrase_input, paint);

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
        "Wallet seed file",
        Point::new(
            layout.identity_path_input.origin.x,
            layout.identity_path_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Lightning invoice sats",
        Point::new(
            layout.invoice_amount_input.origin.x,
            layout.invoice_amount_input.origin.y - 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout(
        "Mnemonic (12/24 words)",
        Point::new(
            layout.mnemonic_phrase_input.origin.x,
            layout.mnemonic_phrase_input.origin.y - 12.0,
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

    let copy_buttons = wallet_copy_button_layout(content_bounds, spark_wallet, details_scroll_offset);
    paint.scene.push_clip(details_clip);
    let mut y = details_clip.origin.y + 22.0 - details_scroll_offset.max(0.0);
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", state.label()),
        Point::new(details_clip.origin.x + 10.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for first refresh to hydrate balance and payment history.",
            Point::new(details_clip.origin.x + 10.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Network",
        spark_wallet.network_name(),
    );
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Connection",
        spark_wallet.network_status_label(),
    );

    let now_epoch_seconds = crate::app_state::current_reference_epoch_seconds();
    let pending_delta_sats = crate::spark_wallet::pending_wallet_delta_sats(
        &spark_wallet.recent_payments,
        now_epoch_seconds,
    );
    let pending_line = if spark_wallet.balance.is_some() {
        crate::spark_wallet::format_wallet_delta_sats(pending_delta_sats)
    } else {
        "LOADING".to_string()
    };

    let (spark_sats, lightning_sats, onchain_sats, balance_sats) =
        if let Some(balance) = spark_wallet.balance.as_ref() {
            let total_sats = balance.total_sats();
            let settled_estimate = (i128::from(total_sats) - i128::from(pending_delta_sats))
                .clamp(0, i128::from(u64::MAX)) as u64;
            (
                balance.spark_sats.to_string(),
                balance.lightning_sats.to_string(),
                balance.onchain_sats.to_string(),
                settled_estimate.to_string(),
            )
        } else {
            (
                "LOADING".to_string(),
                "LOADING".to_string(),
                "LOADING".to_string(),
                "LOADING".to_string(),
            )
        };
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Spark sats",
        spark_sats.as_str(),
    );
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Lightning sats",
        lightning_sats.as_str(),
    );
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Onchain sats",
        onchain_sats.as_str(),
    );
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Balance",
        balance_sats.as_str(),
    );
    y = paint_label_line(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Pending",
        pending_line.as_str(),
    );

    if let Some(path) = spark_wallet.identity_path.as_ref() {
        y = paint_multiline_phrase(
            paint,
            details_clip.origin.x + 10.0,
            y,
            "Identity path",
            &path.display().to_string(),
        );
    }
    y = paint_multiline_phrase(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Custody",
        "Pasted mnemonics are used in-session and are not written to disk.",
    );
    y = paint_multiline_phrase(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Wallet profiles",
        &wallet_profile_summary(),
    );
    y = paint_multiline_phrase(
        paint,
        details_clip.origin.x + 10.0,
        y,
        "Spark wallets",
        &wallet_bucket_summary(spark_wallet.network_name()),
    );
    if let Some(address) = spark_wallet.spark_address.as_deref() {
        y = paint_multiline_phrase(
            paint,
            details_clip.origin.x + 10.0,
            y,
            "Spark address",
            address,
        );
        if let Some(bounds) = copy_buttons.spark_address {
            paint_wallet_copy_icon_button(bounds, wallet_green(), paint);
        }
    }
    if let Some(address) = spark_wallet.bitcoin_address.as_deref() {
        y = paint_multiline_phrase(
            paint,
            details_clip.origin.x + 10.0,
            y,
            "Bitcoin address",
            address,
        );
        if let Some(bounds) = copy_buttons.bitcoin_address {
            paint_wallet_copy_icon_button(bounds, wallet_green(), paint);
        }
    }
    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        y = paint_multiline_phrase(
            paint,
            details_clip.origin.x + 10.0,
            y,
            "Last Lightning invoice",
            invoice,
        );
        if let Some(bounds) = copy_buttons.last_invoice {
            paint_wallet_copy_icon_button(bounds, wallet_cyan(), paint);
        }
    }
    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        y = paint_label_line(
            paint,
            details_clip.origin.x + 10.0,
            y,
            "Last payment id",
            payment_id,
        );
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_label_line(
            paint,
            details_clip.origin.x + 10.0,
            y,
            "Last action",
            last_action,
        );
        if let Some(bounds) = copy_buttons.last_action {
            paint_wallet_copy_icon_button(bounds, wallet_cyan(), paint);
        }
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(details_clip.origin.x + 10.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(details_clip.origin.x + 10.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }

    if !spark_wallet.recent_payments.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Recent payments",
            Point::new(details_clip.origin.x + 10.0, y),
            11.0,
            theme::text::MUTED,
        ));
        y += 16.0;

        for payment in spark_wallet.recent_payments.iter().take(6) {
            let line = format!(
                "{} {} {} [{}]",
                payment.direction,
                payment.status,
                crate::spark_wallet::wallet_payment_amount_summary(payment),
                payment.id
            );
            paint.scene.draw_text(paint.text.layout_mono(
                &line,
                Point::new(details_clip.origin.x + 10.0, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
    }
    paint.scene.pop_clip();
    paint_wallet_copy_icon_button(copy_buttons.send_request, wallet_cyan(), paint);

    let content_height = (y + details_scroll_offset) - (details_clip.origin.y + 8.0);
    let max_scroll = (content_height - details_clip.size.height).max(0.0);
    if max_scroll > 0.0 {
        let track = Bounds::new(details_clip.max_x() - 4.0, details_clip.origin.y, 3.0, details_clip.size.height);
        paint.scene.draw_quad(
            Quad::new(track).with_background(wallet_border().with_alpha(0.35)),
        );
        let thumb_height = (details_clip.size.height * (details_clip.size.height / content_height))
            .clamp(20.0, details_clip.size.height);
        let progress = (details_scroll_offset / max_scroll).clamp(0.0, 1.0);
        let thumb_y = details_clip.origin.y + progress * (details_clip.size.height - thumb_height);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(track.origin.x, thumb_y, track.size.width, thumb_height))
                .with_background(wallet_border().with_alpha(0.85)),
        );
    }
}

fn wallet_panel_bg() -> Hsla {
    Hsla::from_hex(0x060B11)
}

fn wallet_border() -> Hsla {
    Hsla::from_hex(0x1B3245)
}

fn wallet_cyan() -> Hsla {
    Hsla::from_hex(0x35D4E6)
}

fn wallet_green() -> Hsla {
    Hsla::from_hex(0x43C97F)
}

fn wallet_orange() -> Hsla {
    Hsla::from_hex(0xF39B45)
}

fn wallet_profile_summary() -> String {
    let default_path = match nostr::identity_mnemonic_path() {
        Ok(path) => path,
        Err(_) => return "Unable to resolve default seed profile path.".to_string(),
    };
    let Some(parent) = default_path.parent() else {
        return "No wallet profile directory found.".to_string();
    };
    let mut names = vec!["identity.mnemonic [default]".to_string()];
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || path == default_path {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && name.ends_with(".mnemonic")
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.join("  •  ")
}

fn wallet_bucket_summary(network: &str) -> String {
    let Ok(home) = std::env::var("HOME") else {
        return "HOME unavailable".to_string();
    };
    let root = std::path::Path::new(&home)
        .join(".openagents")
        .join("pylon")
        .join("spark")
        .join(network);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return format!("No spark buckets found at {}", root.display());
    };
    let mut rows = Vec::<String>::new();
    for entry in entries.flatten() {
        let bucket_path = entry.path();
        if !bucket_path.is_dir() {
            continue;
        }
        let Some(bucket) = bucket_path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let storage_path = bucket_path.join("storage.sql");
        if !storage_path.exists() {
            rows.push(format!("{bucket}: no storage"));
            continue;
        }
        let balance = read_bucket_cached_balance(storage_path.as_path())
            .map(|value| format!("{value} sats"))
            .unwrap_or_else(|| "balance unknown".to_string());
        rows.push(format!("{bucket}: {balance}"));
    }
    if rows.is_empty() {
        return format!("No spark buckets found at {}", root.display());
    }
    rows.sort();
    rows.join("  •  ")
}

fn read_bucket_cached_balance(storage_path: &std::path::Path) -> Option<u64> {
    let connection = rusqlite::Connection::open_with_flags(
        storage_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    let settings_value = connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'account_info' LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()?;
    let parsed: serde_json::Value = serde_json::from_str(settings_value.as_str()).ok()?;
    parsed
        .get("balance_sats")
        .and_then(serde_json::Value::as_u64)
}

fn paint_wallet_section_panel(bounds: Bounds, label: &str, accent: Hsla, paint: &mut PaintContext) {
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return;
    }
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x0A121B).with_alpha(0.92))
            .with_border(wallet_border().with_alpha(0.55), 0.6)
            .with_corner_radius(2.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y, 4.0, bounds.size.height))
            .with_background(accent.with_alpha(0.9)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 6.0),
        9.0,
        accent,
    ));
    paint_wallet_edge_trim(bounds, accent, paint);
}

fn paint_wallet_button(bounds: Bounds, label: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x0E1823).with_alpha(0.85))
            .with_border(wallet_border().with_alpha(0.55), 0.6)
            .with_corner_radius(2.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height))
            .with_background(accent.with_alpha(0.9)),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 8.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint_wallet_edge_trim(bounds, accent, paint);
}

fn paint_wallet_button_disabled(bounds: Bounds, label: &str, paint: &mut PaintContext) {
    let muted = theme::text::MUTED.with_alpha(0.7);
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x0B121A).with_alpha(0.7))
            .with_border(wallet_border().with_alpha(0.35), 0.6)
            .with_corner_radius(2.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.origin.x, bounds.origin.y, 3.0, bounds.size.height))
            .with_background(Hsla::from_hex(0x36495A).with_alpha(0.5)),
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 8.0),
        10.0,
        muted,
    ));
    paint_wallet_edge_trim(bounds, Hsla::from_hex(0x36495A), paint);
}

fn paint_wallet_copy_icon_button(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x0E1823).with_alpha(0.88))
            .with_border(wallet_border().with_alpha(0.62), 0.7)
            .with_corner_radius(2.0),
    );
    paint.scene.draw_svg(
        SvgQuad::new(
            Bounds::new(
                bounds.origin.x + 4.0,
                bounds.origin.y + 4.0,
                (bounds.size.width - 8.0).max(8.0),
                (bounds.size.height - 8.0).max(8.0),
            ),
            Arc::<[u8]>::from(WALLET_COPY_ICON_SVG_RAW.as_bytes()),
        )
        .with_tint(accent.with_alpha(0.9)),
    );
}

fn paint_wallet_edge_trim(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    if bounds.size.width <= 1.0 || bounds.size.height <= 1.0 {
        return;
    }
    let edge = accent.with_alpha(0.36);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 4.0,
            bounds.origin.y,
            bounds.size.width - 4.0,
            1.0,
        ))
        .with_background(accent.with_alpha(0.52)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(bounds.max_x() - 1.0, bounds.origin.y, 1.0, bounds.size.height))
            .with_background(edge),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 4.0,
            bounds.max_y() - 1.0,
            bounds.size.width - 4.0,
            1.0,
        ))
        .with_background(edge),
    );
}

pub fn wallet_details_scroll_bounds(content_bounds: Bounds) -> Bounds {
    let layout = spark_pane::layout(content_bounds);
    Bounds::new(
        content_bounds.origin.x + 6.0,
        (layout.details_origin.y - 4.0).max(content_bounds.origin.y + 6.0),
        (content_bounds.size.width - 12.0).max(0.0),
        (content_bounds.max_y() - layout.details_origin.y - 8.0).max(40.0),
    )
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

    paint_action_button(
        layout.create_invoice_button,
        "Create Lightning invoice",
        paint,
    );
    paint_action_button(layout.copy_invoice_button, "Copy Lightning invoice", paint);

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
        "Lightning invoice sats",
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
            "No Lightning invoice generated yet. Submit amount, description, and expiry to create one.",
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
            "Generated Lightning invoice",
            invoice,
        );
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Lightning invoice QR",
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
    let copy_buttons = wallet_copy_button_layout(content_bounds, &state.spark_wallet, state.spark_wallet_scroll_offset);
    let mut handled = false;

    handled |= state
        .spark_inputs
        .identity_path
        .event(event, layout.identity_path_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .mnemonic_phrase
        .event(event, layout.mnemonic_phrase_input, &mut state.event_context)
        .is_handled();
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

    if let InputEvent::MouseUp { button, x, y } = event
        && *button == MouseButton::Left
    {
        let point = Point::new(*x, *y);
        if copy_buttons.send_request.contains(point) {
            handled |= copy_wallet_value(
                state,
                state.spark_inputs.send_request.get_value().to_string(),
                "Copied invoice/request field",
                "No invoice/request value to copy",
            );
        } else if copy_buttons
            .spark_address
            .is_some_and(|bounds| bounds.contains(point))
        {
            handled |= copy_wallet_value(
                state,
                state.spark_wallet.spark_address.clone().unwrap_or_default(),
                "Copied Spark address",
                "No Spark address to copy",
            );
        } else if copy_buttons
            .bitcoin_address
            .is_some_and(|bounds| bounds.contains(point))
        {
            handled |= copy_wallet_value(
                state,
                state.spark_wallet.bitcoin_address.clone().unwrap_or_default(),
                "Copied Bitcoin address",
                "No Bitcoin address to copy",
            );
        } else if copy_buttons
            .last_invoice
            .is_some_and(|bounds| bounds.contains(point))
        {
            handled |= copy_wallet_value(
                state,
                state.spark_wallet.last_invoice.clone().unwrap_or_default(),
                "Copied last Lightning invoice",
                "No last Lightning invoice to copy",
            );
        } else if copy_buttons
            .last_action
            .is_some_and(|bounds| bounds.contains(point))
        {
            handled |= copy_wallet_value(
                state,
                state.spark_wallet.last_action.clone().unwrap_or_default(),
                "Copied last action",
                "No last action to copy",
            );
        }
    }

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

fn copy_wallet_value(
    state: &mut RenderState,
    value: String,
    success: &str,
    empty: &str,
) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        state.spark_wallet.last_error = Some(empty.to_string());
        return true;
    }
    match copy_to_clipboard(trimmed) {
        Ok(()) => {
            state.spark_wallet.last_error = None;
            state.spark_wallet.last_action = Some(success.to_string());
        }
        Err(error) => {
            state.spark_wallet.last_error = Some(format!("Failed to copy value: {error}"));
        }
    }
    true
}

fn wallet_copy_button_layout(
    content_bounds: Bounds,
    spark_wallet: &SparkPaneState,
    details_scroll_offset: f32,
) -> WalletCopyButtons {
    let layout = spark_pane::layout(content_bounds);
    let details_clip = wallet_details_scroll_bounds(content_bounds);
    let button_size = 16.0;
    let right_x = details_clip.max_x() - button_size - 8.0;
    let button_y_for_row = |row_y: f32| row_y + 1.0;

    let mut y = details_clip.origin.y + 22.0 - details_scroll_offset.max(0.0);
    y += 16.0;
    if spark_wallet_view_state(spark_wallet) == PaneLoadState::Loading {
        y += 16.0;
    }
    y += 18.0; // Network
    y += 18.0; // Connection
    y += 18.0; // Spark sats
    y += 18.0; // Lightning sats
    y += 18.0; // Onchain sats
    y += 18.0; // Balance
    y += 18.0; // Pending

    if let Some(path) = spark_wallet.identity_path.as_ref() {
        y += wallet_phrase_line_count(path.display().to_string().as_str()) as f32 * 18.0;
    }
    y += wallet_phrase_line_count("Pasted mnemonics are used in-session and are not written to disk.") as f32
        * 18.0;
    y += wallet_phrase_line_count(wallet_profile_summary().as_str()) as f32 * 18.0;
    y += wallet_phrase_line_count(wallet_bucket_summary(spark_wallet.network_name()).as_str()) as f32 * 18.0;

    let spark_address = spark_wallet.spark_address.as_ref().map(|address| {
        let bounds = Bounds::new(right_x, button_y_for_row(y), button_size, button_size);
        y += wallet_phrase_line_count(address.as_str()) as f32 * 18.0;
        bounds
    });
    let bitcoin_address = spark_wallet.bitcoin_address.as_ref().map(|address| {
        let bounds = Bounds::new(right_x, button_y_for_row(y), button_size, button_size);
        y += wallet_phrase_line_count(address.as_str()) as f32 * 18.0;
        bounds
    });
    let last_invoice = spark_wallet.last_invoice.as_ref().map(|invoice| {
        let bounds = Bounds::new(right_x, button_y_for_row(y), button_size, button_size);
        y += wallet_phrase_line_count(invoice.as_str()) as f32 * 18.0;
        bounds
    });

    if spark_wallet.last_payment_id.is_some() {
        y += 18.0;
    }
    let last_action = spark_wallet
        .last_action
        .as_ref()
        .map(|_| Bounds::new(right_x, button_y_for_row(y), button_size, button_size));

    WalletCopyButtons {
        send_request: layout.send_request_copy_button,
        spark_address,
        bitcoin_address,
        last_invoice,
        last_action,
    }
}

fn wallet_phrase_line_count(value: &str) -> usize {
    split_text_for_display(value, 62).len().max(1)
}

fn wallet_value_x_offset(label: &str) -> f32 {
    (label.chars().count() as f32 * 6.4 + 30.0).clamp(140.0, 160.0)
}

fn paint_label_line(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
) -> f32 {
    let value_x = x + wallet_value_x_offset(label);
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(value_x, y),
        theme::font_size::SM,
        theme::text::PRIMARY,
    ));
    y + 18.0
}

fn paint_multiline_phrase(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
) -> f32 {
    let value_x = x + wallet_value_x_offset(label);
    paint.scene.draw_text(paint.text.layout(
        &format!("{label}:"),
        Point::new(x, y),
        theme::font_size::SM,
        theme::text::MUTED,
    ));
    let mut line_y = y;
    for chunk in split_text_for_display(value, 72) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(value_x, line_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        ));
        line_y += 18.0;
    }
    line_y
}
