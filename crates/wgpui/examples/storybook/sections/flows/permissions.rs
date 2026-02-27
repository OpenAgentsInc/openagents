use super::*;

impl Storybook {
    pub(crate) fn paint_permissions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let decisions_height = panel_height(160.0);
        let rules_height = panel_height(240.0);
        let history_height = panel_height(280.0);
        let bar_height = panel_height(200.0);
        let stats_height = panel_height(140.0);

        let panels = panel_stack(
            bounds,
            &[
                decisions_height,
                rules_height,
                history_height,
                bar_height,
                stats_height,
            ],
        );

        // ========== Panel 1: Permission Decisions ==========
        let decisions_bounds = panels[0];
        draw_panel("Permission Decisions", decisions_bounds, cx, |inner, cx| {
            let decisions = [
                (PermissionDecision::Ask, "Ask every time"),
                (PermissionDecision::AllowOnce, "Allow once"),
                (PermissionDecision::AllowAlways, "Allow always"),
                (PermissionDecision::Deny, "Deny"),
            ];

            for (i, (decision, desc)) in decisions.iter().enumerate() {
                let x = inner.origin.x + (i as f32 * 140.0);

                // Decision badge
                let color = decision.color();
                let badge_bounds = Bounds::new(x, inner.origin.y, 120.0, 28.0);
                cx.scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(color.with_alpha(0.2))
                        .with_border(color, 1.0),
                );
                let label = cx.text.layout(
                    decision.label(),
                    Point::new(x + 8.0, inner.origin.y + 6.0),
                    theme::font_size::SM,
                    color,
                );
                cx.scene.draw_text(label);

                // Description
                let desc_text = cx.text.layout(
                    desc,
                    Point::new(x, inner.origin.y + 40.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_text);
            }

            // Short labels row
            let short_y = inner.origin.y + 72.0;
            let short_label = cx.text.layout(
                "Short labels:",
                Point::new(inner.origin.x, short_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(short_label);

            for (i, (decision, _)) in decisions.iter().enumerate() {
                let x = inner.origin.x + 80.0 + (i as f32 * 60.0);
                let short = cx.text.layout(
                    decision.short_label(),
                    Point::new(x, short_y),
                    theme::font_size::SM,
                    decision.color(),
                );
                cx.scene.draw_text(short);
            }
        });

        // ========== Panel 2: Permission Rules ==========
        let rules_bounds = panels[1];
        draw_panel("Permission Rules", rules_bounds, cx, |inner, cx| {
            let rules = [
                PermissionRule::new("rule-1", ToolType::Bash, "Bash")
                    .scope(PermissionScope::Session)
                    .pattern("cargo *")
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-2", ToolType::Write, "Write")
                    .scope(PermissionScope::Project)
                    .pattern("src/*")
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-3", ToolType::Read, "Read")
                    .scope(PermissionScope::Global)
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-4", ToolType::Edit, "Edit")
                    .scope(PermissionScope::Session)
                    .decision(PermissionDecision::Ask),
                PermissionRule::new("rule-5", ToolType::Bash, "Bash")
                    .scope(PermissionScope::Global)
                    .pattern("sudo *")
                    .decision(PermissionDecision::Deny),
            ];

            for (i, rule) in rules.iter().enumerate() {
                let mut row = PermissionRuleRow::new(rule.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 44.0,
                        inner.size.width,
                        40.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Permission History ==========
        let history_bounds = panels[2];
        draw_panel("Permission History", history_bounds, cx, |inner, cx| {
            let histories = [
                PermissionHistory::new("h-1", ToolType::Bash, "Bash", "cargo build --release")
                    .decision(PermissionDecision::AllowOnce)
                    .timestamp("2 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-2", ToolType::Write, "Write", "src/lib.rs")
                    .decision(PermissionDecision::AllowAlways)
                    .timestamp("5 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-3", ToolType::Bash, "Bash", "rm -rf node_modules/")
                    .decision(PermissionDecision::Deny)
                    .timestamp("10 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-4", ToolType::Read, "Read", "/etc/passwd")
                    .decision(PermissionDecision::Deny)
                    .timestamp("15 min ago")
                    .session("sess-002"),
            ];

            for (i, history) in histories.iter().enumerate() {
                let mut item = PermissionHistoryItem::new(history.clone());
                item.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 64.0,
                        inner.size.width,
                        56.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Permission Bar Variants ==========
        let bar_bounds = panels[3];
        draw_panel("Permission Bar Variants", bar_bounds, cx, |inner, cx| {
            // Standard permission bar
            let mut bar1 = PermissionBar::new("Bash wants to execute: cargo test");
            bar1.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 48.0),
                cx,
            );

            // File write permission
            let mut bar2 = PermissionBar::new("Write wants to create: src/new_module.rs");
            bar2.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 56.0,
                    inner.size.width,
                    48.0,
                ),
                cx,
            );

            // Dangerous operation
            let mut bar3 = PermissionBar::new("Bash wants to execute: git push --force");
            bar3.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 112.0,
                    inner.size.width,
                    48.0,
                ),
                cx,
            );
        });

        // ========== Panel 5: Permission Statistics ==========
        let stats_bounds = panels[4];
        draw_panel("Permission Statistics", stats_bounds, cx, |inner, cx| {
            let stats = [
                ("Total Requests", "247", theme::text::PRIMARY),
                ("Allowed", "189", Hsla::new(120.0, 0.7, 0.45, 1.0)),
                ("Denied", "42", Hsla::new(0.0, 0.8, 0.5, 1.0)),
                ("Pending", "16", Hsla::new(45.0, 0.7, 0.5, 1.0)),
            ];

            let stat_w = inner.size.width / 4.0;
            for (i, (label, value, color)) in stats.iter().enumerate() {
                let x = inner.origin.x + i as f32 * stat_w;

                // Value (large)
                let value_text = cx.text.layout(
                    value,
                    Point::new(x + 12.0, inner.origin.y + 8.0),
                    24.0,
                    *color,
                );
                cx.scene.draw_text(value_text);

                // Label
                let label_text = cx.text.layout(
                    label,
                    Point::new(x + 12.0, inner.origin.y + 44.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);
            }

            // Rule counts
            let rule_y = inner.origin.y + 80.0;
            let rule_label = cx.text.layout(
                "Active Rules:",
                Point::new(inner.origin.x + 12.0, rule_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(rule_label);

            let rule_counts = [("Global", 5), ("Project", 12), ("Session", 8)];

            let mut rx = inner.origin.x + 120.0;
            for (scope, count) in rule_counts {
                let scope_text = format!("{}: {}", scope, count);
                let scope_run = cx.text.layout(
                    &scope_text,
                    Point::new(rx, rule_y),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(scope_run);
                rx += 100.0;
            }
        });
    }
}
