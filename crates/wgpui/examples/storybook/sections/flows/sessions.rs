use super::*;

impl Storybook {
    pub(crate) fn paint_sessions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let cards_height = panel_height(280.0);
        let breadcrumb_height = panel_height(120.0);
        let search_height = panel_height(180.0);
        let actions_height = panel_height(160.0);
        let list_height = panel_height(320.0);

        let panels = panel_stack(
            bounds,
            &[
                cards_height,
                breadcrumb_height,
                search_height,
                actions_height,
                list_height,
            ],
        );

        // ========== Panel 1: Session Cards ==========
        let cards_bounds = panels[0];
        draw_panel("Session Cards", cards_bounds, cx, |inner, cx| {
            let card_w = (inner.size.width - 24.0) / 3.0;

            // Running session
            let running_info = SessionInfo::new("sess-001", "Implement auth flow")
                .status(SessionStatus::Running)
                .timestamp("10:30 AM")
                .duration(1847)
                .task_count(12)
                .model("sonnet");
            let mut running = SessionCard::new(running_info);
            running.paint(
                Bounds::new(inner.origin.x, inner.origin.y, card_w, 160.0),
                cx,
            );

            // Completed session
            let completed_info = SessionInfo::new("sess-002", "Fix CI pipeline")
                .status(SessionStatus::Completed)
                .timestamp("Yesterday")
                .duration(3621)
                .task_count(8)
                .model("opus");
            let mut completed = SessionCard::new(completed_info);
            completed.paint(
                Bounds::new(
                    inner.origin.x + card_w + 12.0,
                    inner.origin.y,
                    card_w,
                    160.0,
                ),
                cx,
            );

            // Failed session
            let failed_info = SessionInfo::new("sess-003", "Migrate database")
                .status(SessionStatus::Failed)
                .timestamp("2 days ago")
                .duration(892)
                .task_count(5)
                .model("sonnet");
            let mut failed = SessionCard::new(failed_info);
            failed.paint(
                Bounds::new(
                    inner.origin.x + (card_w + 12.0) * 2.0,
                    inner.origin.y,
                    card_w,
                    160.0,
                ),
                cx,
            );

            // Second row - more states
            let row2_y = inner.origin.y + 172.0;

            let paused_info = SessionInfo::new("sess-004", "Refactor components")
                .status(SessionStatus::Paused)
                .timestamp("1 hour ago")
                .duration(1200)
                .task_count(15)
                .model("sonnet");
            let mut paused = SessionCard::new(paused_info);
            paused.paint(Bounds::new(inner.origin.x, row2_y, card_w, 160.0), cx);

            let aborted_info = SessionInfo::new("sess-005", "Update dependencies")
                .status(SessionStatus::Aborted)
                .timestamp("3 hours ago")
                .duration(456)
                .task_count(3)
                .model("haiku");
            let mut aborted = SessionCard::new(aborted_info);
            aborted.paint(
                Bounds::new(inner.origin.x + card_w + 12.0, row2_y, card_w, 160.0),
                cx,
            );

            let pending_info = SessionInfo::new("sess-006", "Write tests")
                .status(SessionStatus::Pending)
                .timestamp("Queued")
                .model("sonnet");
            let mut pending = SessionCard::new(pending_info);
            pending.paint(
                Bounds::new(
                    inner.origin.x + (card_w + 12.0) * 2.0,
                    row2_y,
                    card_w,
                    160.0,
                ),
                cx,
            );
        });

        // ========== Panel 2: Session Breadcrumbs ==========
        let breadcrumb_bounds = panels[1];
        draw_panel("Session Breadcrumbs", breadcrumb_bounds, cx, |inner, cx| {
            // Simple breadcrumb
            let mut bc1 = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("sess-001", "Main Session"),
                BreadcrumbItem::new("sess-002", "Fork: Auth").current(true),
            ]);
            bc1.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 28.0),
                cx,
            );

            // Deep breadcrumb
            let mut bc2 = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("root", "Root Session"),
                BreadcrumbItem::new("fork-1", "API Changes"),
                BreadcrumbItem::new("fork-2", "Error Handling"),
                BreadcrumbItem::new("current", "Final Polish").current(true),
            ]);
            bc2.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 36.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );

            // Single item
            let mut bc3 = SessionBreadcrumb::new()
                .push_item(BreadcrumbItem::new("standalone", "Standalone Session").current(true));
            bc3.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 72.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );
        });

        // ========== Panel 3: Session Search ==========
        let search_bounds = panels[2];
        draw_panel(
            "Session Search & Filters",
            search_bounds,
            cx,
            |inner, cx| {
                // Empty search bar
                let mut search1 = SessionSearchBar::new();
                search1.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 44.0),
                    cx,
                );

                // Search bar with placeholder
                let mut search2 = SessionSearchBar::new().placeholder("Search auth sessions...");
                search2.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 52.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );

                // With active filter
                let mut search3 = SessionSearchBar::new();
                search3.set_filter(SessionStatus::Running, true);
                search3.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 104.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 4: Session Actions ==========
        let actions_bounds = panels[3];
        draw_panel("Session Actions", actions_bounds, cx, |inner, cx| {
            let label_x = inner.origin.x;
            let badge_x = inner.origin.x + 200.0;
            let mut row_y = inner.origin.y;

            // Resumable session (paused)
            let paused_label = cx.text.layout(
                "Paused → Can Resume:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(paused_label);
            let mut paused_badge = SessionStatusBadge::new(SessionStatus::Paused);
            paused_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            // Forkable sessions
            let completed_label = cx.text.layout(
                "Completed → Can Fork:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(completed_label);
            let mut completed_badge = SessionStatusBadge::new(SessionStatus::Completed);
            completed_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            let failed_label = cx.text.layout(
                "Failed → Can Fork:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(failed_label);
            let mut failed_badge = SessionStatusBadge::new(SessionStatus::Failed);
            failed_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            // Active session
            let running_label = cx.text.layout(
                "Running → Active:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(running_label);
            let mut running_badge = SessionStatusBadge::new(SessionStatus::Running)
                .duration(3621)
                .task_count(8);
            running_badge.paint(Bounds::new(badge_x, row_y, 200.0, 24.0), cx);
        });

        // ========== Panel 5: Complete Session List ==========
        let list_bounds = panels[4];
        draw_panel("Complete Session List", list_bounds, cx, |inner, cx| {
            // Search bar at top
            let mut search = SessionSearchBar::new();
            search.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 44.0),
                cx,
            );

            // Breadcrumb showing current path
            let mut breadcrumb = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("all", "All Sessions"),
                BreadcrumbItem::new("today", "Today").current(true),
            ]);
            breadcrumb.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 52.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );

            // Session cards in a grid
            let cards_y = inner.origin.y + 80.0;
            let card_w = (inner.size.width - 12.0) / 2.0;

            let sessions = [
                ("Current Task", SessionStatus::Running, 1847u64, 12u32),
                ("Yesterday's Work", SessionStatus::Completed, 7200, 15),
                ("Blocked Task", SessionStatus::Paused, 2400, 10),
                ("Failed Migration", SessionStatus::Failed, 600, 8),
            ];

            for (i, (title, status, dur, total)) in sessions.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                let x = inner.origin.x + col as f32 * (card_w + 12.0);
                let y = cards_y + row as f32 * 112.0;

                let info = SessionInfo::new(format!("sess-{}", i), *title)
                    .status(*status)
                    .timestamp("Today")
                    .duration(*dur)
                    .task_count(*total)
                    .model("sonnet");
                let mut card = SessionCard::new(info);
                card.paint(Bounds::new(x, y, card_w, 100.0), cx);
            }
        });
    }
}
