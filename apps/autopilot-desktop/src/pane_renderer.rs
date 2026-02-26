use crate::app_state::{
    AutopilotChatState, AutopilotMessageStatus, AutopilotRole, ChatPaneInputs, DesktopPane,
    JobInboxState, NostrSecretState, PaneKind, PaneLoadState, PayInvoicePaneInputs,
    ProviderBlocker, ProviderRuntimeState, SparkPaneInputs,
};
use crate::pane_system::{
    PANE_TITLE_HEIGHT, chat_composer_input_bounds, chat_send_button_bounds,
    chat_thread_rail_bounds, chat_transcript_bounds, go_online_toggle_button_bounds,
    job_inbox_accept_button_bounds, job_inbox_reject_button_bounds, job_inbox_row_bounds,
    job_inbox_visible_row_count, nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds,
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
        autopilot_chat: &AutopilotChatState,
        provider_runtime: &ProviderRuntimeState,
        provider_blockers: &[ProviderBlocker],
        job_inbox: &JobInboxState,
        spark_wallet: &SparkPaneState,
        spark_inputs: &mut SparkPaneInputs,
        pay_invoice_inputs: &mut PayInvoicePaneInputs,
        chat_inputs: &mut ChatPaneInputs,
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
                PaneKind::AutopilotChat => {
                    paint_autopilot_chat_pane(content_bounds, autopilot_chat, chat_inputs, paint);
                }
                PaneKind::GoOnline => {
                    paint_go_online_pane(
                        content_bounds,
                        provider_runtime,
                        provider_blockers,
                        paint,
                    );
                }
                PaneKind::ProviderStatus => {
                    paint_provider_status_pane(
                        content_bounds,
                        provider_runtime,
                        provider_blockers,
                        paint,
                    );
                }
                PaneKind::JobInbox => {
                    paint_job_inbox_pane(content_bounds, job_inbox, paint);
                }
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

fn paint_autopilot_chat_pane(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    let rail_bounds = chat_thread_rail_bounds(content_bounds);
    let transcript_bounds = chat_transcript_bounds(content_bounds);
    let composer_bounds = chat_composer_input_bounds(content_bounds);
    let send_bounds = chat_send_button_bounds(content_bounds);

    paint.scene.draw_quad(
        Quad::new(rail_bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.72))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_quad(
        Quad::new(transcript_bounds)
            .with_background(theme::bg::APP.with_alpha(0.82))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(4.0),
    );

    paint.scene.draw_text(paint.text.layout(
        "Threads",
        Point::new(rail_bounds.origin.x + 10.0, rail_bounds.origin.y + 14.0),
        11.0,
        theme::text::MUTED,
    ));
    let mut thread_y = rail_bounds.origin.y + 30.0;
    for (idx, thread) in autopilot_chat.threads.iter().enumerate() {
        let color = if idx == autopilot_chat.active_thread {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        };
        paint.scene.draw_text(paint.text.layout(
            thread,
            Point::new(rail_bounds.origin.x + 10.0, thread_y),
            11.0,
            color,
        ));
        thread_y += 16.0;
    }

    let mut y = transcript_bounds.origin.y + 10.0;
    for message in autopilot_chat.messages.iter().rev().take(12).rev() {
        let status = match message.status {
            AutopilotMessageStatus::Queued => "queued",
            AutopilotMessageStatus::Running => "running",
            AutopilotMessageStatus::Done => "done",
            AutopilotMessageStatus::Error => "error",
        };
        let role = match message.role {
            AutopilotRole::User => "you",
            AutopilotRole::Autopilot => "autopilot",
        };
        let status_color = match message.status {
            AutopilotMessageStatus::Queued => theme::text::MUTED,
            AutopilotMessageStatus::Running => theme::accent::PRIMARY,
            AutopilotMessageStatus::Done => theme::status::SUCCESS,
            AutopilotMessageStatus::Error => theme::status::ERROR,
        };

        paint.scene.draw_text(paint.text.layout_mono(
            &format!("[#{:04}] [{role}] [{status}]", message.id),
            Point::new(transcript_bounds.origin.x + 10.0, y),
            10.0,
            status_color,
        ));
        y += 14.0;
        for line in split_text_for_display(&message.content, 78) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(transcript_bounds.origin.x + 10.0, y),
                11.0,
                theme::text::PRIMARY,
            ));
            y += 14.0;
        }
        y += 8.0;
    }

    if let Some(error) = autopilot_chat.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(
                transcript_bounds.origin.x + 10.0,
                transcript_bounds.max_y() - 14.0,
            ),
            11.0,
            theme::status::ERROR,
        ));
    }

    chat_inputs
        .composer
        .set_max_width(composer_bounds.size.width);
    chat_inputs.composer.paint(composer_bounds, paint);
    paint_action_button(send_bounds, "Send", paint);
}

fn paint_go_online_pane(
    content_bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
    provider_blockers: &[ProviderBlocker],
    paint: &mut PaintContext,
) {
    let toggle_bounds = go_online_toggle_button_bounds(content_bounds);
    let toggle_label = if provider_runtime.mode == crate::app_state::ProviderMode::Offline {
        "Go Online"
    } else {
        "Go Offline"
    };
    paint_action_button(toggle_bounds, toggle_label, paint);

    let now = std::time::Instant::now();
    let mut y = toggle_bounds.max_y() + 14.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Provider mode",
        provider_runtime.mode.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Uptime (s)",
        &provider_runtime.uptime_seconds(now).to_string(),
    );

    if provider_blockers.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "Preflight: clear",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::SUCCESS,
        ));
    } else {
        paint.scene.draw_text(paint.text.layout(
            "Preflight blockers:",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::status::ERROR,
        ));
        y += 16.0;
        for blocker in provider_blockers {
            paint.scene.draw_text(paint.text.layout_mono(
                &format!("{} - {}", blocker.code(), blocker.detail()),
                Point::new(content_bounds.origin.x + 12.0, y),
                10.0,
                theme::status::ERROR,
            ));
            y += 14.0;
        }
    }

    if let Some(code) = provider_runtime.degraded_reason_code.as_deref() {
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("Last reason code: {code}"),
            Point::new(content_bounds.origin.x + 12.0, y + 12.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_provider_status_pane(
    content_bounds: Bounds,
    provider_runtime: &ProviderRuntimeState,
    provider_blockers: &[ProviderBlocker],
    paint: &mut PaintContext,
) {
    let now = std::time::Instant::now();
    let heartbeat_age = provider_runtime
        .heartbeat_age_seconds(now)
        .map_or_else(|| "n/a".to_string(), |age| age.to_string());
    let mut y = content_bounds.origin.y + 12.0;
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Mode",
        provider_runtime.mode.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Uptime (s)",
        &provider_runtime.uptime_seconds(now).to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Heartbeat age (s)",
        &heartbeat_age,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Queue depth",
        &provider_runtime.queue_depth.to_string(),
    );
    if let Some(last_completed) = provider_runtime.last_completed_job_at {
        let seconds = now
            .checked_duration_since(last_completed)
            .map_or(0, |duration| duration.as_secs());
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last completed job (s ago)",
            &seconds.to_string(),
        );
    } else {
        y = paint_label_line(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Last completed job",
            "none",
        );
    }
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last result",
        provider_runtime.last_result.as_deref().unwrap_or("none"),
    );

    paint.scene.draw_text(paint.text.layout(
        "Dependencies",
        Point::new(content_bounds.origin.x + 12.0, y + 4.0),
        11.0,
        theme::text::MUTED,
    ));
    let mut dep_y = y + 20.0;
    let identity_status = if provider_blockers.contains(&ProviderBlocker::IdentityMissing) {
        "degraded"
    } else {
        "ready"
    };
    let wallet_status = if provider_blockers.contains(&ProviderBlocker::WalletError) {
        "degraded"
    } else {
        "ready"
    };
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("identity: {identity_status}"),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("wallet: {wallet_status}"),
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));
    dep_y += 14.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "relay: unknown (lane pending)",
        Point::new(content_bounds.origin.x + 12.0, dep_y),
        10.0,
        theme::text::PRIMARY,
    ));

    if let Some(error) = provider_runtime.last_error_detail.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            "Last error",
            Point::new(content_bounds.origin.x + 12.0, dep_y + 18.0),
            11.0,
            theme::status::ERROR,
        ));
        let mut error_y = dep_y + 34.0;
        for line in split_text_for_display(error, 82) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(content_bounds.origin.x + 12.0, error_y),
                11.0,
                theme::status::ERROR,
            ));
            error_y += 14.0;
        }
    }
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

fn paint_job_inbox_pane(
    content_bounds: Bounds,
    job_inbox: &JobInboxState,
    paint: &mut PaintContext,
) {
    let accept_bounds = job_inbox_accept_button_bounds(content_bounds);
    let reject_bounds = job_inbox_reject_button_bounds(content_bounds);
    paint_action_button(accept_bounds, "Accept selected", paint);
    paint_action_button(reject_bounds, "Reject selected", paint);

    let state_color = match job_inbox.load_state {
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Loading => theme::accent::PRIMARY,
        PaneLoadState::Error => theme::status::ERROR,
    };
    let mut y = accept_bounds.max_y() + 12.0;
    paint.scene.draw_text(paint.text.layout(
        &format!("State: {}", job_inbox.load_state.label()),
        Point::new(content_bounds.origin.x + 12.0, y),
        11.0,
        state_color,
    ));
    y += 16.0;

    if let Some(message) = job_inbox.last_action.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            message,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::text::MUTED,
        ));
    }
    y += 16.0;

    if let Some(error) = job_inbox.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(content_bounds.origin.x + 12.0, y),
            10.0,
            theme::status::ERROR,
        ));
        y += 16.0;
    }

    match job_inbox.load_state {
        PaneLoadState::Loading => {
            paint.scene.draw_text(paint.text.layout(
                "Waiting for deterministic replay cursor...",
                Point::new(content_bounds.origin.x + 12.0, y),
                11.0,
                theme::text::MUTED,
            ));
            return;
        }
        PaneLoadState::Error | PaneLoadState::Ready => {}
    }

    let visible_rows = job_inbox_visible_row_count(job_inbox.requests.len());
    if visible_rows == 0 {
        paint.scene.draw_text(paint.text.layout(
            "No requests in inbox.",
            Point::new(content_bounds.origin.x + 12.0, y),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    for row_index in 0..visible_rows {
        let request = &job_inbox.requests[row_index];
        let row_bounds = job_inbox_row_bounds(content_bounds, row_index);
        let selected =
            job_inbox.selected_request_id.as_deref() == Some(request.request_id.as_str());
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if selected {
                    theme::accent::PRIMARY.with_alpha(0.18)
                } else {
                    theme::bg::APP.with_alpha(0.78)
                })
                .with_border(
                    if selected {
                        theme::accent::PRIMARY
                    } else {
                        theme::border::DEFAULT
                    },
                    1.0,
                )
                .with_corner_radius(4.0),
        );

        let status_color = match request.validation {
            crate::app_state::JobInboxValidation::Valid => theme::status::SUCCESS,
            crate::app_state::JobInboxValidation::Pending => theme::accent::PRIMARY,
            crate::app_state::JobInboxValidation::Invalid(_) => theme::status::ERROR,
        };
        let summary = format!(
            "#{} {} {} {} sats ttl:{}s {} {}",
            request.arrival_seq,
            request.request_id,
            request.capability,
            request.price_sats,
            request.ttl_seconds,
            request.validation.label(),
            request.decision.label()
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &summary,
            Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 9.0),
            10.0,
            if selected {
                theme::text::PRIMARY
            } else {
                status_color
            },
        ));
    }

    if let Some(selected) = job_inbox.selected_request() {
        let details_y = job_inbox_row_bounds(content_bounds, visible_rows.saturating_sub(1)).max_y() + 12.0;
        let x = content_bounds.origin.x + 12.0;
        let mut line_y = details_y;
        line_y = paint_label_line(paint, x, line_y, "Selected requester", &selected.requester);
        line_y = paint_label_line(
            paint,
            x,
            line_y,
            "Selected request id",
            &selected.request_id,
        );
        let _ = paint_label_line(
            paint,
            x,
            line_y,
            "Decision",
            &selected.decision.label(),
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
