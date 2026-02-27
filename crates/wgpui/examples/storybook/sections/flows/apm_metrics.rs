use super::*;

impl Storybook {
    pub(crate) fn paint_apm_metrics(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let gauge_height = panel_height(200.0);
        let rows_height = panel_height(220.0);
        let comparison_height = panel_height(280.0);
        let leaderboard_height = panel_height(320.0);
        let trends_height = panel_height(200.0);

        let panels = panel_stack(
            bounds,
            &[
                gauge_height,
                rows_height,
                comparison_height,
                leaderboard_height,
                trends_height,
            ],
        );

        // ========== Panel 1: APM Gauge Variations ==========
        let gauge_bounds = panels[0];
        draw_panel("APM Gauge Variations", gauge_bounds, cx, |inner, cx| {
            let apms = [
                (0.0, "Idle"),
                (25.0, "Slow"),
                (50.0, "Moderate"),
                (75.0, "Fast"),
                (95.0, "Expert"),
                (120.0, "Elite"),
            ];

            let gauge_w = 100.0;
            let gauge_h = 60.0;
            let gap = 20.0;

            for (i, (apm, label)) in apms.iter().enumerate() {
                let x = inner.origin.x + (i as f32 * (gauge_w + gap));

                // APM Gauge
                let mut gauge = ApmGauge::new(*apm);
                gauge.paint(Bounds::new(x, inner.origin.y, gauge_w, gauge_h), cx);

                // Label
                let label_text = cx.text.layout(
                    *label,
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 8.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);

                // APM Value
                let level = ApmLevel::from_apm(*apm);
                let apm_text = cx.text.layout(
                    &format!("{:.0} APM", apm),
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 24.0),
                    theme::font_size::SM,
                    level.color(),
                );
                cx.scene.draw_text(apm_text);

                // Tier label
                let tier_text = cx.text.layout(
                    level.label(),
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 42.0),
                    theme::font_size::XS,
                    level.color(),
                );
                cx.scene.draw_text(tier_text);
            }
        });

        // ========== Panel 2: APM Session Rows ==========
        let rows_bounds = panels[1];
        draw_panel("APM Session Rows", rows_bounds, cx, |inner, cx| {
            let sessions = [
                ApmSessionData::new("sess-1", "Build feature authentication", 92.0)
                    .status(SessionStatus::Completed)
                    .duration(1800)
                    .rank(1),
                ApmSessionData::new("sess-2", "Fix database query bug", 78.5)
                    .status(SessionStatus::Completed)
                    .duration(2400)
                    .rank(2),
                ApmSessionData::new("sess-3", "Refactor API endpoints", 65.0)
                    .status(SessionStatus::Running)
                    .duration(900)
                    .rank(3),
                ApmSessionData::new("sess-4", "Add unit tests", 45.2)
                    .status(SessionStatus::Paused)
                    .duration(600)
                    .rank(4),
            ];

            for (i, session) in sessions.iter().enumerate() {
                let mut row = ApmSessionRow::new(session.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 48.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Session Comparison ==========
        let comparison_bounds = panels[2];
        draw_panel("Session Comparison", comparison_bounds, cx, |inner, cx| {
            let session_a = ComparisonSession::new("sess-a", "Monday Session", 68.5)
                .messages(120)
                .tool_calls(85)
                .duration(3600);

            let session_b = ComparisonSession::new("sess-b", "Tuesday Session", 82.3)
                .messages(95)
                .tool_calls(110)
                .duration(2800);

            let mut card = ApmComparisonCard::new(session_a, session_b);
            card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    220.0,
                ),
                cx,
            );
        });

        // ========== Panel 4: APM Leaderboard ==========
        let leaderboard_bounds = panels[3];
        draw_panel("APM Leaderboard", leaderboard_bounds, cx, |inner, cx| {
            let entries = vec![
                LeaderboardEntry::new("1", "Implement OAuth2 flow", 98.5)
                    .status(SessionStatus::Completed)
                    .messages(150)
                    .tool_calls(120),
                LeaderboardEntry::new("2", "Build payment integration", 92.0)
                    .status(SessionStatus::Completed)
                    .messages(180)
                    .tool_calls(95),
                LeaderboardEntry::new("3", "Create dashboard UI", 85.5)
                    .status(SessionStatus::Completed)
                    .messages(200)
                    .tool_calls(75),
                LeaderboardEntry::new("4", "Add real-time sync", 78.0)
                    .status(SessionStatus::Completed)
                    .messages(90)
                    .tool_calls(60),
                LeaderboardEntry::new("5", "Fix memory leak", 65.0)
                    .status(SessionStatus::Completed)
                    .messages(50)
                    .tool_calls(35),
                LeaderboardEntry::new("6", "Write documentation", 45.0)
                    .status(SessionStatus::Completed)
                    .messages(80)
                    .tool_calls(10),
            ];

            let mut leaderboard = ApmLeaderboard::new()
                .title("Top Sessions This Week")
                .entries(entries);
            leaderboard.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(600.0),
                    260.0,
                ),
                cx,
            );
        });

        // ========== Panel 5: APM Trends Summary ==========
        let trends_bounds = panels[4];
        draw_panel("APM Trends Summary", trends_bounds, cx, |inner, cx| {
            let metrics = [
                ("Avg APM", "72.4", "+5.2%", Hsla::new(120.0, 0.7, 0.45, 1.0)),
                (
                    "Peak APM",
                    "98.5",
                    "+12.1%",
                    Hsla::new(120.0, 0.7, 0.45, 1.0),
                ),
                ("Sessions", "24", "+3", Hsla::new(200.0, 0.7, 0.5, 1.0)),
                (
                    "Tool Calls",
                    "1,847",
                    "-2.3%",
                    Hsla::new(0.0, 0.7, 0.5, 1.0),
                ),
            ];

            let metric_w = inner.size.width / 4.0;
            for (i, (label, value, change, change_color)) in metrics.iter().enumerate() {
                let x = inner.origin.x + i as f32 * metric_w;

                // Value (large)
                let value_text = cx.text.layout(
                    value,
                    Point::new(x + 12.0, inner.origin.y + 8.0),
                    24.0,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(value_text);

                // Change indicator
                let change_text = cx.text.layout(
                    change,
                    Point::new(x + 12.0, inner.origin.y + 40.0),
                    theme::font_size::SM,
                    *change_color,
                );
                cx.scene.draw_text(change_text);

                // Label
                let label_text = cx.text.layout(
                    label,
                    Point::new(x + 12.0, inner.origin.y + 60.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);
            }

            // Period selector hint
            let period_y = inner.origin.y + 100.0;
            let periods = ["1h", "24h", "7d", "30d"];
            let period_label = cx.text.layout(
                "Time Period:",
                Point::new(inner.origin.x + 12.0, period_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(period_label);

            for (i, period) in periods.iter().enumerate() {
                let x = inner.origin.x + 100.0 + i as f32 * 60.0;
                let is_selected = i == 2; // 7d selected

                let bg = if is_selected {
                    theme::accent::PRIMARY.with_alpha(0.3)
                } else {
                    theme::bg::HOVER
                };

                let btn_bounds = Bounds::new(x, period_y - 4.0, 48.0, 24.0);
                cx.scene
                    .draw_quad(Quad::new(btn_bounds).with_background(bg).with_border(
                        if is_selected {
                            theme::accent::PRIMARY
                        } else {
                            theme::border::DEFAULT
                        },
                        1.0,
                    ));

                let period_text = cx.text.layout(
                    period,
                    Point::new(x + 14.0, period_y),
                    theme::font_size::SM,
                    if is_selected {
                        theme::accent::PRIMARY
                    } else {
                        theme::text::MUTED
                    },
                );
                cx.scene.draw_text(period_text);
            }
        });
    }
}
