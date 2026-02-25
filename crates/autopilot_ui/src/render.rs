use super::*;

pub(super) fn paint_chat_pane(chat: &mut ChatPaneState, bounds: Bounds, cx: &mut PaintContext) {
    let padding_x = 24.0;
    let padding_top = 4.0;
    let padding_bottom = 16.0;
    let input_height = input_bar_height(chat, bounds.size.width, cx);

    let mut engine = LayoutEngine::new();
    let content_node = engine.request_layout(&flex_1(v_flex()), &[]);
    let input_node =
        engine.request_leaf(&LayoutStyle::new().height(px(input_height)).flex_shrink(0.0));
    let root = engine.request_layout(
        &v_flex()
            .width(px(bounds.size.width))
            .height(px(bounds.size.height)),
        &[content_node, input_node],
    );
    engine.compute_layout(root, Size::new(bounds.size.width, bounds.size.height));

    let content_bounds = offset_bounds(engine.layout(content_node), bounds.origin);
    let input_bounds = offset_bounds(engine.layout(input_node), bounds.origin);

    let content_inner = Bounds::new(
        content_bounds.origin.x + padding_x,
        content_bounds.origin.y + padding_top,
        (content_bounds.size.width - padding_x * 2.0).max(0.0),
        (content_bounds.size.height - padding_top - padding_bottom).max(0.0),
    );

    let mut dropdown_bounds = Bounds::ZERO;
    let mut queue_bounds = Bounds::ZERO;
    let mut queue_block: Option<Text> = None;

    let selector_height = 30.0;
    let mut queue_height = 0.0;
    if !chat.current_queue().is_empty() {
        let mut queue_text = String::from("Queued");
        for item in chat.current_queue() {
            if item.text.trim().is_empty() {
                continue;
            }
            queue_text.push('\n');
            queue_text.push_str("- ");
            queue_text.push_str(item.text.trim());
        }
        let mut queue_block_local = Text::new(queue_text)
            .font_size(theme::font_size::XS)
            .color(theme::text::SECONDARY);
        let (_, measured_height) = queue_block_local.size_hint_with_width(content_inner.size.width);
        queue_height = measured_height.unwrap_or(0.0);
        queue_block = Some(queue_block_local);
    }

    let mut items = Vec::new();
    let mut dropdown_index = None;
    let mut queue_index = None;

    if SHOW_MODEL_DROPDOWN {
        dropdown_index = Some(items.len());
        items.push(ColumnItem::Fixed(selector_height));
        items.push(ColumnItem::Fixed(6.0));
    }
    if queue_height > 0.0 {
        queue_index = Some(items.len());
        items.push(ColumnItem::Fixed(queue_height));
        items.push(ColumnItem::Fixed(12.0));
    }
    items.push(ColumnItem::Fixed(14.0));
    let feed_index = items.len();
    items.push(ColumnItem::Flex(1.0));

    let content_items = column_bounds(content_inner, &items, 0.0);
    if let Some(index) = dropdown_index {
        dropdown_bounds = *content_items.get(index).unwrap_or(&Bounds::ZERO);
    }
    if let Some(index) = queue_index {
        queue_bounds = *content_items.get(index).unwrap_or(&Bounds::ZERO);
    }
    let feed_bounds = *content_items.get(feed_index).unwrap_or(&Bounds::ZERO);

    if SHOW_MODEL_DROPDOWN {
        let selector_bounds = dropdown_bounds;

        let label_text = "Model";
        let label_width =
            cx.text
                .measure_styled_mono(label_text, theme::font_size::SM, FontStyle::default());
        let available_width = (content_inner.size.width - label_width - 8.0).max(140.0);
        let dropdown_width = available_width.min(MODEL_DROPDOWN_WIDTH);

        let mut engine = LayoutEngine::new();
        let label_node = engine.request_leaf(
            &LayoutStyle::new()
                .width(px(label_width))
                .height(px(selector_height)),
        );
        let dropdown_node = engine.request_leaf(
            &LayoutStyle::new()
                .width(px(dropdown_width))
                .height(px(selector_height)),
        );
        let row = engine.request_layout(
            &gap(
                h_flex()
                    .align_items(AlignItems::Center)
                    .justify_content(JustifyContent::FlexStart),
                8.0,
            ),
            &[label_node, dropdown_node],
        );
        engine.compute_layout(row, Size::new(content_inner.size.width, selector_height));

        let label_bounds = offset_bounds(engine.layout(label_node), selector_bounds.origin);
        dropdown_bounds = offset_bounds(engine.layout(dropdown_node), selector_bounds.origin);

        Text::new(label_text)
            .font_size(theme::font_size::SM)
            .color(theme::text::MUTED)
            .paint(label_bounds, cx);

        chat.model_bounds = dropdown_bounds;
    } else {
        chat.model_bounds = Bounds::ZERO;
    }

    if let Some(mut block) = queue_block {
        block.paint(queue_bounds, cx);
    }

    chat.formatted_thread_bounds = feed_bounds;

    if chat.formatted_thread.entry_count() != 0 {
        chat.formatted_thread.paint(feed_bounds, cx);
    }

    // Paint dropdown last so the menu overlays the rest of the feed.
    if SHOW_MODEL_DROPDOWN {
        chat.model_dropdown.paint(dropdown_bounds, cx);
    }

    paint_input_bar(chat, input_bounds, cx);
}

pub(super) fn metrics_buttons_width(
    queue_width: f32,
    full_auto_width: f32,
    send_width: f32,
    stop_width: f32,
    show_stop: bool,
    gap: f32,
) -> f32 {
    let mut width = queue_width + full_auto_width + send_width + gap * 2.0;
    if show_stop {
        width += stop_width + gap;
    }
    width
}

pub(super) struct InputBarMetrics {
    input_height: f32,
    row_height: f32,
    total_height: f32,
    model_width: f32,
    reasoning_width: f32,
    queue_width: f32,
    full_auto_width: f32,
    send_width: f32,
    stop_width: f32,
    show_stop: bool,
}

pub(super) fn input_bar_metrics(
    chat: &mut ChatPaneState,
    available_width: f32,
    cx: &mut PaintContext,
) -> InputBarMetrics {
    let gap = 8.0;
    let padding_x = 24.0;
    let padding_y = theme::spacing::MD;
    let button_font = theme::font_size::SM;
    let send_label_width = cx
        .text
        .measure_styled_mono("Send", button_font, FontStyle::default());
    let send_width = (send_label_width + 28.0).max(72.0);
    let stop_label_width = cx
        .text
        .measure_styled_mono("Stop", button_font, FontStyle::default());
    let stop_width = (stop_label_width + 28.0).max(72.0);
    let full_auto_label = if chat.full_auto_enabled {
        "Full Auto On"
    } else {
        "Full Auto Off"
    };
    let full_auto_width =
        cx.text
            .measure_styled_mono(full_auto_label, button_font, FontStyle::default())
            + 28.0;
    let full_auto_width = full_auto_width.max(110.0);
    let queue_label_width = cx
        .text
        .measure_styled_mono("Queue", button_font, FontStyle::default())
        .max(
            cx.text
                .measure_styled_mono("Instant", button_font, FontStyle::default()),
        );
    let queue_width = (queue_label_width + 28.0).max(90.0);
    let show_stop = chat.is_processing();

    let mut total_width = (available_width - padding_x * 2.0).max(240.0);
    total_width = total_width.min(available_width - padding_x * 2.0);
    let buttons_width = metrics_buttons_width(
        queue_width,
        full_auto_width,
        send_width,
        stop_width,
        show_stop,
        gap,
    );
    let min_left = REASONING_DROPDOWN_MIN_WIDTH + 180.0 + gap;
    let left_available = (total_width - buttons_width - gap).max(min_left);
    let mut model_width = (left_available * 0.6).min(MODEL_DROPDOWN_WIDTH).max(180.0);
    let mut reasoning_width = (left_available - model_width - gap)
        .clamp(REASONING_DROPDOWN_MIN_WIDTH, REASONING_DROPDOWN_MAX_WIDTH);
    if model_width + reasoning_width + gap > left_available {
        let overflow = model_width + reasoning_width + gap - left_available;
        if reasoning_width - overflow >= REASONING_DROPDOWN_MIN_WIDTH {
            reasoning_width -= overflow;
        } else if model_width - overflow >= 180.0 {
            model_width -= overflow;
        }
    }

    chat.input.set_max_width(total_width);
    let line_height = button_font * 1.4;
    let input_padding_y = theme::spacing::XS;
    let min_height = line_height * INPUT_MIN_LINES as f32 + input_padding_y * 2.0;
    let mut input_height = chat.input.current_height().max(min_height);
    if let Some(max_lines) = INPUT_MAX_LINES {
        let max_height = line_height * max_lines as f32 + input_padding_y * 2.0;
        input_height = input_height.min(max_height);
    }

    let row_height = 28.0;
    let total_height = input_height + row_height + gap + padding_y * 2.0;

    InputBarMetrics {
        input_height,
        row_height,
        total_height,
        model_width,
        reasoning_width,
        queue_width,
        full_auto_width,
        send_width,
        stop_width,
        show_stop,
    }
}

pub(super) fn input_bar_height(
    chat: &mut ChatPaneState,
    available_width: f32,
    cx: &mut PaintContext,
) -> f32 {
    let metrics = input_bar_metrics(chat, available_width, cx);
    metrics.total_height.max(BOTTOM_BAR_MIN_HEIGHT)
}

pub(super) fn paint_input_bar(chat: &mut ChatPaneState, bounds: Bounds, cx: &mut PaintContext) {
    let gap = 8.0;
    let padding_y = theme::spacing::MD;
    let padding_x = 24.0;
    let metrics = input_bar_metrics(chat, bounds.size.width, cx);

    let content_bounds = Bounds::new(
        bounds.origin.x + padding_x,
        bounds.origin.y + padding_y,
        (bounds.size.width - padding_x * 2.0).max(0.0),
        (bounds.size.height - padding_y * 2.0).max(0.0),
    );

    let mut engine = LayoutEngine::new();
    let input_node =
        engine.request_leaf(&v_flex().height(px(metrics.input_height)).flex_shrink(0.0));
    let row_node = engine.request_leaf(&v_flex().height(px(metrics.row_height)).flex_shrink(0.0));
    let column_style = v_flex()
        .gap(length(gap))
        .justify_content(JustifyContent::FlexEnd)
        .width(px(content_bounds.size.width))
        .height(px(content_bounds.size.height));
    let column = engine.request_layout(&column_style, &[input_node, row_node]);
    engine.compute_layout(
        column,
        Size::new(content_bounds.size.width, content_bounds.size.height),
    );

    let input_bounds = offset_bounds(engine.layout(input_node), content_bounds.origin);
    let row_bounds = offset_bounds(engine.layout(row_node), content_bounds.origin);

    let mut row_items = vec![
        wgpui::RowItem::fixed(metrics.model_width),
        wgpui::RowItem::fixed(metrics.reasoning_width),
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(metrics.queue_width),
        wgpui::RowItem::fixed(metrics.full_auto_width),
    ];
    if metrics.show_stop {
        row_items.push(wgpui::RowItem::fixed(metrics.stop_width));
    }
    row_items.push(wgpui::RowItem::fixed(metrics.send_width));

    let row_item_bounds = aligned_row_bounds(
        row_bounds,
        metrics.row_height,
        &row_items,
        gap,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );

    let mut idx = 0;
    let model_bounds = *row_item_bounds.get(idx).unwrap_or(&row_bounds);
    idx += 1;
    let reasoning_bounds = *row_item_bounds.get(idx).unwrap_or(&row_bounds);
    idx += 1;
    idx += 1; // spacer
    let queue_bounds = *row_item_bounds.get(idx).unwrap_or(&row_bounds);
    idx += 1;
    let full_auto_bounds = *row_item_bounds.get(idx).unwrap_or(&row_bounds);
    idx += 1;
    let stop_bounds = if metrics.show_stop {
        let bounds = *row_item_bounds.get(idx).unwrap_or(&row_bounds);
        idx += 1;
        bounds
    } else {
        Bounds::ZERO
    };
    let submit_bounds = *row_item_bounds.get(idx).unwrap_or(&row_bounds);

    chat.input_bounds = input_bounds;
    chat.submit_bounds = submit_bounds;
    chat.stop_bounds = stop_bounds;
    chat.full_auto_bounds = full_auto_bounds;
    chat.queue_toggle_bounds = queue_bounds;
    chat.model_bounds = model_bounds;
    chat.reasoning_bounds = reasoning_bounds;
    chat.input.set_max_width(input_bounds.size.width);
    chat.submit_button
        .set_disabled(chat.input.get_value().trim().is_empty());
    chat.stop_button.set_disabled(!metrics.show_stop);
    let full_auto_label = if chat.full_auto_enabled {
        "Full Auto On"
    } else {
        "Full Auto Off"
    };
    chat.full_auto_button.set_label(full_auto_label);
    chat.full_auto_button
        .set_variant(if chat.full_auto_enabled {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        });
    let queue_label = if chat.queue_mode { "Queue" } else { "Instant" };
    chat.queue_toggle_button.set_label(queue_label);
    chat.queue_toggle_button.set_variant(if chat.queue_mode {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    });
    chat.full_auto_button.set_disabled(chat.thread_id.is_none());
    if chat.input_needs_focus {
        chat.input.focus();
        chat.input_needs_focus = false;
    }

    chat.input.paint(input_bounds, cx);
    chat.model_dropdown.paint(model_bounds, cx);
    chat.reasoning_dropdown.paint(reasoning_bounds, cx);
    chat.queue_toggle_button.paint(queue_bounds, cx);
    chat.full_auto_button.paint(full_auto_bounds, cx);
    if metrics.show_stop {
        chat.stop_button.paint(stop_bounds, cx);
    }
    chat.submit_button.paint(submit_bounds, cx);
}

pub(super) fn paint_auth_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let input_height = 30.0;
    let row_height = 32.0;
    let label_height = 16.0;
    let text_size = theme::font_size::XS;
    let gap = 8.0;

    let mut content_width = (bounds.size.width * 0.86).min(760.0).max(340.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);

    root.runtime_auth_send_button
        .set_disabled(root.runtime_auth_email_input.get_value().trim().is_empty());
    root.runtime_auth_verify_button
        .set_disabled(root.runtime_auth_code_input.get_value().trim().is_empty());

    root.runtime_auth_email_bounds = Bounds::ZERO;
    root.runtime_auth_code_bounds = Bounds::ZERO;
    root.runtime_auth_send_bounds = Bounds::ZERO;
    root.runtime_auth_verify_bounds = Bounds::ZERO;
    root.runtime_auth_status_bounds = Bounds::ZERO;
    root.runtime_auth_logout_bounds = Bounds::ZERO;

    enum AuthStep {
        Label(String),
        InputEmail,
        InputCode,
        Spacer(f32),
        ButtonRow,
        Heading(String),
        Line(String, wgpui::color::Hsla, f32),
    }

    let mut steps = vec![
        AuthStep::Label("Email".to_string()),
        AuthStep::Spacer(4.0),
        AuthStep::InputEmail,
        AuthStep::Spacer(gap),
        AuthStep::Label("Verification code".to_string()),
        AuthStep::Spacer(4.0),
        AuthStep::InputCode,
        AuthStep::Spacer(10.0),
        AuthStep::ButtonRow,
        AuthStep::Spacer(12.0),
        AuthStep::Heading("Status".to_string()),
        AuthStep::Spacer(6.0),
    ];

    let mut push_wrapped_line = |text: String, color: wgpui::color::Hsla| {
        let mut line = Text::new(text.as_str()).font_size(text_size);
        let (_, measured_height) = line.size_hint_with_width(content_width);
        let height = measured_height.unwrap_or(label_height).max(label_height);
        steps.push(AuthStep::Line(text, color, height));
        steps.push(AuthStep::Spacer(6.0));
    };

    let token_line = if root.runtime_auth.token_present {
        "Token: present".to_string()
    } else {
        "Token: missing".to_string()
    };
    push_wrapped_line(token_line, theme::text::PRIMARY);
    push_wrapped_line(
        format!(
            "Base URL: {}",
            root.runtime_auth
                .base_url
                .as_deref()
                .unwrap_or("<not configured>")
        ),
        theme::text::MUTED,
    );
    if let Some(email) = root.runtime_auth.email.as_deref() {
        push_wrapped_line(format!("Email: {}", email), theme::text::MUTED);
    }
    if let Some(user_id) = root.runtime_auth.user_id.as_deref() {
        push_wrapped_line(format!("User ID: {}", user_id), theme::text::MUTED);
    }
    if let Some(pending_email) = root.runtime_auth.pending_email.as_deref() {
        push_wrapped_line(
            format!("Pending verification: {}", pending_email),
            theme::text::MUTED,
        );
    }
    if let Some(updated_at) = root.runtime_auth.updated_at.as_deref() {
        push_wrapped_line(format!("Updated: {}", updated_at), theme::text::MUTED);
    }
    if let Some(message) = root.runtime_auth.last_message.as_deref() {
        push_wrapped_line(message.to_string(), theme::text::PRIMARY);
    }
    if let Some(error) = root.runtime_auth.last_error.as_deref() {
        push_wrapped_line(error.to_string(), theme::accent::RED);
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            AuthStep::Label(_) => ColumnItem::Fixed(label_height),
            AuthStep::InputEmail | AuthStep::InputCode => ColumnItem::Fixed(input_height),
            AuthStep::Spacer(height) => ColumnItem::Fixed(*height),
            AuthStep::ButtonRow => ColumnItem::Fixed(row_height),
            AuthStep::Heading(_) => ColumnItem::Fixed(label_height),
            AuthStep::Line(_, _, height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            AuthStep::Label(text) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(theme::text::MUTED)
                    .paint(bounds, cx);
            }
            AuthStep::InputEmail => {
                root.runtime_auth_email_bounds = bounds;
                root.runtime_auth_email_input.paint(bounds, cx);
            }
            AuthStep::InputCode => {
                root.runtime_auth_code_bounds = bounds;
                root.runtime_auth_code_input.paint(bounds, cx);
            }
            AuthStep::ButtonRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    row_height,
                    &[
                        wgpui::RowItem::fixed(108.0),
                        wgpui::RowItem::fixed(90.0),
                        wgpui::RowItem::fixed(96.0),
                        wgpui::RowItem::fixed(92.0),
                    ],
                    8.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                let send_bounds = *row_bounds.get(0).unwrap_or(&bounds);
                let verify_bounds = *row_bounds.get(1).unwrap_or(&bounds);
                let status_bounds = *row_bounds.get(2).unwrap_or(&bounds);
                let logout_bounds = *row_bounds.get(3).unwrap_or(&bounds);
                root.runtime_auth_send_bounds = send_bounds;
                root.runtime_auth_verify_bounds = verify_bounds;
                root.runtime_auth_status_bounds = status_bounds;
                root.runtime_auth_logout_bounds = logout_bounds;
                root.runtime_auth_send_button.paint(send_bounds, cx);
                root.runtime_auth_verify_button.paint(verify_bounds, cx);
                root.runtime_auth_status_button.paint(status_bounds, cx);
                root.runtime_auth_logout_button.paint(logout_bounds, cx);
            }
            AuthStep::Heading(text) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(theme::text::PRIMARY)
                    .paint(bounds, cx);
            }
            AuthStep::Line(text, color, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(color)
                    .paint(bounds, cx);
            }
            AuthStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_identity_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 30.0;
    let input_height = 30.0;
    let text_size = theme::font_size::XS;
    let value_text_size = theme::font_size::XS + 4.0;
    let label_height = 16.0;
    let row_gap = 10.0;
    let copy_button_width = 70.0;

    let mut content_width = (bounds.size.width * 0.85).min(760.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);

    root.identity_private_key_bounds = Bounds::ZERO;
    root.identity_load_bounds = Bounds::ZERO;
    root.keygen_bounds = Bounds::ZERO;
    root.identity_copy_npub_bounds = Bounds::ZERO;
    root.identity_copy_nsec_bounds = Bounds::ZERO;

    root.identity_load_button.set_disabled(
        root.identity_private_key_input
            .get_value()
            .trim()
            .is_empty(),
    );
    root.identity_copy_npub_button
        .set_disabled(root.nostr_npub.is_none());
    root.identity_copy_nsec_button
        .set_disabled(root.nostr_nsec.is_none());

    fn text_height(text: &str, font_size: f32, width: f32, fallback: f32) -> f32 {
        let mut value = Text::new(text).font_size(font_size);
        let (_, height) = value.size_hint_with_width(width);
        height.unwrap_or(fallback)
    }

    #[derive(Clone, Copy)]
    enum IdentityCopyField {
        Npub,
        Nsec,
    }

    enum IdentityStep {
        Intro(String, f32),
        InputLabel,
        Input,
        ActionRow,
        Label(String, Option<IdentityCopyField>),
        Value(String, wgpui::color::Hsla, f32),
        Spacer(f32),
    }

    let mut steps = Vec::new();
    let intro = "Generate a new NIP-06 identity or load an existing Nostr private key.";
    steps.push(IdentityStep::Intro(
        intro.to_string(),
        text_height(intro, text_size, content_width, label_height),
    ));
    steps.push(IdentityStep::Spacer(8.0));
    steps.push(IdentityStep::InputLabel);
    steps.push(IdentityStep::Input);
    steps.push(IdentityStep::ActionRow);
    steps.push(IdentityStep::Spacer(8.0));

    if let Some(err) = root.nostr_error.as_deref() {
        let err_height = text_height(err, text_size, content_width, label_height);
        steps.push(IdentityStep::Value(
            err.to_string(),
            theme::status::ERROR,
            err_height,
        ));
        steps.push(IdentityStep::Spacer(8.0));
    }

    if let Some(npub) = &root.nostr_npub {
        let npub_height = text_height(npub, value_text_size, content_width, label_height);
        steps.push(IdentityStep::Label(
            "nostr public key".to_string(),
            Some(IdentityCopyField::Npub),
        ));
        steps.push(IdentityStep::Value(
            npub.clone(),
            theme::text::PRIMARY,
            npub_height,
        ));
        steps.push(IdentityStep::Spacer(row_gap));

        let nsec_value = root.nostr_nsec.as_deref().unwrap_or("").to_string();
        let nsec_height = text_height(&nsec_value, value_text_size, content_width, label_height);
        steps.push(IdentityStep::Label(
            "nostr secret key".to_string(),
            Some(IdentityCopyField::Nsec),
        ));
        steps.push(IdentityStep::Value(
            nsec_value,
            theme::text::PRIMARY,
            nsec_height,
        ));
        steps.push(IdentityStep::Spacer(row_gap));

        if let Some(spark_pubkey) = root.spark_pubkey_hex.as_deref() {
            let spark_height =
                text_height(spark_pubkey, value_text_size, content_width, label_height);
            steps.push(IdentityStep::Label("spark public key".to_string(), None));
            steps.push(IdentityStep::Value(
                spark_pubkey.to_string(),
                theme::text::PRIMARY,
                spark_height,
            ));
            steps.push(IdentityStep::Spacer(row_gap));
        }

        if let Some(seed_phrase) = root.seed_phrase.as_deref() {
            let seed_display = format_seed_phrase(seed_phrase);
            let seed_height =
                text_height(&seed_display, value_text_size, content_width, label_height);
            steps.push(IdentityStep::Label("seed phrase".to_string(), None));
            steps.push(IdentityStep::Value(
                seed_display,
                theme::text::PRIMARY,
                seed_height,
            ));
        } else {
            let note = "seed phrase unavailable for imported private keys";
            let note_height = text_height(note, text_size, content_width, label_height);
            steps.push(IdentityStep::Value(
                note.to_string(),
                theme::text::MUTED,
                note_height,
            ));
        }
    } else if root.nostr_error.is_none() {
        let empty = "No identity loaded yet.";
        let empty_height = text_height(empty, text_size, content_width, label_height);
        steps.push(IdentityStep::Value(
            empty.to_string(),
            theme::text::MUTED,
            empty_height,
        ));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            IdentityStep::Intro(_, height) => ColumnItem::Fixed(*height),
            IdentityStep::InputLabel => ColumnItem::Fixed(label_height),
            IdentityStep::Input => ColumnItem::Fixed(input_height),
            IdentityStep::ActionRow => ColumnItem::Fixed(button_height),
            IdentityStep::Label(_, _) => ColumnItem::Fixed(button_height),
            IdentityStep::Value(_, _, height) => ColumnItem::Fixed(*height),
            IdentityStep::Spacer(height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 6.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            IdentityStep::Intro(text, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(theme::text::MUTED)
                    .paint(bounds, cx);
            }
            IdentityStep::InputLabel => {
                Text::new("Private key (nsec or 64-char hex)")
                    .font_size(text_size)
                    .color(theme::text::MUTED)
                    .paint(bounds, cx);
            }
            IdentityStep::Input => {
                root.identity_private_key_bounds = bounds;
                root.identity_private_key_input.paint(bounds, cx);
            }
            IdentityStep::ActionRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[wgpui::RowItem::fixed(150.0), wgpui::RowItem::fixed(110.0)],
                    8.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                let generate_bounds = *row_bounds.get(0).unwrap_or(&bounds);
                let load_bounds = *row_bounds.get(1).unwrap_or(&bounds);
                root.keygen_bounds = generate_bounds;
                root.identity_load_bounds = load_bounds;
                root.keygen_button.paint(generate_bounds, cx);
                root.identity_load_button.paint(load_bounds, cx);
            }
            IdentityStep::Label(label, copy_field) => {
                if let Some(copy_field) = copy_field {
                    let row_bounds = aligned_row_bounds(
                        bounds,
                        button_height,
                        &[
                            wgpui::RowItem::flex(1.0),
                            wgpui::RowItem::fixed(copy_button_width),
                        ],
                        8.0,
                        JustifyContent::FlexStart,
                        AlignItems::Center,
                    );
                    let label_bounds = *row_bounds.first().unwrap_or(&bounds);
                    let copy_bounds = *row_bounds.get(1).unwrap_or(&bounds);
                    Text::new(label)
                        .font_size(value_text_size)
                        .color(theme::text::MUTED)
                        .paint(label_bounds, cx);
                    match copy_field {
                        IdentityCopyField::Npub => {
                            root.identity_copy_npub_bounds = copy_bounds;
                            root.identity_copy_npub_button.paint(copy_bounds, cx);
                        }
                        IdentityCopyField::Nsec => {
                            root.identity_copy_nsec_bounds = copy_bounds;
                            root.identity_copy_nsec_button.paint(copy_bounds, cx);
                        }
                    }
                } else {
                    Text::new(label)
                        .font_size(value_text_size)
                        .color(theme::text::MUTED)
                        .paint(bounds, cx);
                }
            }
            IdentityStep::Value(text, color, _) => {
                let mut value = Text::new(text).font_size(value_text_size).color(color);
                value.paint(bounds, cx);
            }
            IdentityStep::Spacer(_) => {}
        }
    }

    root.copy_bounds = Bounds::ZERO;
    root.event_scroll_bounds = Bounds::ZERO;
}

pub(super) fn paint_pylon_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.8).min(560.0).max(280.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let toggle_width = 120.0;

    root.pylon_toggle_bounds = Bounds::ZERO;
    root.pylon_init_bounds = Bounds::ZERO;
    root.pylon_start_bounds = Bounds::ZERO;
    root.pylon_stop_bounds = Bounds::ZERO;
    root.pylon_refresh_bounds = Bounds::ZERO;

    let toggle_label = if root.pylon_status.running {
        "Turn Off"
    } else {
        "Turn On"
    };
    root.pylon_toggle_button.set_label(toggle_label);
    root.pylon_toggle_button
        .set_variant(if root.pylon_status.running {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Primary
        });
    root.pylon_toggle_button
        .set_disabled(root.pylon_status.last_error.is_some() && !root.pylon_status.running);

    let status_line = if root.pylon_status.running {
        "Provider: ON"
    } else {
        "Provider: OFF"
    };
    let identity_line = if root.pylon_status.identity_exists {
        "Identity: present"
    } else {
        "Identity: missing (auto-generate on first start)"
    };
    enum PylonStep {
        ButtonRow,
        Line(String, wgpui::color::Hsla),
        Spacer(f32),
    }

    let mut steps = vec![
        PylonStep::ButtonRow,
        PylonStep::Spacer(14.0),
        PylonStep::Line(status_line.to_string(), theme::text::PRIMARY),
        PylonStep::Spacer(value_spacing),
        PylonStep::Line(identity_line.to_string(), theme::text::MUTED),
        PylonStep::Spacer(value_spacing),
    ];

    if let Some(uptime) = root.pylon_status.uptime_secs {
        steps.push(PylonStep::Line(
            format!("Uptime: {}s", uptime),
            theme::text::MUTED,
        ));
        steps.push(PylonStep::Spacer(value_spacing));
    }

    if let Some(provider_active) = root.pylon_status.provider_active {
        steps.push(PylonStep::Line(
            format!(
                "Provider: {}",
                if provider_active {
                    "active"
                } else {
                    "inactive"
                }
            ),
            theme::text::MUTED,
        ));
        steps.push(PylonStep::Spacer(value_spacing));
    }

    if let Some(host_active) = root.pylon_status.host_active {
        steps.push(PylonStep::Line(
            format!("Host: {}", if host_active { "active" } else { "inactive" }),
            theme::text::MUTED,
        ));
        steps.push(PylonStep::Spacer(value_spacing));
    }

    steps.push(PylonStep::Line(
        format!("Jobs completed: {}", root.pylon_status.jobs_completed),
        theme::text::MUTED,
    ));
    steps.push(PylonStep::Spacer(value_spacing));

    steps.push(PylonStep::Line(
        format!("Earnings: {} msats", root.pylon_status.earnings_msats),
        theme::text::MUTED,
    ));
    steps.push(PylonStep::Spacer(value_spacing));

    if let Some(err) = root.pylon_status.last_error.as_deref() {
        steps.push(PylonStep::Line(err.to_string(), theme::accent::RED));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            PylonStep::ButtonRow => ColumnItem::Fixed(button_height),
            PylonStep::Line(_, _) => ColumnItem::Fixed(label_height),
            PylonStep::Spacer(height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            PylonStep::ButtonRow => {
                let button_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[wgpui::RowItem::fixed(toggle_width)],
                    0.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                )
                .into_iter()
                .next()
                .unwrap_or(bounds);
                root.pylon_toggle_bounds = button_bounds;
                root.pylon_toggle_button.paint(button_bounds, cx);
            }
            PylonStep::Line(text, color) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(color)
                    .paint(bounds, cx);
            }
            PylonStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_wallet_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.8).min(560.0).max(280.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let refresh_width = 90.0;
    let liquidity_width = 96.0;
    let invoice_button_width = 140.0;
    let invoice_copy_width = 100.0;
    let pay_amount_width = 170.0;
    let pay_button_width = 84.0;
    let gap = 8.0;

    root.wallet_invoice_copy_button
        .set_disabled(root.wallet_status.last_invoice.is_none());
    root.wallet_pay_button
        .set_disabled(root.wallet_pay_request_input.get_value().trim().is_empty());

    root.wallet_refresh_bounds = Bounds::ZERO;
    root.wallet_liquidity_bounds = Bounds::ZERO;
    root.wallet_invoice_amount_bounds = Bounds::ZERO;
    root.wallet_invoice_create_bounds = Bounds::ZERO;
    root.wallet_invoice_copy_bounds = Bounds::ZERO;
    root.wallet_pay_request_bounds = Bounds::ZERO;
    root.wallet_pay_amount_bounds = Bounds::ZERO;
    root.wallet_pay_bounds = Bounds::ZERO;

    let identity_line = if root.wallet_status.identity_exists {
        "Identity: present"
    } else {
        "Identity: missing"
    };
    enum WalletStep {
        ButtonRow,
        InvoiceRow,
        InvoiceCopyRow,
        PayRequestRow,
        PayActionRow,
        Line(String, wgpui::color::Hsla, f32),
        Spacer(f32),
    }

    let mut steps = vec![
        WalletStep::ButtonRow,
        WalletStep::Spacer(14.0),
        WalletStep::Line(identity_line.to_string(), theme::text::MUTED, label_height),
        WalletStep::Spacer(value_spacing),
    ];

    if let Some(network) = root.wallet_status.network.as_deref() {
        steps.push(WalletStep::Line(
            format!("Network: {}", network),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(value_spacing));
    }

    if let Some(network_status) = root.wallet_status.network_status.as_deref() {
        steps.push(WalletStep::Line(
            format!("Connectivity: {}", network_status),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(value_spacing));
    }

    steps.push(WalletStep::Line(
        format!("Total: {} sats", root.wallet_status.total_sats),
        theme::text::PRIMARY,
        label_height,
    ));
    steps.push(WalletStep::Spacer(value_spacing));
    steps.push(WalletStep::Line(
        format!("Spark: {} sats", root.wallet_status.spark_sats),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(WalletStep::Spacer(value_spacing));
    steps.push(WalletStep::Line(
        format!("Lightning: {} sats", root.wallet_status.lightning_sats),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(WalletStep::Spacer(value_spacing));
    steps.push(WalletStep::Line(
        format!("On-chain: {} sats", root.wallet_status.onchain_sats),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(WalletStep::Spacer(value_spacing));

    if let Some(address) = root.wallet_status.spark_address.as_deref() {
        let mut text = Text::new(address).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(WalletStep::Line(
            "Spark address".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(4.0));
        steps.push(WalletStep::Line(
            address.to_string(),
            theme::text::PRIMARY,
            height,
        ));
        steps.push(WalletStep::Spacer(value_spacing));
    }

    if let Some(address) = root.wallet_status.bitcoin_address.as_deref() {
        let mut text = Text::new(address).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(WalletStep::Line(
            "Bitcoin address".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(4.0));
        steps.push(WalletStep::Line(
            address.to_string(),
            theme::text::PRIMARY,
            height,
        ));
        steps.push(WalletStep::Spacer(value_spacing));
    }

    steps.push(WalletStep::Spacer(10.0));
    steps.push(WalletStep::Line(
        "Receive payment".to_string(),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(WalletStep::Spacer(4.0));
    steps.push(WalletStep::InvoiceRow);
    steps.push(WalletStep::Spacer(value_spacing));

    if let Some(invoice) = root.wallet_status.last_invoice.as_deref() {
        let mut text = Text::new(invoice).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(WalletStep::Line(
            "Last invoice".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(4.0));
        steps.push(WalletStep::Line(
            invoice.to_string(),
            theme::text::PRIMARY,
            height,
        ));
        steps.push(WalletStep::Spacer(6.0));
        steps.push(WalletStep::InvoiceCopyRow);
        steps.push(WalletStep::Spacer(value_spacing));
    }

    steps.push(WalletStep::Spacer(8.0));
    steps.push(WalletStep::Line(
        "Send payment".to_string(),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(WalletStep::Spacer(4.0));
    steps.push(WalletStep::PayRequestRow);
    steps.push(WalletStep::Spacer(6.0));
    steps.push(WalletStep::PayActionRow);
    steps.push(WalletStep::Spacer(value_spacing));

    if let Some(last_payment_id) = root.wallet_status.last_payment_id.as_deref() {
        steps.push(WalletStep::Line(
            format!("Last payment id: {}", last_payment_id),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(value_spacing));
    }

    if !root.wallet_status.recent_payments.is_empty() {
        steps.push(WalletStep::Line(
            "Recent payments".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(WalletStep::Spacer(4.0));
        for payment in root.wallet_status.recent_payments.iter().take(5) {
            let direction = if payment.direction.eq_ignore_ascii_case("send") {
                "->"
            } else {
                "<-"
            };
            let short_id = if payment.id.len() > 12 {
                &payment.id[..12]
            } else {
                payment.id.as_str()
            };
            let line = format!(
                "{} {:>8} sats  {}  {}  @{}",
                direction, payment.amount_sats, payment.status, short_id, payment.timestamp
            );
            steps.push(WalletStep::Line(line, theme::text::SECONDARY, label_height));
            steps.push(WalletStep::Spacer(4.0));
        }
    }

    if let Some(err) = root.wallet_status.last_error.as_deref() {
        steps.push(WalletStep::Line(
            err.to_string(),
            theme::accent::RED,
            label_height,
        ));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            WalletStep::ButtonRow => ColumnItem::Fixed(button_height),
            WalletStep::InvoiceRow => ColumnItem::Fixed(button_height),
            WalletStep::InvoiceCopyRow => ColumnItem::Fixed(button_height),
            WalletStep::PayRequestRow => ColumnItem::Fixed(button_height),
            WalletStep::PayActionRow => ColumnItem::Fixed(button_height),
            WalletStep::Line(_, _, height) => ColumnItem::Fixed(*height),
            WalletStep::Spacer(height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            WalletStep::ButtonRow => {
                let button_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[
                        wgpui::RowItem::fixed(refresh_width),
                        wgpui::RowItem::fixed(liquidity_width),
                    ],
                    gap,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if button_bounds.len() >= 2 {
                    root.wallet_refresh_bounds = button_bounds[0];
                    root.wallet_liquidity_bounds = button_bounds[1];
                    root.wallet_refresh_button.paint(button_bounds[0], cx);
                    root.wallet_liquidity_button.paint(button_bounds[1], cx);
                } else if let Some(first) = button_bounds.into_iter().next() {
                    root.wallet_refresh_bounds = first;
                    root.wallet_refresh_button.paint(first, cx);
                }
            }
            WalletStep::InvoiceRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[
                        wgpui::RowItem::flex(1.0),
                        wgpui::RowItem::fixed(invoice_button_width),
                    ],
                    gap,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if row_bounds.len() >= 2 {
                    root.wallet_invoice_amount_bounds = row_bounds[0];
                    root.wallet_invoice_create_bounds = row_bounds[1];
                    root.wallet_invoice_amount_input.paint(row_bounds[0], cx);
                    root.wallet_invoice_create_button.paint(row_bounds[1], cx);
                }
            }
            WalletStep::InvoiceCopyRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[wgpui::RowItem::fixed(invoice_copy_width)],
                    0.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if let Some(copy_bounds) = row_bounds.into_iter().next() {
                    root.wallet_invoice_copy_bounds = copy_bounds;
                    root.wallet_invoice_copy_button.paint(copy_bounds, cx);
                }
            }
            WalletStep::PayRequestRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[wgpui::RowItem::flex(1.0)],
                    0.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if let Some(input_bounds) = row_bounds.into_iter().next() {
                    root.wallet_pay_request_bounds = input_bounds;
                    root.wallet_pay_request_input.paint(input_bounds, cx);
                }
            }
            WalletStep::PayActionRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[
                        wgpui::RowItem::fixed(pay_amount_width),
                        wgpui::RowItem::fixed(pay_button_width),
                    ],
                    gap,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if row_bounds.len() >= 2 {
                    root.wallet_pay_amount_bounds = row_bounds[0];
                    root.wallet_pay_bounds = row_bounds[1];
                    root.wallet_pay_amount_input.paint(row_bounds[0], cx);
                    root.wallet_pay_button.paint(row_bounds[1], cx);
                }
            }
            WalletStep::Line(text, color, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(color)
                    .paint(bounds, cx);
            }
            WalletStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_liquidity_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.9).min(720.0).max(360.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);

    let provider_active = root.liquidity_status.provider_active.unwrap_or(false);
    let running = root.liquidity_status.running;
    root.liquidity_online_button
        .set_disabled(running && provider_active);
    root.liquidity_offline_button.set_disabled(!running);
    root.liquidity_invoice_copy_button
        .set_disabled(root.liquidity_status.last_invoice.is_none());

    let online_width = 92.0;
    let offline_width = 96.0;
    let refresh_width = 86.0;
    let gap = 8.0;
    root.liquidity_online_bounds = Bounds::ZERO;
    root.liquidity_offline_bounds = Bounds::ZERO;
    root.liquidity_refresh_bounds = Bounds::ZERO;
    root.liquidity_invoice_amount_bounds = Bounds::ZERO;
    root.liquidity_invoice_create_bounds = Bounds::ZERO;
    root.liquidity_invoice_copy_bounds = Bounds::ZERO;

    let daemon_line = if running {
        "Daemon: running"
    } else {
        "Daemon: stopped"
    };

    enum LiquidityStep {
        ButtonRow,
        InvoiceRow,
        InvoiceCopyRow,
        Line(String, wgpui::color::Hsla, f32),
        Spacer(f32),
    }

    let mut steps = vec![
        LiquidityStep::ButtonRow,
        LiquidityStep::Spacer(14.0),
        LiquidityStep::Line(daemon_line.to_string(), theme::text::PRIMARY, label_height),
        LiquidityStep::Spacer(value_spacing),
    ];

    if let Some(provider_active) = root.liquidity_status.provider_active {
        steps.push(LiquidityStep::Line(
            format!(
                "Provider: {}",
                if provider_active { "online" } else { "offline" }
            ),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }

    if let Some(worker_id) = root.liquidity_status.worker_id.as_deref() {
        steps.push(LiquidityStep::Line(
            format!("Worker: {}", worker_id),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }

    steps.push(LiquidityStep::Line(
        format!("Earned: {} sats", root.liquidity_status.earned_sats),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(LiquidityStep::Spacer(value_spacing));

    if root.liquidity_status.max_invoice_sats > 0 {
        steps.push(LiquidityStep::Line(
            format!(
                "Max per invoice: {} sats",
                root.liquidity_status.max_invoice_sats
            ),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }
    if root.liquidity_status.max_hourly_sats > 0 {
        steps.push(LiquidityStep::Line(
            format!(
                "Max per hour: {} sats",
                root.liquidity_status.max_hourly_sats
            ),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }
    if root.liquidity_status.max_daily_sats > 0 {
        steps.push(LiquidityStep::Line(
            format!("Max per day: {} sats", root.liquidity_status.max_daily_sats),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }

    if let Some(address) = root.wallet_status.spark_address.as_deref() {
        let mut text = Text::new(address).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(LiquidityStep::Line(
            "Spark address".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(4.0));
        steps.push(LiquidityStep::Line(
            address.to_string(),
            theme::text::PRIMARY,
            height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }

    if let Some(address) = root.wallet_status.bitcoin_address.as_deref() {
        let mut text = Text::new(address).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(LiquidityStep::Line(
            "Bitcoin address".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(4.0));
        steps.push(LiquidityStep::Line(
            address.to_string(),
            theme::text::PRIMARY,
            height,
        ));
        steps.push(LiquidityStep::Spacer(value_spacing));
    }

    steps.push(LiquidityStep::Spacer(10.0));
    steps.push(LiquidityStep::Line(
        "Create invoice".to_string(),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(LiquidityStep::Spacer(4.0));
    steps.push(LiquidityStep::InvoiceRow);
    steps.push(LiquidityStep::Spacer(value_spacing));

    if let Some(invoice) = root.liquidity_status.last_invoice.as_deref() {
        let mut text = Text::new(invoice).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(LiquidityStep::Line(
            "Last invoice".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(LiquidityStep::Spacer(4.0));
        steps.push(LiquidityStep::Line(
            invoice.to_string(),
            theme::text::PRIMARY,
            height,
        ));
        steps.push(LiquidityStep::Spacer(6.0));
        steps.push(LiquidityStep::InvoiceCopyRow);
        steps.push(LiquidityStep::Spacer(value_spacing));
    }

    if let Some(err) = root.liquidity_status.last_error.as_deref() {
        steps.push(LiquidityStep::Line(
            err.to_string(),
            theme::accent::RED,
            label_height,
        ));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            LiquidityStep::ButtonRow => ColumnItem::Fixed(button_height),
            LiquidityStep::InvoiceRow => ColumnItem::Fixed(button_height),
            LiquidityStep::InvoiceCopyRow => ColumnItem::Fixed(button_height),
            LiquidityStep::Line(_, _, height) => ColumnItem::Fixed(*height),
            LiquidityStep::Spacer(height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            LiquidityStep::ButtonRow => {
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[
                        wgpui::RowItem::fixed(online_width),
                        wgpui::RowItem::fixed(offline_width),
                        wgpui::RowItem::fixed(refresh_width),
                    ],
                    gap,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if row_bounds.len() >= 3 {
                    root.liquidity_online_bounds = row_bounds[0];
                    root.liquidity_offline_bounds = row_bounds[1];
                    root.liquidity_refresh_bounds = row_bounds[2];
                    root.liquidity_online_button.paint(row_bounds[0], cx);
                    root.liquidity_offline_button.paint(row_bounds[1], cx);
                    root.liquidity_refresh_button.paint(row_bounds[2], cx);
                }
            }
            LiquidityStep::InvoiceRow => {
                let invoice_button_width = 140.0;
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[
                        wgpui::RowItem::flex(1.0),
                        wgpui::RowItem::fixed(invoice_button_width),
                    ],
                    gap,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if row_bounds.len() >= 2 {
                    root.liquidity_invoice_amount_bounds = row_bounds[0];
                    root.liquidity_invoice_create_bounds = row_bounds[1];
                    root.liquidity_invoice_amount_input.paint(row_bounds[0], cx);
                    root.liquidity_invoice_create_button
                        .paint(row_bounds[1], cx);
                }
            }
            LiquidityStep::InvoiceCopyRow => {
                let copy_width = 76.0;
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &[wgpui::RowItem::fixed(copy_width)],
                    0.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                if let Some(copy_bounds) = row_bounds.into_iter().next() {
                    root.liquidity_invoice_copy_bounds = copy_bounds;
                    root.liquidity_invoice_copy_button.paint(copy_bounds, cx);
                }
            }
            LiquidityStep::Line(text, color, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(color)
                    .paint(bounds, cx);
            }
            LiquidityStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_sell_compute_pane(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 10.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.85).min(600.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);

    let provider_active = root.sell_compute_status.provider_active.unwrap_or(false);
    let running = root.sell_compute_status.running;
    root.sell_compute_online_button
        .set_disabled(running && provider_active);
    root.sell_compute_offline_button.set_disabled(!running);

    let online_width = 92.0;
    let offline_width = 96.0;
    let refresh_width = 86.0;
    let gap = 8.0;
    root.sell_compute_online_bounds = Bounds::ZERO;
    root.sell_compute_offline_bounds = Bounds::ZERO;
    root.sell_compute_refresh_bounds = Bounds::ZERO;

    let status_line = if running {
        "Daemon: running"
    } else {
        "Daemon: stopped"
    };
    enum SellStep {
        ButtonRow,
        Line(String, wgpui::color::Hsla, f32),
        Spacer(f32),
    }

    let mut steps = vec![
        SellStep::ButtonRow,
        SellStep::Spacer(14.0),
        SellStep::Line(status_line.to_string(), theme::text::PRIMARY, label_height),
        SellStep::Spacer(value_spacing),
    ];

    if let Some(provider_active) = root.sell_compute_status.provider_active {
        steps.push(SellStep::Line(
            format!(
                "Provider: {}",
                if provider_active { "online" } else { "offline" }
            ),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(SellStep::Spacer(value_spacing));
    }

    if let Some(host_active) = root.sell_compute_status.host_active {
        steps.push(SellStep::Line(
            format!("Host: {}", if host_active { "active" } else { "inactive" }),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(SellStep::Spacer(value_spacing));
    }

    steps.push(SellStep::Line(
        format!(
            "Min price: {} msats",
            root.sell_compute_status.min_price_msats
        ),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(SellStep::Spacer(value_spacing));
    steps.push(SellStep::Line(
        format!(
            "Require payment: {}",
            if root.sell_compute_status.require_payment {
                "yes"
            } else {
                "no"
            }
        ),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(SellStep::Spacer(value_spacing));
    steps.push(SellStep::Line(
        format!(
            "Payments enabled: {}",
            if root.sell_compute_status.enable_payments {
                "yes"
            } else {
                "no"
            }
        ),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(SellStep::Spacer(value_spacing));
    steps.push(SellStep::Line(
        format!("Network: {}", root.sell_compute_status.network),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(SellStep::Spacer(value_spacing));

    if !root.sell_compute_status.default_model.is_empty() {
        steps.push(SellStep::Line(
            format!("Default model: {}", root.sell_compute_status.default_model),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(SellStep::Spacer(value_spacing));
    }

    if !root.sell_compute_status.backend_preference.is_empty() {
        let list = root.sell_compute_status.backend_preference.join(", ");
        let mut text = Text::new(list.as_str()).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(SellStep::Line(
            "Backends".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(SellStep::Spacer(4.0));
        steps.push(SellStep::Line(list, theme::text::PRIMARY, height));
        steps.push(SellStep::Spacer(value_spacing));
    }

    if !root.sell_compute_status.agent_backends.is_empty() {
        let list = root.sell_compute_status.agent_backends.join(", ");
        let mut text = Text::new(list.as_str()).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(SellStep::Line(
            "Agent backends".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(SellStep::Spacer(4.0));
        steps.push(SellStep::Line(list, theme::text::PRIMARY, height));
        steps.push(SellStep::Spacer(value_spacing));
    }

    if !root.sell_compute_status.supported_bazaar_kinds.is_empty() {
        let kinds = root
            .sell_compute_status
            .supported_bazaar_kinds
            .iter()
            .map(|kind| kind.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        steps.push(SellStep::Line(
            format!("Supported kinds: {}", kinds),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(SellStep::Spacer(value_spacing));
    }

    if let Some(err) = root.sell_compute_status.last_error.as_deref() {
        steps.push(SellStep::Line(
            err.to_string(),
            theme::accent::RED,
            label_height,
        ));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            SellStep::ButtonRow => ColumnItem::Fixed(button_height),
            SellStep::Line(_, _, height) => ColumnItem::Fixed(*height),
            SellStep::Spacer(height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            SellStep::ButtonRow => {
                let items = [
                    wgpui::RowItem::fixed(online_width),
                    wgpui::RowItem::fixed(offline_width),
                    wgpui::RowItem::fixed(refresh_width),
                ];
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &items,
                    gap,
                    JustifyContent::Center,
                    AlignItems::Center,
                );
                let online_bounds = *row_bounds.get(0).unwrap_or(&bounds);
                let offline_bounds = *row_bounds.get(1).unwrap_or(&bounds);
                let refresh_bounds = *row_bounds.get(2).unwrap_or(&bounds);
                root.sell_compute_online_bounds = online_bounds;
                root.sell_compute_offline_bounds = offline_bounds;
                root.sell_compute_refresh_bounds = refresh_bounds;
                root.sell_compute_online_button.paint(online_bounds, cx);
                root.sell_compute_offline_button.paint(offline_bounds, cx);
                root.sell_compute_refresh_button.paint(refresh_bounds, cx);
            }
            SellStep::Line(text, color, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(color)
                    .paint(bounds, cx);
            }
            SellStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_dvm_history_pane(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let padding = 16.0;
    let button_height = 28.0;
    let label_height = 16.0;
    let value_spacing = 8.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.9).min(700.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let refresh_width = 90.0;
    root.dvm_history_refresh_bounds = Bounds::ZERO;

    enum DvmStep {
        Header,
        Line(String, wgpui::color::Hsla, f32),
        Spacer(f32),
    }

    let mut steps = vec![DvmStep::Header, DvmStep::Spacer(10.0)];

    steps.push(DvmStep::Line(
        format!(
            "Total: {} sats ({} msats)",
            root.dvm_history.summary_total_sats, root.dvm_history.summary_total_msats
        ),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(DvmStep::Spacer(value_spacing));
    steps.push(DvmStep::Line(
        format!("Jobs completed: {}", root.dvm_history.summary_job_count),
        theme::text::MUTED,
        label_height,
    ));
    steps.push(DvmStep::Spacer(value_spacing));

    if !root.dvm_history.summary_by_source.is_empty() {
        let mut sources = root.dvm_history.summary_by_source.clone();
        sources.sort_by(|a, b| a.0.cmp(&b.0));
        let joined = sources
            .into_iter()
            .map(|(source, amount)| format!("{source}: {amount} msats"))
            .collect::<Vec<_>>()
            .join(" | ");
        let mut text = Text::new(joined.as_str()).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(DvmStep::Line(joined, theme::text::MUTED, height));
        steps.push(DvmStep::Spacer(value_spacing));
    }

    if !root.dvm_history.status_counts.is_empty() {
        let mut counts = root.dvm_history.status_counts.clone();
        counts.sort_by(|a, b| a.0.cmp(&b.0));
        let joined = counts
            .into_iter()
            .map(|(status, count)| format!("{status}: {count}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let mut text = Text::new(joined.as_str()).font_size(text_size);
        let (_, height) = text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(DvmStep::Line(joined, theme::text::MUTED, height));
        steps.push(DvmStep::Spacer(value_spacing));
    }

    steps.push(DvmStep::Line(
        "Recent jobs".to_string(),
        theme::text::PRIMARY,
        label_height,
    ));
    steps.push(DvmStep::Spacer(value_spacing));

    if root.dvm_history.jobs.is_empty() {
        steps.push(DvmStep::Line(
            "No jobs recorded yet.".to_string(),
            theme::text::MUTED,
            label_height,
        ));
        steps.push(DvmStep::Spacer(value_spacing));
    } else {
        for job in &root.dvm_history.jobs {
            let id = if job.id.len() > 8 {
                &job.id[..8]
            } else {
                &job.id
            };
            let line = format!(
                "{id} | {} | kind {} | {} msats | {}",
                job.status, job.kind, job.price_msats, job.created_at
            );
            steps.push(DvmStep::Line(line, theme::text::MUTED, label_height));
            steps.push(DvmStep::Spacer(4.0));
        }
    }

    if let Some(err) = root.dvm_history.last_error.as_deref() {
        steps.push(DvmStep::Line(
            err.to_string(),
            theme::accent::RED,
            label_height,
        ));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            DvmStep::Header => ColumnItem::Fixed(button_height),
            DvmStep::Line(_, _, height) => ColumnItem::Fixed(*height),
            DvmStep::Spacer(height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            DvmStep::Header => {
                let row_items = [
                    wgpui::RowItem::flex(1.0),
                    wgpui::RowItem::fixed(refresh_width),
                ];
                let row_bounds = aligned_row_bounds(
                    bounds,
                    button_height,
                    &row_items,
                    8.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                );
                let title_bounds = *row_bounds.get(0).unwrap_or(&bounds);
                let refresh_bounds = *row_bounds.get(1).unwrap_or(&bounds);
                Text::new("Earnings summary")
                    .font_size(text_size)
                    .color(theme::text::PRIMARY)
                    .paint(title_bounds, cx);
                root.dvm_history_refresh_bounds = refresh_bounds;
                root.dvm_history_refresh_button.paint(refresh_bounds, cx);
            }
            DvmStep::Line(text, color, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(color)
                    .paint(bounds, cx);
            }
            DvmStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_nip90_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let label_height = 16.0;
    let input_height = 28.0;
    let gap = 8.0;
    let text_size = theme::font_size::XS;

    let mut content_width = (bounds.size.width * 0.9).min(720.0).max(320.0);
    content_width = content_width.min(bounds.size.width - padding * 2.0);
    let content_bounds = centered_column_bounds(bounds, content_width, padding);

    root.nip90_relay_bounds = Bounds::ZERO;
    root.nip90_kind_bounds = Bounds::ZERO;
    root.nip90_provider_bounds = Bounds::ZERO;
    root.nip90_prompt_bounds = Bounds::ZERO;
    root.nip90_submit_bounds = Bounds::ZERO;

    enum NipStep {
        Label(String),
        Input(f32),
        Spacer(f32),
        Submit,
        ActivityTitle,
        Log(String, f32),
    }

    let prompt_height = 64.0;
    let mut steps = vec![
        NipStep::Label("Relays".to_string()),
        NipStep::Spacer(4.0),
        NipStep::Input(input_height),
        NipStep::Spacer(gap),
        NipStep::Label("Job kind".to_string()),
        NipStep::Spacer(4.0),
        NipStep::Input(input_height),
        NipStep::Spacer(gap),
        NipStep::Label("Provider (optional)".to_string()),
        NipStep::Spacer(4.0),
        NipStep::Input(input_height),
        NipStep::Spacer(gap),
        NipStep::Label("Prompt".to_string()),
        NipStep::Spacer(4.0),
        NipStep::Input(prompt_height),
        NipStep::Spacer(gap),
        NipStep::Submit,
        NipStep::Spacer(gap),
        NipStep::ActivityTitle,
        NipStep::Spacer(6.0),
    ];

    if root.nip90_log.is_empty() {
        steps.push(NipStep::Log(
            "No NIP-90 activity yet.".to_string(),
            label_height,
        ));
    } else {
        let mut lines = String::new();
        for line in root.nip90_log.iter().rev().take(12).rev() {
            lines.push_str(line);
            lines.push('\n');
        }
        let mut log_text = Text::new(lines.as_str()).font_size(text_size);
        let (_, height) = log_text.size_hint_with_width(content_width);
        let height = height.unwrap_or(label_height);
        steps.push(NipStep::Log(lines, height));
    }

    let heights: Vec<ColumnItem> = steps
        .iter()
        .map(|step| match step {
            NipStep::Label(_) => ColumnItem::Fixed(label_height),
            NipStep::Input(height) => ColumnItem::Fixed(*height),
            NipStep::Spacer(height) => ColumnItem::Fixed(*height),
            NipStep::Submit => ColumnItem::Fixed(32.0),
            NipStep::ActivityTitle => ColumnItem::Fixed(label_height),
            NipStep::Log(_, height) => ColumnItem::Fixed(*height),
        })
        .collect();
    let bounds_list = column_bounds(content_bounds, &heights, 0.0);

    let mut input_index = 0;
    for (step, bounds) in steps.into_iter().zip(bounds_list) {
        match step {
            NipStep::Label(text) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(theme::text::MUTED)
                    .paint(bounds, cx);
            }
            NipStep::Input(height) => {
                match input_index {
                    0 => {
                        root.nip90_relay_bounds = bounds;
                        root.nip90_relay_input.paint(bounds, cx);
                    }
                    1 => {
                        let kind_bounds = aligned_row_bounds(
                            bounds,
                            height,
                            &[wgpui::RowItem::fixed(140.0)],
                            0.0,
                            JustifyContent::FlexStart,
                            AlignItems::Center,
                        )
                        .into_iter()
                        .next()
                        .unwrap_or(bounds);
                        root.nip90_kind_bounds = kind_bounds;
                        root.nip90_kind_input.paint(kind_bounds, cx);
                    }
                    2 => {
                        root.nip90_provider_bounds = bounds;
                        root.nip90_provider_input.paint(bounds, cx);
                    }
                    3 => {
                        root.nip90_prompt_bounds = bounds;
                        root.nip90_prompt_input.paint(bounds, cx);
                    }
                    _ => {}
                }
                input_index += 1;
            }
            NipStep::Submit => {
                let submit_bounds = aligned_row_bounds(
                    bounds,
                    32.0,
                    &[wgpui::RowItem::fixed(120.0)],
                    0.0,
                    JustifyContent::FlexStart,
                    AlignItems::Center,
                )
                .into_iter()
                .next()
                .unwrap_or(bounds);
                root.nip90_submit_bounds = submit_bounds;
                root.nip90_submit_button.paint(submit_bounds, cx);
            }
            NipStep::ActivityTitle => {
                Text::new("Activity")
                    .font_size(text_size)
                    .color(theme::text::PRIMARY)
                    .paint(bounds, cx);
            }
            NipStep::Log(text, _) => {
                Text::new(text)
                    .font_size(text_size)
                    .color(theme::text::MUTED)
                    .paint(bounds, cx);
            }
            NipStep::Spacer(_) => {}
        }
    }
}

pub(super) fn paint_events_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let header_height = 24.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let copy_button_width = 68.0;
    root.copy_bounds = Bounds::ZERO;
    let copy_label = if let Some(until) = root.copy_feedback_until {
        if Instant::now() < until {
            "Copied"
        } else {
            root.copy_feedback_until = None;
            "Copy"
        }
    } else {
        "Copy"
    };
    root.copy_button.set_label(copy_label);
    root.copy_button.set_disabled(root.event_log.is_empty());
    let items = [ColumnItem::Fixed(header_height), ColumnItem::Flex(1.0)];
    let bounds_list = column_bounds(content_bounds, &items, 8.0);
    let header_bounds = *bounds_list.get(0).unwrap_or(&content_bounds);
    let feed_bounds = *bounds_list.get(1).unwrap_or(&content_bounds);

    let row_items = [
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(copy_button_width),
    ];
    let row_bounds = aligned_row_bounds(
        header_bounds,
        header_height,
        &row_items,
        8.0,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );
    let title_bounds = *row_bounds.get(0).unwrap_or(&header_bounds);
    let copy_bounds = *row_bounds.get(1).unwrap_or(&header_bounds);

    Text::new("CODEX EVENTS")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    root.copy_bounds = copy_bounds;
    root.copy_button.paint(copy_bounds, cx);

    let font_size = theme::font_size::XS;
    root.event_scroll_bounds = feed_bounds;

    if root.event_log.is_empty() {
        Text::new("No events yet.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(feed_bounds, cx);
        return;
    }

    let mut block = String::new();
    for line in &root.event_log {
        block.push_str(line);
        block.push('\n');
    }

    let mut feed_text = Text::new(block.as_str())
        .font_size(font_size)
        .color(theme::text::MUTED);
    let (_, height_opt) = feed_text.size_hint_with_width(feed_bounds.size.width);
    let content_height = height_opt
        .unwrap_or(feed_bounds.size.height)
        .max(feed_bounds.size.height);
    root.event_scroll
        .set_content_size(Size::new(feed_bounds.size.width, content_height));
    root.event_scroll.set_content(feed_text);

    if root.event_log_dirty {
        let max_scroll = (content_height - feed_bounds.size.height).max(0.0);
        root.event_scroll.scroll_to(Point::new(0.0, max_scroll));
        root.event_log_dirty = false;
    }

    root.event_scroll.paint(feed_bounds, cx);
}

pub(super) fn paint_threads_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let header_height = 24.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let refresh_button_width = 72.0;
    let load_more_button_width = 120.0;
    let button_gap = 8.0;
    root.threads_refresh_bounds = Bounds::ZERO;
    root.threads_load_more_bounds = Bounds::ZERO;

    let items = [ColumnItem::Fixed(header_height), ColumnItem::Flex(1.0)];
    let bounds_list = column_bounds(content_bounds, &items, 8.0);
    let header_bounds = *bounds_list.get(0).unwrap_or(&content_bounds);
    let list_bounds = *bounds_list.get(1).unwrap_or(&content_bounds);

    let row_items = [
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(load_more_button_width),
        wgpui::RowItem::fixed(refresh_button_width),
    ];
    let header_row_bounds = aligned_row_bounds(
        header_bounds,
        header_height,
        &row_items,
        button_gap,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );
    let title_bounds = *header_row_bounds.get(0).unwrap_or(&header_bounds);
    let load_more_bounds = *header_row_bounds.get(1).unwrap_or(&header_bounds);
    let refresh_bounds = *header_row_bounds.get(2).unwrap_or(&header_bounds);

    Text::new("RECENT THREADS")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    root.threads_refresh_bounds = refresh_bounds;
    root.threads_load_more_bounds = load_more_bounds;
    root.threads_refresh_button.paint(refresh_bounds, cx);
    root.threads_load_more_button
        .set_disabled(root.threads_next_cursor.is_none());
    root.threads_load_more_button.paint(load_more_bounds, cx);

    let font_size = theme::font_size::XS;
    let row_height = (font_size * 1.4).ceil();
    let row_gap = 6.0;
    let char_width = font_size * 0.6;
    let gap_chars = 2usize;
    let gap_px = char_width * gap_chars as f32;

    if root.thread_entries.is_empty() {
        Text::new("No recent threads.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(list_bounds, cx);
        return;
    }

    let mut updated_labels = Vec::with_capacity(root.thread_entries.len());
    let mut branch_labels = Vec::with_capacity(root.thread_entries.len());
    let mut max_updated_chars = "Updated".chars().count();
    let mut max_branch_chars = "Branch".chars().count();

    for entry in &root.thread_entries {
        let updated = relative_time_label(entry.summary.created_at);
        let branch_raw = entry.branch.as_deref().unwrap_or("");
        let branch = right_elide(branch_raw, 24);
        max_updated_chars = max_updated_chars.max(updated.chars().count());
        max_branch_chars = max_branch_chars.max(branch.chars().count());
        updated_labels.push(updated);
        branch_labels.push(branch);
    }

    let total_chars = (content_width / char_width).floor().max(1.0) as usize;
    let min_preview_chars = 8usize;
    let mut updated_chars = max_updated_chars;
    let mut branch_chars = max_branch_chars;
    let mut preview_chars =
        total_chars.saturating_sub(updated_chars + branch_chars + gap_chars * 2);
    if preview_chars < min_preview_chars {
        let mut deficit = min_preview_chars - preview_chars;
        let branch_min = "Branch".chars().count();
        let reducible_branch = branch_chars.saturating_sub(branch_min);
        let reduce_branch = deficit.min(reducible_branch);
        branch_chars = branch_chars.saturating_sub(reduce_branch);
        deficit = deficit.saturating_sub(reduce_branch);

        let updated_min = "Updated".chars().count();
        let reducible_updated = updated_chars.saturating_sub(updated_min);
        let reduce_updated = deficit.min(reducible_updated);
        updated_chars = updated_chars.saturating_sub(reduce_updated);
        preview_chars = total_chars.saturating_sub(updated_chars + branch_chars + gap_chars * 2);
    }

    let updated_width = (updated_chars as f32 * char_width).ceil();
    let branch_width = (branch_chars as f32 * char_width).ceil();
    let preview_width = (content_width - updated_width - branch_width - gap_px * 2.0).max(0.0);

    let list_items = [ColumnItem::Fixed(row_height), ColumnItem::Flex(1.0)];
    let list_rows = column_bounds(list_bounds, &list_items, row_gap);
    let header_row = *list_rows.get(0).unwrap_or(&list_bounds);
    let rows_bounds = *list_rows.get(1).unwrap_or(&list_bounds);

    let header_columns = wgpui::row_bounds(
        header_row,
        row_height,
        &[
            wgpui::RowItem::fixed(updated_width),
            wgpui::RowItem::fixed(branch_width),
            wgpui::RowItem::fixed(preview_width),
        ],
        gap_px,
    );
    let updated_header = *header_columns.get(0).unwrap_or(&header_row);
    let branch_header = *header_columns.get(1).unwrap_or(&header_row);
    let preview_header = *header_columns.get(2).unwrap_or(&header_row);

    Text::new("Updated")
        .font_size(font_size)
        .bold()
        .no_wrap()
        .color(theme::text::PRIMARY)
        .paint(updated_header, cx);
    Text::new("Branch")
        .font_size(font_size)
        .bold()
        .no_wrap()
        .color(theme::text::PRIMARY)
        .paint(branch_header, cx);
    Text::new("Conversation")
        .font_size(font_size)
        .bold()
        .no_wrap()
        .color(theme::text::PRIMARY)
        .paint(preview_header, cx);

    let entry_heights: Vec<f32> = root.thread_entries.iter().map(|_| row_height).collect();
    let row_bounds = stack_bounds(rows_bounds, &entry_heights, row_gap);

    for ((index, entry), row_bounds) in root
        .thread_entries
        .iter_mut()
        .enumerate()
        .zip(row_bounds.into_iter())
    {
        entry.open_bounds = row_bounds;
        entry.open_button.paint(row_bounds, cx);

        let updated_label = updated_labels.get(index).map(String::as_str).unwrap_or("-");
        let branch_label = branch_labels
            .get(index)
            .map(String::as_str)
            .filter(|label| !label.trim().is_empty())
            .unwrap_or("-");
        let preview_text = if entry.summary.preview.trim().is_empty() {
            "No preview"
        } else {
            entry.summary.preview.trim()
        };
        let preview = truncate_line(preview_text, preview_chars);

        let columns = wgpui::row_bounds(
            row_bounds,
            row_height,
            &[
                wgpui::RowItem::fixed(updated_width),
                wgpui::RowItem::fixed(branch_width),
                wgpui::RowItem::fixed(preview_width),
            ],
            gap_px,
        );
        let updated_bounds = *columns.get(0).unwrap_or(&row_bounds);
        let branch_bounds = *columns.get(1).unwrap_or(&row_bounds);
        let preview_bounds = *columns.get(2).unwrap_or(&row_bounds);

        Text::new(updated_label)
            .font_size(font_size)
            .no_wrap()
            .color(theme::text::MUTED)
            .paint(updated_bounds, cx);
        Text::new(branch_label)
            .font_size(font_size)
            .no_wrap()
            .color(theme::text::MUTED)
            .paint(branch_bounds, cx);
        Text::new(preview)
            .font_size(font_size)
            .no_wrap()
            .color(theme::text::PRIMARY)
            .paint(preview_bounds, cx);
    }
}

pub(super) fn paint_inbox_list_pane(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let padding = 16.0;
    let header_height = 24.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let items = [ColumnItem::Fixed(header_height), ColumnItem::Flex(1.0)];
    let bounds_list = column_bounds(content_bounds, &items, 8.0);
    let header_bounds = *bounds_list.get(0).unwrap_or(&content_bounds);
    let list_bounds = *bounds_list.get(1).unwrap_or(&content_bounds);
    let button_gap = 8.0;
    let refresh_width = 76.0;
    let thread_width = 74.0;
    let approvals_width = 92.0;
    let audit_width = 74.0;
    let has_selected = root.inbox.selected_thread().is_some();
    let font_size = theme::font_size::XS;

    root.inbox_refresh_bounds = Bounds::ZERO;
    root.inbox_open_thread_bounds = Bounds::ZERO;
    root.inbox_open_approvals_bounds = Bounds::ZERO;
    root.inbox_open_audit_bounds = Bounds::ZERO;
    root.inbox.list_row_bounds.clear();

    let row_items = [
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(thread_width),
        wgpui::RowItem::fixed(approvals_width),
        wgpui::RowItem::fixed(audit_width),
        wgpui::RowItem::fixed(refresh_width),
    ];
    let header_row_bounds = aligned_row_bounds(
        header_bounds,
        header_height,
        &row_items,
        button_gap,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );

    let title_bounds = *header_row_bounds.get(0).unwrap_or(&header_bounds);
    root.inbox_open_thread_bounds = *header_row_bounds.get(1).unwrap_or(&header_bounds);
    root.inbox_open_approvals_bounds = *header_row_bounds.get(2).unwrap_or(&header_bounds);
    root.inbox_open_audit_bounds = *header_row_bounds.get(3).unwrap_or(&header_bounds);
    root.inbox_refresh_bounds = *header_row_bounds.get(4).unwrap_or(&header_bounds);

    Text::new("INBOX")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    root.inbox_open_thread_button.set_disabled(!has_selected);
    root.inbox_open_approvals_button.set_disabled(!has_selected);
    root.inbox_open_audit_button.set_disabled(!has_selected);
    root.inbox_open_thread_button
        .paint(root.inbox_open_thread_bounds, cx);
    root.inbox_open_approvals_button
        .paint(root.inbox_open_approvals_bounds, cx);
    root.inbox_open_audit_button
        .paint(root.inbox_open_audit_bounds, cx);
    root.inbox_refresh_button
        .paint(root.inbox_refresh_bounds, cx);

    if root.inbox.threads.is_empty() {
        Text::new("No inbox threads available.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(list_bounds, cx);
        return;
    }

    let row_height = 54.0;
    let row_gap = 8.0;
    let heights: Vec<f32> = root.inbox.threads.iter().map(|_| row_height).collect();
    let row_bounds = stack_bounds(list_bounds, &heights, row_gap);
    let selected_id = root.inbox.selected_thread_id.as_deref();
    let max_subject_chars = ((list_bounds.size.width / (font_size * 0.55)).floor() as usize).max(8);
    let max_meta_chars = ((list_bounds.size.width / (font_size * 0.52)).floor() as usize).max(8);

    for (thread, row_bounds) in root.inbox.threads.iter().zip(row_bounds.into_iter()) {
        let selected = selected_id == Some(thread.id.as_str());
        let background = if selected {
            theme::bg::CODE
        } else {
            theme::bg::MUTED
        };
        let border = if selected {
            theme::accent::PRIMARY
        } else {
            theme::border::DEFAULT
        };
        cx.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(background)
                .with_border(border, 1.0)
                .with_corner_radius(8.0),
        );

        root.inbox
            .list_row_bounds
            .push((thread.id.clone(), row_bounds));

        let inner_bounds = Bounds::new(
            row_bounds.origin.x + 10.0,
            row_bounds.origin.y + 8.0,
            (row_bounds.size.width - 20.0).max(0.0),
            (row_bounds.size.height - 16.0).max(0.0),
        );
        let rows = column_bounds(
            inner_bounds,
            &[
                ColumnItem::Fixed(font_size * 1.35),
                ColumnItem::Fixed(font_size * 1.25),
            ],
            4.0,
        );
        let subject_bounds = *rows.first().unwrap_or(&inner_bounds);
        let meta_bounds = *rows.get(1).unwrap_or(&inner_bounds);
        let subject = truncate_line(thread.subject.trim(), max_subject_chars);
        let pending = if thread.pending_approval {
            "pending approval"
        } else {
            "ready"
        };
        let meta = truncate_line(
            format!(
                "{} · {} · {} · {} · {}",
                thread.from_address, thread.category, thread.risk, thread.policy, pending
            )
            .as_str(),
            max_meta_chars,
        );

        Text::new(subject)
            .font_size(font_size)
            .bold()
            .no_wrap()
            .color(theme::text::PRIMARY)
            .paint(subject_bounds, cx);
        Text::new(meta)
            .font_size(font_size)
            .no_wrap()
            .color(theme::text::MUTED)
            .paint(meta_bounds, cx);
    }
}

pub(super) fn paint_inbox_thread_pane(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let padding = 16.0;
    let header_height = 24.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let items = [ColumnItem::Fixed(header_height), ColumnItem::Flex(1.0)];
    let bounds_list = column_bounds(content_bounds, &items, 8.0);
    let header_bounds = *bounds_list.get(0).unwrap_or(&content_bounds);
    let body_bounds = *bounds_list.get(1).unwrap_or(&content_bounds);
    let font_size = theme::font_size::XS;

    let row_items = [
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(96.0),
        wgpui::RowItem::fixed(78.0),
    ];
    let header_row_bounds = aligned_row_bounds(
        header_bounds,
        header_height,
        &row_items,
        8.0,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );
    let title_bounds = *header_row_bounds.get(0).unwrap_or(&header_bounds);
    root.inbox_open_approvals_bounds = *header_row_bounds.get(1).unwrap_or(&header_bounds);
    root.inbox_open_audit_bounds = *header_row_bounds.get(2).unwrap_or(&header_bounds);

    Text::new("THREAD")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    let has_selected = root.inbox.selected_thread().is_some();
    root.inbox_open_approvals_button.set_disabled(!has_selected);
    root.inbox_open_audit_button.set_disabled(!has_selected);
    root.inbox_open_approvals_button
        .paint(root.inbox_open_approvals_bounds, cx);
    root.inbox_open_audit_button
        .paint(root.inbox_open_audit_bounds, cx);

    let Some(thread) = root.inbox.selected_thread().cloned() else {
        Text::new("Select a thread from Inbox to inspect details.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(body_bounds, cx);
        return;
    };

    let lines = [
        format!("Subject: {}", thread.subject),
        format!("From: {}", thread.from_address),
        format!(
            "Category: {} · Risk: {} · Policy: {}",
            thread.category, thread.risk, thread.policy
        ),
        format!("Updated: {}", thread.updated_at),
        format!("Snippet: {}", thread.snippet),
        "Draft preview".to_string(),
    ];

    let mut y = body_bounds.origin.y;
    for line in lines {
        let mut text = Text::new(line)
            .font_size(font_size)
            .color(theme::text::PRIMARY);
        if y == body_bounds.origin.y {
            text = text.bold();
        }
        let (_, line_height) = text.size_hint_with_width(body_bounds.size.width);
        let line_height = line_height.unwrap_or(font_size * 1.3);
        if y + line_height > body_bounds.origin.y + body_bounds.size.height {
            return;
        }
        let line_bounds = Bounds::new(
            body_bounds.origin.x,
            y,
            body_bounds.size.width,
            line_height.max(font_size * 1.2),
        );
        text.paint(line_bounds, cx);
        y += line_height + 6.0;
    }

    let mut draft_text = Text::new(thread.draft_preview)
        .font_size(font_size)
        .color(theme::text::MUTED);
    let (_, draft_height) = draft_text.size_hint_with_width(body_bounds.size.width);
    let draft_height = draft_height.unwrap_or(font_size * 1.3);
    let remaining = (body_bounds.origin.y + body_bounds.size.height - y).max(0.0);
    let draft_bounds = Bounds::new(
        body_bounds.origin.x,
        y,
        body_bounds.size.width,
        draft_height.min(remaining),
    );
    draft_text.paint(draft_bounds, cx);
}

pub(super) fn paint_inbox_approvals_pane(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let padding = 16.0;
    let header_height = 24.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let items = [ColumnItem::Fixed(header_height), ColumnItem::Flex(1.0)];
    let bounds_list = column_bounds(content_bounds, &items, 8.0);
    let header_bounds = *bounds_list.get(0).unwrap_or(&content_bounds);
    let body_bounds = *bounds_list.get(1).unwrap_or(&content_bounds);
    let font_size = theme::font_size::XS;

    let row_items = [
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(90.0),
        wgpui::RowItem::fixed(112.0),
    ];
    let header_row_bounds = aligned_row_bounds(
        header_bounds,
        header_height,
        &row_items,
        8.0,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );
    let title_bounds = *header_row_bounds.get(0).unwrap_or(&header_bounds);
    root.inbox_approve_bounds = *header_row_bounds.get(1).unwrap_or(&header_bounds);
    root.inbox_reject_bounds = *header_row_bounds.get(2).unwrap_or(&header_bounds);

    Text::new("APPROVALS")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);

    let selected = root.inbox.selected_thread();
    let has_selected = selected.is_some();
    root.inbox_approve_button.set_disabled(!has_selected);
    root.inbox_reject_button.set_disabled(!has_selected);
    root.inbox_approve_button
        .paint(root.inbox_approve_bounds, cx);
    root.inbox_reject_button.paint(root.inbox_reject_bounds, cx);

    let pending_threads = root.inbox.pending_threads();
    let pending_summary = format!(
        "Pending approvals: {} of {}",
        pending_threads.len(),
        root.inbox.threads.len()
    );
    Text::new(pending_summary)
        .font_size(font_size)
        .color(theme::text::MUTED)
        .paint(
            Bounds::new(
                body_bounds.origin.x,
                body_bounds.origin.y,
                body_bounds.size.width,
                font_size * 1.4,
            ),
            cx,
        );

    let mut y = body_bounds.origin.y + font_size * 1.6 + 8.0;
    if let Some(thread) = selected {
        let status = if thread.pending_approval {
            "Pending human approval"
        } else {
            "Approved"
        };
        let selected_line = format!("Selected: {} ({status})", thread.subject);
        let mut selected_text = Text::new(selected_line)
            .font_size(font_size)
            .color(theme::text::PRIMARY)
            .bold();
        let (_, selected_h) = selected_text.size_hint_with_width(body_bounds.size.width);
        let selected_h = selected_h.unwrap_or(font_size * 1.3);
        selected_text.paint(
            Bounds::new(body_bounds.origin.x, y, body_bounds.size.width, selected_h),
            cx,
        );
        y += selected_h + 6.0;
        let preview = truncate_line(thread.draft_preview.trim(), 220);
        Text::new(preview)
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(
                    body_bounds.origin.x,
                    y,
                    body_bounds.size.width,
                    font_size * 1.4,
                ),
                cx,
            );
        y += font_size * 1.7 + 8.0;
    } else {
        Text::new("Select a thread first.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(
                    body_bounds.origin.x,
                    y,
                    body_bounds.size.width,
                    font_size * 1.4,
                ),
                cx,
            );
        y += font_size * 1.7 + 8.0;
    }

    if pending_threads.is_empty() {
        Text::new("No drafts currently awaiting approval.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(
                    body_bounds.origin.x,
                    y,
                    body_bounds.size.width,
                    font_size * 1.4,
                ),
                cx,
            );
        return;
    }

    for thread in pending_threads.into_iter().take(8) {
        if y >= body_bounds.origin.y + body_bounds.size.height {
            break;
        }
        let line = truncate_line(
            format!(
                "• {} · {} · {}",
                thread.subject, thread.from_address, thread.updated_at
            )
            .as_str(),
            120,
        );
        Text::new(line)
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(
                    body_bounds.origin.x,
                    y,
                    body_bounds.size.width,
                    font_size * 1.4,
                ),
                cx,
            );
        y += font_size * 1.5 + 2.0;
    }
}

pub(super) fn paint_inbox_audit_pane(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let padding = 16.0;
    let header_height = 24.0;
    let content_width = bounds.size.width - padding * 2.0;
    let content_bounds = centered_column_bounds(bounds, content_width, padding);
    let items = [ColumnItem::Fixed(header_height), ColumnItem::Flex(1.0)];
    let bounds_list = column_bounds(content_bounds, &items, 8.0);
    let header_bounds = *bounds_list.get(0).unwrap_or(&content_bounds);
    let body_bounds = *bounds_list.get(1).unwrap_or(&content_bounds);
    let font_size = theme::font_size::XS;

    let source = root.inbox.source.as_deref().unwrap_or("unknown");
    Text::new(format!("AUDIT · {}", source))
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(header_bounds, cx);

    if root.inbox.audit_log.is_empty() {
        Text::new("No inbox audit entries yet.")
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(body_bounds, cx);
        return;
    }

    let mut y = body_bounds.origin.y;
    for entry in root.inbox.audit_log.iter().rev().take(20) {
        if y >= body_bounds.origin.y + body_bounds.size.height {
            break;
        }
        let line = truncate_line(
            format!(
                "{} · {} · {} · {}",
                entry.created_at, entry.thread_id, entry.action, entry.detail
            )
            .as_str(),
            170,
        );
        Text::new(line)
            .font_size(font_size)
            .color(theme::text::MUTED)
            .paint(
                Bounds::new(
                    body_bounds.origin.x,
                    y,
                    body_bounds.size.width,
                    font_size * 1.4,
                ),
                cx,
            );
        y += font_size * 1.5 + 2.0;
    }
}

pub(super) fn paint_file_editor_pane(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let padding = FILE_EDITOR_PANEL_PADDING;
    let gap = FILE_EDITOR_PANEL_GAP;
    let content_bounds = Bounds::new(
        bounds.origin.x + padding,
        bounds.origin.y + padding,
        (bounds.size.width - padding * 2.0).max(0.0),
        (bounds.size.height - padding * 2.0).max(0.0),
    );

    let project_width =
        (content_bounds.size.width * 0.28).clamp(FILE_TREE_MIN_WIDTH, FILE_TREE_MAX_WIDTH);
    let row_items = [
        wgpui::RowItem::fixed(project_width),
        wgpui::RowItem::flex(1.0),
    ];
    let columns = aligned_row_bounds(
        content_bounds,
        content_bounds.size.height,
        &row_items,
        gap,
        JustifyContent::FlexStart,
        AlignItems::Stretch,
    );
    let project_bounds = *columns.get(0).unwrap_or(&content_bounds);
    let editor_bounds = *columns.get(1).unwrap_or(&content_bounds);

    paint_file_tree_panel(root, project_bounds, cx);
    paint_editor_workspace_panel(root, editor_bounds, cx);
}

pub(super) fn paint_file_tree_panel(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let header_height = FILE_EDITOR_TOOLBAR_HEIGHT;
    let subheader_height = theme::font_size::XS * 1.4;
    let gap = 6.0;

    let mut items = vec![ColumnItem::Fixed(header_height)];
    if root.file_editor.workspace_root.is_some() {
        items.push(ColumnItem::Fixed(subheader_height));
    }
    items.push(ColumnItem::Flex(1.0));

    let rows = column_bounds(bounds, &items, gap);
    let header_bounds = *rows.get(0).unwrap_or(&bounds);
    let mut index = 1usize;
    let subtitle_bounds = if root.file_editor.workspace_root.is_some() {
        let bounds = *rows.get(index).unwrap_or(&bounds);
        index += 1;
        Some(bounds)
    } else {
        None
    };
    let list_bounds = *rows.get(index).unwrap_or(&bounds);

    let refresh_width = 78.0;
    let header_row = aligned_row_bounds(
        header_bounds,
        header_height,
        &[
            wgpui::RowItem::flex(1.0),
            wgpui::RowItem::fixed(refresh_width),
        ],
        6.0,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );
    let title_bounds = *header_row.get(0).unwrap_or(&header_bounds);
    let refresh_bounds = *header_row.get(1).unwrap_or(&header_bounds);

    root.file_editor.tree_header_bounds = header_bounds;
    root.file_editor.tree_refresh_bounds = refresh_bounds;

    Text::new("PROJECT")
        .font_size(theme::font_size::SM)
        .bold()
        .color(theme::text::PRIMARY)
        .paint(title_bounds, cx);
    root.file_editor
        .tree_refresh_button
        .set_disabled(root.file_editor.workspace_root.is_none());
    root.file_editor
        .tree_refresh_button
        .paint(refresh_bounds, cx);

    if let Some(bounds) = subtitle_bounds {
        let label = root
            .file_editor
            .workspace_root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "No workspace".to_string());
        Text::new(label)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .no_wrap()
            .paint(bounds, cx);
    }

    root.file_editor.tree_bounds = list_bounds;
    root.file_editor.tree_rows.clear();

    let Some(tree_root) = root.file_editor.tree_root.as_ref() else {
        Text::new("Open a workspace to browse files.")
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .paint(list_bounds, cx);
        return;
    };

    collect_tree_rows(tree_root, 0, &mut root.file_editor.tree_rows);
    if root.file_editor.tree_rows.is_empty() {
        Text::new("No files found.")
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .paint(list_bounds, cx);
        return;
    }

    let row_height = FILE_TREE_ROW_HEIGHT;
    let content_height = row_height * root.file_editor.tree_rows.len() as f32;
    root.file_editor.tree_scroll.set_viewport(list_bounds);
    root.file_editor
        .tree_scroll
        .set_content_size(Size::new(list_bounds.size.width, content_height));

    let mut region = ScrollRegion::vertical(list_bounds, content_height);
    region.scroll_offset = root.file_editor.tree_scroll.scroll_offset;
    region.begin(&mut cx.scene);

    let active_path = root.file_editor.active_tab_path();
    for (index, row) in root.file_editor.tree_rows.iter_mut().enumerate() {
        let y = list_bounds.origin.y + index as f32 * row_height;
        let row_bounds = Bounds::new(
            list_bounds.origin.x,
            region.scroll_y(y),
            list_bounds.size.width,
            row_height,
        );
        row.bounds = row_bounds;
        if !region.is_visible_y(row_bounds.origin.y, row_height) {
            continue;
        }

        let is_active = active_path
            .as_ref()
            .map(|path| path == &row.path)
            .unwrap_or(false);
        let is_selected = root
            .file_editor
            .selected_tree_path
            .as_ref()
            .map(|path| path == &row.path)
            .unwrap_or(false);
        if is_active || is_selected {
            let bg = if is_active {
                theme::bg::SELECTED
            } else {
                theme::bg::HOVER
            };
            cx.scene
                .draw_quad(Quad::new(row_bounds).with_background(bg));
        }

        let indent = row.depth as f32 * FILE_TREE_INDENT;
        let icon = if row.is_dir {
            if row.expanded { "v" } else { ">" }
        } else {
            "-"
        };
        let name = row
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| row.path.display().to_string());
        let label = format!("{icon} {name}");
        let text_bounds = Bounds::new(
            row_bounds.origin.x + 6.0 + indent,
            row_bounds.origin.y,
            (row_bounds.size.width - indent - 12.0).max(0.0),
            row_height,
        );
        Text::new(label)
            .font_size(theme::font_size::XS)
            .color(if is_active {
                theme::text::PRIMARY
            } else {
                theme::text::SECONDARY
            })
            .no_wrap()
            .paint(text_bounds, cx);
    }

    region.end(&mut cx.scene);

    if region.can_scroll() {
        let track_bounds = Bounds::new(
            list_bounds.origin.x + list_bounds.size.width - FILE_TREE_SCROLLBAR_WIDTH,
            list_bounds.origin.y,
            FILE_TREE_SCROLLBAR_WIDTH,
            list_bounds.size.height,
        );
        region.draw_scrollbar(
            &mut cx.scene,
            track_bounds,
            theme::bg::SURFACE,
            theme::text::MUTED,
            FILE_TREE_SCROLLBAR_WIDTH / 2.0,
        );
    }
}

pub(super) fn paint_editor_workspace_panel(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let toolbar_height = FILE_EDITOR_TOOLBAR_HEIGHT;
    let status_height = theme::font_size::XS * 1.4;
    let gap = FILE_EDITOR_PANEL_GAP;

    let mut items = vec![ColumnItem::Fixed(toolbar_height)];
    if root.file_editor.status.is_some() {
        items.push(ColumnItem::Fixed(status_height));
    }
    items.push(ColumnItem::Flex(1.0));

    let sections = column_bounds(bounds, &items, gap);
    let toolbar_bounds = *sections.get(0).unwrap_or(&bounds);
    let mut index = 1usize;
    let status_bounds = if root.file_editor.status.is_some() {
        let bounds = *sections.get(index).unwrap_or(&bounds);
        index += 1;
        Some(bounds)
    } else {
        None
    };
    let workspace_bounds = *sections.get(index).unwrap_or(&bounds);

    paint_file_editor_toolbar(root, toolbar_bounds, cx);

    if let Some(bounds) = status_bounds {
        let status_color = if root.file_editor.status_is_error {
            theme::status::ERROR
        } else {
            theme::text::MUTED
        };
        if let Some(status) = root.file_editor.status.as_deref() {
            Text::new(status)
                .font_size(theme::font_size::XS)
                .color(status_color)
                .paint(bounds, cx);
        }
    }

    paint_editor_groups(root, workspace_bounds, cx);
}

pub(super) fn paint_file_editor_toolbar(
    root: &mut MinimalRoot,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let gap = 6.0;
    let button_font = theme::font_size::SM;
    let mut measure = |label: &str| {
        cx.text
            .measure_styled_mono(label, button_font, FontStyle::default())
            + 24.0
    };
    let open_width = measure("Open").max(64.0);
    let reload_width = measure("Reload").max(72.0);
    let save_width = measure("Save").max(64.0);
    let split_h_width = measure("Split H").max(72.0);
    let split_v_width = measure("Split V").max(72.0);

    let row_items = [
        wgpui::RowItem::flex(1.0),
        wgpui::RowItem::fixed(open_width),
        wgpui::RowItem::fixed(reload_width),
        wgpui::RowItem::fixed(save_width),
        wgpui::RowItem::fixed(split_h_width),
        wgpui::RowItem::fixed(split_v_width),
    ];
    let row_bounds = aligned_row_bounds(
        bounds,
        FILE_EDITOR_TOOLBAR_HEIGHT,
        &row_items,
        gap,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );

    let path_bounds = *row_bounds.get(0).unwrap_or(&bounds);
    let open_bounds = *row_bounds.get(1).unwrap_or(&bounds);
    let reload_bounds = *row_bounds.get(2).unwrap_or(&bounds);
    let save_bounds = *row_bounds.get(3).unwrap_or(&bounds);
    let split_h_bounds = *row_bounds.get(4).unwrap_or(&bounds);
    let split_v_bounds = *row_bounds.get(5).unwrap_or(&bounds);

    root.file_editor.path_bounds = path_bounds;
    root.file_editor.open_bounds = open_bounds;
    root.file_editor.reload_bounds = reload_bounds;
    root.file_editor.save_bounds = save_bounds;
    root.file_editor.split_h_bounds = split_h_bounds;
    root.file_editor.split_v_bounds = split_v_bounds;

    root.file_editor
        .path_input
        .set_max_width(path_bounds.size.width);

    let has_path = !root.file_editor.path_input.get_value().trim().is_empty();
    let has_tab = root.file_editor.active_tab_id().is_some();
    root.file_editor.open_button.set_disabled(!has_path);
    root.file_editor.reload_button.set_disabled(!has_tab);
    root.file_editor.save_button.set_disabled(!has_tab);

    root.file_editor.path_input.paint(path_bounds, cx);
    root.file_editor.open_button.paint(open_bounds, cx);
    root.file_editor.reload_button.paint(reload_bounds, cx);
    root.file_editor.save_button.paint(save_bounds, cx);
    root.file_editor
        .split_horizontal_button
        .paint(split_h_bounds, cx);
    root.file_editor
        .split_vertical_button
        .paint(split_v_bounds, cx);
}

pub(super) fn paint_editor_groups(root: &mut MinimalRoot, bounds: Bounds, cx: &mut PaintContext) {
    let split = root.file_editor.split_direction;
    let group_count = if matches!(split, SplitDirection::None) {
        1
    } else {
        2
    };
    let workspace_ready = root.file_editor.workspace_root.is_some();

    if root.file_editor.groups.len() < group_count {
        while root.file_editor.groups.len() < group_count {
            root.file_editor.groups.push(EditorGroup::new());
        }
    }
    if root.file_editor.active_group >= group_count {
        root.file_editor.active_group = 0;
    }

    let group_bounds_list = match split {
        SplitDirection::Horizontal => column_bounds(
            bounds,
            &[ColumnItem::Flex(1.0), ColumnItem::Flex(1.0)],
            FILE_EDITOR_SPLIT_GAP,
        ),
        SplitDirection::Vertical => aligned_row_bounds(
            bounds,
            bounds.size.height,
            &[wgpui::RowItem::flex(1.0), wgpui::RowItem::flex(1.0)],
            FILE_EDITOR_SPLIT_GAP,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
        ),
        SplitDirection::None => vec![bounds],
    };

    let active_group = root.file_editor.active_group;
    let tabs = &mut root.file_editor.tabs;
    let groups = &mut root.file_editor.groups;

    for (index, group_bounds) in group_bounds_list.into_iter().enumerate() {
        if let Some(group) = groups.get_mut(index) {
            group.group_bounds = group_bounds;
            let group_sections = column_bounds(
                group_bounds,
                &[
                    ColumnItem::Fixed(FILE_EDITOR_TAB_HEIGHT),
                    ColumnItem::Flex(1.0),
                ],
                0.0,
            );
            let tab_bar_bounds = *group_sections.get(0).unwrap_or(&group_bounds);
            let editor_bounds = *group_sections.get(1).unwrap_or(&group_bounds);
            group.tab_bar_bounds = tab_bar_bounds;
            group.editor_bounds = editor_bounds;
            group.tab_hits.clear();

            if group.active_tab.is_none() {
                group.active_tab = group.tabs.first().copied();
            }

            paint_tab_bar(tabs, group, tab_bar_bounds, index == active_group, cx);
            paint_editor_content(tabs, workspace_ready, group, editor_bounds, cx);
        }
    }

    for group in groups.iter_mut().skip(group_count) {
        group.group_bounds = Bounds::ZERO;
        group.tab_bar_bounds = Bounds::ZERO;
        group.editor_bounds = Bounds::ZERO;
        group.tab_hits.clear();
    }
}

pub(super) fn paint_tab_bar(
    tabs: &HashMap<usize, EditorTab>,
    group: &mut EditorGroup,
    bounds: Bounds,
    is_active_group: bool,
    cx: &mut PaintContext,
) {
    if group.tabs.is_empty() {
        return;
    }

    let font_size = theme::font_size::XS;
    let padding = FILE_EDITOR_TAB_PADDING;
    let min_width = 90.0;
    let max_width = 220.0;

    let mut labels = Vec::with_capacity(group.tabs.len());
    let mut widths = Vec::with_capacity(group.tabs.len());

    for tab_id in &group.tabs {
        let Some(tab) = tabs.get(tab_id) else {
            labels.push("Missing".to_string());
            widths.push(min_width);
            continue;
        };
        let mut label = tab.title.clone();
        if tab.loading {
            label.push_str(" (loading)");
        }
        if tab.is_dirty() {
            label.push_str(" *");
        }
        let label_width = cx
            .text
            .measure_styled_mono(&label, font_size, FontStyle::default());
        let width = (label_width + padding * 2.0).clamp(min_width, max_width);
        labels.push(label);
        widths.push(width);
    }

    let total_width: f32 =
        widths.iter().sum::<f32>() + FILE_EDITOR_TAB_GAP * (widths.len().saturating_sub(1) as f32);
    if total_width > bounds.size.width && !widths.is_empty() {
        let available = (bounds.size.width
            - FILE_EDITOR_TAB_GAP * (widths.len().saturating_sub(1) as f32))
            .max(min_width);
        let uniform = (available / widths.len() as f32).max(min_width);
        for width in widths.iter_mut() {
            *width = uniform;
        }
    }

    let row_items = widths
        .iter()
        .map(|width| wgpui::RowItem::fixed(*width))
        .collect::<Vec<_>>();
    let row_bounds = aligned_row_bounds(
        bounds,
        FILE_EDITOR_TAB_HEIGHT,
        &row_items,
        FILE_EDITOR_TAB_GAP,
        JustifyContent::FlexStart,
        AlignItems::Center,
    );

    for ((tab_id, label), tab_bounds) in group
        .tabs
        .iter()
        .zip(labels.iter())
        .zip(row_bounds.into_iter())
    {
        let is_active = group.active_tab == Some(*tab_id);
        let bg = if is_active {
            theme::bg::SELECTED
        } else if is_active_group {
            theme::bg::SURFACE
        } else {
            theme::bg::APP
        };
        cx.scene.draw_quad(
            Quad::new(tab_bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let text_bounds = Bounds::new(
            tab_bounds.origin.x + 8.0,
            tab_bounds.origin.y,
            (tab_bounds.size.width - 16.0).max(0.0),
            tab_bounds.size.height,
        );
        Text::new(label)
            .font_size(font_size)
            .color(if is_active {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            })
            .no_wrap()
            .paint(text_bounds, cx);

        group.tab_hits.push(TabHit {
            tab_id: *tab_id,
            bounds: tab_bounds,
        });
    }
}

pub(super) fn paint_editor_content(
    tabs: &mut HashMap<usize, EditorTab>,
    workspace_ready: bool,
    group: &mut EditorGroup,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    if let Some(tab_id) = group.active_tab {
        if let Some(tab) = tabs.get_mut(&tab_id) {
            tab.editor.paint(bounds, cx);
            return;
        }
    }

    let hint = if workspace_ready {
        "Open a file from the project tree or enter a path."
    } else {
        "Open a workspace to start editing."
    };
    Text::new(hint)
        .font_size(theme::font_size::XS)
        .color(theme::text::MUTED)
        .paint(bounds, cx);
}

pub(super) fn build_file_node(path: &Path, remaining: &mut usize) -> FileNode {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.display().to_string());
    let is_dir = path.is_dir();
    if *remaining == 0 {
        return FileNode {
            path: path.to_path_buf(),
            name,
            is_dir,
            expanded: false,
            children: Vec::new(),
        };
    }

    *remaining = remaining.saturating_sub(1);

    let mut node = FileNode {
        path: path.to_path_buf(),
        name,
        is_dir,
        expanded: false,
        children: Vec::new(),
    };

    if !is_dir || *remaining == 0 {
        return node;
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if *remaining == 0 {
                break;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if is_ignored_entry(&file_name) {
                continue;
            }
            let entry_path = entry.path();
            let child = build_file_node(&entry_path, remaining);
            if child.is_dir {
                dirs.push(child);
            } else {
                files.push(child);
            }
        }
    }

    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    node.children.extend(dirs);
    node.children.extend(files);
    node
}

pub(super) fn is_ignored_entry(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "node_modules" | ".idea" | ".vscode" | ".DS_Store" | ".cache"
    )
}

pub(super) fn toggle_tree_node(node: &mut FileNode, target: &Path) -> bool {
    if node.path == target {
        node.expanded = !node.expanded;
        return true;
    }

    for child in &mut node.children {
        if child.is_dir && toggle_tree_node(child, target) {
            return true;
        }
    }
    false
}

pub(super) fn collect_tree_rows(node: &FileNode, depth: usize, rows: &mut Vec<FileTreeRow>) {
    for child in &node.children {
        rows.push(FileTreeRow {
            path: child.path.clone(),
            depth,
            is_dir: child.is_dir,
            expanded: child.expanded,
            bounds: Bounds::ZERO,
        });
        if child.is_dir && child.expanded {
            collect_tree_rows(child, depth + 1, rows);
        }
    }
}

pub(super) fn is_save_chord(modifiers: &Modifiers, key: &Key) -> bool {
    if modifiers.alt {
        return false;
    }
    let has_modifier = modifiers.ctrl || modifiers.meta;
    let is_s = matches!(key, Key::Character(value) if value.eq_ignore_ascii_case("s"));
    has_modifier && is_s
}
