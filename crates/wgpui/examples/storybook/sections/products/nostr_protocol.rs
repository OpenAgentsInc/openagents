use super::*;

impl Storybook {
    pub(crate) fn paint_nostr_protocol(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Relay Status Indicators ==========
        let status_height = panel_height(160.0);
        let status_bounds = Bounds::new(bounds.origin.x, y, width, status_height);
        draw_panel("Relay Status Indicators", status_bounds, cx, |inner, cx| {
            let statuses = [
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
                RelayStatus::Authenticating,
            ];

            // Row 1: Status dots
            let mut dot_x = inner.origin.x;
            let dot_y = inner.origin.y;
            let dot_run = cx.text.layout(
                "Status Dots:",
                Point::new(dot_x, dot_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(dot_run);

            dot_x = inner.origin.x;
            for status in &statuses {
                let mut dot = RelayStatusDot::new(*status).size(12.0).show_label(true);
                dot.paint(Bounds::new(dot_x, dot_y + 20.0, 60.0, 16.0), cx);
                dot_x += 80.0;
            }

            // Row 2: Status badges
            let badge_y = dot_y + 56.0;
            let badge_run = cx.text.layout(
                "Status Badges:",
                Point::new(inner.origin.x, badge_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(badge_run);

            let mut badge_x = inner.origin.x;
            for status in &statuses {
                let mut badge = RelayStatusBadge::new(*status);
                badge.paint(Bounds::new(badge_x, badge_y + 20.0, 90.0, 22.0), cx);
                badge_x += 100.0;
            }
        });
        y += status_height + SECTION_GAP;

        // ========== Panel 2: Event Kind Badges ==========
        let kinds_height = panel_height(280.0);
        let kinds_bounds = Bounds::new(bounds.origin.x, y, width, kinds_height);
        draw_panel("Event Kind Badges", kinds_bounds, cx, |inner, cx| {
            let kinds = [
                (EventKind::TextNote, "Social"),
                (EventKind::Metadata, "Identity"),
                (EventKind::Contacts, "Identity"),
                (EventKind::EncryptedDm, "Messaging"),
                (EventKind::Reaction, "Social"),
                (EventKind::ZapReceipt, "Payments"),
                (EventKind::RepoAnnounce, "Git"),
                (EventKind::Issue, "Git"),
                (EventKind::Patch, "Git"),
                (EventKind::PullRequest, "Git"),
                (EventKind::AgentProfile, "Agents"),
                (EventKind::TrajectorySession, "Agents"),
                (EventKind::DvmTextRequest, "DVM"),
                (EventKind::DvmTextResult, "DVM"),
                (EventKind::LongFormContent, "Content"),
                (EventKind::Custom(99999), "Custom"),
            ];

            let tile_w = 100.0;
            let tile_h = 50.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (kind, category)) in kinds.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Category label
                let cat_run = cx.text.layout(
                    *category,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(cat_run);

                // Event kind badge
                let mut badge = EventKindBadge::new(kind.clone()).show_number(true);
                badge.paint(Bounds::new(tile_x, tile_y + 16.0, tile_w - 4.0, 22.0), cx);
            }
        });
        y += kinds_height + SECTION_GAP;

        // ========== Panel 3: Bech32 Entities ==========
        let entities_height = panel_height(200.0);
        let entities_bounds = Bounds::new(bounds.origin.x, y, width, entities_height);
        draw_panel(
            "Bech32 Entities (NIP-19)",
            entities_bounds,
            cx,
            |inner, cx| {
                let entities = [
                    (
                        Bech32Type::Npub,
                        "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsutj2v5",
                    ),
                    (
                        Bech32Type::Note,
                        "note1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5nkr4f",
                    ),
                    (
                        Bech32Type::Nevent,
                        "nevent1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqnmupg",
                    ),
                    (
                        Bech32Type::Nprofile,
                        "nprofile1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsdp8el",
                    ),
                    (
                        Bech32Type::Nsec,
                        "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq9wlz3w",
                    ),
                    (Bech32Type::Nrelay, "nrelay1qqxrgvfex9j3n8qerc94kk"),
                ];

                let row_h = 40.0;
                let gap = 8.0;

                for (idx, (entity_type, value)) in entities.iter().enumerate() {
                    let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                    let mut entity = Bech32Entity::new(*entity_type, *value)
                        .show_prefix_badge(true)
                        .truncate(true);
                    entity.paint(
                        Bounds::new(
                            inner.origin.x,
                            row_y,
                            inner.size.width.min(400.0),
                            row_h - 4.0,
                        ),
                        cx,
                    );
                }
            },
        );
        y += entities_height + SECTION_GAP;

        // ========== Panel 4: Relay Connection List ==========
        let relays_height = panel_height(300.0);
        let relays_bounds = Bounds::new(bounds.origin.x, y, width, relays_height);
        draw_panel("Relay Connection List", relays_bounds, cx, |inner, cx| {
            let relays = [
                RelayInfo::new("wss://relay.damus.io")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(true)
                    .events(15420, 342)
                    .latency(45),
                RelayInfo::new("wss://nos.lol")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(true)
                    .events(8934, 156)
                    .latency(78),
                RelayInfo::new("wss://relay.nostr.band")
                    .status(RelayStatus::Connecting)
                    .read(true)
                    .write(false)
                    .events(0, 0),
                RelayInfo::new("wss://purplepag.es")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(false)
                    .events(2341, 0)
                    .latency(120),
                RelayInfo::new("wss://relay.snort.social")
                    .status(RelayStatus::Disconnected)
                    .read(true)
                    .write(true)
                    .events(0, 0),
                RelayInfo::new("wss://offchain.pub")
                    .status(RelayStatus::Error)
                    .read(true)
                    .write(true)
                    .events(0, 0),
            ];

            let row_h = 44.0;
            let gap = 4.0;

            for (idx, relay) in relays.iter().enumerate() {
                let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                let mut row = RelayRow::new(relay.clone());
                row.paint(
                    Bounds::new(inner.origin.x, row_y, inner.size.width.min(500.0), row_h),
                    cx,
                );
            }
        });
        y += relays_height + SECTION_GAP;

        // ========== Panel 5: Complete Relay Dashboard ==========
        let dashboard_height = panel_height(320.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Complete Relay Dashboard",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Dashboard header
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        40.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                let header_run = cx.text.layout(
                    "Nostr Relay Pool",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 12.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(header_run);

                // Stats summary
                let stats_run = cx.text.layout(
                    "4 Connected | 1 Connecting | 1 Error",
                    Point::new(
                        inner.origin.x + inner.size.width - 220.0,
                        inner.origin.y + 14.0,
                    ),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(stats_run);

                // Split into two columns
                let col_gap = 24.0;
                let col_w = (inner.size.width - col_gap) / 2.0;
                let content_y = inner.origin.y + 52.0;

                // Left column: Active relays
                let left_x = inner.origin.x;
                let active_label = cx.text.layout(
                    "Active Relays",
                    Point::new(left_x, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(active_label);

                let active_relays = [
                    RelayInfo::new("wss://relay.damus.io")
                        .status(RelayStatus::Connected)
                        .events(15420, 342)
                        .latency(45),
                    RelayInfo::new("wss://nos.lol")
                        .status(RelayStatus::Connected)
                        .events(8934, 156)
                        .latency(78),
                ];

                let row_h = 36.0;
                let row_gap = 4.0;
                let relay_y = content_y + 24.0;

                for (idx, relay) in active_relays.iter().enumerate() {
                    let row_y = relay_y + idx as f32 * (row_h + row_gap);
                    let mut row = RelayRow::new(relay.clone()).compact(true);
                    row.paint(Bounds::new(left_x, row_y, col_w.min(320.0), row_h), cx);
                }

                // Right column: Event statistics
                let right_x = inner.origin.x + col_w + col_gap;
                let stats_label = cx.text.layout(
                    "Event Statistics",
                    Point::new(right_x, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(stats_label);

                // Event kind summary
                let event_summary = [
                    ("Notes (kind:1)", "12,453"),
                    ("Reactions (kind:7)", "8,921"),
                    ("Profiles (kind:0)", "1,234"),
                    ("Zaps (kind:9735)", "456"),
                    ("DMs (kind:4)", "89"),
                ];

                let stat_y = content_y + 24.0;
                for (idx, (label, count)) in event_summary.iter().enumerate() {
                    let y_pos = stat_y + idx as f32 * 28.0;

                    let label_run = cx.text.layout(
                        *label,
                        Point::new(right_x, y_pos),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    let count_run = cx.text.layout(
                        *count,
                        Point::new(right_x + 140.0, y_pos),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(count_run);
                }
            },
        );
    }
}
