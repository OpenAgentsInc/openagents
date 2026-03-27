use wgpui::{Bounds, Component, Hsla, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    CreateInvoicePaneInputs, PaneKind, PaneLoadState, PayInvoicePaneInputs, RenderState,
    SparkPaneInputs,
};
use crate::pane_renderer::{
    app_text_style, mission_control_muted_color, mission_control_panel_header_color,
    paint_primary_button, paint_secondary_button, paint_source_badge,
    split_text_for_display,
};
use crate::pane_system::pane_content_bounds;
use crate::spark_pane;
use crate::spark_wallet::SparkPaneState;
use crate::ui_style::AppTextRole;

const WALLET_PANEL_RADIUS: f32 = 3.0;
const WALLET_CARD_RADIUS: f32 = 3.0;

pub fn wallet_details_scroll_bounds(content_bounds: Bounds) -> Bounds {
    spark_pane::scroll_viewport_bounds(content_bounds)
}

pub fn paint_wallet_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    spark_inputs: &mut SparkPaneInputs,
    details_scroll_offset: f32,
    paint: &mut PaintContext,
) {
    let scroll_content_bounds = spark_pane::scroll_content_bounds(content_bounds);
    let viewport = spark_pane::scroll_viewport_bounds(content_bounds);
    let content_height = spark_wallet_content_height(scroll_content_bounds, spark_wallet);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = details_scroll_offset.clamp(0.0, max_scroll);
    let layout = spark_pane::layout_with_scroll(scroll_content_bounds, scroll_offset);
    let state = spark_wallet_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let now_epoch_seconds = crate::app_state::current_reference_epoch_seconds();
    let pending_delta_sats = spark_wallet.pending_wallet_delta_sats(now_epoch_seconds);
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

    paint.scene.push_clip(viewport);
    paint_wallet_overview(
        layout.overview_bounds,
        state,
        state_color,
        balance_sats.as_str(),
        pending_line.as_str(),
        spark_wallet.network_name(),
        spark_wallet.network_status_label(),
        paint,
    );

    let label_style = app_text_style(AppTextRole::FormLabel);
    paint_wallet_utility_section(
        layout.utility_section_bounds,
        "Wallet utilities",
        "Refresh state or copy your Spark address.",
        paint,
    );
    paint_secondary_button(layout.refresh_button, "Refresh wallet", paint);
    paint_secondary_button(layout.copy_spark_address_button, "Copy Spark", paint);

    paint_wallet_flow_section(
        layout.receive_section_bounds,
        "Receive",
        "Create or refresh addresses, then generate a Lightning invoice.",
        paint,
    );
    paint_secondary_button(layout.spark_address_button, "Spark receive", paint);
    paint_secondary_button(layout.bitcoin_address_button, "Bitcoin receive", paint);
    paint_primary_button(layout.create_invoice_button, "Create Lightning", paint);

    paint_wallet_flow_section(
        layout.send_section_bounds,
        "Send",
        "Paste a request or invoice, then optionally set the sats to send.",
        paint,
    );
    paint_primary_button(layout.send_payment_button, "Send payment", paint);
    let details_section_bounds = Bounds::new(
        layout.details_section_bounds.origin.x,
        layout.details_section_bounds.origin.y,
        layout.details_section_bounds.size.width,
        wallet_supporting_details_height(
            layout.details_section_bounds.size.width,
            spark_wallet,
            state,
        ),
    );
    paint_wallet_supporting_section(
        details_section_bounds,
        "Supporting details",
        "Balances, addresses, and recent wallet activity.",
        paint,
    );

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

    paint_wallet_input_label(
        paint,
        layout.invoice_amount_input,
        "Lightning invoice sats",
        label_style,
    );
    paint_wallet_input_label(
        paint,
        layout.send_request_input,
        "Send request / invoice",
        label_style,
    );
    paint_wallet_input_label(
        paint,
        layout.send_amount_input,
        "Send sats (optional)",
        label_style,
    );

    let mut y = layout.details_origin.y;
    let row_x = details_section_bounds.origin.x + 12.0;
    let row_width = (details_section_bounds.size.width - 24.0).max(180.0);
    let row_chunk_len = (((row_width - 130.0).max(120.0)) / 6.2).floor() as usize;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for first refresh to hydrate balance and payment history.",
            Point::new(row_x, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    y = paint_wallet_supporting_heading(row_x, y, "Balances", paint);
    y = paint_wallet_data_row(
        paint,
        row_x,
        y,
        "Spark sats",
        spark_sats.as_str(),
        row_chunk_len,
        row_width,
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y = paint_wallet_data_row(
        paint,
        row_x,
        y,
        "Lightning sats",
        lightning_sats.as_str(),
        row_chunk_len,
        row_width,
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y = paint_wallet_data_row(
        paint,
        row_x,
        y,
        "Onchain sats",
        onchain_sats.as_str(),
        row_chunk_len,
        row_width,
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y = paint_wallet_data_row(
        paint,
        row_x,
        y,
        "Balance",
        balance_sats.as_str(),
        row_chunk_len,
        row_width,
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y = paint_wallet_data_row(
        paint,
        row_x,
        y,
        "Pending",
        pending_line.as_str(),
        row_chunk_len,
        row_width,
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y += 8.0;
    y = paint_wallet_supporting_heading(row_x, y, "Wallet details", paint);

    if let Some(path) = spark_wallet.identity_path.as_ref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Identity path",
            &path.display().to_string(),
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }
    if let Some(source) = spark_wallet.wallet_identity_source.as_deref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Identity source",
            source,
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }
    if let Some(fingerprint) = spark_wallet.wallet_fingerprint_label() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Wallet fingerprint",
            fingerprint.as_str(),
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::FormValue).color,
        );
    }
    y = paint_wallet_data_row(
        paint,
        row_x,
        y,
        "Custody",
        "Mnemonic and nsec handling live in the Nostr Identity pane.",
        row_chunk_len,
        row_width,
        app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
        app_text_style(AppTextRole::SecondaryMetadata)
            .color
            .with_alpha(0.82),
    );
    if let Some(address) = spark_wallet.spark_address.as_deref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Spark address",
            address,
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::FormLabel).color,
            app_text_style(AppTextRole::FormValue).color,
        );
    }
    if let Some(address) = spark_wallet.bitcoin_address.as_deref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Bitcoin address",
            address,
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::FormLabel).color,
            app_text_style(AppTextRole::FormValue).color,
        );
    }
    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Last Lightning invoice",
            invoice,
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::FormLabel).color,
            app_text_style(AppTextRole::FormValue).color,
        );
    }
    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Last payment id",
            payment_id,
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_wallet_data_row(
            paint,
            row_x,
            y,
            "Last action",
            last_action,
            row_chunk_len,
            row_width,
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(row_x, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(row_x, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }
    if let Some(error) = spark_wallet.last_operation_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Last wallet operation error:",
            Point::new(row_x, y),
            11.0,
            theme::status::WARNING,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(row_x, y),
                11.0,
                theme::status::WARNING,
            ));
            y += 16.0;
        }
    }
    if let Some(warning) = spark_wallet.wallet_context_warning.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Wallet continuity warning:",
            Point::new(row_x, y),
            11.0,
            theme::status::WARNING,
        ));
        y += 16.0;
        for line in split_text_for_display(warning, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(row_x, y),
                11.0,
                theme::status::WARNING,
            ));
            y += 16.0;
        }
    }

    if !spark_wallet.recent_payments.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Recent payments",
            Point::new(row_x, y),
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
                Point::new(row_x, y),
                10.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
    }
    paint.scene.pop_clip();
    paint_wallet_scrollbar(viewport, content_height, scroll_offset, paint);
}

fn paint_wallet_overview(
    bounds: Bounds,
    state: PaneLoadState,
    state_color: Hsla,
    balance_sats: &str,
    pending: &str,
    network: &str,
    connection: &str,
    paint: &mut PaintContext,
) {
    paint_wallet_panel_shell(bounds, state_color, 0.18, true, paint);
    paint_wallet_panel_heading(bounds, "WALLET OVERVIEW", state_color, true, paint);

    let helper_style = app_text_style(AppTextRole::Helper);
    let section_style = app_text_style(AppTextRole::SectionHeading);
    let value_style = app_text_style(AppTextRole::FormValue);
    let balance_x = bounds.origin.x + 18.0;
    let balance_y = bounds.origin.y + 24.0;
    let status_chip = Bounds::new(balance_x, balance_y - 2.0, 164.0, 24.0);
    paint.scene.draw_quad(
        Quad::new(status_chip)
            .with_background(state_color.with_alpha(0.12))
            .with_border(state_color.with_alpha(0.24), 1.0)
            .with_corner_radius(WALLET_CARD_RADIUS),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("Wallet {}", state.label()),
        Point::new(status_chip.origin.x + 12.0, status_chip.origin.y + 7.0),
        helper_style.font_size,
        state_color.with_alpha(0.92),
    ));
    let status_helper = wallet_status_helper_text(state, network, connection);
    paint.scene.draw_text(paint.text.layout_mono(
        &status_helper,
        Point::new(balance_x, balance_y + 28.0),
        helper_style.font_size,
        helper_style.color.with_alpha(0.70),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        "Total balance",
        Point::new(balance_x, balance_y + 46.0),
        section_style.font_size,
        helper_style.color.with_alpha(0.74),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        balance_sats,
        Point::new(balance_x, balance_y + 70.0),
        (value_style.font_size + 16.0).max(30.0),
        value_style.color,
    ));
    let pending_chip = Bounds::new(balance_x + 118.0, balance_y + 60.0, 122.0, 22.0);
    paint.scene.draw_quad(
        Quad::new(pending_chip)
            .with_background(theme::bg::SURFACE.with_alpha(0.18))
            .with_border(theme::border::DEFAULT.with_alpha(0.10), 1.0)
            .with_corner_radius(WALLET_CARD_RADIUS),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("Pending {pending}"),
        Point::new(pending_chip.origin.x + 10.0, pending_chip.origin.y + 6.0),
        helper_style.font_size,
        helper_style.color.with_alpha(0.76),
    ));

    let card_width = 186.0;
    let card_height = 34.0;
    let right_x = bounds.origin.x + bounds.size.width - card_width - 18.0;
    let network_bounds = Bounds::new(right_x, balance_y + 2.0, card_width, card_height);
    let connection_bounds = Bounds::new(right_x, balance_y + 44.0, card_width, card_height);
    paint_wallet_summary_card(
        network_bounds,
        "Network",
        &network.to_ascii_uppercase(),
        paint,
    );
    paint_wallet_summary_card(connection_bounds, "Connection", connection, paint);
}

fn paint_wallet_utility_section(
    bounds: Bounds,
    title: &str,
    helper: &str,
    paint: &mut PaintContext,
) {
    paint_wallet_panel_shell(bounds, theme::accent::PRIMARY, 0.10, true, paint);
    paint_wallet_panel_heading(bounds, title, theme::accent::PRIMARY, true, paint);
    let helper_style = app_text_style(AppTextRole::Helper);
    let title_x = bounds.origin.x + 15.0;
    let title_y = bounds.origin.y + 34.0;
    paint.scene.draw_text(paint.text.layout_mono(
        helper,
        Point::new(title_x, title_y),
        helper_style.font_size,
        helper_style.color.with_alpha(0.62),
    ));
}

fn paint_wallet_flow_section(bounds: Bounds, title: &str, helper: &str, paint: &mut PaintContext) {
    paint_wallet_panel_shell(bounds, theme::accent::PRIMARY, 0.18, true, paint);
    paint_wallet_panel_heading(bounds, title, theme::accent::PRIMARY, true, paint);

    let helper_style = app_text_style(AppTextRole::Helper);
    let title_x = bounds.origin.x + 15.0;
    let title_y = bounds.origin.y + 34.0;
    paint.scene.draw_text(paint.text.layout_mono(
        helper,
        Point::new(title_x, title_y),
        helper_style.font_size,
        helper_style.color.with_alpha(0.72),
    ));
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            title_x,
            bounds.origin.y + 48.0,
            (bounds.size.width - 30.0).max(0.0),
            1.0,
        ))
        .with_background(theme::border::DEFAULT.with_alpha(0.08)),
    );
}

fn paint_wallet_supporting_section(
    bounds: Bounds,
    title: &str,
    helper: &str,
    paint: &mut PaintContext,
) {
    paint_wallet_panel_shell(bounds, theme::accent::PRIMARY, 0.12, true, paint);
    paint_wallet_panel_heading(bounds, title, theme::accent::PRIMARY, true, paint);
    let helper_style = app_text_style(AppTextRole::Helper);
    let title_x = bounds.origin.x + 15.0;
    let title_y = bounds.origin.y + 34.0;
    paint.scene.draw_text(paint.text.layout_mono(
        helper,
        Point::new(title_x, title_y),
        helper_style.font_size,
        helper_style.color.with_alpha(0.68),
    ));
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            title_x,
            bounds.origin.y + 48.0,
            (bounds.size.width - 24.0).max(0.0),
            1.0,
        ))
        .with_background(theme::border::DEFAULT.with_alpha(0.08)),
    );
}

fn paint_wallet_panel_heading(
    bounds: Bounds,
    title: &str,
    accent: Hsla,
    has_left_rail: bool,
    paint: &mut PaintContext,
) {
    let header_x = if has_left_rail {
        bounds.origin.x + 4.0
    } else {
        bounds.origin.x
    };
    let header_width = if has_left_rail {
        (bounds.size.width - 4.0).max(0.0)
    } else {
        bounds.size.width.max(0.0)
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(header_x, bounds.origin.y, header_width, 26.0))
            .with_background(mission_control_panel_header_color().with_alpha(0.88)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(header_x, bounds.origin.y, header_width, 1.0))
            .with_background(accent.with_alpha(0.22)),
    );
    let marker_style = app_text_style(AppTextRole::SectionHeading);
    let title_style = app_text_style(AppTextRole::SectionHeading);
    let marker_origin = Point::new(bounds.origin.x + 14.0, bounds.origin.y + 8.0);
    let marker = paint
        .text
        .layout_mono("\\\\", marker_origin, marker_style.font_size, accent);
    let marker_width = marker.bounds().size.width;
    let heading = title.to_ascii_uppercase();
    paint.scene.draw_text(marker);
    paint.scene.draw_text(paint.text.layout_mono(
        &heading,
        Point::new(marker_origin.x + marker_width + 6.0, marker_origin.y),
        title_style.font_size,
        title_style.color,
    ));
}

fn paint_wallet_panel_shell(
    bounds: Bounds,
    accent: Hsla,
    background_alpha: f32,
    show_left_rail: bool,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE.with_alpha(background_alpha))
            .with_border(accent.with_alpha(0.52), 1.0)
            .with_corner_radius(WALLET_PANEL_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_border(accent.with_alpha(0.06), 1.0)
            .with_corner_radius(WALLET_PANEL_RADIUS),
    );
    if show_left_rail {
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                4.0,
                bounds.size.height.max(0.0),
            ))
            .with_background(accent.with_alpha(0.82))
            .with_corner_radius(WALLET_PANEL_RADIUS),
        );
    }
}

fn paint_wallet_supporting_heading(x: f32, y: f32, label: &str, paint: &mut PaintContext) -> f32 {
    let style = app_text_style(AppTextRole::SectionHeading);
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(x, y),
        style.font_size,
        style.color.with_alpha(0.72),
    ));
    paint.scene.draw_quad(
        Quad::new(Bounds::new(x, y + 14.0, 220.0, 1.0))
            .with_background(theme::border::DEFAULT.with_alpha(0.08)),
    );
    y + 22.0
}

fn paint_wallet_summary_card(bounds: Bounds, label: &str, value: &str, paint: &mut PaintContext) {
    let label_style = app_text_style(AppTextRole::FormLabel);
    let value_style = app_text_style(AppTextRole::FormValue);
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.14))
            .with_border(theme::border::DEFAULT.with_alpha(0.12), 1.0)
            .with_corner_radius(WALLET_CARD_RADIUS),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 8.0),
        label_style.font_size,
        label_style.color.with_alpha(0.82),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 96.0, bounds.origin.y + 8.0),
        value_style.font_size,
        value_style.color,
    ));
}

fn paint_wallet_status_summary(
    bounds: Bounds,
    label: &str,
    helper: &str,
    color: Hsla,
    paint: &mut PaintContext,
) -> f32 {
    let helper_style = app_text_style(AppTextRole::Helper);
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.14))
            .with_border(theme::border::DEFAULT.with_alpha(0.12), 1.0)
            .with_corner_radius(WALLET_CARD_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 6.0,
            2.0,
            (bounds.size.height - 12.0).max(0.0),
        ))
        .with_background(color.with_alpha(0.86))
        .with_corner_radius(2.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 7.0),
        helper_style.font_size,
        color.with_alpha(0.92),
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        helper,
        Point::new(bounds.origin.x + 110.0, bounds.origin.y + 7.0),
        helper_style.font_size,
        helper_style.color.with_alpha(0.70),
    ));
    bounds.max_y()
}

fn wallet_status_helper_text(state: PaneLoadState, network: &str, connection: &str) -> String {
    match state {
        PaneLoadState::Ready => format!("{connection} on {}", network.to_ascii_uppercase()),
        PaneLoadState::Loading => "Waiting for balance and connection status".to_string(),
        PaneLoadState::Error => "Resolve the wallet issue before transacting".to_string(),
    }
}

fn paint_wallet_input_label(
    paint: &mut PaintContext,
    input_bounds: Bounds,
    label: &str,
    style: crate::ui_style::AppTextStyle,
) {
    paint_wallet_input_label_with_offset(paint, input_bounds, label, style, 0.0);
}

fn paint_wallet_input_label_with_offset(
    paint: &mut PaintContext,
    input_bounds: Bounds,
    label: &str,
    style: crate::ui_style::AppTextStyle,
    y_offset: f32,
) {
    let label_y = input_bounds.origin.y - (style.font_size + 10.0) + y_offset;
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(input_bounds.origin.x, label_y),
        style.font_size,
        style.color.with_alpha(0.82),
    ));
}

fn wallet_supporting_details_height(
    section_width: f32,
    spark_wallet: &SparkPaneState,
    state: PaneLoadState,
) -> f32 {
    let row_width = (section_width - 24.0).max(180.0);
    let row_chunk_len = (((row_width - 130.0).max(120.0)) / 6.2).floor() as usize;
    let mut height = 48.0;
    if state == PaneLoadState::Loading {
        height += 16.0;
    }
    height += 22.0;
    height += wallet_data_row_height(
        &spark_wallet
            .balance
            .as_ref()
            .map_or("LOADING".to_string(), |b| b.spark_sats.to_string()),
        row_chunk_len,
    );
    height += wallet_data_row_height(
        &spark_wallet
            .balance
            .as_ref()
            .map_or("LOADING".to_string(), |b| b.lightning_sats.to_string()),
        row_chunk_len,
    );
    height += wallet_data_row_height(
        &spark_wallet
            .balance
            .as_ref()
            .map_or("LOADING".to_string(), |b| b.onchain_sats.to_string()),
        row_chunk_len,
    );
    let now_epoch_seconds = crate::app_state::current_reference_epoch_seconds();
    let pending_delta_sats = spark_wallet.pending_wallet_delta_sats(now_epoch_seconds);
    let pending_line = if spark_wallet.balance.is_some() {
        crate::spark_wallet::format_wallet_delta_sats(pending_delta_sats)
    } else {
        "LOADING".to_string()
    };
    let balance_sats = if let Some(balance) = spark_wallet.balance.as_ref() {
        let total_sats = balance.total_sats();
        let settled_estimate = (i128::from(total_sats) - i128::from(pending_delta_sats))
            .clamp(0, i128::from(u64::MAX)) as u64;
        settled_estimate.to_string()
    } else {
        "LOADING".to_string()
    };
    height += wallet_data_row_height(&balance_sats, row_chunk_len);
    height += wallet_data_row_height(&pending_line, row_chunk_len);
    height += 8.0 + 22.0;
    if let Some(path) = spark_wallet.identity_path.as_ref() {
        height += wallet_data_row_height(&path.display().to_string(), row_chunk_len);
    }
    if let Some(source) = spark_wallet.wallet_identity_source.as_deref() {
        height += wallet_data_row_height(source, row_chunk_len);
    }
    if let Some(fingerprint) = spark_wallet.wallet_fingerprint_label() {
        height += wallet_data_row_height(fingerprint.as_str(), row_chunk_len);
    }
    height += wallet_data_row_height(
        "Mnemonic and nsec handling live in the Nostr Identity pane.",
        row_chunk_len,
    );
    if let Some(address) = spark_wallet.spark_address.as_deref() {
        height += wallet_data_row_height(address, row_chunk_len);
    }
    if let Some(address) = spark_wallet.bitcoin_address.as_deref() {
        height += wallet_data_row_height(address, row_chunk_len);
    }
    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        height += wallet_data_row_height(invoice, row_chunk_len);
    }
    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        height += wallet_data_row_height(payment_id, row_chunk_len);
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        height += wallet_data_row_height(last_action, row_chunk_len);
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        height += 16.0 + split_text_for_display(error, 88).len() as f32 * 16.0;
    }
    if let Some(error) = spark_wallet.last_operation_error.as_deref() {
        height += 16.0 + split_text_for_display(error, 88).len() as f32 * 16.0;
    }
    if let Some(warning) = spark_wallet.wallet_context_warning.as_deref() {
        height += 16.0 + split_text_for_display(warning, 88).len() as f32 * 16.0;
    }
    if !spark_wallet.recent_payments.is_empty() {
        height += 16.0 + spark_wallet.recent_payments.iter().take(6).count() as f32 * 14.0;
    }
    height + 12.0
}

fn spark_wallet_content_height(content_bounds: Bounds, spark_wallet: &SparkPaneState) -> f32 {
    let layout = spark_pane::layout_with_scroll(content_bounds, 0.0);
    let state = spark_wallet_view_state(spark_wallet);
    let details_height = wallet_supporting_details_height(
        layout.details_section_bounds.size.width,
        spark_wallet,
        state,
    );
    let content_top = content_bounds.origin.y + 12.0;
    let content_bottom = layout.details_section_bounds.origin.y + details_height;
    (content_bottom - content_top).max(0.0)
}

fn wallet_data_row_height(value: &str, value_chunk_len: usize) -> f32 {
    split_text_for_display(value, value_chunk_len.max(1)).len() as f32 * 18.0 + 13.0
}

fn paint_wallet_scrollbar(
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
        viewport.max_x() - 2.0,
        viewport.origin.y,
        2.0,
        viewport.size.height,
    );
    let thumb_height = ((viewport.size.height / content_height) * viewport.size.height)
        .clamp(16.0, viewport.size.height.max(0.0));
    let thumb_y = viewport.origin.y
        + ((scroll_offset / max_offset.max(1.0)) * (viewport.size.height - thumb_height));
    paint.scene.draw_quad(
        Quad::new(track_bounds)
            .with_background(mission_control_panel_header_color().with_alpha(0.45))
            .with_corner_radius(1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            track_bounds.origin.x,
            thumb_y,
            track_bounds.size.width,
            thumb_height,
        ))
        .with_background(mission_control_muted_color().with_alpha(0.72))
        .with_corner_radius(1.0),
    );
}

fn paint_wallet_data_row(
    paint: &mut PaintContext,
    x: f32,
    y: f32,
    label: &str,
    value: &str,
    value_chunk_len: usize,
    row_width: f32,
    label_color: Hsla,
    value_color: Hsla,
) -> f32 {
    let label_style = app_text_style(AppTextRole::FormLabel);
    let value_style = app_text_style(AppTextRole::FormValue);
    let label_column_width = 122.0;
    let mut line_y = y;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("{label}:"),
        Point::new(x, y),
        label_style.font_size,
        label_color,
    ));

    for chunk in split_text_for_display(value, value_chunk_len.max(1)) {
        paint.scene.draw_text(paint.text.layout_mono(
            &chunk,
            Point::new(x + label_column_width, line_y),
            value_style.font_size,
            value_color,
        ));
        line_y += 18.0;
    }
    let divider_y = line_y + 2.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(x, divider_y, row_width.max(0.0), 1.0))
            .with_background(theme::border::DEFAULT.with_alpha(0.08)),
    );
    divider_y + 11.0
}

pub fn paint_create_invoice_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    create_invoice_inputs: &mut CreateInvoicePaneInputs,
    details_scroll_offset: f32,
    paint: &mut PaintContext,
) {
    paint.scene.push_clip(content_bounds);

    let viewport = spark_pane::create_invoice_scroll_viewport_bounds(content_bounds);
    let content_height = create_invoice_content_height(content_bounds, spark_wallet);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = details_scroll_offset.clamp(0.0, max_scroll);
    let layout = spark_pane::create_invoice_layout_with_scroll(content_bounds, scroll_offset);
    let state = create_invoice_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let label_style = app_text_style(AppTextRole::FormLabel);
    let form_section_top = content_bounds.origin.y + 12.0;
    let form_section_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        form_section_top,
        (content_bounds.size.width - 24.0).max(0.0),
        (layout.copy_invoice_button.max_y() - form_section_top + 12.0).max(96.0),
    );
    let details_section_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        form_section_bounds.max_y() + 16.0,
        (content_bounds.size.width - 24.0).max(0.0),
        (content_bounds.max_y() - form_section_bounds.max_y() - 28.0).max(86.0),
    );
    paint_wallet_flow_section(
        form_section_bounds,
        "Receive",
        "Enter invoice details, then generate a Lightning request.",
        paint,
    );
    paint_wallet_supporting_section(
        details_section_bounds,
        "Invoice details",
        "Status, generated invoice output, and recent wallet activity.",
        paint,
    );
    paint.scene.push_clip(viewport);
    paint_primary_button(layout.create_invoice_button, "Create Lightning invoice", paint);
    paint_secondary_button(layout.copy_invoice_button, "Copy Lightning invoice", paint);

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

    paint_wallet_input_label_with_offset(
        paint,
        layout.amount_input,
        "Invoice sats",
        label_style,
        4.0,
    );
    paint_wallet_input_label_with_offset(
        paint,
        layout.expiry_input,
        "Expiry (s)",
        label_style,
        4.0,
    );
    paint_wallet_input_label_with_offset(
        paint,
        layout.description_input,
        "Description (optional)",
        label_style,
        4.0,
    );

    let mut y = details_section_bounds.origin.y + 48.0;
    y = paint_wallet_status_summary(
        Bounds::new(
            content_bounds.origin.x + 24.0,
            y - 2.0,
            (content_bounds.size.width - 48.0).max(160.0),
            34.0,
        ),
        &format!("Invoice {}", state.label()),
        match state {
            PaneLoadState::Ready => "Latest invoice is ready to copy or reuse.",
            PaneLoadState::Loading => "No invoice yet. Submit the form to generate one.",
            PaneLoadState::Error => "Resolve the wallet issue before creating another invoice.",
        },
        state_color,
        paint,
    );
    y += 8.0;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "No Lightning invoice generated yet. Submit amount, description, and expiry to create one.",
            Point::new(content_bounds.origin.x + 24.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }

    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        y = paint_wallet_data_row(
            paint,
            content_bounds.origin.x + 24.0,
            y,
            "Generated invoice",
            invoice,
            84,
            (content_bounds.size.width - 48.0).max(180.0),
            app_text_style(AppTextRole::FormLabel).color,
            app_text_style(AppTextRole::FormValue).color,
        );
        y = paint_wallet_data_row(
            paint,
            content_bounds.origin.x + 24.0,
            y,
            "Lightning invoice QR",
            invoice,
            84,
            (content_bounds.size.width - 48.0).max(180.0),
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }

    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_wallet_data_row(
            paint,
            content_bounds.origin.x + 24.0,
            y,
            "Last action",
            last_action,
            84,
            (content_bounds.size.width - 48.0).max(180.0),
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }

    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(content_bounds.origin.x + 24.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 24.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }
    paint.scene.pop_clip();
    paint_wallet_scrollbar(viewport, content_height, scroll_offset, paint);
    paint.scene.pop_clip();
}

pub fn paint_pay_invoice_pane(
    content_bounds: wgpui::Bounds,
    spark_wallet: &SparkPaneState,
    pay_invoice_inputs: &mut PayInvoicePaneInputs,
    details_scroll_offset: f32,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "wallet", paint);

    let viewport = spark_pane::pay_invoice_scroll_viewport_bounds(content_bounds);
    let content_height = pay_invoice_content_height(content_bounds, spark_wallet);
    let max_scroll = (content_height - viewport.size.height).max(0.0);
    let scroll_offset = details_scroll_offset.clamp(0.0, max_scroll);
    let layout = spark_pane::pay_invoice_layout_with_scroll(content_bounds, scroll_offset);
    let state = pay_invoice_view_state(spark_wallet);
    let state_color = match state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let label_style = app_text_style(AppTextRole::FormLabel);
    let form_section_top = content_bounds.origin.y + 12.0;
    let form_section_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        form_section_top,
        (content_bounds.size.width - 24.0).max(0.0),
        (layout.send_payment_button.max_y() - form_section_top + 12.0).max(84.0),
    );
    let details_section_bounds = Bounds::new(
        content_bounds.origin.x + 12.0,
        form_section_bounds.max_y() + 16.0,
        (content_bounds.size.width - 24.0).max(0.0),
        (content_bounds.max_y() - form_section_bounds.max_y() - 28.0).max(86.0),
    );
    paint_wallet_flow_section(
        form_section_bounds,
        "Send",
        "Paste a request, then confirm the sats to pay.",
        paint,
    );
    paint_wallet_supporting_section(
        details_section_bounds,
        "Payment details",
        "Connection status, payment outcome, and recent wallet activity.",
        paint,
    );
    paint.scene.push_clip(viewport);
    paint_primary_button(layout.send_payment_button, "Pay invoice", paint);

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

    paint_wallet_input_label(
        paint,
        layout.payment_request_input,
        "Lightning invoice / payment request",
        label_style,
    );
    paint_wallet_input_label(
        paint,
        layout.amount_input,
        "Send sats (optional)",
        label_style,
    );

    let mut y = layout.details_origin.y;
    y = paint_wallet_status_summary(
        Bounds::new(
            content_bounds.origin.x + 24.0,
            y - 2.0,
            (content_bounds.size.width - 48.0).max(160.0),
            34.0,
        ),
        &format!("Payment {}", state.label()),
        match state {
            PaneLoadState::Ready => "Wallet is connected and ready to send a payment.",
            PaneLoadState::Loading => "Waiting for wallet connection and payment state.",
            PaneLoadState::Error => "Resolve the wallet issue before attempting payment.",
        },
        state_color,
        paint,
    );
    y += 8.0;
    if state == PaneLoadState::Loading {
        paint.scene.draw_text(paint.text.layout(
            "Waiting for wallet connection and first payment lifecycle update.",
            Point::new(content_bounds.origin.x + 24.0, y),
            10.0,
            theme::text::MUTED,
        ));
        y += 16.0;
    }
    y = paint_wallet_data_row(
        paint,
        content_bounds.origin.x + 24.0,
        y,
        "Network",
        spark_wallet.network_name(),
        84,
        (content_bounds.size.width - 48.0).max(180.0),
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y = paint_wallet_data_row(
        paint,
        content_bounds.origin.x + 24.0,
        y,
        "Connection",
        spark_wallet.network_status_label(),
        84,
        (content_bounds.size.width - 48.0).max(180.0),
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );
    y = paint_wallet_data_row(
        paint,
        content_bounds.origin.x + 24.0,
        y,
        "Payment status",
        payment_terminal_status(spark_wallet),
        84,
        (content_bounds.size.width - 48.0).max(180.0),
        app_text_style(AppTextRole::FormLabel).color,
        app_text_style(AppTextRole::FormValue).color,
    );

    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        y = paint_wallet_data_row(
            paint,
            content_bounds.origin.x + 24.0,
            y,
            "Last payment id",
            payment_id,
            84,
            (content_bounds.size.width - 48.0).max(180.0),
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }

    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        y = paint_wallet_data_row(
            paint,
            content_bounds.origin.x + 24.0,
            y,
            "Last action",
            last_action,
            84,
            (content_bounds.size.width - 48.0).max(180.0),
            app_text_style(AppTextRole::Helper).color.with_alpha(0.72),
            app_text_style(AppTextRole::SecondaryMetadata)
                .color
                .with_alpha(0.82),
        );
    }

    if let Some(error) = spark_wallet.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Error:",
            Point::new(content_bounds.origin.x + 24.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for line in split_text_for_display(error, 88) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 24.0, y),
                11.0,
                theme::status::ERROR,
            ));
            y += 16.0;
        }
    }
    paint.scene.pop_clip();
    paint_wallet_scrollbar(viewport, content_height, scroll_offset, paint);
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
    let layout = spark_pane::layout_with_scroll(
        spark_pane::scroll_content_bounds(content_bounds),
        state.spark_wallet_scroll_offset,
    );
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
    let layout = spark_pane::create_invoice_layout_with_scroll(
        content_bounds,
        state.spark_wallet_pane.scroll_offset(),
    );
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
    let layout = spark_pane::pay_invoice_layout_with_scroll(
        content_bounds,
        state.spark_wallet_pane.scroll_offset(),
    );
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

fn create_invoice_content_height(content_bounds: Bounds, spark_wallet: &SparkPaneState) -> f32 {
    let layout = spark_pane::create_invoice_layout_with_scroll(content_bounds, 0.0);
    let mut height = (layout.details_origin.y - content_bounds.origin.y).max(0.0);
    let row_chunk_len = 84usize;

    height += 34.0 + 8.0;
    if spark_wallet.last_invoice.is_none() {
        height += 16.0;
    }
    if let Some(invoice) = spark_wallet.last_invoice.as_deref() {
        height += wallet_data_row_height(invoice, row_chunk_len);
        height += wallet_data_row_height(invoice, row_chunk_len);
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        height += wallet_data_row_height(last_action, row_chunk_len);
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        height += 16.0 + split_text_for_display(error, 88).len() as f32 * 16.0;
    }
    height + 16.0
}

fn pay_invoice_content_height(content_bounds: Bounds, spark_wallet: &SparkPaneState) -> f32 {
    let layout = spark_pane::pay_invoice_layout_with_scroll(content_bounds, 0.0);
    let mut height = (layout.details_origin.y - content_bounds.origin.y).max(0.0);
    let row_width = (content_bounds.size.width - 48.0).max(180.0);
    let row_chunk_len = ((row_width - 122.0) / 6.1).max(18.0) as usize;

    height += 34.0 + 8.0;
    if pay_invoice_view_state(spark_wallet) == PaneLoadState::Loading {
        height += 16.0;
    }
    height += wallet_data_row_height(spark_wallet.network_name(), row_chunk_len);
    height += wallet_data_row_height(spark_wallet.network_status_label(), row_chunk_len);
    height += wallet_data_row_height(payment_terminal_status(spark_wallet), row_chunk_len);
    if let Some(payment_id) = spark_wallet.last_payment_id.as_deref() {
        height += wallet_data_row_height(payment_id, row_chunk_len);
    }
    if let Some(last_action) = spark_wallet.last_action.as_deref() {
        height += wallet_data_row_height(last_action, row_chunk_len);
    }
    if let Some(error) = spark_wallet.last_error.as_deref() {
        height += 16.0 + split_text_for_display(error, 88).len() as f32 * 16.0;
    }
    height + 16.0
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
